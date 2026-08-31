use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::error::AppResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub status_bind_addr: String,
    pub status_bind_port: u16,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_user: String,
    pub smtp_from: String,
    pub smtp_to: String,
    pub smtp_tls: String,
    pub default_llm_provider: String,
    pub default_llm_model: String,
    pub agent_max_steps: u32,
    pub triage_after_scan: bool,
    pub smtp_password_set: bool,
}

pub fn get(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
    let mut rows = stmt.query([key])?;
    if let Some(row) = rows.next()? {
        Ok(Some(row.get(0)?))
    } else {
        Ok(None)
    }
}

pub fn get_or(conn: &Connection, key: &str, default: &str) -> AppResult<String> {
    Ok(get(conn, key)?.unwrap_or_else(|| default.to_string()))
}

pub fn set(conn: &Connection, key: &str, value: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![key, value],
    )?;
    Ok(())
}

pub fn load(conn: &Connection) -> AppResult<AppSettings> {
    let port: u16 = get_or(conn, "status_bind_port", "8787")?
        .parse()
        .unwrap_or(8787);
    let smtp_port: u16 = get_or(conn, "smtp_port", "587")?.parse().unwrap_or(587);
    let steps: u32 = get_or(conn, "agent_max_steps", "40")?.parse().unwrap_or(40);
    let triage = get_or(conn, "triage_after_scan", "1")? == "1";
    let smtp_password_set = crate::secrets::is_set(conn, crate::secrets::SMTP_PASSWORD);
    Ok(AppSettings {
        status_bind_addr: get_or(conn, "status_bind_addr", "127.0.0.1")?,
        status_bind_port: port,
        smtp_host: get_or(conn, "smtp_host", "")?,
        smtp_port,
        smtp_user: get_or(conn, "smtp_user", "")?,
        smtp_from: get_or(conn, "smtp_from", "")?,
        smtp_to: get_or(conn, "smtp_to", "")?,
        smtp_tls: get_or(conn, "smtp_tls", "starttls")?,
        default_llm_provider: get_or(conn, "default_llm_provider", "ollama-cloud")?,
        default_llm_model: get_or(conn, "default_llm_model", "gpt-oss:120b")?,
        agent_max_steps: steps,
        triage_after_scan: triage,
        smtp_password_set,
    })
}

#[derive(Debug, Deserialize)]
pub struct SettingsPatch {
    pub status_bind_addr: Option<String>,
    pub status_bind_port: Option<u16>,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<u16>,
    pub smtp_user: Option<String>,
    pub smtp_from: Option<String>,
    pub smtp_to: Option<String>,
    pub smtp_tls: Option<String>,
    pub smtp_password: Option<String>,
    pub default_llm_provider: Option<String>,
    pub default_llm_model: Option<String>,
    pub agent_max_steps: Option<u32>,
    pub triage_after_scan: Option<bool>,
}

pub fn apply_patch(conn: &Connection, patch: &SettingsPatch) -> AppResult<()> {
    if let Some(v) = &patch.status_bind_addr {
        set(conn, "status_bind_addr", v)?;
    }
    if let Some(v) = patch.status_bind_port {
        set(conn, "status_bind_port", &v.to_string())?;
    }
    if let Some(v) = &patch.smtp_host {
        set(conn, "smtp_host", v)?;
    }
    if let Some(v) = patch.smtp_port {
        set(conn, "smtp_port", &v.to_string())?;
    }
    if let Some(v) = &patch.smtp_user {
        set(conn, "smtp_user", v)?;
    }
    if let Some(v) = &patch.smtp_from {
        set(conn, "smtp_from", v)?;
    }
    if let Some(v) = &patch.smtp_to {
        set(conn, "smtp_to", v)?;
    }
    if let Some(v) = &patch.smtp_tls {
        set(conn, "smtp_tls", v)?;
    }
    if let Some(v) = &patch.default_llm_provider {
        set(conn, "default_llm_provider", v)?;
    }
    if let Some(v) = &patch.default_llm_model {
        set(conn, "default_llm_model", v)?;
    }
    if let Some(v) = patch.agent_max_steps {
        set(conn, "agent_max_steps", &v.to_string())?;
    }
    if let Some(v) = patch.triage_after_scan {
        set(conn, "triage_after_scan", if v { "1" } else { "0" })?;
    }
    if let Some(pw) = &patch.smtp_password {
        if !pw.is_empty() {
            crate::secrets::set(conn, crate::secrets::SMTP_PASSWORD, pw)?;
        }
    }
    Ok(())
}
