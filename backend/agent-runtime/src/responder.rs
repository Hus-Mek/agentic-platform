use agenticos_core::llm::LiteLLMClient;
use agenticos_core::state::Message;

pub struct Responder {
    llm_client: LiteLLMClient,
    model: String,
}

impl Responder {
    pub fn new(llm_client: LiteLLMClient, model: &str) -> Self {
        Self {
            llm_client,
            model: model.to_string(),
        }
    }

    pub async fn format(
        &self,
        raw_output: &str,
        original_input: &str,
    ) -> String {
        let messages = vec![
            Message::new(
                "system",
                "You are a response formatter. Your job is to take a raw agent output and \
                 format it into a clear, well-structured, and user-friendly response. \
                 Preserve all information, but improve readability. Return ONLY the formatted response.",
            ),
            Message::new(
                "user",
                &format!(
                    "Original user request: {}\n\nRaw agent output:\n{}",
                    original_input, raw_output
                ),
            ),
        ];

        match self.llm_client.chat_completion(&self.model, &messages, &[]).await {
            Ok(response) => response.content.unwrap_or_else(|| raw_output.to_string()),
            Err(e) => {
                tracing::warn!("Responder formatting failed: {e}");
                raw_output.to_string()
            }
        }
    }

    pub async fn summarize(
        &self,
        conversation: &str,
        max_length: usize,
    ) -> String {
        let messages = vec![
            Message::new(
                "system",
                "You are a conversation summarizer. Produce a concise summary.",
            ),
            Message::new(
                "user",
                &format!(
                    "Summarize the following conversation in {} characters or fewer:\n\n{}",
                    max_length, conversation
                ),
            ),
        ];

        match self.llm_client.chat_completion(&self.model, &messages, &[]).await {
            Ok(response) => response.content.unwrap_or_else(|| conversation.to_string()),
            Err(e) => {
                tracing::warn!("Responder summarization failed: {e}");
                conversation.to_string()
            }
        }
    }
}
