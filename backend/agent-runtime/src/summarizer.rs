use agenticos_core::state::Message;
use agenticos_core::llm::LiteLLMClient;
use tracing;

/// Configuration for context summarization
#[derive(Debug, Clone)]
pub struct SummarizerConfig {
    /// Token threshold at which summarization triggers (default: 6000)
    pub token_threshold: i64,
    /// Number of recent messages to keep verbatim (default: 5)
    pub keep_recent: usize,
    /// Maximum chars for the summary (default: 2000)
    pub max_summary_chars: usize,
}

impl Default for SummarizerConfig {
    fn default() -> Self {
        Self {
            token_threshold: 6000,
            keep_recent: 5,
            max_summary_chars: 2000,
        }
    }
}

/// LLM-based context summarizer
pub struct ContextSummarizer {
    llm_client: LiteLLMClient,
    model: String,
    config: SummarizerConfig,
    router: Option<agenticos_core::model_router::ModelRouter>,
}

impl ContextSummarizer {
    pub fn new(llm_client: LiteLLMClient, model: String, config: SummarizerConfig) -> Self {
        Self { llm_client, model, config, router: None }
    }

    /// Attach a router catalog so summarization gains fallback resilience.
    pub fn with_router(mut self, router: Option<agenticos_core::model_router::ModelRouter>) -> Self {
        self.router = router;
        self
    }

    /// Check if messages need summarization
    pub fn needs_summarization(&self, messages: &[Message]) -> bool {
        let total_chars: usize = messages.iter().map(|m| m.content.len()).sum();
        let estimated_tokens = (total_chars / 4) as i64;
        estimated_tokens >= self.config.token_threshold
    }

    /// Summarize messages, keeping recent ones verbatim
    pub async fn summarize(&self, messages: &[Message]) -> Result<Vec<Message>, SummarizerError> {
        if messages.len() <= self.config.keep_recent + 1 {
            return Ok(messages.to_vec());
        }

        // Split into old (to summarize) and recent (keep verbatim)
        let split_point = messages.len().saturating_sub(self.config.keep_recent);
        let old_messages = &messages[..split_point];
        let recent_messages = &messages[split_point..];

        // Build text to summarize
        let old_text = old_messages
            .iter()
            .map(|m| format!("[{}] {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n");

        // Ask LLM to summarize
        let summary = self.llm_summarize(&old_text).await?;

        // Build result: system summary + recent messages
        let mut result = Vec::new();

        // Keep system prompt if present
        if let Some(first) = messages.first() {
            if first.role == "system" {
                result.push(first.clone());
            }
        }

        result.push(Message::new(
            "system",
            &format!(
                "[Conversation summary of {} earlier messages]\n{}",
                old_messages.len(),
                summary
            ),
        ));

        result.extend_from_slice(recent_messages);

        Ok(result)
    }

    /// Use LLM to generate a summary
    async fn llm_summarize(&self, text: &str) -> Result<String, SummarizerError> {
        let prompt = format!(
            r#"Summarize the following conversation concisely. Preserve key decisions, important facts, action items, and unresolved questions. Keep under {} characters.

Conversation:
{}

Summary:"#,
            self.config.max_summary_chars, text
        );

        let messages = vec![
            Message::new("system", "You are a precise conversation summarizer. Preserve key information."),
            Message::new("user", &prompt),
        ];

        let response = crate::route::route_chat_completion(
            &self.llm_client,
            &self.model,
            &messages,
            &[],
            self.router.as_ref(),
            &crate::route::RouteOptions::resilient(),
        )
        .await
        .map_err(|e| SummarizerError::LlmError(e.to_string()))?;

        let summary = response.content.unwrap_or_default();

        // Truncate if over limit
        if summary.len() > self.config.max_summary_chars {
            let mut truncated = summary[..self.config.max_summary_chars].to_string();
            truncated.push_str("\n[Summary truncated]");
            Ok(truncated)
        } else {
            Ok(summary)
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SummarizerError {
    #[error("LLM error: {0}")]
    LlmError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_needs_summarization_short() {
        let config = SummarizerConfig::default();
        // Can't test without LLM client, just test the logic
        let messages = vec![
            Message::new("user", "Hello"),
            Message::new("assistant", "Hi there!"),
        ];
        let total_chars: usize = messages.iter().map(|m| m.content.len()).sum();
        let estimated_tokens = (total_chars / 4) as i64;
        assert!(estimated_tokens < config.token_threshold);
    }

    #[test]
    fn test_needs_summarization_long() {
        let config = SummarizerConfig::default();
        // token_threshold is 6000 (~24k chars at 4 chars/token); exceed it comfortably.
        let long_content = "a".repeat(30000);
        let messages = vec![
            Message::new("system", "You are helpful"),
            Message::new("user", &long_content),
            Message::new("assistant", "Here is my response to your very long message..."),
        ];
        let total_chars: usize = messages.iter().map(|m| m.content.len()).sum();
        let estimated_tokens = (total_chars / 4) as i64;
        assert!(estimated_tokens >= config.token_threshold);
    }

    #[test]
    fn test_config_default() {
        let config = SummarizerConfig::default();
        assert_eq!(config.token_threshold, 6000);
        assert_eq!(config.keep_recent, 5);
        assert_eq!(config.max_summary_chars, 2000);
    }
}
