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
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::Mutex,
    task::JoinHandle,
    time::{Instant, timeout},
};

const SUPPORTED_CODEX_MAJOR: u64 = 0;
const SUPPORTED_CODEX_MINOR: u64 = 118;
const RPC_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_DIAGNOSTIC_LINES: usize = 100;

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

struct JsonlRpcSession {
    child: Child,
    stdin: ChildStdin,
    stdout_lines: Lines<BufReader<ChildStdout>>,
    stderr_lines: Arc<Mutex<VecDeque<String>>>,
    stderr_task: JoinHandle<()>,
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

        Ok(Self {
            child,
            stdin,
            stdout_lines: BufReader::new(stdout).lines(),
            stderr_lines,
            stderr_task,
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

            let line = timeout(remaining, self.stdout_lines.next_line())
                .await
                .map_err(|_| AppServerError::Timeout)??
                .ok_or(AppServerError::ProcessExited)?;
            let response: Value = serde_json::from_str(&line)?;
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
        let mut bytes = serde_json::to_vec(message)?;
        bytes.push(b'\n');
        self.stdin.write_all(&bytes).await?;
        self.stdin.flush().await?;
        Ok(())
    }

    async fn diagnostics(&self) -> Vec<String> {
        self.stderr_lines.lock().await.iter().cloned().collect()
    }

    async fn shutdown(mut self) {
        let _ = self.stdin.shutdown().await;
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
        self.stderr_task.abort();
        let _ = self.stderr_task.await;
    }
}

#[tauri::command]
pub async fn probe_codex_environment(app: tauri::AppHandle) -> Result<Gate0ProbeReport, String> {
    probe(&app).await.map_err(|error| error.to_string())
}

async fn probe(app: &tauri::AppHandle) -> Result<Gate0ProbeReport, AppServerError> {
    let codex = detect_codex().await?;
    let version_supported = is_supported_version(&codex.version);
    let isolated_home = app
        .path()
        .app_data_dir()
        .map_err(|error| AppServerError::IsolatedHome(error.to_string()))?
        .join("codex-home");
    tokio::fs::create_dir_all(&isolated_home)
        .await
        .map_err(|error| AppServerError::IsolatedHome(error.to_string()))?;

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

    let mut session = JsonlRpcSession::start(&codex, &isolated_home).await?;
    let probe_result = probe_session(&mut session, &isolated_home, &mut report).await;
    if let Err(error) = &probe_result {
        report.diagnostics.extend(session.diagnostics().await);
        report.diagnostics.push(error.to_string());
    }
    session.shutdown().await;
    probe_result.map(|_| report)
}

async fn probe_session(
    session: &mut JsonlRpcSession,
    isolated_home: &Path,
    report: &mut Gate0ProbeReport,
) -> Result<(), AppServerError> {
    let initialized = session
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
    validate_codex_home(&initialized, isolated_home)?;
    report.platform_family = initialized
        .get("platformFamily")
        .and_then(Value::as_str)
        .map(str::to_string);
    report.platform_os = initialized
        .get("platformOs")
        .and_then(Value::as_str)
        .map(str::to_string);
    session.notify("initialized", json!({})).await?;
    report.app_server_initialized = true;

    let account_result = session
        .request(1, "account/read", Some(json!({ "refreshToken": false })))
        .await?;
    let account = parse_account(&account_result)?;
    let is_chatgpt = account.auth_mode.as_deref() == Some("chatgpt");
    report.account = Some(account);

    if is_chatgpt {
        match session.request(2, "account/rateLimits/read", None).await {
            Ok(result) => {
                report.rate_limits_available = result.get("rateLimits").is_some();
            }
            Err(error) => report
                .diagnostics
                .push(format!("利用上限情報を取得できませんでした: {error}")),
        }
    }

    Ok(())
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
