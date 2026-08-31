use reqwest::Client;
use serde_json::{json, Value};

use super::types::{ChatRequest, ChatResponse, LlmProvider, Role, ToolCall};
use crate::error::{AppError, AppResult};

pub struct OllamaCloud {
    pub http: Client,
    pub api_key: String,
    pub host: String,
}

#[async_trait::async_trait]
impl LlmProvider for OllamaCloud {
    fn id(&self) -> &'static str {
        "ollama-cloud"
    }

    async fn chat(&self, req: ChatRequest) -> AppResult<ChatResponse> {
        let messages: Vec<Value> = req
            .messages
            .iter()
            .map(|m| {
                let role = match m.role {
                    Role::System => "system",
                    Role::User => "user",
                    Role::Assistant => "assistant",
                    Role::Tool => "tool",
                };
                let mut obj = json!({
                    "role": role,
                    "content": m.content,
                });
                if !m.tool_calls.is_empty() {
                    obj["tool_calls"] = json!(m
                        .tool_calls
                        .iter()
                        .map(|t| json!({
                            "function": {
                                "name": t.name,
                                "arguments": t.arguments
                            }
                        }))
                        .collect::<Vec<_>>());
                }
                obj
            })
            .collect();

        let tools: Vec<Value> = req
            .tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters
                    }
                })
            })
            .collect();

        let mut body = json!({
            "model": req.model,
            "messages": messages,
            "stream": false,
        });
        if !tools.is_empty() {
            body["tools"] = json!(tools);
        }

        let url = format!("{}/api/chat", self.host.trim_end_matches('/'));
        let resp = self
            .http
            .post(url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?;
        let status = resp.status();
        let val: Value = resp.json().await?;
        if !status.is_success() {
            return Err(AppError::msg(format!(
                "ollama cloud {status}: {}",
                val.to_string()
            )));
        }
        parse_ollama(val)
    }

    async fn test_connection(&self) -> AppResult<String> {
        let models = self.list_models().await?;
        Ok(format!(
            "Ollama Cloud ok — {} models available",
            models.len()
        ))
    }

    async fn list_models(&self) -> AppResult<Vec<String>> {
        let url = format!("{}/api/tags", self.host.trim_end_matches('/'));
        let resp = self.http.get(url).bearer_auth(&self.api_key).send().await?;
        let status = resp.status();
        let val: Value = resp.json().await?;
        if !status.is_success() {
            return Err(AppError::msg(format!(
                "ollama cloud tags {status}: {val}"
            )));
        }
        let mut names = Vec::new();
        if let Some(arr) = val.get("models").and_then(|m| m.as_array()) {
            for m in arr {
                if let Some(name) = m.get("name").and_then(|n| n.as_str()) {
                    names.push(name.to_string());
                } else if let Some(name) = m.get("model").and_then(|n| n.as_str()) {
                    names.push(name.to_string());
                }
            }
        }
        names.sort();
        names.dedup();
        Ok(names)
    }
}

fn parse_ollama(val: Value) -> AppResult<ChatResponse> {
    let msg = val
        .get("message")
        .cloned()
        .unwrap_or(json!({}));
    let content = msg
        .get("content")
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();
    let mut tool_calls = Vec::new();
    if let Some(arr) = msg.get("tool_calls").and_then(|t| t.as_array()) {
        for (i, t) in arr.iter().enumerate() {
            let func = t.get("function").cloned().unwrap_or(t.clone());
            let name = func
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();
            let args = func.get("arguments").cloned().unwrap_or(json!({}));
            let arguments = match args {
                Value::String(s) => serde_json::from_str(&s).unwrap_or(json!({ "raw": s })),
                other => other,
            };
            let id = t
                .get("id")
                .and_then(|i| i.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("call_{i}"));
            if !name.is_empty() {
                tool_calls.push(ToolCall {
                    id,
                    name,
                    arguments,
                });
            }
        }
    }
    Ok(ChatResponse {
        content,
        tool_calls,
    })
}
