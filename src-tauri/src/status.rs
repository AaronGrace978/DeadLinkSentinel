use axum::extract::State;
use axum::response::{Html, IntoResponse, Json};
use axum::routing::get;
use axum::Router;
use serde::Serialize;
use tauri::{AppHandle, Manager};
use tokio::sync::watch;
use tower_http::cors::CorsLayer;

use crate::db;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

#[derive(Clone)]
struct StatusState {
    db: db::Pool,
}

#[derive(Serialize)]
pub struct PublicStatus {
    pub overall: String,
    pub generated_at: String,
    pub stale: bool,
    pub sites: Vec<SiteStatus>,
}

#[derive(Serialize)]
pub struct SiteStatus {
    pub name: String,
    pub seed_url: String,
    pub last_check: Option<String>,
    pub broken_count: i64,
    pub pages_crawled: i64,
    pub links_checked: i64,
    pub health: String,
    pub summary: Option<String>,
}

pub fn compute(pool: &db::Pool) -> AppResult<PublicStatus> {
    let conn = db::conn(pool)?;
    let rows = db::scans::latest_completed_per_site(&conn)?;
    let sites_cfg = db::sites::list(&conn)?;
    let enabled = sites_cfg.iter().filter(|s| s.enabled).count();
    let mut sites = Vec::new();
    let mut worst = 0u8;
    for (scan, name, seed) in rows {
        let health = if scan.broken_count == 0 {
            "ok"
        } else if scan.broken_count < 5 {
            "degraded"
        } else {
            "down"
        };
        worst = worst.max(match health {
            "down" => 2,
            "degraded" => 1,
            _ => 0,
        });
        sites.push(SiteStatus {
            name,
            seed_url: seed,
            last_check: scan.finished_at.or(Some(scan.started_at)),
            broken_count: scan.broken_count,
            pages_crawled: scan.pages_crawled,
            links_checked: scan.links_checked,
            health: health.into(),
            summary: scan.summary,
        });
    }
    let stale = sites.is_empty();
    let overall = if stale {
        "unknown"
    } else if worst >= 2 {
        "down"
    } else if worst == 1 {
        "degraded"
    } else {
        "ok"
    };
    let _ = enabled;
    Ok(PublicStatus {
        overall: overall.into(),
        generated_at: crate::db::now_rfc3339(),
        stale,
        sites,
    })
}

fn render_html(status: &PublicStatus) -> String {
    let mut cards = String::new();
    for s in &status.sites {
        cards.push_str(&format!(
            r#"<article class="card {health}">
              <header><h2>{name}</h2><span class="pill">{health}</span></header>
              <p class="muted"><a href="{seed}">{seed}</a></p>
              <dl>
                <div><dt>Last check</dt><dd>{last}</dd></div>
                <div><dt>Broken</dt><dd>{broken}</dd></div>
                <div><dt>Checked</dt><dd>{links} links / {pages} pages</dd></div>
              </dl>
              {summary}
            </article>"#,
            health = html_escape::encode_text(&s.health),
            name = html_escape::encode_text(&s.name),
            seed = html_escape::encode_text(&s.seed_url),
            last = html_escape::encode_text(s.last_check.as_deref().unwrap_or("—")),
            broken = s.broken_count,
            links = s.links_checked,
            pages = s.pages_crawled,
            summary = s
                .summary
                .as_ref()
                .map(|t| format!("<p class=\"sum\">{}</p>", html_escape::encode_text(t)))
                .unwrap_or_default(),
        ));
    }
    if cards.is_empty() {
        cards = "<p class=\"empty\">No completed scans yet. Status updates after the first run.</p>"
            .into();
    }
    let banner = if status.stale {
        "<div class=\"banner\">Waiting for the first completed scan. DeadLinkSentinel must stay open for this page to stay live.</div>"
    } else {
        ""
    };
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8"/>
<meta name="viewport" content="width=device-width, initial-scale=1"/>
<title>DeadLinkSentinel status</title>
<style>
  :root {{ --bg:#12151a; --card:#1a1f27; --line:#2a3340; --text:#e8edf2; --muted:#8b98a8; --ok:#3dbf8a; --deg:#e0b04a; --down:#e05d5d; --accent:#d4a054; }}
  * {{ box-sizing:border-box; }}
  body {{ margin:0; font:15px/1.45 "IBM Plex Sans", "Segoe UI", system-ui, sans-serif; background:var(--bg); color:var(--text); }}
  header.top {{ padding:28px 32px 12px; border-bottom:1px solid var(--line); }}
  h1 {{ margin:0 0 6px; font-size:22px; font-weight:650; letter-spacing:-0.02em; }}
  .overall {{ display:inline-flex; align-items:center; gap:8px; font-weight:600; text-transform:uppercase; font-size:12px; letter-spacing:0.08em; }}
  .dot {{ width:8px; height:8px; border-radius:50%; background:var(--muted); }}
  .ok .dot, article.ok .pill {{ background:var(--ok); }}
  .degraded .dot, article.degraded .pill {{ background:var(--deg); color:#1a1408; }}
  .down .dot, article.down .pill {{ background:var(--down); }}
  .unknown .dot {{ background:var(--muted); }}
  main {{ padding:24px 32px 48px; display:grid; gap:16px; grid-template-columns:repeat(auto-fill,minmax(280px,1fr)); }}
  .card {{ background:var(--card); border:1px solid var(--line); border-radius:10px; padding:16px 18px; }}
  .card header {{ display:flex; justify-content:space-between; align-items:center; gap:8px; }}
  h2 {{ margin:0; font-size:16px; }}
  .pill {{ font-size:11px; font-weight:700; text-transform:uppercase; letter-spacing:0.06em; padding:3px 8px; border-radius:999px; color:#0e1116; background:var(--muted); }}
  .muted, .muted a {{ color:var(--muted); font-size:13px; word-break:break-all; }}
  a {{ color:var(--accent); }}
  dl {{ display:grid; gap:8px; margin:12px 0 0; }}
  dt {{ color:var(--muted); font-size:11px; text-transform:uppercase; letter-spacing:0.06em; }}
  dd {{ margin:0; }}
  .sum {{ color:var(--muted); font-size:13px; }}
  .banner {{ margin:16px 32px 0; padding:10px 14px; border:1px solid var(--line); background:#1f1810; color:var(--accent); border-radius:8px; }}
  .stamp {{ padding:0 32px 24px; color:var(--muted); font-size:12px; }}
  .empty {{ grid-column:1/-1; color:var(--muted); }}
</style>
</head>
<body>
<header class="top {overall}">
  <h1>DeadLinkSentinel</h1>
  <div class="overall"><span class="dot"></span> {overall} · public status</div>
</header>
{banner}
<main>{cards}</main>
<p class="stamp">Generated {when} · JSON at /api/status.json · served only while the desktop app is running</p>
</body></html>"#,
        overall = html_escape::encode_text(&status.overall),
        banner = banner,
        cards = cards,
        when = html_escape::encode_text(&status.generated_at),
    )
}

async fn html_handler(State(st): State<StatusState>) -> impl IntoResponse {
    match compute(&st.db) {
        Ok(s) => Html(render_html(&s)).into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn json_handler(State(st): State<StatusState>) -> impl IntoResponse {
    match compute(&st.db) {
        Ok(s) => Json(s).into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn start(app: &AppHandle) -> AppResult<String> {
    let state = app.state::<AppState>();
    let prev = state.status_shutdown.lock().unwrap().take();
    if let Some(tx) = prev {
        let _ = tx.send(true);
    }
    let (addr, port) = {
        let conn = db::conn(&state.db)?;
        let s = db::settings::load(&conn)?;
        (s.status_bind_addr, s.status_bind_port)
    };
    let bind = format!("{addr}:{port}");
    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .map_err(|e| AppError::msg(format!("status server bind {bind}: {e}")))?;
    let actual = listener
        .local_addr()
        .map(|a| a.to_string())
        .unwrap_or(bind.clone());

    let (tx, mut rx) = watch::channel(false);
    *state.status_shutdown.lock().unwrap() = Some(tx);
    *state.status_bind.lock().unwrap() = format!("http://{actual}");

    let router = Router::new()
        .route("/", get(html_handler))
        .route("/api/status.json", get(json_handler))
        .layer(CorsLayer::permissive())
        .with_state(StatusState {
            db: state.db.clone(),
        });

    tauri::async_runtime::spawn(async move {
        let _ = axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                loop {
                    if *rx.borrow() {
                        break;
                    }
                    if rx.changed().await.is_err() {
                        break;
                    }
                }
            })
            .await;
    });

    Ok(format!("http://{actual}"))
}

pub fn current_url(state: &AppState) -> String {
    state.status_bind.lock().unwrap().clone()
}
