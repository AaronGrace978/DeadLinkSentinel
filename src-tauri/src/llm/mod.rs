pub mod ollama;
pub mod openai;
pub mod types;

pub use types::*;

use crate::db;
use crate::error::{AppError, AppResult};
use crate::secrets;
use crate::state::AppState;

pub fn resolve_provider(
    state: &AppState,
    provider_id: Option<&str>,
) -> AppResult<Box<dyn LlmProvider>> {
    let conn = db::conn(&state.db)?;
    let default_id = db::settings::get_or(&conn, "default_llm_provider", "ollama-cloud")?;
    let id = provider_id.unwrap_or(&default_id);
    let cfg = db::providers::get(&conn, id)?;
    if !cfg.enabled && provider_id.is_none() {
        // still allow explicit test of a disabled provider
    }
    let key = secrets::get(&conn, &secrets::provider_key(id)).unwrap_or_default();
    let http = state.http.clone();
    match id {
        "ollama-cloud" => {
            if key.is_empty() {
                return Err(AppError::msg("Ollama Cloud API key is not set"));
            }
            Ok(Box::new(ollama::OllamaCloud {
                http,
                api_key: key,
                host: cfg
                    .base_url
                    .unwrap_or_else(|| "https://ollama.com".into()),
            }))
        }
        "openai" => {
            if key.is_empty() {
                return Err(AppError::msg("OpenAI API key is not set"));
            }
            Ok(Box::new(openai::OpenAiCompat {
                http,
                api_key: key,
                base_url: cfg
                    .base_url
                    .unwrap_or_else(|| "https://api.openai.com/v1".into()),
                provider_id: "openai",
            }))
        }
        "openai-compat" => {
            let base = cfg.base_url.unwrap_or_default();
            if base.is_empty() {
                return Err(AppError::msg(
                    "OpenAI-compatible base URL is required (e.g. https://openrouter.ai/api/v1)",
                ));
            }
            Ok(Box::new(openai::OpenAiCompat {
                http,
                api_key: key,
                base_url: base,
                provider_id: "openai-compat",
            }))
        }
        "anthropic" => {
            if key.is_empty() {
                return Err(AppError::msg("Anthropic API key is not set"));
            }
            Ok(Box::new(anthropic::Anthropic {
                http,
                api_key: key,
                base_url: cfg
                    .base_url
                    .unwrap_or_else(|| "https://api.anthropic.com".into()),
            }))
        }
        other => Err(AppError::msg(format!("unknown provider {other}"))),
    }
}

pub fn default_model(state: &AppState, provider_id: Option<&str>) -> AppResult<String> {
    let conn = db::conn(&state.db)?;
    let default_id = db::settings::get_or(&conn, "default_llm_provider", "ollama-cloud")?;
    let id = provider_id.unwrap_or(&default_id);
    let cfg = db::providers::get(&conn, id)?;
    if let Some(m) = cfg.default_model.filter(|s| !s.is_empty()) {
        return Ok(m);
    }
    db::settings::get_or(&conn, "default_llm_model", "gpt-oss:120b")
}

pub mod anthropic;
