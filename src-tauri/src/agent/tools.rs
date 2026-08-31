use std::collections::{HashSet, VecDeque};
use std::time::Duration;

use serde_json::{json, Value};
use url::Url;

use crate::db;
use crate::db::sites::Site;
use crate::error::{AppError, AppResult};
use crate::llm::{self, ChatRequest, Message};
use crate::scan::checker::{check_url, fetch_html};
use crate::scan::crawler::{
    self, allowlist_for, extract_links, host_allowed, is_probably_html, normalize_url, CrawlStats,
    ProgressCb,
};
use crate::state::AppState;

pub struct AgentSession {
    pub scan_id: i64,
    pub site: Site,
    pub allow: HashSet<String>,
    pub timeout: Duration,
    pub max_pages: usize,
    pub max_links: usize,
    pub frontier: VecDeque<String>,
    pub visited: HashSet<String>,
    pub checked: HashSet<String>,
    pub html: std::collections::HashMap<String, (String, String)>,
    pub stats: CrawlStats,
    pub finished: bool,
    pub summary: Option<String>,
    pub last_page: Option<String>,
}

impl AgentSession {
    pub fn new(site: Site, scan_id: i64) -> Self {
        let allow = allowlist_for(&site);
        let timeout = Duration::from_secs(site.timeout_secs.clamp(3, 120) as u64);
        let mut frontier = VecDeque::new();
        frontier.push_back(site.seed_url.clone());
        Self {
            scan_id,
            max_pages: site.max_pages.max(1) as usize,
            max_links: site.max_links.max(1) as usize,
            site,
            allow,
            timeout,
            frontier,
            visited: HashSet::new(),
            checked: HashSet::new(),
            html: std::collections::HashMap::new(),
            stats: CrawlStats::default(),
            finished: false,
            summary: None,
            last_page: None,
        }
    }
}

pub async fn dispatch(
    state: &AppState,
    session: &mut AgentSession,
    name: &str,
    args: &Value,
) -> AppResult<Value> {
    match name {
        "list_frontier" => Ok(json!({
            "frontier": session.frontier.iter().take(25).cloned().collect::<Vec<_>>(),
            "frontier_len": session.frontier.len(),
            "pages_used": session.stats.pages,
            "pages_max": session.max_pages,
            "links_used": session.stats.links,
            "links_max": session.max_links,
            "broken": session.stats.broken,
        })),
        "fetch_page" => {
            let url = arg_str(args, "url")?;
            fetch_tool(state, session, &url).await
        }
        "extract_links" => {
            let url = args
                .get("url")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| session.last_page.clone())
                .ok_or_else(|| AppError::msg("extract_links needs a url"))?;
            extract_tool(session, &url)
        }
        "check_url" => {
            let url = arg_str(args, "url")?;
            let source = args
                .get("source_url")
                .and_then(|v| v.as_str())
                .unwrap_or(session.last_page.as_deref().unwrap_or(&session.site.seed_url))
                .to_string();
            check_tool(state, session, &source, &url).await
        }
        "mark_page_done" => {
            let url = arg_str(args, "url")?;
            session.frontier.retain(|u| u != &url);
            session.visited.insert(url.clone());
            Ok(json!({ "ok": true, "url": url }))
        }
        "classify_failure" => {
            let target = arg_str(args, "target_url")?;
            let class = arg_str(args, "classification")?;
            let reason = args
                .get("reason")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Ok(json!({
                "ok": true,
                "target_url": target,
                "classification": class,
                "reason": reason
            }))
        }
        "write_triage_note" => {
            let classification = arg_str(args, "classification")?;
            let draft = arg_str(args, "draft_text")?;
            let grouping = args.get("grouping_key").and_then(|v| v.as_str());
            let conn = db::conn(&state.db)?;
            let settings = db::settings::load(&conn)?;
            let id = db::triage::insert(
                &conn,
                session.scan_id,
                &classification,
                grouping,
                &draft,
                Some(&settings.default_llm_provider),
                Some(&settings.default_llm_model),
            )?;
            Ok(json!({ "ok": true, "id": id }))
        }
        "finish_summary" => {
            let summary = arg_str(args, "summary")?;
            session.summary = Some(summary.clone());
            session.finished = true;
            let conn = db::conn(&state.db)?;
            conn.execute(
                "UPDATE scans SET summary = ?1 WHERE id = ?2",
                rusqlite::params![summary, session.scan_id],
            )?;
            Ok(json!({ "ok": true }))
        }
        other => Err(AppError::msg(format!("unknown tool {other}"))),
    }
}

fn arg_str(args: &Value, key: &str) -> AppResult<String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| AppError::msg(format!("missing string argument {key}")))
}

async fn fetch_tool(state: &AppState, session: &mut AgentSession, url: &str) -> AppResult<Value> {
    if session.stats.pages as usize >= session.max_pages {
        return Ok(json!({ "error": "page budget exhausted" }));
    }
    let Some(parsed) = normalize_url(url, None) else {
        return Ok(json!({ "error": "invalid url" }));
    };
    if !host_allowed(&parsed, &session.allow) {
        return Ok(json!({ "error": "url not on allowlist" }));
    }
    let url_s = parsed.to_string();
    match fetch_html(&state.http, &url_s, session.timeout).await {
        Ok((final_url, html, code)) => {
            session.stats.pages += 1;
            session.last_page = Some(final_url.clone());
            session.visited.insert(url_s.clone());
            session.frontier.retain(|u| u != &url_s && u != &final_url);
            let snippet: String = html.chars().take(1500).collect();
            session
                .html
                .insert(final_url.clone(), (final_url.clone(), html));
            Ok(json!({
                "final_url": final_url,
                "status_code": code,
                "snippet": snippet,
            }))
        }
        Err(e) => Ok(json!({ "error": e })),
    }
}

fn extract_tool(session: &mut AgentSession, url: &str) -> AppResult<Value> {
    let entry = session
        .html
        .get(url)
        .or_else(|| session.last_page.as_ref().and_then(|p| session.html.get(p)));
    let Some((final_url, html)) = entry.cloned() else {
        return Ok(json!({ "error": "page not fetched yet" }));
    };
    let base = Url::parse(&final_url).map_err(|e| AppError::msg(e.to_string()))?;
    let links = extract_links(&html, &base);
    let mut listed = Vec::new();
    for link in links {
        let ls = link.to_string();
        listed.push(ls.clone());
        if host_allowed(&link, &session.allow)
            && is_probably_html(&link)
            && !session.visited.contains(&ls)
            && !session.frontier.iter().any(|u| u == &ls)
            && session.visited.len() + session.frontier.len() < session.max_pages
        {
            session.frontier.push_back(ls);
        }
    }
    Ok(json!({
        "count": listed.len(),
        "links": listed.into_iter().take(80).collect::<Vec<_>>(),
        "frontier_len": session.frontier.len()
    }))
}

async fn check_tool(
    state: &AppState,
    session: &mut AgentSession,
    source: &str,
    url: &str,
) -> AppResult<Value> {
    if session.stats.links as usize >= session.max_links {
        return Ok(json!({ "error": "link budget exhausted" }));
    }
    let Some(parsed) = normalize_url(url, None) else {
        return Ok(json!({ "error": "invalid url" }));
    };
    let url_s = parsed.to_string();
    if !session.checked.insert(url_s.clone()) {
        return Ok(json!({ "skipped": true, "reason": "already checked" }));
    }
    let check = check_url(&state.http, &url_s, session.timeout).await;
    crawler::record_link(
        state,
        session.scan_id,
        source,
        &url_s,
        check.status_code.map(|c| c as i64),
        check.error.as_deref(),
        check.final_url.as_deref(),
        check.is_broken,
        &mut session.stats,
    )?;
    Ok(json!({
        "url": url_s,
        "status_code": check.status_code,
        "final_url": check.final_url,
        "error": check.error,
        "is_broken": check.is_broken
    }))
}

pub async fn run_loop(
    state: &AppState,
    session: &mut AgentSession,
    system: &str,
    user: String,
    tools: Vec<llm::ToolSpec>,
    cancel: &tokio_util::sync::CancellationToken,
    progress: Option<&ProgressCb>,
) -> AppResult<()> {
    let provider = llm::resolve_provider(state, None)?;
    let model = llm::default_model(state, Some(provider.id()))?;
    let conn = db::conn(&state.db)?;
    let max_steps: u32 = db::settings::get_or(&conn, "agent_max_steps", "40")?
        .parse()
        .unwrap_or(40);
    drop(conn);

    let mut messages = vec![Message::system(system), Message::user(user)];
    let mut idle = 0u32;

    for step in 0..max_steps {
        if cancel.is_cancelled() || session.finished {
            break;
        }
        let resp = provider
            .chat(ChatRequest {
                model: model.clone(),
                messages: messages.clone(),
                tools: tools.clone(),
            })
            .await?;
        messages.push(Message::assistant(resp.content.clone(), resp.tool_calls.clone()));
        if resp.tool_calls.is_empty() {
            idle += 1;
            if !resp.content.is_empty() && idle >= 2 {
                session.summary = Some(resp.content.clone());
                session.finished = true;
                break;
            }
            messages.push(Message::user(
                "You must call tools. If you are done, call finish_summary.",
            ));
            continue;
        }
        idle = 0;
        for call in resp.tool_calls {
            if cancel.is_cancelled() {
                break;
            }
            let result = match dispatch(state, session, &call.name, &call.arguments).await {
                Ok(v) => v,
                Err(e) => json!({ "error": e.to_string() }),
            };
            messages.push(Message::tool(&call.id, &call.name, result.to_string()));
            if let Some(cb) = progress {
                (cb.on_progress)(
                    session.stats.clone(),
                    format!("agent:{} step {step}", call.name),
                );
            }
        }
    }
    Ok(())
}
