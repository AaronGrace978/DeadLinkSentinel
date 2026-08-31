use rusqlite::Connection;

use crate::error::AppResult;

pub fn migrate(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS sites (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            seed_url TEXT NOT NULL,
            allowlist_hosts TEXT NOT NULL DEFAULT '',
            schedule_cron TEXT NOT NULL DEFAULT '0 0 * * *',
            mode TEXT NOT NULL DEFAULT 'deterministic',
            enabled INTEGER NOT NULL DEFAULT 1,
            max_pages INTEGER NOT NULL DEFAULT 100,
            max_links INTEGER NOT NULL DEFAULT 500,
            concurrency INTEGER NOT NULL DEFAULT 8,
            timeout_secs INTEGER NOT NULL DEFAULT 15,
            triage_enabled INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS scans (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            site_id INTEGER NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
            started_at TEXT NOT NULL,
            finished_at TEXT,
            status TEXT NOT NULL,
            pages_crawled INTEGER NOT NULL DEFAULT 0,
            links_checked INTEGER NOT NULL DEFAULT 0,
            broken_count INTEGER NOT NULL DEFAULT 0,
            summary TEXT,
            error TEXT
        );

        CREATE TABLE IF NOT EXISTS link_results (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            scan_id INTEGER NOT NULL REFERENCES scans(id) ON DELETE CASCADE,
            source_url TEXT NOT NULL,
            target_url TEXT NOT NULL,
            status_code INTEGER,
            error TEXT,
            final_url TEXT,
            is_broken INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS triage_notes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            scan_id INTEGER NOT NULL REFERENCES scans(id) ON DELETE CASCADE,
            classification TEXT NOT NULL,
            grouping_key TEXT,
            draft_text TEXT NOT NULL,
            provider TEXT,
            model TEXT,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS provider_configs (
            id TEXT PRIMARY KEY,
            enabled INTEGER NOT NULL DEFAULT 0,
            base_url TEXT,
            default_model TEXT,
            extra_json TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_scans_site ON scans(site_id, started_at DESC);
        CREATE INDEX IF NOT EXISTS idx_links_scan ON link_results(scan_id);
        CREATE INDEX IF NOT EXISTS idx_links_broken ON link_results(scan_id, is_broken);
        CREATE INDEX IF NOT EXISTS idx_triage_scan ON triage_notes(scan_id);
        "#,
    )?;

    seed_providers(conn)?;
    seed_settings(conn)?;
    Ok(())
}

fn seed_providers(conn: &Connection) -> AppResult<()> {
    let defaults: &[(&str, i64, &str, &str)] = &[
        ("ollama-cloud", 1, "https://ollama.com", "gpt-oss:120b"),
        ("openai", 0, "https://api.openai.com/v1", "gpt-4o-mini"),
        ("anthropic", 0, "https://api.anthropic.com", "claude-sonnet-4-5"),
        ("openai-compat", 0, "", ""),
    ];
    for (id, enabled, base, model) in defaults {
        conn.execute(
            "INSERT OR IGNORE INTO provider_configs (id, enabled, base_url, default_model, extra_json)
             VALUES (?1, ?2, ?3, ?4, '{}')",
            rusqlite::params![id, enabled, base, model],
        )?;
    }
    Ok(())
}

fn seed_settings(conn: &Connection) -> AppResult<()> {
    let defaults: &[(&str, &str)] = &[
        ("status_bind_addr", "127.0.0.1"),
        ("status_bind_port", "8787"),
        ("smtp_host", ""),
        ("smtp_port", "587"),
        ("smtp_user", ""),
        ("smtp_from", ""),
        ("smtp_to", ""),
        ("smtp_tls", "starttls"),
        ("default_llm_provider", "ollama-cloud"),
        ("default_llm_model", "gpt-oss:120b"),
        ("agent_max_steps", "40"),
        ("triage_after_scan", "1"),
    ];
    for (k, v) in defaults {
        conn.execute(
            "INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)",
            rusqlite::params![k, v],
        )?;
    }
    Ok(())
}
