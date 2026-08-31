use scraper::{Html, Selector};
use std::collections::{HashSet, VecDeque};
use std::time::Duration;
use url::Url;

use super::checker::{check_url, fetch_html};
use crate::db;
use crate::db::sites::Site;
use crate::error::AppResult;
use crate::state::AppState;

#[derive(Debug, Clone, Default)]
pub struct CrawlStats {
    pub pages: i64,
    pub links: i64,
    pub broken: i64,
}

pub struct CrawlBudget {
    pub max_pages: usize,
    pub max_links: usize,
    pub concurrency: usize,
    pub timeout: Duration,
}

impl CrawlBudget {
    pub fn from_site(site: &Site) -> Self {
        Self {
            max_pages: site.max_pages.max(1) as usize,
            max_links: site.max_links.max(1) as usize,
            concurrency: site.concurrency.clamp(1, 32) as usize,
            timeout: Duration::from_secs(site.timeout_secs.clamp(3, 120) as u64),
        }
    }
}

pub fn allowlist_for(site: &Site) -> HashSet<String> {
    let mut hosts: HashSet<String> = db::parse_hosts(&site.allowlist_hosts).into_iter().collect();
    if let Ok(u) = Url::parse(&site.seed_url) {
        if let Some(h) = u.host_str() {
            hosts.insert(h.to_lowercase());
        }
    }
    hosts
}

pub fn host_allowed(url: &Url, allow: &HashSet<String>) -> bool {
    url.host_str()
        .map(|h| allow.contains(&h.to_lowercase()))
        .unwrap_or(false)
}

pub fn normalize_url(raw: &str, base: Option<&Url>) -> Option<Url> {
    let trimmed = raw.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('#')
        || trimmed.starts_with("javascript:")
        || trimmed.starts_with("mailto:")
        || trimmed.starts_with("tel:")
        || trimmed.starts_with("data:")
    {
        return None;
    }
    let mut url = if let Some(base) = base {
        base.join(trimmed).ok()?
    } else {
        Url::parse(trimmed).ok()?
    };
    if url.scheme() != "http" && url.scheme() != "https" {
        return None;
    }
    url.set_fragment(None);
    Some(url)
}

pub fn extract_links(html: &str, base: &Url) -> Vec<Url> {
    let doc = Html::parse_document(html);
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let selectors = ["a[href]", "link[href]", "img[src]", "script[src]"];
    for sel in selectors {
        let Ok(selector) = Selector::parse(sel) else {
            continue;
        };
        for el in doc.select(&selector) {
            let attr = if sel.contains("href") { "href" } else { "src" };
            if let Some(href) = el.value().attr(attr) {
                if let Some(u) = normalize_url(href, Some(base)) {
                    let key = u.to_string();
                    if seen.insert(key) {
                        out.push(u);
                    }
                }
            }
        }
    }
    out
}

pub fn is_probably_html(url: &Url) -> bool {
    let path = url.path().to_lowercase();
    const SKIP: &[&str] = &[
        ".png", ".jpg", ".jpeg", ".gif", ".svg", ".webp", ".ico", ".pdf", ".zip", ".gz", ".tgz",
        ".woff", ".woff2", ".ttf", ".eot", ".mp4", ".mp3", ".css", ".js", ".json", ".xml", ".map",
        ".rss",
    ];
    !SKIP.iter().any(|ext| path.ends_with(ext))
}

#[derive(Clone)]
pub struct ProgressCb {
    pub on_progress: std::sync::Arc<dyn Fn(CrawlStats, String) + Send + Sync>,
}

pub async fn run_deterministic(
    state: &AppState,
    site: &Site,
    scan_id: i64,
    seed: Option<&str>,
    already_visited: &HashSet<String>,
    already_checked: &HashSet<String>,
    stats: &mut CrawlStats,
    cancel: &tokio_util::sync::CancellationToken,
    progress: Option<&ProgressCb>,
) -> AppResult<()> {
    let budget = CrawlBudget::from_site(site);
    let allow = allowlist_for(site);
    let start = seed.unwrap_or(&site.seed_url);
    let Some(seed_url) = normalize_url(start, None) else {
        return Err(crate::error::AppError::msg("invalid seed URL"));
    };

    let mut queue: VecDeque<Url> = VecDeque::new();
    let mut visited: HashSet<String> = already_visited.clone();
    let mut checked: HashSet<String> = already_checked.clone();
    queue.push_back(seed_url);

    let client = &state.http;

    while let Some(page) = queue.pop_front() {
        if cancel.is_cancelled() {
            break;
        }
        if visited.len() >= budget.max_pages {
            break;
        }
        let page_s = page.to_string();
        if !visited.insert(page_s.clone()) {
            continue;
        }
        if !host_allowed(&page, &allow) {
            continue;
        }

        match fetch_html(client, &page_s, budget.timeout).await {
            Ok((final_url, html, code)) => {
                stats.pages += 1;
                if code >= 400 {
                    record_link(
                        state,
                        scan_id,
                        &page_s,
                        &page_s,
                        Some(code as i64),
                        None,
                        Some(&final_url),
                        true,
                        stats,
                    )?;
                }
                let base = Url::parse(&final_url).unwrap_or(page.clone());
                let links = extract_links(&html, &base);
                let mut to_check = Vec::new();
                for link in links {
                    let ls = link.to_string();
                    if host_allowed(&link, &allow) && is_probably_html(&link) && !visited.contains(&ls)
                    {
                        if visited.len() + queue.len() < budget.max_pages {
                            queue.push_back(link.clone());
                        }
                    }
                    if checked.len() + to_check.len() >= budget.max_links {
                        continue;
                    }
                    if checked.insert(ls.clone()) {
                        to_check.push((page_s.clone(), link));
                    }
                }
                check_batch(
                    state,
                    scan_id,
                    client,
                    &to_check,
                    budget.timeout,
                    budget.concurrency,
                    stats,
                    cancel,
                )
                .await?;
                if let Some(cb) = progress {
                    (cb.on_progress)(stats.clone(), page_s);
                }
            }
            Err(e) => {
                stats.pages += 1;
                record_link(
                    state,
                    scan_id,
                    &page_s,
                    &page_s,
                    None,
                    Some(&e),
                    None,
                    true,
                    stats,
                )?;
            }
        }
    }
    Ok(())
}

async fn check_batch(
    state: &AppState,
    scan_id: i64,
    client: &reqwest::Client,
    batch: &[(String, Url)],
    timeout: Duration,
    concurrency: usize,
    stats: &mut CrawlStats,
    cancel: &tokio_util::sync::CancellationToken,
) -> AppResult<()> {
    use futures::stream::{self, StreamExt};
    if batch.is_empty() {
        return Ok(());
    }
    let concurrency = concurrency.clamp(1, 32).min(batch.len());
    let owned: Vec<(String, String)> = batch
        .iter()
        .map(|(src, url)| (src.clone(), url.to_string()))
        .collect();
    let results = stream::iter(owned)
        .map(|(src, url_s)| {
            let client = client.clone();
            let cancel = cancel.clone();
            async move {
                if cancel.is_cancelled() {
                    return None;
                }
                let check = check_url(&client, &url_s, timeout).await;
                Some((src, url_s, check))
            }
        })
        .buffer_unordered(concurrency)
        .collect::<Vec<_>>()
        .await;

    for item in results.into_iter().flatten() {
        let (src, url_s, check) = item;
        record_link(
            state,
            scan_id,
            &src,
            &url_s,
            check.status_code.map(|c| c as i64),
            check.error.as_deref(),
            check.final_url.as_deref(),
            check.is_broken,
            stats,
        )?;
    }
    Ok(())
}

pub fn record_link(
    state: &AppState,
    scan_id: i64,
    source: &str,
    target: &str,
    status_code: Option<i64>,
    error: Option<&str>,
    final_url: Option<&str>,
    is_broken: bool,
    stats: &mut CrawlStats,
) -> AppResult<()> {
    stats.links += 1;
    if is_broken {
        stats.broken += 1;
    }
    let conn = db::conn(&state.db)?;
    db::scans::insert_link(
        &conn,
        scan_id,
        source,
        target,
        status_code,
        error,
        final_url,
        is_broken,
    )?;
    db::scans::update_progress(&conn, scan_id, stats.pages, stats.links, stats.broken)?;
    Ok(())
}

pub fn existing_checked(state: &AppState, scan_id: i64) -> AppResult<(HashSet<String>, HashSet<String>)> {
    let conn = db::conn(&state.db)?;
    let links = db::scans::list_links(&conn, scan_id, false)?;
    let mut checked = HashSet::new();
    let mut pages = HashSet::new();
    for l in links {
        checked.insert(l.target_url.clone());
        pages.insert(l.source_url);
    }
    Ok((pages, checked))
}
