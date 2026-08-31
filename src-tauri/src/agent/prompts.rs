use serde_json::json;

use crate::llm::ToolSpec;

pub const CRAWL_SYSTEM: &str = r#"You are DeadLinkSentinel's crawl agent for documentation sites.
Stay inside the host allowlist and remaining page/link budget. Prefer high-value docs pages
(index, getting started, guides, API reference) before changelogs or language switchers.
Never invent URLs that were not extracted from a fetched page. Use tools to fetch, extract,
and check. When the budget is exhausted or the frontier is empty, call finish_summary.
Docs-site quirks to watch: auth walls, locale prefixes, versioned paths, CDN soft-404s that
still return HTTP 200 with "not found" copy."#;

pub const TRIAGE_SYSTEM: &str = r#"You are DeadLinkSentinel's triage agent.
Classify broken-link findings for a documentation site. Use tools; do not only chat.
Classifications: true_break, soft_403, auth_wall, temporary, redirect_login, asset_noise, rate_limit, other.
Group related failures that share a path prefix or missing section. Draft concise fix notes
a docs maintainer can act on. Call finish_summary when grouping is complete.
Treat 401/403 as possible auth walls, 429 as rate_limit, connection errors as temporary unless repeated."#;

pub fn crawl_tools() -> Vec<ToolSpec> {
    vec![
        spec(
            "list_frontier",
            "List queued page URLs still to crawl and remaining budget.",
            json!({ "type": "object", "properties": {} }),
        ),
        spec(
            "fetch_page",
            "Fetch an allowlisted HTML page and store it for extract_links.",
            json!({
                "type": "object",
                "properties": { "url": { "type": "string" } },
                "required": ["url"]
            }),
        ),
        spec(
            "extract_links",
            "Extract links from the last fetched page (or a given URL already fetched).",
            json!({
                "type": "object",
                "properties": { "url": { "type": "string" } }
            }),
        ),
        spec(
            "check_url",
            "HTTP-check a single URL (HEAD then GET) and record the result.",
            json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string" },
                    "source_url": { "type": "string" }
                },
                "required": ["url"]
            }),
        ),
        spec(
            "mark_page_done",
            "Mark a page as fully processed so it leaves the frontier.",
            json!({
                "type": "object",
                "properties": { "url": { "type": "string" } },
                "required": ["url"]
            }),
        ),
        spec(
            "finish_summary",
            "End the crawl and store a short site-health summary.",
            json!({
                "type": "object",
                "properties": { "summary": { "type": "string" } },
                "required": ["summary"]
            }),
        ),
    ]
}

pub fn triage_tools() -> Vec<ToolSpec> {
    vec![
        spec(
            "classify_failure",
            "Classify one broken target URL.",
            json!({
                "type": "object",
                "properties": {
                    "target_url": { "type": "string" },
                    "classification": { "type": "string" },
                    "reason": { "type": "string" }
                },
                "required": ["target_url", "classification"]
            }),
        ),
        spec(
            "write_triage_note",
            "Persist a grouped triage note with a maintainer-facing draft.",
            json!({
                "type": "object",
                "properties": {
                    "classification": { "type": "string" },
                    "grouping_key": { "type": "string" },
                    "draft_text": { "type": "string" }
                },
                "required": ["classification", "draft_text"]
            }),
        ),
        spec(
            "finish_summary",
            "Finish triage with an overall summary.",
            json!({
                "type": "object",
                "properties": { "summary": { "type": "string" } },
                "required": ["summary"]
            }),
        ),
    ]
}

fn spec(name: &str, description: &str, parameters: serde_json::Value) -> ToolSpec {
    ToolSpec {
        name: name.into(),
        description: description.into(),
        parameters,
    }
}
