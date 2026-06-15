use std::time::Duration;

use agenticos_core::state::{AgentState, GuardAction, GuardDecision, ToolCall as CoreToolCall};
use agenticos_core::udm::{AgentEvent, EventResult, EventType};
use tokio::sync::mpsc;

use crate::evaluate::evaluate_step;
use crate::plan::{plan_step, LiteLLMClient, PlanResult};
use crate::tool::ToolRegistry;

#[derive(Debug, Clone, serde::Serialize)]
pub enum StreamEvent {
    Token(String),
    ToolCall {
        name: String,
        args: serde_json::Value,
    },
    ToolResult {
        name: String,
        result: String,
    },
    Done,
    Error(String),
}

pub async fn run_agent_streaming(
    state: &mut AgentState,
    llm_client: &LiteLLMClient,
    tool_registry: &ToolRegistry,
    guard_engine: &agenticos_guards::GuardEngine,
    memory_client: &agenticos_memory::MemoryClient,
    tx: mpsc::Sender<StreamEvent>,
) -> anyhow::Result<()> {
    AgentEvent::new(
        EventType::AgentRunStarted,
        state.workspace_id.clone(),
        state.run_id.clone(),
        state.agent_id.clone(),
        state.user_id.clone(),
    )
    .emit();

    let result = run_inner(state, llm_client, tool_registry, guard_engine, memory_client, &tx).await;

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
                EventResult::success().with_tokens(state.total_tokens_used, state.total_cost),
            )
            .emit();

            let _ = tx.send(StreamEvent::Done).await;
        }
        Err(e) => {
            let err_msg = e.to_string();
            AgentEvent::new(
                EventType::AgentRunFailed,
                state.workspace_id.clone(),
                state.run_id.clone(),
                state.agent_id.clone(),
                state.user_id.clone(),
            )
            .with_result(EventResult::failure(&err_msg))
            .emit();

            let _ = tx.send(StreamEvent::Error(err_msg)).await;
        }
    }

    result
}

async fn run_inner(
    state: &mut AgentState,
    llm_client: &LiteLLMClient,
    tool_registry: &ToolRegistry,
    guard_engine: &agenticos_guards::GuardEngine,
    memory_client: &agenticos_memory::MemoryClient,
    tx: &mpsc::Sender<StreamEvent>,
) -> anyhow::Result<()> {
    let mut token_buffer: Vec<String> = Vec::new();
    let flush_interval = Duration::from_millis(50);

    while !state.should_stop() {
        let plan_result = plan_step(state, llm_client, memory_client).await?;

        match plan_result {
            PlanResult::Text(text) => {
                if !text.is_empty() {
                    stream_tokens_buffered(&text, tx, &mut token_buffer, flush_interval).await?;
                }
                flush_buffer(tx, &mut token_buffer).await?;
                return Ok(());
            }
            PlanResult::ToolCalls(tool_calls) => {
                for tool_call in tool_calls {
                    let _ = tx
                        .send(StreamEvent::ToolCall {
                            name: tool_call.name.clone(),
                            args: tool_call.args.clone(),
                        })
                        .await;

                    let user_id = state.user_id.as_deref().unwrap_or("");
                    let workspace_id = state.workspace_id.as_deref().unwrap_or("");
                    let guard_result = guard_engine
                        .check_tool_access(
                            &tool_call.name,
                            &tool_call.args,
                            user_id,
                            workspace_id,
                            "agent",
                        )
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

                    let action = evaluate_step(state, guard_decision, "bodyguard", Some(&tool_call.name)).await?;

                    match action {
                        GuardAction::Abort => {
                            let _ = tx
                                .send(StreamEvent::Error(format!(
                                    "Tool access denied: {}",
                                    tool_call.name
                                )))
                                .await;
                            return Ok(());
                        }
                        GuardAction::Retry => {
                            continue;
                        }
                        GuardAction::Edit | GuardAction::Continue => {}
                    }

                    let mut core_tc = CoreToolCall::new(&tool_call.name, tool_call.args.clone());

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
                            state.add_message(agenticos_core::state::Message::tool_result(
                                &tool_call.name,
                                &core_tc.id,
                                &output,
                            ));

                            let _ = tx
                                .send(StreamEvent::ToolResult {
                                    name: tool_call.name.clone(),
                                    result: output,
                                })
                                .await;

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
                                Some(&tool_call.name),
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
                            let err_msg = e.to_string();
                            state.add_message(agenticos_core::state::Message::tool_result(
                                &tool_call.name,
                                &core_tc.id,
                                &err_msg,
                            ));

                            let _ = tx
                                .send(StreamEvent::Error(err_msg))
                                .await;

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
                flush_buffer(tx, &mut token_buffer).await?;
                return Ok(());
            }
            PlanResult::Retry => {
                state.consecutive_errors += 1;
                tokio::time::sleep(Duration::from_millis(500)).await;
                continue;
            }
        }
    }

    flush_buffer(tx, &mut token_buffer).await?;
    Ok(())
}

async fn stream_tokens_buffered(
    text: &str,
    tx: &mpsc::Sender<StreamEvent>,
    buffer: &mut Vec<String>,
    flush_interval: Duration,
) -> anyhow::Result<()> {
    for chunk in split_into_chunks(text, 5) {
        buffer.push(chunk);
        if buffer.len() >= 10 {
            flush_buffer(tx, buffer).await?;
            tokio::time::sleep(flush_interval).await;
        }
    }
    Ok(())
}

async fn flush_buffer(
    tx: &mpsc::Sender<StreamEvent>,
    buffer: &mut Vec<String>,
) -> anyhow::Result<()> {
    if buffer.is_empty() {
        return Ok(());
    }
    let batch = buffer.join("");
    buffer.clear();
    let _ = tx.send(StreamEvent::Token(batch)).await;
    Ok(())
}

fn split_into_chunks(text: &str, chunk_size: usize) -> Vec<String> {
    text.chars()
        .collect::<Vec<_>>()
        .chunks(chunk_size)
        .map(|c| c.iter().collect::<String>())
        .collect()
}
