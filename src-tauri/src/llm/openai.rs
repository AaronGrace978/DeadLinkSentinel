use reqwest::Client;
use serde_json::{json, Value};

use super::types::{ChatRequest, ChatResponse, LlmProvider, Role, ToolCall};
use crate::error::{AppError, AppResult};

pub struct OpenAiCompat {
    pub http: Client,
    pub api_key: String,
    pub base_url: String,
    pub provider_id: &'static str,
}

#[async_trait::async_trait]
impl LlmProvider for OpenAiCompat {
    fn id(&self) -> &'static str {
        self.provider_id
    }

    async fn chat(&self, req: ChatRequest) -> AppResult<ChatResponse> {
        let mut messages = Vec::new();
        for m in &req.messages {
            match m.role {
                Role::Tool => {
                    messages.push(json!({
                        "role": "tool",
                        "tool_call_id": m.tool_call_id.clone().unwrap_or_default(),
                        "content": m.content,
                    }));
                }
                Role::Assistant if !m.tool_calls.is_empty() => {
                    let calls: Vec<Value> = m
                        .tool_calls
                        .iter()
                        .map(|t| {
                            json!({
                                "id": t.id,
                                "type": "function",
                                "function": {
                                    "name": t.name,
                                    "arguments": t.arguments.to_string()
                                }
                            })
                        })
                        .collect();
                    messages.push(json!({
                        "role": "assistant",
                        "content": if m.content.is_empty() { Value::Null } else { json!(m.content) },
                        "tool_calls": calls
                    }));
                }
                _ => {
                    let role = match m.role {
                        Role::System => "system",
                        Role::User => "user",
                        Role::Assistant => "assistant",
                        Role::Tool => "tool",
                    };
                    messages.push(json!({ "role": role, "content": m.content }));
                }
            }
        }

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
        });
        if !tools.is_empty() {
            body["tools"] = json!(tools);
            body["tool_choice"] = json!("auto");
        }

        let url = format!(
            "{}/chat/completions",
            self.base_url.trim_end_matches('/')
        );
        let mut builder = self.http.post(url).json(&body);
        if !self.api_key.is_empty() {
            builder = builder.bearer_auth(&self.api_key);
        }
        let resp = builder.send().await?;
        let status = resp.status();
        let val: Value = resp.json().await?;
        if !status.is_success() {
            return Err(AppError::msg(format!(
                "{} chat {status}: {val}",
                self.provider_id
            )));
        }
        parse_openai(val)
    }

    async fn test_connection(&self) -> AppResult<String> {
        let models = self.list_models().await?;
        Ok(format!(
            "{} ok — {} models listed",
            self.provider_id,
            models.len()
        ))
    }

    async fn list_models(&self) -> AppResult<Vec<String>> {
        let url = format!("{}/models", self.base_url.trim_end_matches('/'));
        let mut builder = self.http.get(url);
        if !self.api_key.is_empty() {
            builder = builder.bearer_auth(&self.api_key);
        }
        let resp = builder.send().await?;
        let status = resp.status();
        let val: Value = resp.json().await?;
        if !status.is_success() {
            return Err(AppError::msg(format!(
                "{} models {status}: {val}",
                self.provider_id
            )));
        }
        let mut names = Vec::new();
        if let Some(arr) = val.get("data").and_then(|d| d.as_array()) {
            for m in arr {
                if let Some(id) = m.get("id").and_then(|i| i.as_str()) {
                    names.push(id.to_string());
                }
            }
        }
        names.sort();
        Ok(names)
    }
}

fn parse_openai(val: Value) -> AppResult<ChatResponse> {
    let choice = val
        .pointer("/choices/0/message")
        .cloned()
        .ok_or_else(|| AppError::msg("no message in OpenAI response"))?;
    let content = choice
        .get("content")
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();
    let mut tool_calls = Vec::new();
    if let Some(arr) = choice.get("tool_calls").and_then(|t| t.as_array()) {
        for t in arr {
            let id = t
                .get("id")
                .and_then(|i| i.as_str())
                .unwrap_or("call")
                .to_string();
            let func = t.get("function").cloned().unwrap_or(json!({}));
            let name = func
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();
            let args = func.get("arguments").cloned().unwrap_or(json!("{}"));
            let arguments = match args {
                Value::String(s) => serde_json::from_str(&s).unwrap_or(json!({ "raw": s })),
                other => other,
            };
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
