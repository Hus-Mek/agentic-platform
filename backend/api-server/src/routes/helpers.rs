use agenticos_core::state::{AgentState, GuardConfig, MemoryConfig, Message, ToolDefinition};
use axum::Json;
use crate::models::AgentRow;
use crate::middleware::auth::Claims;
use uuid::Uuid;

/// Shared error-response helpers used across route modules.
pub fn bad_request(msg: &str) -> (axum::http::StatusCode, Json<serde_json::Value>) {
    (axum::http::StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": msg })))
}

pub fn not_found(msg: &str) -> (axum::http::StatusCode, Json<serde_json::Value>) {
    (axum::http::StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": msg })))
}

pub fn internal_error<E: std::fmt::Display>(e: E) -> (axum::http::StatusCode, Json<serde_json::Value>) {
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "error": e.to_string() })),
    )
}

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
    if let Some(gc_val) = agent.guard_config.clone() {
        if let Ok(gc) = serde_json::from_value::<GuardConfig>(gc_val) {
            state.guard_config = gc;
        }
    }
    if let Some(mc_val) = agent.memory_config.clone() {
        if let Ok(mc) = serde_json::from_value::<MemoryConfig>(mc_val) {
            state.memory_config = mc;
        }
    }
    state.add_message(Message::new("user", input));
    state
}
