use agenticos_core::llm::LiteLLMClient;
use agenticos_core::model_router::ModelRouter;
use agenticos_core::state::Message;
use serde::{Deserialize, Serialize};

/// Resilient (retry + fallback) completion shared by the extractors.
async fn extractor_chat(
    llm_client: &LiteLLMClient,
    model: &str,
    router: Option<&ModelRouter>,
    messages: &[Message],
) -> agenticos_core::error::Result<agenticos_core::llm::LlmResponse> {
    crate::route::route_chat_completion(
        llm_client,
        model,
        messages,
        &[],
        router,
        &crate::route::RouteOptions::resilient(),
    )
    .await
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtractedFact {
    pub fact: String,
    pub confidence: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct FactExtractionResponse {
    facts: Vec<ExtractedFact>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ProcedureExtractionResponse {
    procedure_name: String,
    steps: Vec<String>,
    applicable_task_types: Vec<String>,
}

pub async fn extract_facts(
    messages: &[Message],
    llm_client: &LiteLLMClient,
    model: &str,
    router: Option<&ModelRouter>,
) -> Vec<String> {
    let conversation = messages
        .iter()
        .filter(|m| m.role != "system")
        .map(|m| format!("{}: {}", m.role, m.content))
        .collect::<Vec<_>>()
        .join("\n");

    if conversation.trim().is_empty() {
        return Vec::new();
    }

    let prompt = format!(
        r#"Extract factual statements from the following conversation. 
Return only verifiable facts (user preferences, stated facts, confirmed information).
Do NOT include opinions, guesses, or instructions.
Format as a JSON object with a "facts" array of objects each having "fact" and "confidence" ("high"/"medium"/"low") fields.
Return ONLY valid JSON, no other text.

Conversation:
{conversation}
"#
    );

    let messages = vec![
        Message::new("system", "You are a fact extraction assistant. Extract only verifiable factual statements as JSON."),
        Message::new("user", &prompt),
    ];

    match extractor_chat(llm_client, model, router, &messages).await {
        Ok(response) => {
            let content = response.content.unwrap_or_default();
            match serde_json::from_str::<FactExtractionResponse>(&content) {
                Ok(parsed) => parsed
                    .facts
                    .into_iter()
                    .filter(|f| f.confidence == "high" || f.confidence == "medium")
                    .map(|f| f.fact)
                    .collect(),
                Err(_) => {
                    let facts: Vec<String> = content
                        .lines()
                        .filter(|l| l.starts_with("- ") || l.starts_with("* "))
                        .map(|l| l.trim_start_matches("- ").trim_start_matches("* ").to_string())
                        .collect();
                    facts
                }
            }
        }
        Err(e) => {
            tracing::warn!("Fact extraction LLM call failed: {e}");
            Vec::new()
        }
    }
}

pub async fn extract_procedure(
    messages: &[Message],
    llm_client: &LiteLLMClient,
    model: &str,
    router: Option<&ModelRouter>,
) -> Option<String> {
    let tool_interactions: Vec<String> = messages
        .iter()
        .filter(|m| m.role == "assistant" || m.role == "tool")
        .filter_map(|m| {
            if let Some(tcs) = &m.tool_calls {
                let names: Vec<&str> = tcs.iter().map(|t| t.name.as_str()).collect();
                Some(format!("Called tools: {}", names.join(", ")))
            } else if !m.content.is_empty() {
                Some(format!("Result: {}", truncate(&m.content, 200)))
            } else {
                None
            }
        })
        .collect();

    if tool_interactions.is_empty() {
        return None;
    }

    let tool_history = tool_interactions.join("\n");

    let prompt = format!(
        r#"Analyze the following tool usage pattern and extract a reusable procedure.
Describe the procedure name, steps, and what types of tasks it applies to.
Return as JSON with fields: "procedure_name", "steps" (array of strings), "applicable_task_types" (array of strings).
Return ONLY valid JSON, no other text.

Tool usage history:
{tool_history}
"#
    );

    let messages = vec![
        Message::new("system", "You are a procedure extraction assistant. Extract reusable procedures from tool usage patterns as JSON."),
        Message::new("user", &prompt),
    ];

    match extractor_chat(llm_client, model, router, &messages).await {
        Ok(response) => {
            let content = response.content.unwrap_or_default();
            match serde_json::from_str::<ProcedureExtractionResponse>(&content) {
                Ok(parsed) => {
                    let steps = parsed.steps.join("\n");
                    Some(format!(
                        "Procedure: {}\nSteps:\n{}\nApplies to: {}",
                        parsed.procedure_name,
                        steps,
                        parsed.applicable_task_types.join(", ")
                    ))
                }
                Err(_) => Some(content),
            }
        }
        Err(e) => {
            tracing::warn!("Procedure extraction LLM call failed: {e}");
            None
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
    }
}
