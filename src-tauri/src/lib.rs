mod agent;
mod alert;
mod commands;
mod db;
mod error;
mod llm;
mod scan;
mod secrets;
mod state;
mod status;

use std::fs;
use tauri::Manager;

use crate::state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "deadlink_sentinel_lib=info".into()),
        )
        .try_init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            fs::create_dir_all(&dir)?;
            let db_path = dir.join("deadlinksentinel.db");
            let pool = db::open_pool(&db_path)?;
            app.manage(AppState::new(pool));

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                match status::start(&handle).await {
                    Ok(url) => tracing::info!("status page at {url}"),
                    Err(e) => tracing::warn!("status server failed: {e}"),
                }
                if let Err(e) = scan::scheduler::start(&handle).await {
                    tracing::warn!("scheduler failed: {e}");
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_sites,
            commands::get_site,
            commands::create_site,
            commands::update_site,
            commands::delete_site,
            commands::run_scan_now,
            commands::cancel_scan,
            commands::list_scans,
            commands::get_scan,
            commands::list_scan_links,
            commands::list_triage_notes,
            commands::get_settings,
            commands::save_settings,
            commands::send_test_email,
            commands::get_status_url,
            commands::get_public_status,
            commands::restart_status_server,
            commands::list_providers,
            commands::save_provider,
            commands::test_provider,
            commands::list_provider_models,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
