use rusqlite::Connection;

use crate::error::AppResult;

pub const SERVICE: &str = "com.deadlinksentinel.app";
pub const SMTP_PASSWORD: &str = "smtp_password";

pub fn provider_key(provider_id: &str) -> String {
    format!("provider:{provider_id}")
}

pub fn set(conn: &Connection, key: &str, value: &str) -> AppResult<()> {
    match keyring::Entry::new(SERVICE, key) {
        Ok(entry) => {
            if entry.set_password(value).is_ok() {
                // Drop any SQLite fallback copy once keyring succeeds.
                let _ = conn.execute("DELETE FROM settings WHERE key = ?1", [fallback_key(key)]);
                return Ok(());
            }
        }
        Err(_) => {}
    }
    crate::db::settings::set(conn, &fallback_key(key), value)
}

pub fn get(conn: &Connection, key: &str) -> Option<String> {
    if let Ok(entry) = keyring::Entry::new(SERVICE, key) {
        if let Ok(v) = entry.get_password() {
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    crate::db::settings::get(conn, &fallback_key(key))
        .ok()
        .flatten()
        .filter(|v| !v.is_empty())
}

pub fn is_set(conn: &Connection, key: &str) -> bool {
    get(conn, key).is_some()
}

fn fallback_key(key: &str) -> String {
    format!("secret:{key}")
}
