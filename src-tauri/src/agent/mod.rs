pub mod prompts;
pub mod tools;

use crate::db;
use crate::db::sites::Site;
use crate::error::AppResult;
use crate::scan::crawler::{CrawlStats, ProgressCb};
use crate::state::AppState;
use tokio_util::sync::CancellationToken;

pub async fn run_agentic_crawl(
    state: &AppState,
    site: &Site,
    scan_id: i64,
    cancel: &CancellationToken,
    progress: &ProgressCb,
) -> AppResult<CrawlStats> {
    let mut session = tools::AgentSession::new(site.clone(), scan_id);
    let user = format!(
        "Crawl the docs site.\nSeed: {}\nAllowlist hosts: {:?}\nMax pages: {}\nMax links: {}\nStart by listing the frontier, then fetch the seed, extract links, and check them.",
        site.seed_url,
        crate::scan::crawler::allowlist_for(site),
        site.max_pages,
        site.max_links
    );
    tools::run_loop(
        state,
        &mut session,
        prompts::CRAWL_SYSTEM,
        user,
        prompts::crawl_tools(),
        cancel,
        Some(progress),
    )
    .await?;
    Ok(session.stats)
}

pub async fn run_triage(
    state: &AppState,
    site: &Site,
    scan_id: i64,
    cancel: &CancellationToken,
) -> AppResult<()> {
    let conn = db::conn(&state.db)?;
    let broken = db::scans::list_links(&conn, scan_id, true)?;
    drop(conn);
    if broken.is_empty() {
        return Ok(());
    }
    let mut lines = String::new();
    for (i, link) in broken.iter().take(60).enumerate() {
        lines.push_str(&format!(
            "{}. {} <- {} status={:?} err={:?}\n",
            i + 1,
            link.target_url,
            link.source_url,
            link.status_code,
            link.error
        ));
    }
    let user = format!(
        "Triage broken links for site '{}' ({})\n{} findings (showing up to 60):\n{}",
        site.name,
        site.seed_url,
        broken.len(),
        lines
    );
    let mut session = tools::AgentSession::new(site.clone(), scan_id);
    session.stats.broken = broken.len() as i64;
    tools::run_loop(
        state,
        &mut session,
        prompts::TRIAGE_SYSTEM,
        user,
        prompts::triage_tools(),
        cancel,
        None,
    )
    .await?;
    Ok(())
}
