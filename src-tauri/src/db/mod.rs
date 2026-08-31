pub mod schema;
pub mod settings;
pub mod sites;
pub mod scans;
pub mod triage;
pub mod providers;

use crate::error::{AppError, AppResult};
use r2d2::Pool as R2d2Pool;
use r2d2_sqlite::SqliteConnectionManager;
use std::path::Path;

pub type Pool = R2d2Pool<SqliteConnectionManager>;

pub fn open_pool(path: &Path) -> AppResult<Pool> {
    let manager = SqliteConnectionManager::file(path).with_init(|c| {
        c.busy_timeout(std::time::Duration::from_secs(8))?;
        c.pragma_update(None, "journal_mode", "WAL")?;
        c.pragma_update(None, "foreign_keys", "ON")?;
        Ok(())
    });
    let pool = R2d2Pool::builder()
        .max_size(8)
        .build(manager)
        .map_err(|e| AppError::msg(e.to_string()))?;
    {
        let conn = pool.get()?;
        schema::migrate(&conn)?;
    }
    Ok(pool)
}

pub fn conn(pool: &Pool) -> AppResult<r2d2::PooledConnection<SqliteConnectionManager>> {
    Ok(pool.get()?)
}

pub fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub fn parse_hosts(raw: &str) -> Vec<String> {
    raw.split([',', ' ', '\n', '\t'])
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}
