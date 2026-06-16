use agenticos_core::output_schema::{OutputSchema, ValidationError};
use agenticos_core::state::Message;
use agenticos_core::llm::LiteLLMClient;
use tracing;

/// The Responder agent validates agent output against a schema
pub struct Responder {
    llm_client: LiteLLMClient,
    model: String,
}

impl Responder {
    pub fn new(llm_client: LiteLLMClient, model: String) -> Self {
        Self { llm_client, model }
    }

    /// Validate and potentially fix agent output
    pub async fn validate_and_fix(
        &self,
        output: &str,
        schema: &OutputSchema,
    ) -> Result<String, ResponderError> {
        let mut current_output = output.to_string();

        for attempt in 0..schema.max_retries {
            match schema.validate_str(&current_output) {
                Ok(_) => return Ok(current_output),
                Err(e) => {
                    tracing::warn!(
                        attempt = attempt + 1,
                        max_retries = schema.max_retries,
                        errors = %e,
                        "Output schema validation failed, retrying"
                    );

                    if attempt + 1 >= schema.max_retries {
                        return Err(ResponderError::ValidationFailed(e));
                    }

                    let fix_prompt = self.build_fix_prompt(&current_output, &e, schema);
                    let messages = vec![
                        Message::new("system", &fix_prompt),
                        Message::new("user", &format!(
                            "Original output:\n{}\n\nPlease fix and return valid JSON.",
                            current_output
                        )),
                    ];

                    let response = self.llm_client
                        .chat_completion(&self.model, &messages, &[])
                        .await
                        .map_err(|e| ResponderError::LlmError(e.to_string()))?;

                    current_output = response.content.unwrap_or_default();
                }
            }
        }

        Err(ResponderError::MaxRetriesExceeded(schema.max_retries))
    }

    fn build_fix_prompt(
        &self,
        original: &str,
        error: &ValidationError,
        schema: &OutputSchema,
    ) -> String {
        let schema_json = serde_json::to_string_pretty(&schema.schema)
            .unwrap_or_default();
        let error_json = serde_json::to_string_pretty(
            &serde_json::to_value(error).unwrap_or_default()
        ).unwrap_or_default();

        format!(
            r#"You are a response validator. Output must conform to this JSON Schema:

{}

Validation errors in the last response:
{}

Return ONLY the corrected JSON matching the schema. No explanation, no markdown, just the JSON."#,
            schema_json, error_json
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ResponderError {
    #[error("Validation failed after retries: {0}")]
    ValidationFailed(ValidationError),
    #[error("Max retries ({0}) exceeded")]
    MaxRetriesExceeded(u32),
    #[error("LLM error: {0}")]
    LlmError(String),
}
