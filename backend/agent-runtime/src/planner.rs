use agenticos_core::state::Message;
use agenticos_core::llm::LiteLLMClient;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct PlanStep {
    pub id: String,
    pub description: String,
    pub dependencies: Vec<String>,
    pub estimated_complexity: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Plan {
    pub goal: String,
    pub steps: Vec<PlanStep>,
    pub risks: Vec<String>,
}

pub async fn create_plan(
    goal: &str,
    context: &str,
    llm_client: &LiteLLMClient,
    model: &str,
) -> anyhow::Result<Plan> {
    let prompt = format!(
        r#"Create a detailed implementation plan for the following goal.

Goal: {}

Context: {}

Return a JSON object with:
- "goal": the restated goal
- "steps": array of objects with "id" (short slug), "description" (one sentence), "dependencies" (array of step IDs that must complete first, empty if none), "estimated_complexity" ("low"/"medium"/"high")
- "risks": array of potential risks or blockers

Return ONLY valid JSON, no other text."#,
        goal, context
    );

    let messages = vec![
        Message::new("system", "You are a senior software architect. Create detailed, actionable implementation plans."),
        Message::new("user", &prompt),
    ];

    let response = llm_client.chat_completion(model, &messages, &[]).await?;
    let content = response.content.unwrap_or_default();

    let plan: Plan = serde_json::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse plan: {e}. Content: {content}"))?;

    Ok(plan)
}
