use rusqlite::Connection;
use serde::Serialize;

use crate::db::now_rfc3339;
use crate::error::AppResult;

#[derive(Debug, Clone, Serialize)]
pub struct TriageNote {
    pub id: i64,
    pub scan_id: i64,
    pub classification: String,
    pub grouping_key: Option<String>,
    pub draft_text: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub created_at: String,
}

pub fn insert(
    conn: &Connection,
    scan_id: i64,
    classification: &str,
    grouping_key: Option<&str>,
    draft_text: &str,
    provider: Option<&str>,
    model: Option<&str>,
) -> AppResult<i64> {
    let now = now_rfc3339();
    conn.execute(
        "INSERT INTO triage_notes (scan_id, classification, grouping_key, draft_text, provider, model, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            scan_id,
            classification,
            grouping_key,
            draft_text,
            provider,
            model,
            now
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn list_for_scan(conn: &Connection, scan_id: i64) -> AppResult<Vec<TriageNote>> {
    let mut stmt = conn.prepare(
        "SELECT id, scan_id, classification, grouping_key, draft_text, provider, model, created_at
         FROM triage_notes WHERE scan_id = ?1 ORDER BY id",
    )?;
    let rows = stmt.query_map([scan_id], |row| {
        Ok(TriageNote {
            id: row.get(0)?,
            scan_id: row.get(1)?,
            classification: row.get(2)?,
            grouping_key: row.get(3)?,
            draft_text: row.get(4)?,
            provider: row.get(5)?,
            model: row.get(6)?,
            created_at: row.get(7)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}
