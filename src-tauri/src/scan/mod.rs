pub mod checker;
pub mod crawler;
pub mod scheduler;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio_util::sync::CancellationToken;

use crate::db;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

#[derive(Clone, Serialize)]
pub struct ScanProgress {
    pub scan_id: i64,
    pub site_id: i64,
    pub pages_crawled: i64,
    pub links_checked: i64,
    pub broken_count: i64,
    pub current_url: String,
    pub status: String,
}

pub async fn run_site_scan(app: AppHandle, site_id: i64) -> AppResult<i64> {
    let state = app.state::<AppState>().inner().clone();
    {
        let conn = db::conn(&state.db)?;
        if db::scans::has_running(&conn, site_id)? {
            return Err(AppError::msg("a scan is already running for this site"));
        }
    }

    let site = {
        let conn = db::conn(&state.db)?;
        db::sites::get(&conn, site_id)?
    };

    let scan = {
        let conn = db::conn(&state.db)?;
        db::scans::start(&conn, site_id)?
    };
    let scan_id = scan.id;

    let cancel = CancellationToken::new();
    {
        let mut tokens = state.scan_tokens.lock().unwrap();
        tokens.insert(site_id, cancel.clone());
    }

    let result = run_inner(&app, &state, &site, scan_id, &cancel).await;

    {
        let mut tokens = state.scan_tokens.lock().unwrap();
        tokens.remove(&site_id);
    }

    match result {
        Ok(()) => {
            let conn = db::conn(&state.db)?;
            let finished = db::scans::get(&conn, scan_id)?;
            let _ = app.emit(
                "scan-progress",
                ScanProgress {
                    scan_id,
                    site_id,
                    pages_crawled: finished.pages_crawled,
                    links_checked: finished.links_checked,
                    broken_count: finished.broken_count,
                    current_url: String::new(),
                    status: finished.status,
                },
            );
            Ok(scan_id)
        }
        Err(e) => {
            let conn = db::conn(&state.db)?;
            let _ = db::scans::finish(&conn, scan_id, "failed", None, Some(&e.to_string()));
            Err(e)
        }
    }
}

async fn run_inner(
    app: &AppHandle,
    state: &AppState,
    site: &db::sites::Site,
    scan_id: i64,
    cancel: &CancellationToken,
) -> AppResult<()> {
    let mut stats = crawler::CrawlStats::default();
    let site_id = site.id;
    let progress = crawler::ProgressCb {
        on_progress: {
            let app = app.clone();
            std::sync::Arc::new(move |s: crawler::CrawlStats, url: String| {
                let _ = app.emit(
                    "scan-progress",
                    ScanProgress {
                        scan_id,
                        site_id,
                        pages_crawled: s.pages,
                        links_checked: s.links,
                        broken_count: s.broken,
                        current_url: url,
                        status: "running".into(),
                    },
                );
            })
        },
    };

    let _ = app.emit(
        "scan-progress",
        ScanProgress {
            scan_id,
            site_id,
            pages_crawled: 0,
            links_checked: 0,
            broken_count: 0,
            current_url: site.seed_url.clone(),
            status: "running".into(),
        },
    );

    if site.mode == "agentic" {
        match crate::agent::run_agentic_crawl(state, site, scan_id, cancel, &progress).await {
            Ok(s) => stats = s,
            Err(e) => tracing::warn!("agentic crawl fell back to deterministic: {e}"),
        }
    }

    if !cancel.is_cancelled() {
        let (visited, checked) = crawler::existing_checked(state, scan_id).unwrap_or_default();
        crawler::run_deterministic(
            state,
            site,
            scan_id,
            None,
            &visited,
            &checked,
            &mut stats,
            cancel,
            Some(&progress),
        )
        .await?;
    }

    let status = if cancel.is_cancelled() {
        "cancelled"
    } else {
        "completed"
    };

    if status == "completed" && site.triage_enabled && stats.broken > 0 {
        let _ = app.emit(
            "scan-progress",
            ScanProgress {
                scan_id,
                site_id,
                pages_crawled: stats.pages,
                links_checked: stats.links,
                broken_count: stats.broken,
                current_url: String::new(),
                status: "triaging".into(),
            },
        );
        if let Err(e) = crate::agent::run_triage(state, site, scan_id, cancel).await {
            tracing::warn!("triage failed: {e}");
        }
    }

    let summary = {
        let conn = db::conn(&state.db)?;
        db::scans::get(&conn, scan_id)?
            .summary
            .unwrap_or_else(|| {
                format!(
                    "Crawled {} pages, checked {} links, {} broken.",
                    stats.pages, stats.links, stats.broken
                )
            })
    };

    {
        let conn = db::conn(&state.db)?;
        db::scans::update_progress(&conn, scan_id, stats.pages, stats.links, stats.broken)?;
        db::scans::finish(
            &conn,
            scan_id,
            status,
            Some(&summary),
            if status == "cancelled" {
                Some("cancelled by user")
            } else {
                None
            },
        )?;
    }

    if status == "completed" {
        if let Err(e) = crate::alert::maybe_send_regression(state, site, scan_id).await {
            tracing::warn!("alert failed: {e}");
        }
    }

    Ok(())
}

pub fn cancel_site_scan(state: &AppState, site_id: i64) -> bool {
    let tokens = state.scan_tokens.lock().unwrap();
    if let Some(t) = tokens.get(&site_id) {
        t.cancel();
        true
    } else {
        false
    }
}
