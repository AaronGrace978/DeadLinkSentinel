use crate::db;
use crate::db::sites::SiteInput;
use crate::error::AppResult;
use crate::scan;
use crate::state::AppState;
use tauri::{AppHandle, State};

#[tauri::command]
pub fn list_sites(state: State<'_, AppState>) -> AppResult<Vec<db::sites::Site>> {
    let conn = db::conn(&state.db)?;
    db::sites::list(&conn)
}

#[tauri::command]
pub fn get_site(state: State<'_, AppState>, id: i64) -> AppResult<db::sites::Site> {
    let conn = db::conn(&state.db)?;
    db::sites::get(&conn, id)
}

#[tauri::command]
pub async fn create_site(
    app: AppHandle,
    state: State<'_, AppState>,
    input: SiteInput,
) -> AppResult<db::sites::Site> {
    let site = {
        let conn = db::conn(&state.db)?;
        db::sites::create(&conn, &input)?
    };
    let _ = scan::scheduler::reload(&app).await;
    Ok(site)
}

#[tauri::command]
pub async fn update_site(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    input: SiteInput,
) -> AppResult<db::sites::Site> {
    let site = {
        let conn = db::conn(&state.db)?;
        db::sites::update(&conn, id, &input)?
    };
    let _ = scan::scheduler::reload(&app).await;
    Ok(site)
}

#[tauri::command]
pub async fn delete_site(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
) -> AppResult<()> {
    {
        let conn = db::conn(&state.db)?;
        db::sites::delete(&conn, id)?;
    }
    let _ = scan::scheduler::reload(&app).await;
    Ok(())
}

#[tauri::command]
pub async fn run_scan_now(app: AppHandle, site_id: i64) -> AppResult<i64> {
    scan::run_site_scan(app, site_id).await
}

#[tauri::command]
pub fn cancel_scan(state: State<'_, AppState>, site_id: i64) -> bool {
    scan::cancel_site_scan(&state, site_id)
}

#[tauri::command]
pub fn list_scans(
    state: State<'_, AppState>,
    site_id: Option<i64>,
    limit: Option<i64>,
) -> AppResult<Vec<db::scans::Scan>> {
    let conn = db::conn(&state.db)?;
    db::scans::list(&conn, site_id, limit.unwrap_or(50))
}

#[tauri::command]
pub fn get_scan(state: State<'_, AppState>, id: i64) -> AppResult<db::scans::Scan> {
    let conn = db::conn(&state.db)?;
    db::scans::get(&conn, id)
}

#[tauri::command]
pub fn list_scan_links(
    state: State<'_, AppState>,
    scan_id: i64,
    broken_only: Option<bool>,
) -> AppResult<Vec<db::scans::LinkResult>> {
    let conn = db::conn(&state.db)?;
    db::scans::list_links(&conn, scan_id, broken_only.unwrap_or(false))
}

#[tauri::command]
pub fn list_triage_notes(
    state: State<'_, AppState>,
    scan_id: i64,
) -> AppResult<Vec<db::triage::TriageNote>> {
    let conn = db::conn(&state.db)?;
    db::triage::list_for_scan(&conn, scan_id)
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> AppResult<db::settings::AppSettings> {
    let conn = db::conn(&state.db)?;
    db::settings::load(&conn)
}

#[tauri::command]
pub async fn save_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    patch: db::settings::SettingsPatch,
) -> AppResult<db::settings::AppSettings> {
    let restart_status = patch.status_bind_addr.is_some() || patch.status_bind_port.is_some();
    {
        let conn = db::conn(&state.db)?;
        db::settings::apply_patch(&conn, &patch)?;
    }
    if restart_status {
        let _ = crate::status::start(&app).await;
    }
    let conn = db::conn(&state.db)?;
    db::settings::load(&conn)
}

#[tauri::command]
pub async fn send_test_email(state: State<'_, AppState>) -> AppResult<String> {
    crate::alert::send_test(&state).await
}

#[tauri::command]
pub fn get_status_url(state: State<'_, AppState>) -> String {
    crate::status::current_url(&state)
}

#[tauri::command]
pub fn get_public_status(state: State<'_, AppState>) -> AppResult<crate::status::PublicStatus> {
    crate::status::compute(&state.db)
}

#[tauri::command]
pub async fn restart_status_server(app: AppHandle) -> AppResult<String> {
    crate::status::start(&app).await
}

#[tauri::command]
pub fn list_providers(state: State<'_, AppState>) -> AppResult<Vec<db::providers::ProviderConfig>> {
    let conn = db::conn(&state.db)?;
    db::providers::list(&conn)
}

#[tauri::command]
pub fn save_provider(
    state: State<'_, AppState>,
    id: String,
    patch: db::providers::ProviderPatch,
) -> AppResult<db::providers::ProviderConfig> {
    let conn = db::conn(&state.db)?;
    db::providers::update(&conn, &id, &patch)
}

#[tauri::command]
pub async fn test_provider(
    state: State<'_, AppState>,
    id: String,
) -> AppResult<String> {
    let provider = crate::llm::resolve_provider(&state, Some(&id))?;
    provider.test_connection().await
}

#[tauri::command]
pub async fn list_provider_models(
    state: State<'_, AppState>,
    id: String,
) -> AppResult<Vec<String>> {
    let provider = crate::llm::resolve_provider(&state, Some(&id))?;
    provider.list_models().await
}
