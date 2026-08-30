use semver::Version;
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tauri::Manager;
use thiserror::Error;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, Command},
    sync::{Mutex, mpsc},
    task::JoinHandle,
    time::{Instant, timeout},
};

const SUPPORTED_CODEX_MAJOR: u64 = 0;
const SUPPORTED_CODEX_MINOR: u64 = 118;
const RPC_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_DIAGNOSTIC_LINES: usize = 100;
const MAX_NOTIFICATION_MESSAGES: usize = 256;
const RESPONSE_CHANNEL_CAPACITY: usize = 16;
const APP_CODEX_CONFIG: &str = r#"forced_login_method = "chatgpt"
cli_auth_credentials_store = "keyring"
web_search = "live"
file_opener = "none"
hide_agent_reasoning = true
check_for_update_on_startup = false
approval_policy = "never"
sandbox_mode = "read-only"

[history]
persistence = "none"

[features]
shell_tool = false
skill_mcp_dependency_install = false

[tools]
view_image = false

[tools.web_search]
context_size = "medium"
allowed_domains = ["wikiwiki.jp", "game8.jp", "wiki.hoyolab.com"]
"#;
const APP_AGENTS_INSTRUCTIONS: &str = r#"# 原神ビルド調査エージェント

- ホストアプリから渡された編成と調査対象だけを扱うこと。
- ゲーム情報の調査にはWeb検索だけを使い、ローカルコマンドやファイル操作を行わないこと。
- 指定されたJSON Schemaに厳密に従い、確認できない情報を推測で補わないこと。
- 引用候補には実際に確認したURLと、主張を直接支える短い抜粋または要約を含めること。
"#;

#[derive(Debug, Error)]
enum AppServerError {
    #[error("Codex CLIが見つかりませんでした")]
    CodexNotFound,
    #[error("Codex CLIの出力からバージョンを解析できませんでした: {0}")]
    VersionInvalid(String),
    #[error("Codex App Serverを起動できませんでした: {0}")]
    StartFailed(String),
    #[error("Codex App Serverとの通信に失敗しました: {0}")]
    Io(#[from] std::io::Error),
    #[error("Codex App Serverが不正なJSONを返しました: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("Codex App Serverの標準出力をJSONとして解析できません: {0}")]
    InvalidJsonLine(String),
    #[error("Codex App Serverからの応答がタイムアウトしました")]
    Timeout,
    #[error("Codex App Serverが応答前に終了しました")]
    ProcessExited,
    #[error("Codex App Serverがエラーを返しました（{code}）: {message}")]
    Rpc { code: i64, message: String },
    #[error("Codex App Serverの応答形式が不正です: {0}")]
    Protocol(String),
    #[error("アプリ専用Codexホームの準備に失敗しました: {0}")]
    IsolatedHome(String),
}

impl AppServerError {
    fn invalidates_session(&self) -> bool {
        matches!(
            self,
            Self::Io(_)
                | Self::InvalidJson(_)
                | Self::InvalidJsonLine(_)
                | Self::Timeout
                | Self::ProcessExited
        )
    }
}

#[derive(Debug)]
struct CodexBinary {
    path: PathBuf,
    version: Version,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Gate0Account {
    auth_mode: Option<String>,
    plan_type: Option<String>,
    requires_openai_auth: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Gate0ProbeReport {
    codex_path: String,
    codex_version: String,
    version_supported: bool,
    app_server_initialized: bool,
    isolated_home: String,
    platform_family: Option<String>,
    platform_os: Option<String>,
    account: Option<Gate0Account>,
    rate_limits_available: bool,
    diagnostics: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceLoginStart {
    login_id: String,
    verification_url: String,
    user_code: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceLoginStatus {
    authenticated: bool,
    account: Gate0Account,
    login_completed: Option<bool>,
    login_error: Option<String>,
}

#[derive(Default)]
pub struct AppServerSupervisor {
    session: Mutex<Option<ManagedAppServer>>,
}

struct ManagedAppServer {
    rpc: JsonlRpcSession,
    next_request_id: u64,
    active_login_id: Option<String>,
    codex_path: String,
    codex_version: String,
    codex_home: String,
    platform_family: Option<String>,
    platform_os: Option<String>,
}

impl ManagedAppServer {
    async fn request(
        &mut self,
        method: &str,
        params: Option<Value>,
    ) -> Result<Value, AppServerError> {
        let id = self.next_request_id;
        self.next_request_id += 1;
        self.rpc.request(id, method, params).await
    }
}

struct JsonlRpcSession {
    child: Child,
    stdin: Arc<Mutex<ChildStdin>>,
    response_rx: mpsc::Receiver<IncomingResponse>,
    stdout_task: JoinHandle<()>,
    stderr_lines: Arc<Mutex<VecDeque<String>>>,
    stderr_task: JoinHandle<()>,
    notifications: Arc<Mutex<VecDeque<Value>>>,
}

enum IncomingResponse {
    Message(Value),
    InvalidJson(String),
    Closed,
}

fn build_server_request_response(request: &Value) -> Value {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let method = request.get("method").and_then(Value::as_str);
    let result = match method {
        Some("item/commandExecution/requestApproval") | Some("item/fileChange/requestApproval") => {
            json!({ "decision": "decline" })
        }
        Some("item/tool/requestUserInput") => json!({ "answers": {} }),
        Some("mcpServer/elicitation/request") => {
            json!({ "action": "decline", "content": null, "_meta": null })
        }
        Some("item/permissions/requestApproval") => {
            json!({ "permissions": {}, "scope": "turn" })
        }
        Some("item/tool/call") => json!({ "contentItems": [], "success": false }),
        Some("applyPatchApproval") | Some("execCommandApproval") => {
            json!({ "decision": "denied" })
        }
        _ => {
            return json!({
                "id": id,
                "error": {
                    "code": -32601,
                    "message": "このアプリでは要求されたサーバー要求を処理できません"
                }
            });
        }
    };
    json!({ "id": id, "result": result })
}

async fn write_jsonl(
    stdin: &Arc<Mutex<ChildStdin>>,
    message: &Value,
) -> Result<(), AppServerError> {
    let mut bytes = serde_json::to_vec(message)?;
    bytes.push(b'\n');
    let mut stdin = stdin.lock().await;
    stdin.write_all(&bytes).await?;
    stdin.flush().await?;
    Ok(())
}

async fn enqueue_notification(notifications: &Arc<Mutex<VecDeque<Value>>>, message: Value) {
    let mut buffer = notifications.lock().await;
    if buffer.len() == MAX_NOTIFICATION_MESSAGES {
        buffer.pop_front();
    }
    buffer.push_back(message);
}

fn take_matching_notification(
    notifications: &mut VecDeque<Value>,
    method: &str,
    login_id: Option<&str>,
) -> Option<Value> {
    let index = notifications.iter().position(|notification| {
        let method_matches = notification.get("method").and_then(Value::as_str) == Some(method);
        let login_matches = login_id.is_none_or(|expected| {
            let actual = notification
                .get("params")
                .and_then(|params| params.get("loginId"))
                .and_then(Value::as_str);
            actual.is_none() || actual == Some(expected)
        });
        method_matches && login_matches
    })?;
    notifications.remove(index)
}

impl JsonlRpcSession {
    async fn start(codex: &CodexBinary, codex_home: &Path) -> Result<Self, AppServerError> {
        let mut command = Command::new(&codex.path);
        command
            .args(["app-server", "--listen", "stdio://"])
            .env("CODEX_HOME", codex_home)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);

        let mut child = command
            .spawn()
            .map_err(|error| AppServerError::StartFailed(error.to_string()))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| AppServerError::StartFailed("標準入力を取得できません".into()))?;
        let stdin = Arc::new(Mutex::new(stdin));
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AppServerError::StartFailed("標準出力を取得できません".into()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| AppServerError::StartFailed("標準エラーを取得できません".into()))?;

        let stderr_lines = Arc::new(Mutex::new(VecDeque::new()));
        let stderr_buffer = Arc::clone(&stderr_lines);
        let stderr_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let mut buffer = stderr_buffer.lock().await;
                if buffer.len() == MAX_DIAGNOSTIC_LINES {
                    buffer.pop_front();
                }
                buffer.push_back(line);
            }
        });
        let notifications = Arc::new(Mutex::new(VecDeque::new()));
        let notification_buffer = Arc::clone(&notifications);
        let server_request_stdin = Arc::clone(&stdin);
        let (response_tx, response_rx) = mpsc::channel(RESPONSE_CHANNEL_CAPACITY);
        let stdout_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            loop {
                let message = match lines.next_line().await {
                    Ok(Some(line)) => match serde_json::from_str::<Value>(&line) {
                        Ok(message) => message,
                        Err(error) => {
                            let _ = response_tx
                                .send(IncomingResponse::InvalidJson(error.to_string()))
                                .await;
                            continue;
                        }
                    },
                    Ok(None) => {
                        let _ = response_tx.send(IncomingResponse::Closed).await;
                        break;
                    }
                    Err(error) => {
                        let _ = response_tx
                            .send(IncomingResponse::InvalidJson(error.to_string()))
                            .await;
                        break;
                    }
                };
                let has_method = message.get("method").and_then(Value::as_str).is_some();
                if has_method && message.get("id").is_some() {
                    let response = build_server_request_response(&message);
                    if let Err(error) = write_jsonl(&server_request_stdin, &response).await {
                        let _ = response_tx
                            .send(IncomingResponse::InvalidJson(error.to_string()))
                            .await;
                        break;
                    }
                } else if has_method {
                    enqueue_notification(&notification_buffer, message).await;
                } else if response_tx
                    .send(IncomingResponse::Message(message))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        });

        Ok(Self {
            child,
            stdin,
            response_rx,
            stdout_task,
            stderr_lines,
            stderr_task,
            notifications,
        })
    }

    async fn request(
        &mut self,
        id: u64,
        method: &str,
        params: Option<Value>,
    ) -> Result<Value, AppServerError> {
        let mut message = serde_json::Map::from_iter([
            ("method".into(), Value::String(method.into())),
            ("id".into(), Value::Number(id.into())),
        ]);
        if let Some(params) = params {
            message.insert("params".into(), params);
        }
        self.write(&Value::Object(message)).await?;

        let deadline = Instant::now() + RPC_TIMEOUT;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(AppServerError::Timeout);
            }

            let incoming = timeout(remaining, self.response_rx.recv())
                .await
                .map_err(|_| AppServerError::Timeout)?
                .ok_or(AppServerError::ProcessExited)?;
            let response = match incoming {
                IncomingResponse::Message(response) => response,
                IncomingResponse::InvalidJson(error) => {
                    return Err(AppServerError::InvalidJsonLine(error));
                }
                IncomingResponse::Closed => return Err(AppServerError::ProcessExited),
            };
            if response.get("id").and_then(Value::as_u64) != Some(id) {
                continue;
            }
            if let Some(error) = response.get("error") {
                return Err(AppServerError::Rpc {
                    code: error.get("code").and_then(Value::as_i64).unwrap_or(-1),
                    message: error
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("詳細不明")
                        .to_string(),
                });
            }
            return response
                .get("result")
                .cloned()
                .ok_or_else(|| AppServerError::Protocol("resultがありません".into()));
        }
    }

    async fn notify(&mut self, method: &str, params: Value) -> Result<(), AppServerError> {
        self.write(&json!({ "method": method, "params": params }))
            .await
    }

    async fn write(&mut self, message: &Value) -> Result<(), AppServerError> {
        write_jsonl(&self.stdin, message).await
    }

    async fn diagnostics(&self) -> Vec<String> {
        self.stderr_lines.lock().await.iter().cloned().collect()
    }

    async fn take_notification(&self, method: &str, login_id: Option<&str>) -> Option<Value> {
        let mut notifications = self.notifications.lock().await;
        take_matching_notification(&mut notifications, method, login_id)
    }

    async fn clear_notifications(&self, method: &str) {
        self.notifications.lock().await.retain(|notification| {
            notification.get("method").and_then(Value::as_str) != Some(method)
        });
    }

    async fn shutdown(mut self) {
        let _ = self.stdin.lock().await.shutdown().await;
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
        self.stdout_task.abort();
        let _ = self.stdout_task.await;
        self.stderr_task.abort();
        let _ = self.stderr_task.await;
    }
}

#[tauri::command]
pub async fn probe_codex_environment(
    app: tauri::AppHandle,
    supervisor: tauri::State<'_, AppServerSupervisor>,
) -> Result<Gate0ProbeReport, String> {
    let mut guard = supervisor.session.lock().await;
    probe(&app, &mut guard)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn start_codex_device_login(
    app: tauri::AppHandle,
    supervisor: tauri::State<'_, AppServerSupervisor>,
) -> Result<DeviceLoginStart, String> {
    let mut guard = supervisor.session.lock().await;
    if guard
        .as_ref()
        .and_then(|session| session.active_login_id.as_ref())
        .is_some()
    {
        return Err("進行中のdevice code認証があります".into());
    }
    if let Some(session) = guard.as_ref() {
        session
            .rpc
            .clear_notifications("account/login/completed")
            .await;
    }
    let result = supervised_request(
        &app,
        &mut guard,
        "account/login/start",
        Some(json!({ "type": "chatgptDeviceCode" })),
    )
    .await
    .map_err(|error| error.to_string())?;
    let login = parse_device_login_start(&result).map_err(|error| error.to_string())?;
    if let Some(session) = guard.as_mut() {
        session.active_login_id = Some(login.login_id.clone());
    }
    Ok(login)
}

#[tauri::command]
pub async fn read_codex_login_status(
    app: tauri::AppHandle,
    supervisor: tauri::State<'_, AppServerSupervisor>,
) -> Result<DeviceLoginStatus, String> {
    let mut guard = supervisor.session.lock().await;
    let result = supervised_request(
        &app,
        &mut guard,
        "account/read",
        Some(json!({ "refreshToken": false })),
    )
    .await
    .map_err(|error| error.to_string())?;
    let account = parse_account(&result).map_err(|error| error.to_string())?;
    let active_login_id = guard
        .as_ref()
        .and_then(|session| session.active_login_id.clone());
    let login_notification = if let Some(session) = guard.as_ref() {
        session
            .rpc
            .take_notification("account/login/completed", active_login_id.as_deref())
            .await
            .and_then(|notification| notification.get("params").cloned())
    } else {
        None
    };
    let login_completed = login_notification
        .as_ref()
        .and_then(|params| params.get("success"))
        .and_then(Value::as_bool);
    if (account.auth_mode.as_deref() == Some("chatgpt") || login_completed.is_some())
        && let Some(session) = guard.as_mut()
    {
        session.active_login_id = None;
    }
    Ok(DeviceLoginStatus {
        authenticated: account.auth_mode.as_deref() == Some("chatgpt"),
        account,
        login_completed,
        login_error: login_notification
            .as_ref()
            .and_then(|params| params.get("error"))
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

#[tauri::command]
pub async fn cancel_codex_device_login(
    app: tauri::AppHandle,
    supervisor: tauri::State<'_, AppServerSupervisor>,
    login_id: String,
) -> Result<(), String> {
    let mut guard = supervisor.session.lock().await;
    supervised_request(
        &app,
        &mut guard,
        "account/login/cancel",
        Some(json!({ "loginId": &login_id })),
    )
    .await
    .map_err(|error| error.to_string())?;
    if let Some(session) = guard.as_mut()
        && session.active_login_id.as_deref() == Some(login_id.as_str())
    {
        session.active_login_id = None;
    }
    Ok(())
}

async fn probe(
    app: &tauri::AppHandle,
    slot: &mut Option<ManagedAppServer>,
) -> Result<Gate0ProbeReport, AppServerError> {
    let codex = detect_codex().await?;
    let version_supported = is_supported_version(&codex.version);
    let isolated_home = app
        .path()
        .app_data_dir()
        .map_err(|error| AppServerError::IsolatedHome(error.to_string()))?
        .join("codex-home");

    let mut report = Gate0ProbeReport {
        codex_path: codex.path.display().to_string(),
        codex_version: codex.version.to_string(),
        version_supported,
        app_server_initialized: false,
        isolated_home: isolated_home.display().to_string(),
        platform_family: None,
        platform_os: None,
        account: None,
        rate_limits_available: false,
        diagnostics: Vec::new(),
    };

    if !version_supported {
        report.diagnostics.push(format!(
            "対応するCodex CLIは{SUPPORTED_CODEX_MAJOR}.{SUPPORTED_CODEX_MINOR}.xです"
        ));
        return Ok(report);
    }

    let account_result = supervised_request(
        app,
        slot,
        "account/read",
        Some(json!({ "refreshToken": false })),
    )
    .await?;
    let account = parse_account(&account_result)?;
    let is_chatgpt = account.auth_mode.as_deref() == Some("chatgpt");
    report.account = Some(account);

    if let Some(session) = slot.as_ref() {
        report.codex_path.clone_from(&session.codex_path);
        report.codex_version.clone_from(&session.codex_version);
        report.isolated_home.clone_from(&session.codex_home);
        report.platform_family.clone_from(&session.platform_family);
        report.platform_os.clone_from(&session.platform_os);
        report.app_server_initialized = true;
        report.diagnostics.extend(session.rpc.diagnostics().await);
    }

    if is_chatgpt {
        match supervised_request(app, slot, "account/rateLimits/read", None).await {
            Ok(result) => {
                report.rate_limits_available = result.get("rateLimits").is_some();
            }
            Err(error) => report
                .diagnostics
                .push(format!("利用上限情報を取得できませんでした: {error}")),
        }
        if slot.is_none() {
            report.app_server_initialized = false;
            report.rate_limits_available = false;
        }
    }

    Ok(report)
}

async fn prepare_isolated_home(codex_home: &Path) -> Result<(), AppServerError> {
    for relative_path in ["logs", "plugins", "skills", "workspace"] {
        tokio::fs::create_dir_all(codex_home.join(relative_path))
            .await
            .map_err(|error| AppServerError::IsolatedHome(error.to_string()))?;
    }
    tokio::fs::write(codex_home.join("config.toml"), APP_CODEX_CONFIG)
        .await
        .map_err(|error| AppServerError::IsolatedHome(error.to_string()))?;
    tokio::fs::write(
        codex_home.join("workspace").join("AGENTS.md"),
        APP_AGENTS_INSTRUCTIONS,
    )
    .await
    .map_err(|error| AppServerError::IsolatedHome(error.to_string()))?;
    Ok(())
}

async fn ensure_managed_session<'a>(
    app: &tauri::AppHandle,
    slot: &'a mut Option<ManagedAppServer>,
) -> Result<&'a mut ManagedAppServer, AppServerError> {
    if slot.is_none() {
        *slot = Some(start_managed_session(app).await?);
    }
    slot.as_mut()
        .ok_or_else(|| AppServerError::Protocol("常駐セッションを取得できません".into()))
}

async fn supervised_request(
    app: &tauri::AppHandle,
    slot: &mut Option<ManagedAppServer>,
    method: &str,
    params: Option<Value>,
) -> Result<Value, AppServerError> {
    let result = {
        let session = ensure_managed_session(app, slot).await?;
        session.request(method, params).await
    };
    if result
        .as_ref()
        .err()
        .is_some_and(AppServerError::invalidates_session)
        && let Some(session) = slot.take()
    {
        session.rpc.shutdown().await;
    }
    result
}

async fn start_managed_session(app: &tauri::AppHandle) -> Result<ManagedAppServer, AppServerError> {
    let codex = detect_codex().await?;
    if !is_supported_version(&codex.version) {
        return Err(AppServerError::Protocol(format!(
            "対応するCodex CLIは{SUPPORTED_CODEX_MAJOR}.{SUPPORTED_CODEX_MINOR}.xです"
        )));
    }
    let codex_home = app
        .path()
        .app_data_dir()
        .map_err(|error| AppServerError::IsolatedHome(error.to_string()))?
        .join("codex-home");
    prepare_isolated_home(&codex_home).await?;

    let mut rpc = JsonlRpcSession::start(&codex, &codex_home).await?;
    let initialized = rpc
        .request(
            0,
            "initialize",
            Some(json!({
                "clientInfo": {
                    "name": "genshin_reco",
                    "title": "原神 聖遺物レコメンダー",
                    "version": env!("CARGO_PKG_VERSION")
                }
            })),
        )
        .await?;
    validate_codex_home(&initialized, &codex_home)?;
    rpc.notify("initialized", json!({})).await?;
    let platform_family = initialized
        .get("platformFamily")
        .and_then(Value::as_str)
        .map(str::to_string);
    let platform_os = initialized
        .get("platformOs")
        .and_then(Value::as_str)
        .map(str::to_string);

    Ok(ManagedAppServer {
        rpc,
        next_request_id: 1,
        active_login_id: None,
        codex_path: codex.path.display().to_string(),
        codex_version: codex.version.to_string(),
        codex_home: codex_home.display().to_string(),
        platform_family,
        platform_os,
    })
}

async fn detect_codex() -> Result<CodexBinary, AppServerError> {
    let where_output = Command::new("where.exe")
        .arg("codex")
        .output()
        .await
        .map_err(|_| AppServerError::CodexNotFound)?;
    if !where_output.status.success() {
        return Err(AppServerError::CodexNotFound);
    }

    let candidates = String::from_utf8_lossy(&where_output.stdout);
    let mut fallback = None;
    for path in candidates.lines().map(str::trim).map(PathBuf::from) {
        let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
            continue;
        };
        if !extension.eq_ignore_ascii_case("exe") && !extension.eq_ignore_ascii_case("cmd") {
            continue;
        }

        let path = if extension.eq_ignore_ascii_case("cmd") {
            let Some(native_path) = resolve_npm_native_binary(&path) else {
                continue;
            };
            native_path
        } else {
            path
        };
        let mut command = Command::new(&path);
        let Ok(version_output) = command.arg("--version").output().await else {
            continue;
        };
        if !version_output.status.success() {
            continue;
        }
        let raw_version = String::from_utf8_lossy(&version_output.stdout)
            .trim()
            .to_string();
        let Ok(version) = parse_codex_version(&raw_version) else {
            continue;
        };
        let candidate = CodexBinary { path, version };
        if is_supported_version(&candidate.version) {
            return Ok(candidate);
        }
        fallback.get_or_insert(candidate);
    }

    fallback.ok_or(AppServerError::CodexNotFound)
}

fn resolve_npm_native_binary(shim_path: &Path) -> Option<PathBuf> {
    let npm_root = shim_path.parent()?;
    let (package_name, target_name) = match std::env::consts::ARCH {
        "x86_64" => ("codex-win32-x64", "x86_64-pc-windows-msvc"),
        "aarch64" => ("codex-win32-arm64", "aarch64-pc-windows-msvc"),
        _ => return None,
    };
    let native_path = npm_root
        .join("node_modules")
        .join("@openai")
        .join("codex")
        .join("node_modules")
        .join("@openai")
        .join(package_name)
        .join("vendor")
        .join(target_name)
        .join("codex")
        .join("codex.exe");
    native_path.is_file().then_some(native_path)
}

fn parse_codex_version(output: &str) -> Result<Version, AppServerError> {
    output
        .split_whitespace()
        .find_map(|part| Version::parse(part.trim_start_matches('v')).ok())
        .ok_or_else(|| AppServerError::VersionInvalid(output.to_string()))
}

fn is_supported_version(version: &Version) -> bool {
    version.major == SUPPORTED_CODEX_MAJOR && version.minor == SUPPORTED_CODEX_MINOR
}

fn validate_codex_home(response: &Value, expected: &Path) -> Result<(), AppServerError> {
    let actual = response
        .get("codexHome")
        .and_then(Value::as_str)
        .ok_or_else(|| AppServerError::Protocol("codexHomeがありません".into()))?;
    let actual = std::fs::canonicalize(actual)
        .map_err(|error| AppServerError::IsolatedHome(error.to_string()))?;
    let expected = std::fs::canonicalize(expected)
        .map_err(|error| AppServerError::IsolatedHome(error.to_string()))?;
    if actual
        .to_string_lossy()
        .eq_ignore_ascii_case(&expected.to_string_lossy())
    {
        Ok(())
    } else {
        Err(AppServerError::Protocol(format!(
            "分離Codexホームが一致しません（actual={}, expected={}）",
            actual.display(),
            expected.display()
        )))
    }
}

fn parse_account(result: &Value) -> Result<Gate0Account, AppServerError> {
    let requires_openai_auth = result
        .get("requiresOpenaiAuth")
        .and_then(Value::as_bool)
        .ok_or_else(|| AppServerError::Protocol("requiresOpenaiAuthがありません".into()))?;
    let account = result.get("account").filter(|value| !value.is_null());
    let auth_mode = account
        .and_then(|value| value.get("type"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let plan_type = account
        .and_then(|value| value.get("planType"))
        .and_then(Value::as_str)
        .map(str::to_string);

    Ok(Gate0Account {
        auth_mode,
        plan_type,
        requires_openai_auth,
    })
}

fn parse_device_login_start(result: &Value) -> Result<DeviceLoginStart, AppServerError> {
    if result.get("type").and_then(Value::as_str) != Some("chatgptDeviceCode") {
        return Err(AppServerError::Protocol(
            "device code認証以外の応答を受け取りました".into(),
        ));
    }
    let required_string = |field: &str| {
        result
            .get(field)
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| AppServerError::Protocol(format!("{field}がありません")))
    };
    Ok(DeviceLoginStart {
        login_id: required_string("loginId")?,
        verification_url: required_string("verificationUrl")?,
        user_code: required_string("userCode")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncWriteExt, duplex};

    #[test]
    fn codexのバージョンを解析できる() {
        let version = parse_codex_version("codex-cli 0.118.0").expect("解析できること");
        assert_eq!(version, Version::new(0, 118, 0));
    }

    #[test]
    fn 対応マイナーバージョンだけを許可する() {
        assert!(is_supported_version(&Version::new(0, 118, 3)));
        assert!(!is_supported_version(&Version::new(0, 119, 0)));
    }

    #[test]
    fn 未認証のアカウント状態を解析できる() {
        let account = parse_account(&json!({
            "account": null,
            "requiresOpenaiAuth": true
        }))
        .expect("解析できること");
        assert_eq!(account.auth_mode, None);
        assert!(account.requires_openai_auth);
    }

    #[test]
    fn device_code認証の開始応答を解析できる() {
        let login = parse_device_login_start(&json!({
            "type": "chatgptDeviceCode",
            "loginId": "login-1",
            "verificationUrl": "https://auth.openai.com/codex/device",
            "userCode": "ABCD-1234"
        }))
        .expect("解析できること");

        assert_eq!(login.login_id, "login-1");
        assert_eq!(login.user_code, "ABCD-1234");
    }

    #[tokio::test]
    async fn 通知キューを上限件数に保つ() {
        let notifications = Arc::new(Mutex::new(VecDeque::new()));
        for index in 0..=MAX_NOTIFICATION_MESSAGES {
            enqueue_notification(
                &notifications,
                json!({ "method": "test/event", "params": { "index": index } }),
            )
            .await;
        }

        let notifications = notifications.lock().await;
        assert_eq!(notifications.len(), MAX_NOTIFICATION_MESSAGES);
        assert_eq!(
            notifications.front().expect("通知があること")["params"]["index"],
            1
        );
    }

    #[test]
    fn login_idが一致する完了通知だけを取り出す() {
        let mut notifications = VecDeque::from([
            json!({
                "method": "account/login/completed",
                "params": { "loginId": "old-login", "success": false }
            }),
            json!({
                "method": "account/login/completed",
                "params": { "loginId": "current-login", "success": true }
            }),
        ]);

        let notification = take_matching_notification(
            &mut notifications,
            "account/login/completed",
            Some("current-login"),
        )
        .expect("一致する通知があること");

        assert_eq!(notification["params"]["success"], true);
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0]["params"]["loginId"], "old-login");
    }

    #[test]
    fn login_idがnullの完了通知を現在の認証へ対応付ける() {
        let mut notifications = VecDeque::from([json!({
            "method": "account/login/completed",
            "params": { "loginId": null, "success": false, "error": "cancelled" }
        })]);

        let notification = take_matching_notification(
            &mut notifications,
            "account/login/completed",
            Some("current-login"),
        )
        .expect("nullの完了通知を現在の認証へ対応付けること");

        assert_eq!(notification["params"]["error"], "cancelled");
        assert!(notifications.is_empty());
    }

    #[test]
    fn サーバーからの実行承認要求を明示的に拒否する() {
        let response = build_server_request_response(&json!({
            "id": 42,
            "method": "item/commandExecution/requestApproval",
            "params": {}
        }));

        assert_eq!(response["id"], 42);
        assert_eq!(response["result"]["decision"], "decline");
    }

    #[test]
    fn 未対応のサーバー要求へmethod_not_foundを返す() {
        let response = build_server_request_response(&json!({
            "id": "server-request-1",
            "method": "unknown/request",
            "params": {}
        }));

        assert_eq!(response["id"], "server-request-1");
        assert_eq!(response["error"]["code"], -32601);
    }

    #[tokio::test]
    async fn アプリ専用codexホームを準備できる() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("現在時刻を取得できること")
            .as_nanos();
        let codex_home = std::env::temp_dir().join(format!(
            "genshin-reco-home-test-{}-{unique}",
            std::process::id()
        ));

        prepare_isolated_home(&codex_home)
            .await
            .expect("専用ホームを準備できること");

        let config = tokio::fs::read_to_string(codex_home.join("config.toml"))
            .await
            .expect("設定を読めること");
        assert!(config.contains("forced_login_method = \"chatgpt\""));
        assert!(config.contains("persistence = \"none\""));
        assert!(config.contains("shell_tool = false"));
        assert!(codex_home.join("workspace").join("AGENTS.md").is_file());

        tokio::fs::remove_dir_all(codex_home)
            .await
            .expect("一時ホームを削除できること");
    }

    #[tokio::test]
    async fn 分割されたjsonlを一行へ復元できる() {
        let (mut writer, reader) = duplex(64);
        let writer_task = tokio::spawn(async move {
            writer
                .write_all(b"{\"id\":1,")
                .await
                .expect("前半を書けること");
            tokio::task::yield_now().await;
            writer
                .write_all(b"\"result\":{}}\n")
                .await
                .expect("後半を書けること");
        });
        let mut lines = BufReader::new(reader).lines();
        let line = lines
            .next_line()
            .await
            .expect("読み取れること")
            .expect("一行あること");
        writer_task.await.expect("書き込みが完了すること");

        let value: Value = serde_json::from_str(&line).expect("JSONとして復元できること");
        assert_eq!(value["id"], 1);
    }

    #[tokio::test]
    #[ignore = "ローカルのCodex CLIを実際に起動するため通常テストから除外する"]
    async fn 実codexで初期化と認証状態確認ができる() {
        let codex = detect_codex().await.expect("Codexを検出できること");
        assert!(is_supported_version(&codex.version));

        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("現在時刻を取得できること")
            .as_nanos();
        let codex_home = std::env::temp_dir().join(format!(
            "genshin-reco-codex-home-{}-{unique}",
            std::process::id()
        ));
        tokio::fs::create_dir_all(&codex_home)
            .await
            .expect("一時Codexホームを作成できること");
        prepare_isolated_home(&codex_home)
            .await
            .expect("専用ホームを準備できること");

        let mut session = JsonlRpcSession::start(&codex, &codex_home)
            .await
            .expect("App Serverを起動できること");
        let initialized = session
            .request(
                0,
                "initialize",
                Some(json!({
                    "clientInfo": {
                        "name": "genshin_reco_gate0_test",
                        "title": "原神 聖遺物レコメンダー Gate 0",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                })),
            )
            .await
            .expect("初期化できること");
        validate_codex_home(&initialized, &codex_home).expect("分離ホームが一致すること");
        session
            .notify("initialized", json!({}))
            .await
            .expect("初期化完了を通知できること");
        let account = session
            .request(1, "account/read", Some(json!({ "refreshToken": false })))
            .await
            .expect("認証状態を確認できること");
        parse_account(&account).expect("認証応答を解析できること");
        let login = session
            .request(
                2,
                "account/login/start",
                Some(json!({ "type": "chatgptDeviceCode" })),
            )
            .await
            .expect("device code認証を開始できること");
        let login = parse_device_login_start(&login).expect("認証開始応答を解析できること");
        session
            .request(
                3,
                "account/login/cancel",
                Some(json!({ "loginId": login.login_id })),
            )
            .await
            .expect("device code認証を取り消せること");

        session.shutdown().await;
        let mut removed = false;
        for _ in 0..5 {
            match tokio::fs::remove_dir_all(&codex_home).await {
                Ok(()) => {
                    removed = true;
                    break;
                }
                Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
            }
        }
        assert!(removed, "一時Codexホームを削除できること");
    }
}
