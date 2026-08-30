use semver::Version;
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    collections::{HashMap, VecDeque},
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
const TURN_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_DIAGNOSTIC_LINES: usize = 100;
const MAX_NOTIFICATION_MESSAGES: usize = 256;
const MAX_TURN_COMPLETIONS: usize = 64;
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Gate0SmokeReport {
    structured_output_valid: bool,
    web_search_observed: bool,
    cancellation_observed: bool,
    instruction_sources_supported: bool,
    model_rerouted: bool,
    rerouted_from: Option<String>,
    rerouted_to: Option<String>,
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
    turn_completions: Arc<Mutex<VecDeque<Value>>>,
    turn_observations: Arc<Mutex<HashMap<(String, String), TurnObservations>>>,
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

fn should_buffer_notification(method: &str) -> bool {
    matches!(
        method,
        "account/login/completed"
            | "account/rateLimits/updated"
            | "error"
            | "item/started"
            | "item/completed"
            | "model/rerouted"
            | "turn/started"
    )
}

async fn route_notification(
    notifications: &Arc<Mutex<VecDeque<Value>>>,
    turn_completions: &Arc<Mutex<VecDeque<Value>>>,
    turn_observations: &Arc<Mutex<HashMap<(String, String), TurnObservations>>>,
    message: Value,
) {
    let Some(method) = message.get("method").and_then(Value::as_str) else {
        return;
    };
    observe_turn_notification(turn_observations, &message).await;
    if method == "turn/completed" {
        let mut completions = turn_completions.lock().await;
        if completions.len() == MAX_TURN_COMPLETIONS {
            completions.pop_front();
        }
        completions.push_back(message);
    } else if should_buffer_notification(method) {
        enqueue_notification(notifications, message).await;
    }
}

async fn observe_turn_notification(
    turn_observations: &Arc<Mutex<HashMap<(String, String), TurnObservations>>>,
    notification: &Value,
) {
    let params = &notification["params"];
    let Some(thread_id) = params.get("threadId").and_then(Value::as_str) else {
        return;
    };
    let Some(turn_id) = params
        .get("turnId")
        .and_then(Value::as_str)
        .or_else(|| params["turn"]["id"].as_str())
    else {
        return;
    };
    let method = notification.get("method").and_then(Value::as_str);
    let item = &params["item"];
    let is_web_search = matches!(method, Some("item/started") | Some("item/completed"))
        && item["type"].as_str() == Some("webSearch");
    let agent_message = (method == Some("item/completed")
        && item["type"].as_str() == Some("agentMessage"))
    .then(|| item["text"].as_str().map(str::to_string))
    .flatten();
    let is_reroute = method == Some("model/rerouted");
    if !is_web_search && agent_message.is_none() && !is_reroute {
        return;
    }

    let mut observations = turn_observations.lock().await;
    let observation = observations
        .entry((thread_id.to_string(), turn_id.to_string()))
        .or_default();
    observation.web_search_observed |= is_web_search;
    if let Some(message) = agent_message {
        observation.agent_message = Some(message);
    }
    if is_reroute {
        observation.rerouted_from = params["fromModel"].as_str().map(str::to_string);
        observation.rerouted_to = params["toModel"].as_str().map(str::to_string);
    }
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
        let turn_completions = Arc::new(Mutex::new(VecDeque::new()));
        let turn_completion_buffer = Arc::clone(&turn_completions);
        let turn_observations = Arc::new(Mutex::new(HashMap::new()));
        let turn_observation_buffer = Arc::clone(&turn_observations);
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
                    route_notification(
                        &notification_buffer,
                        &turn_completion_buffer,
                        &turn_observation_buffer,
                        message,
                    )
                    .await;
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
            turn_completions,
            turn_observations,
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

    async fn wait_for_turn_completion(
        &mut self,
        thread_id: &str,
        turn_id: &str,
    ) -> Result<Value, AppServerError> {
        let deadline = Instant::now() + TURN_TIMEOUT;
        loop {
            {
                let mut completions = self.turn_completions.lock().await;
                if let Some(index) = completions.iter().position(|notification| {
                    notification.get("method").and_then(Value::as_str) == Some("turn/completed")
                        && notification["params"]["threadId"].as_str() == Some(thread_id)
                        && notification["params"]["turn"]["id"].as_str() == Some(turn_id)
                }) {
                    return completions.remove(index).ok_or_else(|| {
                        AppServerError::Protocol("完了通知を取得できません".into())
                    });
                }
            }
            if self.child.try_wait()?.is_some() {
                return Err(AppServerError::ProcessExited);
            }
            if Instant::now() >= deadline {
                return Err(AppServerError::Timeout);
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    async fn take_turn_observations(&self, thread_id: &str, turn_id: &str) -> TurnObservations {
        self.turn_observations
            .lock()
            .await
            .remove(&(thread_id.to_string(), turn_id.to_string()))
            .unwrap_or_default()
    }

    async fn clear_thread_state(&self, thread_id: &str) {
        self.notifications
            .lock()
            .await
            .retain(|notification| notification["params"]["threadId"].as_str() != Some(thread_id));
        self.turn_completions
            .lock()
            .await
            .retain(|notification| notification["params"]["threadId"].as_str() != Some(thread_id));
        self.turn_observations
            .lock()
            .await
            .retain(|(event_thread_id, _), _| event_thread_id != thread_id);
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

#[derive(Default)]
struct TurnObservations {
    agent_message: Option<String>,
    web_search_observed: bool,
    rerouted_from: Option<String>,
    rerouted_to: Option<String>,
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

#[tauri::command]
pub async fn run_codex_gate0_smoke(
    app: tauri::AppHandle,
    supervisor: tauri::State<'_, AppServerSupervisor>,
) -> Result<Gate0SmokeReport, String> {
    let mut guard = supervisor.session.lock().await;
    run_gate0_smoke(&app, &mut guard)
        .await
        .map_err(|error| error.to_string())
}

async fn run_gate0_smoke(
    app: &tauri::AppHandle,
    slot: &mut Option<ManagedAppServer>,
) -> Result<Gate0SmokeReport, AppServerError> {
    let account_result = supervised_request(
        app,
        slot,
        "account/read",
        Some(json!({ "refreshToken": false })),
    )
    .await?;
    if parse_account(&account_result)?.auth_mode.as_deref() != Some("chatgpt") {
        return Err(AppServerError::Protocol(
            "Gate 0スモークを実行するにはCodexへのログインが必要です".into(),
        ));
    }

    let workspace = slot
        .as_ref()
        .map(|session| PathBuf::from(&session.codex_home).join("workspace"))
        .ok_or_else(|| AppServerError::Protocol("常駐セッションがありません".into()))?;
    let thread_result = supervised_request(
        app,
        slot,
        "thread/start",
        Some(json!({
            "cwd": workspace,
            "approvalPolicy": "never",
            "sandbox": "read-only",
            "serviceName": "genshin_reco_gate0",
            "developerInstructions": "ローカルコマンドとファイル操作を使わず、指定されたWeb検索とJSON出力だけを行ってください。",
            "ephemeral": true,
            "experimentalRawEvents": false,
            "persistExtendedHistory": false
        })),
    )
    .await?;
    let thread_id = match required_json_string(&thread_result, &["thread", "id"]) {
        Ok(thread_id) => thread_id,
        Err(error) => {
            if let Some(session) = slot.take() {
                session.rpc.shutdown().await;
            }
            return Err(error);
        }
    };

    let smoke_result = async {
        let instruction_sources_supported =
            validate_instruction_sources(&thread_result, &workspace)?;
        run_gate0_smoke_turns(app, slot, &thread_id, instruction_sources_supported).await
    }
    .await;
    let cleanup_result = cleanup_smoke_thread(slot, &thread_id).await;
    match (smoke_result, cleanup_result) {
        (Ok(report), Ok(())) => Ok(report),
        (Err(error), _) => {
            if let Some(session) = slot.take() {
                session.rpc.shutdown().await;
            }
            Err(error)
        }
        (Ok(_), Err(error)) => Err(error),
    }
}

async fn run_gate0_smoke_turns(
    app: &tauri::AppHandle,
    slot: &mut Option<ManagedAppServer>,
    thread_id: &str,
    instruction_sources_supported: bool,
) -> Result<Gate0SmokeReport, AppServerError> {
    let turn_result = supervised_request(
        app,
        slot,
        "turn/start",
        Some(json!({
            "threadId": thread_id,
            "input": [{
                "type": "text",
                "text": "Web検索を必ず1回使い、HoYoWikiの原神トップページを確認してください。確認後、markerはgate0、okはtrue、sourceUrlは実際に確認したURLとして出力してください。",
                "text_elements": []
            }],
            "outputSchema": {
                "type": "object",
                "properties": {
                    "marker": { "type": "string", "const": "gate0" },
                    "ok": { "type": "boolean", "const": true },
                    "sourceUrl": { "type": "string" }
                },
                "required": ["marker", "ok", "sourceUrl"],
                "additionalProperties": false
            }
        })),
    )
    .await?;
    let structured_turn_id = required_json_string(&turn_result, &["turn", "id"])?;
    let structured_completion =
        supervised_wait_for_turn(slot, thread_id, &structured_turn_id).await?;
    if structured_completion["params"]["turn"]["status"].as_str() != Some("completed") {
        return Err(AppServerError::Protocol(
            "構造化出力ターンが正常完了しませんでした".into(),
        ));
    }
    let observations = slot
        .as_ref()
        .ok_or_else(|| AppServerError::Protocol("常駐セッションがありません".into()))?
        .rpc
        .take_turn_observations(thread_id, &structured_turn_id)
        .await;
    let structured_output_valid =
        validate_gate0_structured_output(observations.agent_message.as_deref())?;

    let cancel_turn_result = supervised_request(
        app,
        slot,
        "turn/start",
        Some(json!({
            "threadId": thread_id,
            "input": [{
                "type": "text",
                "text": "Web検索を使って原神の全キャラクターを調査し、長い報告書を作成してください。",
                "text_elements": []
            }]
        })),
    )
    .await?;
    let cancel_turn_id = required_json_string(&cancel_turn_result, &["turn", "id"])?;
    if cancel_turn_result["turn"]["status"].as_str() != Some("inProgress") {
        return Err(AppServerError::Protocol(
            "中断対象ターンが開始時点で実行中ではないため、中断を検証できませんでした".into(),
        ));
    }
    if let Err(error) = supervised_request(
        app,
        slot,
        "turn/interrupt",
        Some(json!({ "threadId": thread_id, "turnId": cancel_turn_id })),
    )
    .await
    {
        if let Some(session) = slot.take() {
            session.rpc.shutdown().await;
        }
        return Err(AppServerError::Protocol(format!(
            "ターンが先に完了したか、中断要求に失敗したため中断を検証できませんでした: {error}"
        )));
    }
    let cancel_completion = supervised_wait_for_turn(slot, thread_id, &cancel_turn_id).await?;
    let cancellation_observed =
        cancel_completion["params"]["turn"]["status"].as_str() == Some("interrupted");
    if !cancellation_observed {
        return Err(AppServerError::Protocol(
            "ターンの完了が中断より先行したため、中断を検証できませんでした".into(),
        ));
    }

    Ok(Gate0SmokeReport {
        structured_output_valid,
        web_search_observed: observations.web_search_observed,
        cancellation_observed,
        instruction_sources_supported,
        model_rerouted: observations.rerouted_to.is_some(),
        rerouted_from: observations.rerouted_from,
        rerouted_to: observations.rerouted_to,
    })
}

async fn cleanup_smoke_thread(
    slot: &mut Option<ManagedAppServer>,
    thread_id: &str,
) -> Result<(), AppServerError> {
    let result = match slot.as_mut() {
        Some(session) => session
            .request("thread/unsubscribe", Some(json!({ "threadId": thread_id })))
            .await
            .map(|_| ()),
        None => return Ok(()),
    };
    if let Some(session) = slot.as_ref() {
        session.rpc.clear_thread_state(thread_id).await;
    }
    if result.is_err()
        && let Some(session) = slot.take()
    {
        session.rpc.shutdown().await;
    }
    result.map_err(|error| {
        AppServerError::Protocol(format!(
            "Gate 0スモーク用スレッドの購読解除に失敗しました: {error}"
        ))
    })
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

async fn supervised_wait_for_turn(
    slot: &mut Option<ManagedAppServer>,
    thread_id: &str,
    turn_id: &str,
) -> Result<Value, AppServerError> {
    let result = match slot.as_mut() {
        Some(session) => {
            session
                .rpc
                .wait_for_turn_completion(thread_id, turn_id)
                .await
        }
        None => Err(AppServerError::Protocol(
            "常駐セッションがありません".into(),
        )),
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

fn required_json_string(value: &Value, path: &[&str]) -> Result<String, AppServerError> {
    let mut current = value;
    for segment in path {
        current = current
            .get(segment)
            .ok_or_else(|| AppServerError::Protocol(format!("{}がありません", path.join("."))))?;
    }
    current.as_str().map(str::to_string).ok_or_else(|| {
        AppServerError::Protocol(format!("{}が文字列ではありません", path.join(".")))
    })
}

fn validate_instruction_sources(
    thread_result: &Value,
    workspace: &Path,
) -> Result<bool, AppServerError> {
    let Some(sources) = thread_result.get("instructionSources") else {
        return Ok(false);
    };
    let sources = sources
        .as_array()
        .ok_or_else(|| AppServerError::Protocol("instructionSourcesが配列ではありません".into()))?;
    if sources.is_empty() {
        return Err(AppServerError::Protocol(
            "アプリ専用AGENTS.mdが指示元へ読み込まれていません".into(),
        ));
    }
    let expected = workspace.join("AGENTS.md").to_string_lossy().to_string();
    for source in sources {
        let source = source.as_str().ok_or_else(|| {
            AppServerError::Protocol("instructionSourcesに文字列以外が含まれています".into())
        })?;
        if !source.eq_ignore_ascii_case(&expected) {
            return Err(AppServerError::Protocol(format!(
                "想定外の指示ファイルを検出しました: {source}"
            )));
        }
    }
    Ok(true)
}

fn validate_gate0_structured_output(message: Option<&str>) -> Result<bool, AppServerError> {
    let message =
        message.ok_or_else(|| AppServerError::Protocol("最終agentMessageがありません".into()))?;
    let output: Value = serde_json::from_str(message)?;
    let Some(object) = output.as_object() else {
        return Ok(false);
    };
    let source_url = output
        .get("sourceUrl")
        .and_then(Value::as_str)
        .unwrap_or_default();
    Ok(object.len() == 3
        && output.get("marker").and_then(Value::as_str) == Some("gate0")
        && output.get("ok").and_then(Value::as_bool) == Some(true)
        && source_url.starts_with("https://wiki.hoyolab.com/"))
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

    #[tokio::test]
    async fn ターン完了通知を通常通知の上限から分離する() {
        let notifications = Arc::new(Mutex::new(VecDeque::new()));
        let turn_completions = Arc::new(Mutex::new(VecDeque::new()));
        let turn_observations = Arc::new(Mutex::new(HashMap::new()));
        route_notification(
            &notifications,
            &turn_completions,
            &turn_observations,
            json!({
                "method": "item/started",
                "params": {
                    "threadId": "thread-1",
                    "turnId": "turn-1",
                    "item": { "type": "webSearch" }
                }
            }),
        )
        .await;
        for index in 0..=MAX_NOTIFICATION_MESSAGES {
            route_notification(
                &notifications,
                &turn_completions,
                &turn_observations,
                json!({
                    "method": "item/started",
                    "params": {
                        "threadId": "thread-1",
                        "turnId": "turn-1",
                        "index": index,
                        "item": { "type": "commandExecution" }
                    }
                }),
            )
            .await;
        }
        route_notification(
            &notifications,
            &turn_completions,
            &turn_observations,
            json!({
                "method": "turn/completed",
                "params": {
                    "threadId": "thread-1",
                    "turn": { "id": "turn-1", "status": "completed" }
                }
            }),
        )
        .await;

        assert_eq!(notifications.lock().await.len(), MAX_NOTIFICATION_MESSAGES);
        let completions = turn_completions.lock().await;
        assert_eq!(completions.len(), 1);
        assert_eq!(completions[0]["params"]["turn"]["id"], "turn-1");
        drop(completions);
        assert!(
            turn_observations
                .lock()
                .await
                .get(&("thread-1".into(), "turn-1".into()))
                .expect("ターン観測があること")
                .web_search_observed
        );
    }

    #[tokio::test]
    async fn 高頻度の差分通知を保持しない() {
        let notifications = Arc::new(Mutex::new(VecDeque::new()));
        let turn_completions = Arc::new(Mutex::new(VecDeque::new()));
        let turn_observations = Arc::new(Mutex::new(HashMap::new()));
        route_notification(
            &notifications,
            &turn_completions,
            &turn_observations,
            json!({
                "method": "item/agentMessage/delta",
                "params": { "threadId": "thread-1", "turnId": "turn-1", "delta": "a" }
            }),
        )
        .await;

        assert!(notifications.lock().await.is_empty());
        assert!(turn_completions.lock().await.is_empty());
        assert!(turn_observations.lock().await.is_empty());
    }

    #[tokio::test]
    async fn ターン完了通知キューを上限件数に保つ() {
        let notifications = Arc::new(Mutex::new(VecDeque::new()));
        let turn_completions = Arc::new(Mutex::new(VecDeque::new()));
        let turn_observations = Arc::new(Mutex::new(HashMap::new()));
        for index in 0..=MAX_TURN_COMPLETIONS {
            route_notification(
                &notifications,
                &turn_completions,
                &turn_observations,
                json!({
                    "method": "turn/completed",
                    "params": {
                        "threadId": "thread-1",
                        "turn": { "id": format!("turn-{index}"), "status": "completed" }
                    }
                }),
            )
            .await;
        }

        let completions = turn_completions.lock().await;
        assert_eq!(completions.len(), MAX_TURN_COMPLETIONS);
        assert_eq!(completions[0]["params"]["turn"]["id"], "turn-1");
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

    #[test]
    fn gate0の構造化出力を検証できる() {
        let message = r#"{"marker":"gate0","ok":true,"sourceUrl":"https://wiki.hoyolab.com/pc/genshin/home"}"#;
        assert!(validate_gate0_structured_output(Some(message)).expect("検証できること"));
        assert!(
            !validate_gate0_structured_output(Some(
                r#"{"marker":"gate0","ok":true,"sourceUrl":"https://example.com/"}"#
            ))
            .expect("不許可URLを不合格にできること")
        );
        assert!(
            !validate_gate0_structured_output(Some(
                r#"{"marker":"gate0","ok":true,"sourceUrl":"https://wiki.hoyolab.com/pc/genshin/home","extra":true}"#
            ))
            .expect("余分なフィールドを不合格にできること")
        );
    }

    #[test]
    fn 想定外のinstruction_sourceを拒否する() {
        let workspace = Path::new(r"C:\app\codex-home\workspace");
        assert!(
            !validate_instruction_sources(&json!({ "thread": { "id": "thread-1" } }), workspace)
                .expect("未対応版を判定できること")
        );
        assert!(
            validate_instruction_sources(
                &json!({
                    "instructionSources": [r"C:\app\codex-home\workspace\AGENTS.md"]
                }),
                workspace
            )
            .expect("専用指示だけを許可できること")
        );
        assert!(
            validate_instruction_sources(
                &json!({ "instructionSources": [r"C:\Users\user\AGENTS.md"] }),
                workspace
            )
            .is_err()
        );
        assert!(
            validate_instruction_sources(&json!({ "instructionSources": [] }), workspace).is_err()
        );
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
