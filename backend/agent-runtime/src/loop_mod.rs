use agenticos_core::state::{GuardAction, GuardDecision};
use agenticos_core::udm::{AgentEvent, EventResult, EventType, Observable};

use crate::evaluate::evaluate_step;
use crate::plan::{plan_step, LiteLLMClient, PlanResult};
use crate::tool::ToolRegistry;

pub async fn run_agent(
    state: &mut agenticos_core::state::AgentState,
    llm_client: &LiteLLMClient,
    tool_registry: &ToolRegistry,
    guard_engine: &agenticos_guards::GuardEngine,
    memory_client: &agenticos_memory::MemoryClient,
) -> anyhow::Result<()> {
    AgentEvent::new(
        EventType::AgentRunStarted,
        state.workspace_id.clone(),
        state.run_id.clone(),
        state.agent_id.clone(),
        state.user_id.clone(),
    )
    .emit();

    let result = run_inner(state, llm_client, tool_registry, guard_engine, memory_client).await;

    match &result {
        Ok(_) => {
            AgentEvent::new(
                EventType::AgentRunCompleted,
                state.workspace_id.clone(),
                state.run_id.clone(),
                state.agent_id.clone(),
                state.user_id.clone(),
            )
            .with_result(
                EventResult::success()
                    .with_tokens(state.total_tokens_used, state.total_cost),
            )
            .with_observable(Observable::message(&format!(
                "Agent run completed in {} iteration(s) with {} tool call(s)",
                state.iteration,
                count_tool_calls(state)
            )))
            .emit();
        }
        Err(e) => {
            let err_msg = format!("{:?}", e);
            AgentEvent::new(
                EventType::AgentRunFailed,
                state.workspace_id.clone(),
                state.run_id.clone(),
                state.agent_id.clone(),
                state.user_id.clone(),
            )
            .with_result(EventResult::failure(&err_msg))
            .emit();
        }
    }

    result
}

async fn run_inner(
    state: &mut agenticos_core::state::AgentState,
    llm_client: &LiteLLMClient,
    tool_registry: &ToolRegistry,
    guard_engine: &agenticos_guards::GuardEngine,
    memory_client: &agenticos_memory::MemoryClient,
) -> anyhow::Result<()> {
    while !state.should_stop() {
        let plan_result = plan_step(state, llm_client, memory_client).await?;

        match plan_result {
            PlanResult::Text(_text) => {
                return Ok(());
            }
            PlanResult::ToolCalls(tool_calls) => {
                for tc in &tool_calls {
                    let user_id = state.user_id.as_deref().unwrap_or("");
                    let workspace_id = state.workspace_id.as_deref().unwrap_or("");
                    let guard_result = guard_engine
                        .check_tool_access(&tc.name, &tc.args, user_id, workspace_id, "agent")
                        .await;
                    let guard_decision = match guard_result {
                        Ok(()) => GuardDecision {
                            action: GuardAction::Continue,
                            reason: "Tool access granted".into(),
                            edited_content: None,
                            policy: None,
                        },
                        Err(e) => GuardDecision {
                            action: GuardAction::Abort,
                            reason: format!("Tool access denied: {e}"),
                            edited_content: None,
                            policy: Some("bodyguard".into()),
                        },
                    };

                    let action = evaluate_step(
                        state,
                        guard_decision,
                        "bodyguard",
                        Some(&tc.name),
                    )
                    .await?;

                    match action {
                        GuardAction::Abort => {
                            tracing::error!("Tool access aborted: {} — {:?}", tc.name, tc.args);
                            return Ok(());
                        }
                        GuardAction::Retry => {
                            tracing::warn!("Tool access retry: {}", tc.name);
                            continue;
                        }
                        GuardAction::Edit | GuardAction::Continue => {}
                    }

                    let mut core_tc = tc.clone();

                    let exec_result = tool_registry
                        .execute(
                            &mut core_tc,
                            state.workspace_id.as_deref(),
                            state.run_id.as_deref(),
                            state.agent_id.as_deref(),
                        )
                        .await;

                    match exec_result {
                        Ok(output) => {
                            state.add_message(
                                agenticos_core::state::Message::tool_result(
                                    &tc.name,
                                    &core_tc.id,
                                    &output,
                                ),
                            );

                            let inspect_decision = guard_engine
                                .inspect(state)
                                .await
                                .unwrap_or(GuardDecision {
                                    action: GuardAction::Continue,
                                    reason: "No inspection needed".into(),
                                    edited_content: None,
                                    policy: None,
                                });

                            let inspect_action = evaluate_step(
                                state,
                                inspect_decision,
                                "inspector",
                                Some(&tc.name),
                            )
                            .await?;

                            match inspect_action {
                                GuardAction::Abort => {
                                    return Ok(());
                                }
                                GuardAction::Retry => {
                                    state.consecutive_errors += 1;
                                }
                                GuardAction::Edit | GuardAction::Continue => {
                                    state.consecutive_errors = 0;
                                }
                            }
                        }
                        Err(e) => {
                            let err_msg = format!("{:?}", e);
                            state.add_message(
                                agenticos_core::state::Message::tool_result(
                                    &tc.name,
                                    &core_tc.id,
                                    &err_msg,
                                ),
                            );
                            state.consecutive_errors += 1;

                            if state.should_stop() {
                                return Ok(());
                            }
                        }
                    }
                }

                state.increment_iteration();
            }
            PlanResult::Stop => {
                return Ok(());
            }
            PlanResult::Retry => {
                state.consecutive_errors += 1;
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                continue;
            }
        }
    }

    Ok(())
}

fn count_tool_calls(state: &agenticos_core::state::AgentState) -> usize {
    state
        .messages
        .iter()
        .filter_map(|m| m.tool_calls.as_ref())
        .map(|tcs| tcs.len())
        .sum()
}
