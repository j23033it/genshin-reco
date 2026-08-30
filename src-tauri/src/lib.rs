mod app_server;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(app_server::AppServerSupervisor::default())
        .invoke_handler(tauri::generate_handler![
            app_server::probe_codex_environment,
            app_server::start_codex_device_login,
            app_server::read_codex_login_status,
            app_server::cancel_codex_device_login
        ])
        .run(tauri::generate_context!())
        .expect("Tauriアプリケーションを起動できませんでした");
}
