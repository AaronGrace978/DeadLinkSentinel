use tauri::{AppHandle, Manager};
use tokio_cron_scheduler::{Job, JobScheduler};
use uuid::Uuid;

use crate::db;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

pub fn normalize_cron(expr: &str) -> String {
    let parts: Vec<&str> = expr.split_whitespace().collect();
    match parts.len() {
        5 => format!("0 {}", expr.trim()),
        6 | 7 => expr.trim().to_string(),
        _ => "0 0 0 * * *".into(),
    }
}

pub async fn start(app: &AppHandle) -> AppResult<()> {
    let sched = JobScheduler::new()
        .await
        .map_err(|e| AppError::msg(e.to_string()))?;
    sched
        .start()
        .await
        .map_err(|e| AppError::msg(e.to_string()))?;
    {
        let state = app.state::<AppState>();
        *state.scheduler.lock().await = Some(sched);
    }
    reload(app).await
}

pub async fn reload(app: &AppHandle) -> AppResult<()> {
    let state = app.state::<AppState>();
    let sites = {
        let conn = db::conn(&state.db)?;
        db::sites::list(&conn)?
    };

    let mut sched_guard = state.scheduler.lock().await;
    let Some(sched) = sched_guard.as_mut() else {
        return Ok(());
    };

    let old_ids: Vec<Uuid> = {
        let mut ids = state.job_ids.lock().unwrap();
        let vals: Vec<Uuid> = ids.values().copied().collect();
        ids.clear();
        vals
    };
    for uuid in old_ids {
        let _ = sched.remove(&uuid).await;
    }

    for site in sites {
        if !site.enabled {
            continue;
        }
        let cron = normalize_cron(&site.schedule_cron);
        let app_clone = app.clone();
        let site_id = site.id;
        let job = Job::new_async(cron.as_str(), move |_uuid, _lock| {
            let app = app_clone.clone();
            Box::pin(async move {
                tracing::info!("scheduled scan starting for site {site_id}");
                if let Err(e) = crate::scan::run_site_scan(app, site_id).await {
                    tracing::warn!("scheduled scan failed for site {site_id}: {e}");
                }
            }) as std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
        })
        .map_err(|e| AppError::msg(format!("invalid cron for site {}: {e}", site.name)))?;
        let uuid = sched
            .add(job)
            .await
            .map_err(|e| AppError::msg(e.to_string()))?;
        state.job_ids.lock().unwrap().insert(site.id, uuid);
    }
    Ok(())
}
