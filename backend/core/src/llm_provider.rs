use async_trait::async_trait;
use crate::state::{Message, ToolDefinition};
use crate::llm::LlmResponse;

pub trait LlmProvider: Send + Sync {
    async fn chat_completion(
        &self,
        model: &str,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> crate::error::Result<LlmResponse>;
}

#[derive(Debug, PartialEq)]
pub enum ProviderKind { OpenAI, Anthropic, Google }

pub fn detect_provider(model: &str) -> ProviderKind {
    if model.starts_with("claude") || model.starts_with("anthropic") {
        ProviderKind::Anthropic
    } else if model.starts_with("gemini") || model.starts_with("google") {
        ProviderKind::Google
    } else {
        ProviderKind::OpenAI
    }
}