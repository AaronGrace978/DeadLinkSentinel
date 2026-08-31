# DeadLinkSentinel

Desktop watchdog for documentation sites: scheduled broken-link scans, SMTP alerts on regressions, an agentic crawl/triage harness, and a public status page served while the app is running.

Built with **Tauri 2**, **React**, **SQLite**, and a Rust core. LLM providers: **Ollama Cloud** (default), OpenAI, Anthropic, and any OpenAI-compatible base URL.

## Prerequisites

- Rust (stable, with MSVC on Windows)
- Node.js 20+
- npm

## Run

```bash
npm install
npm run tauri dev
```

Data lives in the OS app-data directory (`com.deadlinksentinel.app`) as `deadlinksentinel.db`. API keys prefer the OS keyring and fall back to local settings storage.

## Sites and scans

1. Add a site with a seed URL (docs homepage). The seed host is always allowlisted; add extra hosts if docs span subdomains.
2. Choose **Deterministic crawl** (BFS) or **Agentic crawl + drain** (agent picks pages, then a deterministic pass finishes remaining budget).
3. Caps (`max pages`, `max links`, concurrency, timeout) always apply. The agent cannot exceed them.
4. Cron is 5-field (`0 0 * * *`) or 6-field with seconds (`0 0 0 * * *`). The scheduler only fires while the app is open.
5. **Run now** starts a scan immediately. Broken links and triage notes show on the Scans view.

## Public status page

While DeadLinkSentinel is running, an embedded HTTP server serves:

- `GET /` — human-readable status
- `GET /api/status.json` — machine-readable health

Default bind is `127.0.0.1:8787`. Binding `0.0.0.0` exposes the page on your LAN **with no authentication**. There is no always-on daemon: closing the app stops scans, alerts, and the status page.

## Email alerts

Configure SMTP under **Alerts**. Mail is sent only when a completed scan introduces **new or regressed** broken targets compared with the previous completed scan for that site.

## Agent / providers

Configure keys under **Agent / providers**.

| Provider | Notes |
| --- | --- |
| Ollama Cloud | `https://ollama.com` native `/api/chat` + `/api/tags`. Create a key and set it here (`OLLAMA_API_KEY` equivalent). Default model: `gpt-oss:120b`. |
| OpenAI | `https://api.openai.com/v1` |
| Anthropic | Messages API with tools |
| OpenAI-compatible | Any `/v1` host (OpenRouter, local proxies, etc.) — set base URL |

**Agentic crawl** uses tools (`list_frontier`, `fetch_page`, `extract_links`, `check_url`, `mark_page_done`, `finish_summary`). **Triage** classifies failures (`true_break`, `soft_403`, `auth_wall`, `temporary`, `redirect_login`, `asset_noise`, `rate_limit`, `other`) and writes grouped fix notes.

If the agent fails or exhausts steps, remaining budget is drained with the deterministic crawler.

## Out of scope (MVP)

- Always-on service when the UI is closed
- Hosted multi-user status / auth
- Full JS rendering / browser automation
- Opening PRs against docs repos
