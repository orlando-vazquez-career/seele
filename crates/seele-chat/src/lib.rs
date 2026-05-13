//! Multi-provider AI chat backend for SEELE.
//!
//! - [`OpenAICompatibleProvider`] — wire-compatible with `/v1/chat/completions`
//!   for Minimax, OpenAI, OpenRouter, Together, Groq, DeepSeek, etc.
//! - [`AnthropicProvider`] — Anthropic's `/v1/messages` schema.
//!
//! The crate is intentionally storage-agnostic: tool execution is supplied
//! by the caller via a [`ToolHandler`] closure, so `seele-chat` can be
//! reused by other binaries that bind their own tools.

use std::future::Future;
use std::pin::Pin;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, info, warn};

#[derive(Debug, Error)]
pub enum ChatError {
    #[error("provider HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("provider returned error status {status}: {body}")]
    ProviderStatus { status: u16, body: String },
    #[error("provider response parse error: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("tool execution error: {0}")]
    Tool(String),
    #[error("tool-use loop exceeded {max_iterations} iterations")]
    LoopBudget { max_iterations: u32 },
    #[error("unsupported provider: {0}")]
    UnsupportedProvider(String),
}

/// One turn in the conversation. The `role` is one of `"system"`, `"user"`,
/// `"assistant"`, `"tool"`. For `"assistant"` turns that requested tool use,
/// `tool_calls` carries the structured calls. For `"tool"` turns, `tool_call_id`
/// references the call this is responding to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(default = "default_tool_type")]
    pub r#type: String,
    pub function: ToolCallFunction,
}

fn default_tool_type() -> String {
    "function".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    /// JSON-encoded arguments string (OpenAI convention).
    pub arguments: String,
}

/// Provider abstraction.
#[async_trait]
pub trait ChatProvider: Send + Sync {
    /// Send one round of messages, return the next assistant message.
    async fn complete(&self, messages: &[Message]) -> Result<Message, ChatError>;
    fn name(&self) -> &str;
    fn model(&self) -> &str;
}

/// A tool the caller exposes to the model. The handler is invoked with the
/// JSON-encoded argument string the model produced; it must return a string
/// (typically JSON) that the model can read.
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    /// JSON-schema for the input.
    pub parameters: serde_json::Value,
}

/// Boxed async tool handler: `fn(args_json) -> String result`.
pub type ToolHandler = Box<
    dyn Fn(String) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send>> + Send + Sync,
>;

/// Configuration for one chat run.
pub struct ChatConfig {
    pub system_prompt: String,
    pub max_iterations: u32,
}

impl Default for ChatConfig {
    fn default() -> Self {
        Self {
            system_prompt: "You are SEELE's assistant. You can search the local memory database via the seele_search tool. Use it whenever the user asks about saved memories, notes, decisions, or anything in their database. Answer concisely. Cite retrieved memories by `id` and `project`. Respond in the language of the user's question.".to_string(),
            max_iterations: 5,
        }
    }
}

// ============================================================================
// OpenAI-compatible (Minimax, OpenAI, OpenRouter, Together, Groq, ...)
// ============================================================================

pub struct OpenAICompatibleProvider {
    pub provider_name: String,
    pub endpoint: String,
    pub api_key: String,
    pub model: String,
    pub tools: Vec<ToolSpec>,
    client: reqwest::Client,
}

impl OpenAICompatibleProvider {
    pub fn new(
        provider_name: impl Into<String>,
        endpoint: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
        tools: Vec<ToolSpec>,
    ) -> Self {
        Self {
            provider_name: provider_name.into(),
            endpoint: endpoint.into(),
            api_key: api_key.into(),
            model: model.into(),
            tools,
            client: reqwest::Client::new(),
        }
    }

    fn tools_json(&self) -> serde_json::Value {
        serde_json::Value::Array(
            self.tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters,
                        }
                    })
                })
                .collect(),
        )
    }
}

#[async_trait]
impl ChatProvider for OpenAICompatibleProvider {
    fn name(&self) -> &str {
        &self.provider_name
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn complete(&self, messages: &[Message]) -> Result<Message, ChatError> {
        let mut body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "stream": false,
        });
        if !self.tools.is_empty() {
            body["tools"] = self.tools_json();
            body["tool_choice"] = serde_json::Value::String("auto".to_string());
        }

        debug!(provider = %self.provider_name, model = %self.model, "sending OpenAI-compatible request");

        let resp = self
            .client
            .post(&self.endpoint)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(ChatError::ProviderStatus {
                status: status.as_u16(),
                body: text,
            });
        }

        let parsed: OpenAIResponse = resp.json().await?;
        let choice = parsed
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| ChatError::Tool("provider returned zero choices".to_string()))?;
        Ok(choice.message)
    }
}

#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    choices: Vec<OpenAIChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAIChoice {
    message: Message,
}

// ============================================================================
// Anthropic Claude (different wire format)
// ============================================================================

pub struct AnthropicProvider {
    pub api_key: String,
    pub model: String,
    pub endpoint: String,
    pub tools: Vec<ToolSpec>,
    client: reqwest::Client,
}

impl AnthropicProvider {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>, tools: Vec<ToolSpec>) -> Self {
        Self {
            api_key: api_key.into(),
            model: model.into(),
            endpoint: "https://api.anthropic.com/v1/messages".to_string(),
            tools,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl ChatProvider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn complete(&self, messages: &[Message]) -> Result<Message, ChatError> {
        let (system_text, conversation) = split_system(messages);
        let anthropic_messages = anthropic_translate(&conversation);

        let tools_payload: Vec<serde_json::Value> = self
            .tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.parameters,
                })
            })
            .collect();

        let mut body = serde_json::json!({
            "model": self.model,
            "max_tokens": 2048,
            "system": system_text,
            "messages": anthropic_messages,
        });
        if !tools_payload.is_empty() {
            body["tools"] = serde_json::Value::Array(tools_payload);
        }

        let resp = self
            .client
            .post(&self.endpoint)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(ChatError::ProviderStatus {
                status: status.as_u16(),
                body: text,
            });
        }

        let parsed: AnthropicResponse = resp.json().await?;
        Ok(anthropic_to_internal(parsed))
    }
}

fn split_system(messages: &[Message]) -> (String, Vec<Message>) {
    let mut sys = String::new();
    let mut rest = Vec::new();
    for m in messages {
        if m.role == "system" {
            if let Some(c) = &m.content {
                if !sys.is_empty() {
                    sys.push('\n');
                }
                sys.push_str(c);
            }
        } else {
            rest.push(m.clone());
        }
    }
    (sys, rest)
}

fn anthropic_translate(messages: &[Message]) -> Vec<serde_json::Value> {
    messages
        .iter()
        .map(|m| {
            if m.role == "tool" {
                serde_json::json!({
                    "role": "user",
                    "content": [{
                        "type": "tool_result",
                        "tool_use_id": m.tool_call_id.clone().unwrap_or_default(),
                        "content": m.content.clone().unwrap_or_default(),
                    }]
                })
            } else if !m.tool_calls.is_empty() {
                let mut blocks: Vec<serde_json::Value> = Vec::new();
                if let Some(c) = &m.content {
                    if !c.is_empty() {
                        blocks.push(serde_json::json!({"type": "text", "text": c}));
                    }
                }
                for tc in &m.tool_calls {
                    let input: serde_json::Value = serde_json::from_str(&tc.function.arguments)
                        .unwrap_or(serde_json::json!({}));
                    blocks.push(serde_json::json!({
                        "type": "tool_use",
                        "id": tc.id,
                        "name": tc.function.name,
                        "input": input,
                    }));
                }
                serde_json::json!({ "role": "assistant", "content": blocks })
            } else {
                serde_json::json!({
                    "role": m.role,
                    "content": m.content.clone().unwrap_or_default(),
                })
            }
        })
        .collect()
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicContent {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
}

fn anthropic_to_internal(resp: AnthropicResponse) -> Message {
    let mut text_parts = Vec::new();
    let mut tool_calls = Vec::new();
    for c in resp.content {
        match c {
            AnthropicContent::Text { text } => text_parts.push(text),
            AnthropicContent::ToolUse { id, name, input } => {
                tool_calls.push(ToolCall {
                    id,
                    r#type: "function".to_string(),
                    function: ToolCallFunction {
                        name,
                        arguments: input.to_string(),
                    },
                });
            }
        }
    }
    Message {
        role: "assistant".to_string(),
        content: if text_parts.is_empty() {
            None
        } else {
            Some(text_parts.join("\n"))
        },
        tool_calls,
        tool_call_id: None,
        name: None,
    }
}

// ============================================================================
// Tool-use orchestrator
// ============================================================================

pub async fn run_chat(
    provider: &dyn ChatProvider,
    tool_handler: &ToolHandler,
    user_messages: Vec<Message>,
    config: &ChatConfig,
) -> Result<Vec<Message>, ChatError> {
    let mut history = Vec::with_capacity(user_messages.len() + 4);
    history.push(Message {
        role: "system".to_string(),
        content: Some(config.system_prompt.clone()),
        tool_calls: Vec::new(),
        tool_call_id: None,
        name: None,
    });
    history.extend(user_messages);

    info!(
        provider = %provider.name(),
        model = %provider.model(),
        history_len = history.len(),
        "starting chat loop"
    );

    for iteration in 0..config.max_iterations {
        let assistant_msg = provider.complete(&history).await?;
        history.push(assistant_msg.clone());

        if assistant_msg.tool_calls.is_empty() {
            debug!(iteration, "assistant returned text, loop done");
            return Ok(history);
        }

        debug!(
            iteration,
            tool_calls = assistant_msg.tool_calls.len(),
            "running tool calls"
        );
        for tc in &assistant_msg.tool_calls {
            let result = tool_handler(tc.function.arguments.clone()).await;
            let result_text = match result {
                Ok(r) => r,
                Err(e) => format!("(tool error) {}", e),
            };
            history.push(Message {
                role: "tool".to_string(),
                content: Some(result_text),
                tool_calls: Vec::new(),
                tool_call_id: Some(tc.id.clone()),
                name: Some(tc.function.name.clone()),
            });
        }
    }

    warn!(
        max_iterations = config.max_iterations,
        "chat loop exhausted iteration budget"
    );
    Err(ChatError::LoopBudget {
        max_iterations: config.max_iterations,
    })
}
