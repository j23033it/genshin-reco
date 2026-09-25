mod analysis;
mod app_server;
pub mod candidate_validation;
pub mod catalog;
pub mod database;
mod database_commands;
pub mod domain;
pub mod hashing;
mod on_demand;
pub mod on_demand_domain;
mod on_demand_store;
pub mod reconciler;
pub mod research_provider;
pub mod solver;
pub mod source_policy;
mod tavily;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            database_commands::initialize_database(app.handle()).map_err(std::io::Error::other)?;
            Ok(())
        })
        .manage(app_server::AppServerSupervisor::default())
        .manage(analysis::AnalysisCoordinator::default())
        .manage(on_demand::OnDemandResearchCoordinator::default())
        .invoke_handler(tauri::generate_handler![
            app_server::probe_codex_environment,
            app_server::start_codex_device_login,
            app_server::read_codex_login_status,
            app_server::cancel_codex_device_login,
            app_server::run_codex_gate0_smoke,
            tavily::read_tavily_settings_status,
            tavily::save_tavily_api_key,
            tavily::test_tavily_connection,
            tavily::delete_tavily_api_key,
            catalog::load_catalog,
            database_commands::save_party_draft,
            database_commands::load_party_draft,
            database_commands::list_party_drafts,
            database_commands::delete_party_draft,
            database_commands::load_current_analysis_result,
            database_commands::save_analysis_variant_selection,
            analysis::start_analysis,
            analysis::cancel_analysis,
            on_demand::send_on_demand_message,
            on_demand::update_on_demand_conditions,
            on_demand::start_on_demand_research,
            on_demand::cancel_on_demand_research,
            on_demand::list_researched_teams,
            on_demand::load_researched_team,
            on_demand::load_on_demand_conversation
        ])
        .run(tauri::generate_context!())
        .expect("Tauriアプリケーションを起動できませんでした");
}
