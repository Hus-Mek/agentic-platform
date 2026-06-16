use agenticos_core::state::{AgentState, GuardConfig, MemoryConfig, Message, ToolDefinition};
use crate::models::AgentRow;
use crate::middleware::auth::Claims;
use uuid::Uuid;

pub fn build_agent_state(
    agent: &AgentRow,
    claims: &Claims,
    input: &str,
    model_override: Option<&str>,
) -> AgentState {
    let run_id = format!("run_{}", Uuid::new_v4());
    let model = model_override.unwrap_or(&agent.model);
    let mut state = AgentState::new(
        Some(agent.id.to_string()),
        Some(run_id),
        Some(agent.workspace_id.to_string()),
        Some(claims.sub.clone()),
        &agent.instructions,
        model,
    );
    if let Ok(tools) = serde_json::from_value::<Vec<ToolDefinition>>(agent.tools.clone()) {
        state.tools = tools;
    }
    if let Ok(gc) = serde_json::from_value::<GuardConfig>(agent.guard_config.clone()) {
        state.guard_config = gc;
    }
    if let Ok(mc) = serde_json::from_value::<MemoryConfig>(agent.memory_config.clone()) {
        state.memory_config = mc;
    }
    state.add_message(Message::new("user", input));
    state
}
