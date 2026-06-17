use serde::{Deserialize, Serialize};
use agenticos_core::state::Message;
use agenticos_core::llm::LiteLLMClient;
use tracing;

/// Request to the architect
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectRequest {
    pub description: String,
    pub constraints: Option<Vec<String>>,
}

/// Response from the architect
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectResponse {
    pub agent_name: String,
    pub agent_description: String,
    pub system_prompt: String,
    pub model: String,
    pub tools: Vec<String>,
    pub team: Option<TeamDesign>,
    pub pipeline: Option<PipelineDesign>,
    pub guards: GuardDesign,
}

/// Team design for multi-agent systems
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamDesign {
    pub sub_agents: Vec<SubAgentDesign>,
    pub execution_mode: String,
}

/// Sub-agent design
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgentDesign {
    pub name: String,
    pub description: String,
    pub instructions: String,
    pub model: String,
}

/// Pipeline design
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineDesign {
    pub steps: Vec<PipelineStepDesign>,
}

/// Pipeline step design
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStepDesign {
    pub name: String,
    pub step_type: String,
    pub description: String,
}

/// Guard design
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardDesign {
    pub enable_pii_detection: bool,
    pub enable_content_moderation: bool,
    pub enable_token_budget: bool,
    pub max_iterations: u32,
}

/// The Architect agent
pub struct Architect {
    llm_client: LiteLLMClient,
    model: String,
}



impl Architect {
    pub fn new(llm_client: LiteLLMClient, model: String) -> Self {
        Self { llm_client, model }
    }

    pub async fn design(&self, request: &ArchitectRequest) -> Result<ArchitectResponse, ArchitectError> {
        let prompt = self.build_prompt(request);

        let messages = vec![
            Message::new("user", &prompt),
        ];

        let response = self.llm_client
            .chat_completion(&self.model, &messages, &[])
            .await
            .map_err(|e| ArchitectError::LlmError(e.to_string()))?;

        let content = response.content.unwrap_or_default();

        let design: ArchitectResponse = serde_json::from_str(&content)
            .map_err(|e| ArchitectError::ParseError(format!("{}: {}", e, content)))?;

        tracing::info!(
            agent_name = %design.agent_name,
            has_team = design.team.is_some(),
            has_pipeline = design.pipeline.is_some(),
            "Architect designed agent"
        );

        Ok(design)
    }

    fn build_prompt(&self, request: &ArchitectRequest) -> String {
        let constraints = request.constraints.as_ref()
            .map(|c| format!("\nConstraints:\n- {}", c.join("\n- ")))
            .unwrap_or_default();

        format!(
            r#"Design an AI agent for the following business need:

{}{}

Return a JSON object with this exact structure:
{{
  "agent_name": "snake_case_name",
  "agent_description": "One sentence description",
  "system_prompt": "Detailed system prompt for the agent",
  "model": "gpt-4",
  "tools": ["tool1", "tool2"],
  "team": null,
  "pipeline": null,
  "guards": {{
    "enable_pii_detection": true,
    "enable_content_moderation": true,
    "enable_token_budget": true,
    "max_iterations": 10
  }}
}}

If the task requires multiple specialized agents, set "team" to:
{{
  "sub_agents": [
    {{"name": "researcher", "description": "...", "instructions": "...", "model": "gpt-4"}}
  ],
  "execution_mode": "sequential"
}}

If the task is a multi-step workflow, set "pipeline" to:
{{
  "steps": [
    {{"name": "input", "step_type": "input", "description": "Receive user query"}},
    {{"name": "process", "step_type": "model", "description": "Process with LLM"}},
    {{"name": "output", "step_type": "output", "description": "Return result"}}
  ]
}}

Return ONLY valid JSON, no markdown, no explanation."#,
            request.description, constraints
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ArchitectError {
    #[error("LLM error: {0}")]
    LlmError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guard_design_default() {
        let guards = GuardDesign {
            enable_pii_detection: true,
            enable_content_moderation: true,
            enable_token_budget: true,
            max_iterations: 10,
        };
        assert!(guards.enable_pii_detection);
        assert_eq!(guards.max_iterations, 10);
    }

    #[test]
    fn test_build_prompt() {
        let description = "A test agent".to_string();
        let constraints = Some(vec!["Be fast".to_string()]);

        let prompt = format!(
            "Design an AI agent for the following business need:\n\n{}\n\nConstraints:\n- {}",
            description,
            constraints.as_ref().unwrap().join("\n- ")
        );

        assert!(prompt.contains("A test agent"));
        assert!(prompt.contains("Be fast"));
    }
}
