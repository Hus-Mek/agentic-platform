use crate::config::AppConfig;
use crate::error::{AgentError, Result};
use crate::state::{Message, ToolCall, ToolDefinition};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct LiteLLMClient {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub cost: f64,
}

#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub finish_reason: Option<String>,
    pub usage: LlmUsage,
}

#[derive(Debug, Deserialize)]
struct LiteLLMResponseBody {
    choices: Vec<LiteLLMChoice>,
    usage: LiteLLMUsage,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    model: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LiteLLMChoice {
    index: u32,
    message: LiteLLMMessage,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LiteLLMMessage {
    role: String,
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<LiteLLMToolCall>>,
}

#[derive(Debug, Deserialize)]
struct LiteLLMToolCall {
    id: String,
    #[serde(rename = "type")]
    type_: Option<String>,
    function: LiteLLMFunction,
}

#[derive(Debug, Deserialize)]
struct LiteLLMFunction {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
struct LiteLLMUsage {
    #[serde(default)]
    prompt_tokens: u32,
    #[serde(default)]
    completion_tokens: u32,
    #[serde(default)]
    total_tokens: u32,
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ChatTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ChatToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Debug, Serialize)]
struct ChatToolCall {
    id: String,
    #[serde(rename = "type")]
    type_: String,
    function: ChatFunctionCall,
}

#[derive(Debug, Serialize)]
struct ChatFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Debug, Serialize)]
struct ChatTool {
    #[serde(rename = "type")]
    type_: String,
    function: ChatToolFunction,
}

#[derive(Debug, Serialize)]
struct ChatToolFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

impl LiteLLMClient {
    pub fn new(config: &AppConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            base_url: config.llm_base_url.clone(),
            api_key: config.llm_api_key.clone(),
        }
    }

    pub fn new_from_url(base_url: &str, api_key: &str) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
        }
    }

    fn convert_messages(messages: &[Message]) -> Vec<ChatMessage> {
        messages
            .iter()
            .map(|m| {
                let (content, role) = if m.role == "tool" {
                    (m.content.clone(), "tool".to_string())
                } else {
                    (m.content.clone(), m.role.clone())
                };

                ChatMessage {
                    role,
                    content,
                    tool_calls: m.tool_calls.as_ref().map(|calls| {
                        calls
                            .iter()
                            .map(|tc| ChatToolCall {
                                id: tc.id.clone(),
                                type_: "function".to_string(),
                                function: ChatFunctionCall {
                                    name: tc.name.clone(),
                                    arguments: tc.args.to_string(),
                                },
                            })
                            .collect()
                    }),
                    tool_call_id: m.tool_call_id.clone(),
                    name: m.name.clone(),
                }
            })
            .collect()
    }

    fn convert_tools(tools: &[ToolDefinition]) -> Vec<ChatTool> {
        tools
            .iter()
            .map(|t| ChatTool {
                type_: "function".to_string(),
                function: ChatToolFunction {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    parameters: t.parameters.clone(),
                },
            })
            .collect()
    }

    pub async fn chat_completion(
        &self,
        model: &str,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<LlmResponse> {
        let request_body = ChatCompletionRequest {
            model: model.to_string(),
            messages: Self::convert_messages(messages),
            tools: if tools.is_empty() {
                None
            } else {
                Some(Self::convert_tools(tools))
            },
            stream: Some(false),
        };

        let response = self
            .client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request_body)
            .send()
            .await
            .map_err(|e| AgentError::LlmProvider(format!("Request failed: {e}")))?;

        let status = response.status();
        if !status.is_success() {
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown".to_string());
            return Err(AgentError::LlmProvider(format!(
                "API error ({}): {text}",
                status.as_u16()
            )));
        }

        let body: LiteLLMResponseBody = response
            .json()
            .await
            .map_err(|e| AgentError::LlmProvider(format!("Parse error: {e}")))?;

        let choice = body.choices.into_iter().next().ok_or_else(|| {
            AgentError::LlmProvider("No choices in response".to_string())
        })?;

        let tool_calls = choice.message.tool_calls.map(|calls| {
            calls
                .into_iter()
                .map(|tc| {
                    let args: serde_json::Value =
                        serde_json::from_str(&tc.function.arguments).unwrap_or_default();
                    ToolCall::new(&tc.function.name, args)
                })
                .collect()
        });

        let cost = calculate_cost(
            model,
            body.usage.prompt_tokens,
            body.usage.completion_tokens,
        );

        Ok(LlmResponse {
            content: choice.message.content,
            tool_calls,
            finish_reason: choice.finish_reason,
            usage: LlmUsage {
                prompt_tokens: body.usage.prompt_tokens,
                completion_tokens: body.usage.completion_tokens,
                total_tokens: body.usage.total_tokens,
                cost,
            },
        })
    }

    pub async fn embed_text(&self, text: &str) -> Result<Vec<f32>> {
        let url = format!("{}/v1/embeddings", self.base_url);
        let body = serde_json::json!({
            "model": "text-embedding-3-small",
            "input": text,
        });

        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| AgentError::Internal(format!("Embedding request failed: {e}")))?;

        let data: serde_json::Value = response
            .json()
            .await
            .map_err(|e| AgentError::Internal(format!("Embedding parse failed: {e}")))?;

        let embedding: Vec<f64> = serde_json::from_value(
            data["data"][0]["embedding"].clone()
        ).map_err(|e| AgentError::Internal(format!("Embedding format error: {e}")))?;

        Ok(embedding.into_iter().map(|v| v as f32).collect())
    }

    pub async fn chat_completion_stream(
        &self,
        model: &str,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> std::result::Result<reqwest::Response, AgentError> {
        let request_body = ChatCompletionRequest {
            model: model.to_string(),
            messages: Self::convert_messages(messages),
            tools: if tools.is_empty() {
                None
            } else {
                Some(Self::convert_tools(tools))
            },
            stream: Some(true),
        };

        let response = self
            .client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request_body)
            .send()
            .await
            .map_err(|e| AgentError::LlmProvider(format!("Stream request failed: {e}")))?;

        let status = response.status();
        if !status.is_success() {
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown".to_string());
            return Err(AgentError::LlmProvider(format!(
                "Stream API error ({}): {text}",
                status.as_u16()
            )));
        }

        Ok(response)
    }
}

fn calculate_cost(model: &str, prompt_tokens: u32, completion_tokens: u32) -> f64 {
    let (prompt_rate, completion_rate) = match model {
        m if m.contains("gpt-4o") => (2.50 / 1_000_000.0, 10.00 / 1_000_000.0),
        m if m.contains("gpt-4") => (30.00 / 1_000_000.0, 60.00 / 1_000_000.0),
        m if m.contains("gpt-3.5") => (1.50 / 1_000_000.0, 2.00 / 1_000_000.0),
        m if m.contains("claude-3-5-sonnet") => (3.00 / 1_000_000.0, 15.00 / 1_000_000.0),
        m if m.contains("claude-3-haiku") => (0.25 / 1_000_000.0, 1.25 / 1_000_000.0),
        m if m.contains("claude") => (8.00 / 1_000_000.0, 24.00 / 1_000_000.0),
        _ => (1.00 / 1_000_000.0, 2.00 / 1_000_000.0),
    };
    prompt_tokens as f64 * prompt_rate + completion_tokens as f64 * completion_rate
}
