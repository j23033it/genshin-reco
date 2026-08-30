mod app_server;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            app_server::probe_codex_environment
        ])
        .run(tauri::generate_context!())
        .expect("Tauriアプリケーションを起動できませんでした");
}
