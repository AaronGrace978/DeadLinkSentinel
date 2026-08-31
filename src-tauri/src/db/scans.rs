use rusqlite::Connection;
use serde::Serialize;

use crate::db::now_rfc3339;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize)]
pub struct Scan {
    pub id: i64,
    pub site_id: i64,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub status: String,
    pub pages_crawled: i64,
    pub links_checked: i64,
    pub broken_count: i64,
    pub summary: Option<String>,
    pub error: Option<String>,
    pub site_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LinkResult {
    pub id: i64,
    pub scan_id: i64,
    pub source_url: String,
    pub target_url: String,
    pub status_code: Option<i64>,
    pub error: Option<String>,
    pub final_url: Option<String>,
    pub is_broken: bool,
}

fn row_scan(row: &rusqlite::Row<'_>) -> rusqlite::Result<Scan> {
    Ok(Scan {
        id: row.get(0)?,
        site_id: row.get(1)?,
        started_at: row.get(2)?,
        finished_at: row.get(3)?,
        status: row.get(4)?,
        pages_crawled: row.get(5)?,
        links_checked: row.get(6)?,
        broken_count: row.get(7)?,
        summary: row.get(8)?,
        error: row.get(9)?,
        site_name: row.get(10)?,
    })
}

pub fn start(conn: &Connection, site_id: i64) -> AppResult<Scan> {
    let now = now_rfc3339();
    conn.execute(
        "INSERT INTO scans (site_id, started_at, status) VALUES (?1, ?2, 'running')",
        rusqlite::params![site_id, now],
    )?;
    get(conn, conn.last_insert_rowid())
}

pub fn get(conn: &Connection, id: i64) -> AppResult<Scan> {
    let mut stmt = conn.prepare(
        "SELECT s.id, s.site_id, s.started_at, s.finished_at, s.status, s.pages_crawled,
                s.links_checked, s.broken_count, s.summary, s.error, t.name
         FROM scans s LEFT JOIN sites t ON t.id = s.site_id WHERE s.id = ?1",
    )?;
    stmt.query_row([id], row_scan)
        .map_err(|_| AppError::msg(format!("scan {id} not found")))
}

pub fn list(conn: &Connection, site_id: Option<i64>, limit: i64) -> AppResult<Vec<Scan>> {
    if let Some(sid) = site_id {
        let mut stmt = conn.prepare(
            "SELECT s.id, s.site_id, s.started_at, s.finished_at, s.status, s.pages_crawled,
                    s.links_checked, s.broken_count, s.summary, s.error, t.name
             FROM scans s LEFT JOIN sites t ON t.id = s.site_id
             WHERE s.site_id = ?1 ORDER BY s.started_at DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(rusqlite::params![sid, limit], row_scan)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    } else {
        let mut stmt = conn.prepare(
            "SELECT s.id, s.site_id, s.started_at, s.finished_at, s.status, s.pages_crawled,
                    s.links_checked, s.broken_count, s.summary, s.error, t.name
             FROM scans s LEFT JOIN sites t ON t.id = s.site_id
             ORDER BY s.started_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit], row_scan)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
}

pub fn has_running(conn: &Connection, site_id: i64) -> AppResult<bool> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM scans WHERE site_id = ?1 AND status = 'running'",
        [site_id],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

pub fn update_progress(
    conn: &Connection,
    id: i64,
    pages: i64,
    links: i64,
    broken: i64,
) -> AppResult<()> {
    conn.execute(
        "UPDATE scans SET pages_crawled=?1, links_checked=?2, broken_count=?3 WHERE id=?4",
        rusqlite::params![pages, links, broken, id],
    )?;
    Ok(())
}

pub fn finish(
    conn: &Connection,
    id: i64,
    status: &str,
    summary: Option<&str>,
    error: Option<&str>,
) -> AppResult<()> {
    let now = now_rfc3339();
    conn.execute(
        "UPDATE scans SET status=?1, finished_at=?2, summary=?3, error=?4 WHERE id=?5",
        rusqlite::params![status, now, summary, error, id],
    )?;
    Ok(())
}

pub fn insert_link(
    conn: &Connection,
    scan_id: i64,
    source_url: &str,
    target_url: &str,
    status_code: Option<i64>,
    error: Option<&str>,
    final_url: Option<&str>,
    is_broken: bool,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO link_results (scan_id, source_url, target_url, status_code, error, final_url, is_broken)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            scan_id,
            source_url,
            target_url,
            status_code,
            error,
            final_url,
            if is_broken { 1 } else { 0 },
        ],
    )?;
    Ok(())
}

pub fn list_links(
    conn: &Connection,
    scan_id: i64,
    broken_only: bool,
) -> AppResult<Vec<LinkResult>> {
    let sql = if broken_only {
        "SELECT id, scan_id, source_url, target_url, status_code, error, final_url, is_broken
         FROM link_results WHERE scan_id = ?1 AND is_broken = 1 ORDER BY target_url"
    } else {
        "SELECT id, scan_id, source_url, target_url, status_code, error, final_url, is_broken
         FROM link_results WHERE scan_id = ?1 ORDER BY is_broken DESC, target_url"
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([scan_id], |row| {
        Ok(LinkResult {
            id: row.get(0)?,
            scan_id: row.get(1)?,
            source_url: row.get(2)?,
            target_url: row.get(3)?,
            status_code: row.get(4)?,
            error: row.get(5)?,
            final_url: row.get(6)?,
            is_broken: row.get::<_, i64>(7)? != 0,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn broken_targets(conn: &Connection, scan_id: i64) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT target_url FROM link_results WHERE scan_id = ?1 AND is_broken = 1",
    )?;
    let rows = stmt.query_map([scan_id], |r| r.get::<_, String>(0))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn previous_completed(conn: &Connection, site_id: i64, before_id: i64) -> AppResult<Option<Scan>> {
    let mut stmt = conn.prepare(
        "SELECT s.id, s.site_id, s.started_at, s.finished_at, s.status, s.pages_crawled,
                s.links_checked, s.broken_count, s.summary, s.error, t.name
         FROM scans s LEFT JOIN sites t ON t.id = s.site_id
         WHERE s.site_id = ?1 AND s.id < ?2 AND s.status = 'completed'
         ORDER BY s.id DESC LIMIT 1",
    )?;
    match stmt.query_row(rusqlite::params![site_id, before_id], row_scan) {
        Ok(s) => Ok(Some(s)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn latest_completed_per_site(conn: &Connection) -> AppResult<Vec<(Scan, String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT s.id, s.site_id, s.started_at, s.finished_at, s.status, s.pages_crawled,
                s.links_checked, s.broken_count, s.summary, s.error, t.name, t.seed_url
         FROM scans s
         JOIN sites t ON t.id = s.site_id
         WHERE s.id IN (
            SELECT MAX(id) FROM scans WHERE status = 'completed' GROUP BY site_id
         )
         ORDER BY t.name COLLATE NOCASE",
    )?;
    let mut rows = stmt.query([])?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        let scan = Scan {
            id: row.get(0)?,
            site_id: row.get(1)?,
            started_at: row.get(2)?,
            finished_at: row.get(3)?,
            status: row.get(4)?,
            pages_crawled: row.get(5)?,
            links_checked: row.get(6)?,
            broken_count: row.get(7)?,
            summary: row.get(8)?,
            error: row.get(9)?,
            site_name: row.get(10)?,
        };
        let name: String = row.get(10)?;
        let seed: String = row.get(11)?;
        out.push((scan, name, seed));
    }
    Ok(out)
}
