use reqwest::Client;
use serde_json::{json, Value};

use super::types::{ChatRequest, ChatResponse, LlmProvider, Role, ToolCall};
use crate::error::{AppError, AppResult};

pub struct Anthropic {
    pub http: Client,
    pub api_key: String,
    pub base_url: String,
}

#[async_trait::async_trait]
impl LlmProvider for Anthropic {
    fn id(&self) -> &'static str {
        "anthropic"
    }

    async fn chat(&self, req: ChatRequest) -> AppResult<ChatResponse> {
        let mut system = String::new();
        let mut messages = Vec::new();
        for m in &req.messages {
            match m.role {
                Role::System => {
                    if !system.is_empty() {
                        system.push('\n');
                    }
                    system.push_str(&m.content);
                }
                Role::User => messages.push(json!({
                    "role": "user",
                    "content": m.content
                })),
                Role::Assistant => {
                    let mut content: Vec<Value> = Vec::new();
                    if !m.content.is_empty() {
                        content.push(json!({ "type": "text", "text": m.content }));
                    }
                    for t in &m.tool_calls {
                        content.push(json!({
                            "type": "tool_use",
                            "id": t.id,
                            "name": t.name,
                            "input": t.arguments
                        }));
                    }
                    messages.push(json!({ "role": "assistant", "content": content }));
                }
                Role::Tool => {
                    messages.push(json!({
                        "role": "user",
                        "content": [{
                            "type": "tool_result",
                            "tool_use_id": m.tool_call_id.clone().unwrap_or_default(),
                            "content": m.content
                        }]
                    }));
                }
            }
        }

        messages = merge_user_blocks(messages);

        let tools: Vec<Value> = req
            .tools
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.parameters
                })
            })
            .collect();

        let mut body = json!({
            "model": req.model,
            "max_tokens": 4096,
            "messages": messages,
        });
        if !system.is_empty() {
            body["system"] = json!(system);
        }
        if !tools.is_empty() {
            body["tools"] = json!(tools);
        }

        let url = format!("{}/v1/messages", self.base_url.trim_end_matches('/'));
        let resp = self
            .http
            .post(url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;
        let status = resp.status();
        let val: Value = resp.json().await?;
        if !status.is_success() {
            return Err(AppError::msg(format!("anthropic {status}: {val}")));
        }
        parse_anthropic(val)
    }

    async fn test_connection(&self) -> AppResult<String> {
        let req = ChatRequest {
            model: "claude-sonnet-4-5".into(),
            messages: vec![super::types::Message::user(
                "Reply with the single word pong.",
            )],
            tools: vec![],
        };
        let resp = self.chat(req).await?;
        Ok(format!(
            "Anthropic ok — sample: {}",
            resp.content.chars().take(80).collect::<String>()
        ))
    }

    async fn list_models(&self) -> AppResult<Vec<String>> {
        Ok(vec![
            "claude-sonnet-4-5".into(),
            "claude-opus-4-6".into(),
            "claude-haiku-4-5".into(),
            "claude-3-5-sonnet-latest".into(),
            "claude-3-5-haiku-latest".into(),
        ])
    }
}

fn merge_user_blocks(messages: Vec<Value>) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    for m in messages {
        if m.get("role").and_then(|r| r.as_str()) == Some("user") {
            if let Some(last) = out.last_mut() {
                if last.get("role").and_then(|r| r.as_str()) == Some("user") {
                    let mut a = last.get("content").cloned().unwrap_or(json!([]));
                    let b = m.get("content").cloned().unwrap_or(json!([]));
                    if let (Some(aa), Some(bb)) = (a.as_array_mut(), b.as_array()) {
                        aa.extend(bb.iter().cloned());
                        last["content"] = json!(aa);
                        continue;
                    }
                }
            }
        }
        out.push(m);
    }
    out
}

fn parse_anthropic(val: Value) -> AppResult<ChatResponse> {
    let mut content = String::new();
    let mut tool_calls = Vec::new();
    if let Some(arr) = val.get("content").and_then(|c| c.as_array()) {
        for block in arr {
            match block.get("type").and_then(|t| t.as_str()) {
                Some("text") => {
                    if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                        content.push_str(t);
                    }
                }
                Some("tool_use") => {
                    let id = block
                        .get("id")
                        .and_then(|i| i.as_str())
                        .unwrap_or("tool")
                        .to_string();
                    let name = block
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("")
                        .to_string();
                    let input = block.get("input").cloned().unwrap_or(json!({}));
                    if !name.is_empty() {
                        tool_calls.push(ToolCall {
                            id,
                            name,
                            arguments: input,
                        });
                    }
                }
                _ => {}
            }
        }
    }
    Ok(ChatResponse {
        content,
        tool_calls,
    })
}
