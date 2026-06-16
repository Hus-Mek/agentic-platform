use agenticos_core::state::{AgentState, GuardAction, GuardDecision};
use agenticos_core::udm::{AgentEvent, EventResult, EventType};

#[tracing::instrument(skip_all, fields(guard_type, action = ?guard_decision.action))]
pub async fn evaluate_step(
    state: &mut AgentState,
    guard_decision: GuardDecision,
    guard_type: &str,
    _tool_name: Option<&str>,
) -> anyhow::Result<GuardAction> {
    state.current_node = Some("evaluate".into());

    AgentEvent::new(
        EventType::EvaluateStepStarted,
        state.workspace_id.clone(),
        state.run_id.clone(),
        state.agent_id.clone(),
        state.user_id.clone(),
    )
    .with_observable(agenticos_core::udm::Observable::message(&format!(
        "Evaluating guard decision: {:?} for {}",
        guard_decision.action, guard_type
    )))
    .emit();

    let start = std::time::Instant::now();

    state.guard_decision = Some(guard_decision.clone());

    let action = guard_decision.action.clone();
    let reason = guard_decision.reason.clone();

    match &action {
        GuardAction::Continue => {
            state.consecutive_errors = 0;
            tracing::debug!("Guard Continue: {}", reason);
        }
        GuardAction::Retry => {
            state.consecutive_errors += 1;
            tracing::warn!("Guard Retry: {} (error #{})", reason, state.consecutive_errors);
        }
        GuardAction::Abort => {
            tracing::error!("Guard Abort: {}", reason);
        }
        GuardAction::Edit => {
            state.consecutive_errors = 0;
            if let Some(edited) = &guard_decision.edited_content {
                if let Some(msg) = state.last_message() {
                    let last_idx = state.messages.len() - 1;
                    let mut edited_msg = msg.clone();
                    edited_msg.content = edited.clone();
                    state.messages[last_idx] = edited_msg;
                }
                tracing::info!("Guard Edit: content sanitized — {}", reason);
            }
        }
    }

    let duration_ms = start.elapsed().as_millis() as i64;

    AgentEvent::new(
        EventType::EvaluateStepCompleted,
        state.workspace_id.clone(),
        state.run_id.clone(),
        state.agent_id.clone(),
        state.user_id.clone(),
    )
    .with_result(
        EventResult::success()
            .with_duration(duration_ms),
    )
    .with_observable(agenticos_core::udm::Observable::message(&format!(
        "Guard action: {:?} — {}",
        action, reason
    )))
    .emit();

    Ok(action)
}
