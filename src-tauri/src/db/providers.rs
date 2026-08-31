use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::secrets;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub id: String,
    pub enabled: bool,
    pub base_url: Option<String>,
    pub default_model: Option<String>,
    pub extra_json: Option<String>,
    pub api_key_set: bool,
}

pub fn list(conn: &Connection) -> AppResult<Vec<ProviderConfig>> {
    let mut stmt = conn.prepare(
        "SELECT id, enabled, base_url, default_model, extra_json FROM provider_configs ORDER BY id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (id, enabled, base_url, default_model, extra_json) = row?;
        let api_key_set = secrets::is_set(conn, &secrets::provider_key(&id));
        out.push(ProviderConfig {
            id,
            enabled: enabled != 0,
            base_url,
            default_model,
            extra_json,
            api_key_set,
        });
    }
    Ok(out)
}

pub fn get(conn: &Connection, id: &str) -> AppResult<ProviderConfig> {
    list(conn)?
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| AppError::msg(format!("unknown provider {id}")))
}

#[derive(Debug, Deserialize)]
pub struct ProviderPatch {
    pub enabled: Option<bool>,
    pub base_url: Option<String>,
    pub default_model: Option<String>,
    pub api_key: Option<String>,
}

pub fn update(conn: &Connection, id: &str, patch: &ProviderPatch) -> AppResult<ProviderConfig> {
    let current = get(conn, id)?;
    let enabled = patch.enabled.unwrap_or(current.enabled);
    let base_url = patch
        .base_url
        .clone()
        .or(current.base_url)
        .unwrap_or_default();
    let default_model = patch
        .default_model
        .clone()
        .or(current.default_model)
        .unwrap_or_default();
    conn.execute(
        "UPDATE provider_configs SET enabled=?1, base_url=?2, default_model=?3 WHERE id=?4",
        rusqlite::params![if enabled { 1 } else { 0 }, base_url, default_model, id],
    )?;
    if let Some(key) = &patch.api_key {
        if !key.is_empty() {
            secrets::set(conn, &secrets::provider_key(id), key)?;
        }
    }
    get(conn, id)
}
