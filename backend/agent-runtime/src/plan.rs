pub use agenticos_core::llm::{LiteLLMClient, LlmResponse, LlmUsage};
use agenticos_core::state::{AgentState, Message, ToolCall as CoreToolCall, ToolDefinition};
use agenticos_core::udm::{AgentEvent, EventResult, EventType, Observable};
use chrono::Utc;

use crate::rag::KbRetriever;

#[derive(Debug)]
pub enum PlanResult {
    Text(String),
    ToolCalls(Vec<CoreToolCall>),
    Stop,
    Retry,
}

// ── Multi-step planning types ──────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum PlanStepStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Replanned,
}

#[derive(Debug, Clone)]
pub struct PlanStep {
    pub id: String,
    pub description: String,
    pub status: PlanStepStatus,
    pub tool_call: Option<CoreToolCall>,
    pub result: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PlanExecution {
    pub steps: Vec<PlanStep>,
    pub current_index: usize,
    pub replan_count: u32,
}

impl PlanExecution {
    pub fn new(steps: Vec<PlanStep>) -> Self {
        Self {
            steps,
            current_index: 0,
            replan_count: 0,
        }
    }

    pub fn current_step(&self) -> Option<&PlanStep> {
        self.steps.get(self.current_index)
    }

    pub fn current_step_mut(&mut self) -> Option<&mut PlanStep> {
        self.steps.get_mut(self.current_index)
    }

    pub fn is_complete(&self) -> bool {
        self.current_index >= self.steps.len()
    }

    pub fn mark_current_completed(&mut self, result: String) {
        if let Some(step) = self.current_step_mut() {
            step.status = PlanStepStatus::Completed;
            step.result = Some(result);
        }
        self.current_index += 1;
    }

    pub fn mark_current_failed(&mut self, error: String) {
        if let Some(step) = self.current_step_mut() {
            step.status = PlanStepStatus::Failed;
            step.result = Some(error);
        }
    }

    pub fn replan_from_current(&mut self) -> bool {
        if self.replan_count >= 3 {
            return false;
        }
        self.replan_count += 1;
        // Mark remaining steps as pending
        for step in self.steps.iter_mut().skip(self.current_index) {
            step.status = PlanStepStatus::Pending;
            step.result = None;
        }
        true
    }
}

#[tracing::instrument(skip_all, fields(agent_id = state.agent_id.as_deref().unwrap_or("unknown"), model = state.model, num_messages = state.messages.len(), has_kb = kb_retriever.is_some()))]
pub async fn plan_step(
    state: &mut AgentState,
    llm_client: &LiteLLMClient,
    _memory_client: &agenticos_memory::MemoryClient,
    kb_retriever: Option<&KbRetriever>,
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

    if let Some(kb) = kb_retriever {
        if state.memory_config.enabled {
            let last_user_msg = state
                .messages
                .iter()
                .rev()
                .find(|m| m.role == "user")
                .map(|m| m.content.clone());
            if let Some(query) = last_user_msg {
                if let Some(ws_id) = &state.workspace_id {
                    match kb.retrieve(&query, ws_id, 5).await {
                        Ok(contents) => {
                            state.kb_contents = contents;
                        }
                        Err(e) => {
                            tracing::warn!("KB retrieval failed: {e}");
                            state.kb_contents = Vec::new();
                        }
                    }
                }
            }
        }
    }

    let system_prompt = crate::prompt_builder::build_system_prompt(state, &state.memory_contents);

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

    let opts = crate::route::RouteOptions {
        auto_route: state.auto_route,
        required_capabilities: &[],
        token_budget_remaining: Some((state.token_budget - state.total_tokens_used).max(0)),
        ..Default::default()
    };
    let response = crate::route::route_chat_completion(
        llm_client,
        &state.model,
        &api_messages,
        &tool_defs,
        state.router.as_ref(),
        &opts,
    )
    .await
    .map_err(|e| {
        state.consecutive_errors += 1;
        anyhow::anyhow!("LLM call failed: {}", e)
    })?;

    state.total_tokens_used += response.usage.total_tokens as i64;
    state.total_cost += response.usage.cost;
    state.consecutive_errors = 0;

    let _usage = &response.usage;

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
