pub use agenticos_core::llm::{LiteLLMClient, LlmResponse, LlmUsage};
use agenticos_core::state::{AgentState, Message, ToolCall as CoreToolCall, ToolDefinition};
use agenticos_core::udm::{AgentEvent, EventResult, EventType, Observable};
use chrono::Utc;

#[derive(Debug)]
pub enum PlanResult {
    Text(String),
    ToolCalls(Vec<CoreToolCall>),
    Stop,
    Retry,
}

pub async fn plan_step(
    state: &mut AgentState,
    llm_client: &LiteLLMClient,
    memory_client: &agenticos_memory::MemoryClient,
) -> anyhow::Result<PlanResult> {
    state.current_node = Some("plan".into());

    AgentEvent::new(
        EventType::PlanStepStarted,
        state.workspace_id.clone(),
        state.run_id.clone(),
        state.agent_id.clone(),
        state.user_id.clone(),
    )
    .emit();

    if state.consecutive_errors >= state.max_consecutive_errors {
        tracing::warn!(
            consecutive_errors = state.consecutive_errors,
            max = state.max_consecutive_errors,
            "Circuit breaker triggered"
        );
        return Ok(PlanResult::Stop);
    }

    let system_prompt = build_system_prompt(state).await;

    let mut api_messages = vec![Message {
        id: "system".into(),
        role: "system".into(),
        content: system_prompt,
        tool_calls: None,
        tool_call_id: None,
        name: None,
        timestamp: Utc::now(),
    }];

    for msg in &state.messages {
        if msg.role == "system" {
            continue;
        }
        api_messages.push(msg.clone());
    }

    let tool_defs: Vec<ToolDefinition> = state.tools.clone();

    let response = llm_client
        .chat_completion(&state.model, &api_messages, &tool_defs)
        .await
        .map_err(|e| {
            state.consecutive_errors += 1;
            anyhow::anyhow!("LLM call failed: {}", e)
        })?;

    state.total_tokens_used += response.usage.total_tokens as i64;
    state.total_cost += response.usage.cost;
    state.consecutive_errors = 0;

    if let Some(usage) = Option::<(&LlmUsage, &str)>::None {
        // unused
    }

    if let Some(content) = &response.content {
        if !content.is_empty() {
            state.add_message(Message {
                id: format!("msg_{}", uuid::Uuid::new_v4()),
                role: "assistant".into(),
                content: content.clone(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
                timestamp: Utc::now(),
            });
        }
    }

    if let Some(tcs) = response.tool_calls {
        if !tcs.is_empty() {
            let msg_id = format!("msg_{}", uuid::Uuid::new_v4());
            state.add_message(Message {
                id: msg_id,
                role: "assistant".into(),
                content: String::new(),
                tool_calls: Some(tcs.clone()),
                tool_call_id: None,
                name: None,
                timestamp: Utc::now(),
            });

            AgentEvent::new(
                EventType::PlanStepCompleted,
                state.workspace_id.clone(),
                state.run_id.clone(),
                state.agent_id.clone(),
                state.user_id.clone(),
            )
            .with_result(
                EventResult::success()
                    .with_duration(0)
                    .with_tokens(response.usage.total_tokens as i64, response.usage.cost),
            )
            .with_observable(Observable::message(&format!(
                "Generated {} tool call(s)",
                tcs.len()
            )))
            .emit();

            return Ok(PlanResult::ToolCalls(tcs));
        }
    }

    if let Some(content) = response.content {
        AgentEvent::new(
            EventType::PlanStepCompleted,
            state.workspace_id.clone(),
            state.run_id.clone(),
            state.agent_id.clone(),
            state.user_id.clone(),
        )
        .with_result(
            EventResult::success()
                .with_duration(0)
                .with_tokens(response.usage.total_tokens as i64, response.usage.cost),
        )
        .with_observable(Observable::message(&format!(
            "Generated text response ({} chars)",
            content.len()
        )))
        .emit();

        return Ok(PlanResult::Text(content));
    }

    Ok(PlanResult::Stop)
}

async fn build_system_prompt(state: &AgentState) -> String {
    let mut parts = vec![state.instructions.clone()];

    if state.memory_config.enabled && !state.memory_contents.is_empty() {
        let memories = state.memory_contents.join("\n- ");
        parts.push(format!("\n\n[Relevant memories]\n- {}", memories));
    }

    parts.join("\n")
}
