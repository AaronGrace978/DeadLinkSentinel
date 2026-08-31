use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::db::now_rfc3339;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Site {
    pub id: i64,
    pub name: String,
    pub seed_url: String,
    pub allowlist_hosts: String,
    pub schedule_cron: String,
    pub mode: String,
    pub enabled: bool,
    pub max_pages: i64,
    pub max_links: i64,
    pub concurrency: i64,
    pub timeout_secs: i64,
    pub triage_enabled: bool,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct SiteInput {
    pub name: String,
    pub seed_url: String,
    pub allowlist_hosts: Option<String>,
    pub schedule_cron: Option<String>,
    pub mode: Option<String>,
    pub enabled: Option<bool>,
    pub max_pages: Option<i64>,
    pub max_links: Option<i64>,
    pub concurrency: Option<i64>,
    pub timeout_secs: Option<i64>,
    pub triage_enabled: Option<bool>,
}

fn row_to_site(row: &rusqlite::Row<'_>) -> rusqlite::Result<Site> {
    Ok(Site {
        id: row.get(0)?,
        name: row.get(1)?,
        seed_url: row.get(2)?,
        allowlist_hosts: row.get(3)?,
        schedule_cron: row.get(4)?,
        mode: row.get(5)?,
        enabled: row.get::<_, i64>(6)? != 0,
        max_pages: row.get(7)?,
        max_links: row.get(8)?,
        concurrency: row.get(9)?,
        timeout_secs: row.get(10)?,
        triage_enabled: row.get::<_, i64>(11)? != 0,
        created_at: row.get(12)?,
    })
}

const COLS: &str = "id, name, seed_url, allowlist_hosts, schedule_cron, mode, enabled,
    max_pages, max_links, concurrency, timeout_secs, triage_enabled, created_at";

pub fn list(conn: &Connection) -> AppResult<Vec<Site>> {
    let mut stmt = conn.prepare(&format!("SELECT {COLS} FROM sites ORDER BY name COLLATE NOCASE"))?;
    let rows = stmt.query_map([], row_to_site)?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn get(conn: &Connection, id: i64) -> AppResult<Site> {
    let mut stmt = conn.prepare(&format!("SELECT {COLS} FROM sites WHERE id = ?1"))?;
    stmt.query_row([id], row_to_site)
        .map_err(|_| AppError::msg(format!("site {id} not found")))
}

pub fn create(conn: &Connection, input: &SiteInput) -> AppResult<Site> {
    validate(input)?;
    let now = now_rfc3339();
    conn.execute(
        "INSERT INTO sites (name, seed_url, allowlist_hosts, schedule_cron, mode, enabled,
            max_pages, max_links, concurrency, timeout_secs, triage_enabled, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        rusqlite::params![
            input.name.trim(),
            input.seed_url.trim(),
            input.allowlist_hosts.clone().unwrap_or_default(),
            input
                .schedule_cron
                .clone()
                .unwrap_or_else(|| "0 0 * * *".into()),
            input.mode.clone().unwrap_or_else(|| "deterministic".into()),
            if input.enabled.unwrap_or(true) { 1 } else { 0 },
            input.max_pages.unwrap_or(100),
            input.max_links.unwrap_or(500),
            input.concurrency.unwrap_or(8),
            input.timeout_secs.unwrap_or(15),
            if input.triage_enabled.unwrap_or(true) {
                1
            } else {
                0
            },
            now,
        ],
    )?;
    get(conn, conn.last_insert_rowid())
}

pub fn update(conn: &Connection, id: i64, input: &SiteInput) -> AppResult<Site> {
    validate(input)?;
    let n = conn.execute(
        "UPDATE sites SET name=?1, seed_url=?2, allowlist_hosts=?3, schedule_cron=?4, mode=?5,
            enabled=?6, max_pages=?7, max_links=?8, concurrency=?9, timeout_secs=?10,
            triage_enabled=?11 WHERE id=?12",
        rusqlite::params![
            input.name.trim(),
            input.seed_url.trim(),
            input.allowlist_hosts.clone().unwrap_or_default(),
            input
                .schedule_cron
                .clone()
                .unwrap_or_else(|| "0 0 * * *".into()),
            input.mode.clone().unwrap_or_else(|| "deterministic".into()),
            if input.enabled.unwrap_or(true) { 1 } else { 0 },
            input.max_pages.unwrap_or(100),
            input.max_links.unwrap_or(500),
            input.concurrency.unwrap_or(8),
            input.timeout_secs.unwrap_or(15),
            if input.triage_enabled.unwrap_or(true) {
                1
            } else {
                0
            },
            id,
        ],
    )?;
    if n == 0 {
        return Err(AppError::msg(format!("site {id} not found")));
    }
    get(conn, id)
}

pub fn delete(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute("DELETE FROM sites WHERE id = ?1", [id])?;
    Ok(())
}

fn validate(input: &SiteInput) -> AppResult<()> {
    if input.name.trim().is_empty() {
        return Err(AppError::msg("name is required"));
    }
    let url = url::Url::parse(input.seed_url.trim())
        .map_err(|_| AppError::msg("seed URL must be a valid http(s) URL"))?;
    if url.scheme() != "http" && url.scheme() != "https" {
        return Err(AppError::msg("seed URL must be http or https"));
    }
    let mode = input.mode.as_deref().unwrap_or("deterministic");
    if mode != "deterministic" && mode != "agentic" {
        return Err(AppError::msg("mode must be deterministic or agentic"));
    }
    Ok(())
}
