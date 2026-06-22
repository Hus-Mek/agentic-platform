use agenticos_core::state::{GuardAction, GuardDecision};
use crate::responder::Responder;
use agenticos_core::udm::{AgentEvent, EventResult, EventType, Observable};

use crate::evaluate::evaluate_step;
use crate::memory_extractor;
use crate::plan::{plan_step, LiteLLMClient, PlanResult};
use crate::rag::KbRetriever;
use crate::tool::ToolRegistry;
use crate::context::{ContextConfig, ContextManager};
use sqlx::PgPool;
use uuid::Uuid;

#[tracing::instrument(skip_all, fields(agent_id = state.agent_id, run_id = state.run_id, workspace_id = state.workspace_id))]
pub async fn run_agent(
    state: &mut agenticos_core::state::AgentState,
    llm_client: &LiteLLMClient,
    tool_registry: &ToolRegistry,
    guard_engine: &agenticos_guards::GuardEngine,
    memory_client: &agenticos_memory::MemoryClient,
    kb_retriever: Option<&KbRetriever>,
    db: &PgPool,
    output_schema: Option<agenticos_core::OutputSchema>,
) -> anyhow::Result<()> {
    AgentEvent::new(
        EventType::AgentRunStarted,
        state.workspace_id.clone(),
        state.run_id.clone(),
        state.agent_id.clone(),
        state.user_id.clone(),
    )
    .emit();

    let result = run_inner(state, llm_client, tool_registry, guard_engine, memory_client, kb_retriever, db, &output_schema).await;

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

#[tracing::instrument(skip_all, fields(iteration = state.iteration))]
async fn run_inner(
    state: &mut agenticos_core::state::AgentState,
    llm_client: &LiteLLMClient,
    tool_registry: &ToolRegistry,
    guard_engine: &agenticos_guards::GuardEngine,
    memory_client: &agenticos_memory::MemoryClient,
    kb_retriever: Option<&KbRetriever>,
    db: &PgPool,
    output_schema: &Option<agenticos_core::OutputSchema>,
) -> anyhow::Result<()> {
    while !state.should_stop() {
        // Context window management: prune messages if approaching limit
        let context_mgr = ContextManager::new(ContextConfig::default());
        context_mgr.maybe_prune(&mut state.messages);

        if state.memory_config.enabled {
            let query = state
                .messages
                .iter()
                .rev()
                .find(|m| m.role == "user")
                .map(|m| m.content.clone())
                .unwrap_or_default();
            if !query.is_empty() {
                match memory_client
                    .retrieve_relevant(
                        &query,
                        state.user_id.as_deref(),
                        state.agent_id.as_deref(),
                        5,
                    )
                    .await
                {
                    Ok(contents) => state.memory_contents = contents,
                    Err(e) => tracing::warn!("Memory retrieval failed: {e}"),
                }
            }
        }

        let tokens_before = state.total_tokens_used;
        let cost_before = state.total_cost;

        if !state.can_add_tokens(0) {
            tracing::error!("Budget exceeded for workspace {} (tokens: {}, budget: {})",
                state.workspace_id.as_deref().unwrap_or("unknown"),
                state.total_tokens_used,
                state.token_budget
            );
            return Ok(());
        }

        let plan_start = std::time::Instant::now();
        let plan_result = plan_step(state, llm_client, memory_client, kb_retriever).await?;
        let plan_duration = plan_start.elapsed().as_millis() as i64;
        let plan_tokens = state.total_tokens_used - tokens_before;
        let plan_cost = state.total_cost - cost_before;

        record_workspace_usage(
            db,
            &state.workspace_id,
            plan_tokens,
            plan_cost,
            1,
        )
        .await?;

        match plan_result {
            PlanResult::Text(text) => {
                insert_run_step(
                    db,
                    &state.run_id,
                    "plan",
                    "plan",
                    state.messages.last().map(|m| m.content.clone()),
                    Some(text.clone()),
                    plan_duration,
                    plan_tokens,
                    serde_json::json!({"event_type": "PlanStepCompleted"}),
                )
                .await?;
                return Ok(());
            }
            PlanResult::ToolCalls(tool_calls) => {
                insert_run_step(
                    db,
                    &state.run_id,
                    "plan",
                    "plan",
                    state.messages.last().map(|m| m.content.clone()),
                    Some(serde_json::to_string(&tool_calls).unwrap_or_default()),
                    plan_duration,
                    plan_tokens,
                    serde_json::json!({"event_type": "PlanStepCompleted", "tool_count": tool_calls.len()}),
                )
                .await?;

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

                    let evaluate_start = std::time::Instant::now();
                    let action = evaluate_step(
                        state,
                        guard_decision.clone(),
                        "bodyguard",
                        Some(&tc.name),
                    )
                    .await?;
                    let evaluate_duration = evaluate_start.elapsed().as_millis() as i64;

                    insert_run_step(
                        db,
                        &state.run_id,
                        "evaluate",
                        "evaluate",
                        Some(serde_json::to_string(&guard_decision).unwrap_or_default()),
                        Some(format!("{:?}", action)),
                        evaluate_duration,
                        0,
                        serde_json::json!({"event_type": "EvaluateStepCompleted", "guard_type": "bodyguard"}),
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

                    let exec_start = std::time::Instant::now();
                    let exec_result = tool_registry
                        .execute_with_retry(
                            &mut core_tc,
                            state.workspace_id.as_deref(),
                            state.run_id.as_deref(),
                            state.agent_id.as_deref(),
                        )
                        .await;
                    let exec_duration = exec_start.elapsed().as_millis() as i64;

                    match exec_result {
                        Err(ref e) if e.to_string().starts_with("APPROVAL_REQUIRED:") => {
                            let approval_id = e.to_string().trim_start_matches("APPROVAL_REQUIRED:").to_string();
                            tracing::info!(approval_id = %approval_id, tool = %tc.name, "Tool paused — awaiting human approval");
                            state.add_message(agenticos_core::state::Message::tool_result(&tc.name, &core_tc.id, &format!("APPROVAL_REQUIRED:{}", approval_id)));
                            return Ok(());
                        }
                        Ok(output) => {
                            state.add_message(
                                agenticos_core::state::Message::tool_result(
                                    &tc.name,
                                    &core_tc.id,
                                    &output,
                                ),
                            );

                            insert_run_step(
                                db,
                                &state.run_id,
                                "tool",
                                &tc.name,
                                Some(serde_json::to_string(&tc.args).unwrap_or_default()),
                                Some(output.clone()),
                                exec_duration,
                                0,
                                serde_json::json!({"event_type": "ToolCallCompleted", "tool_name": tc.name}),
                            )
                            .await?;

                            let inspect_decision = guard_engine
                                .inspect(state)
                                .await
                                .unwrap_or(GuardDecision {
                                    action: GuardAction::Continue,
                                    reason: "No inspection needed".into(),
                                    edited_content: None,
                                    policy: None,
                                });

                            let inspect_start = std::time::Instant::now();
                            let inspect_action = evaluate_step(
                                state,
                                inspect_decision.clone(),
                                "inspector",
                                Some(&tc.name),
                            )
                            .await?;
                            let inspect_duration = inspect_start.elapsed().as_millis() as i64;

                            insert_run_step(
                                db,
                                &state.run_id,
                                "evaluate",
                                "evaluate",
                                Some(serde_json::to_string(&inspect_decision).unwrap_or_default()),
                                Some(format!("{:?}", inspect_action)),
                                inspect_duration,
                                0,
                                serde_json::json!({"event_type": "EvaluateStepCompleted", "guard_type": "inspector"}),
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

                            insert_run_step(
                                db,
                                &state.run_id,
                                "tool",
                                &tc.name,
                                Some(serde_json::to_string(&tc.args).unwrap_or_default()),
                                Some(err_msg.clone()),
                                exec_duration,
                                0,
                                serde_json::json!({"event_type": "ToolCallFailed", "tool_name": tc.name}),
                            )
                            .await?;

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

    if state.memory_config.enabled && !state.messages.is_empty() {
        let facts = memory_extractor::extract_facts(
            &state.messages,
            llm_client,
            &state.model,
            state.router.as_ref(),
        )
        .await;

        for fact in &facts {
            if let Err(e) = memory_client
                .store_semantic(fact, state.user_id.as_deref(), state.agent_id.as_deref())
                .await
            {
                tracing::warn!("Failed to store semantic memory: {e}");
            }
        }

        if count_tool_calls(state) > 0 {
            if let Some(procedure) = memory_extractor::extract_procedure(
                &state.messages,
                llm_client,
                &state.model,
                state.router.as_ref(),
            )
            .await
            {
                if let Err(e) = memory_client
                    .store_procedural(&procedure, state.user_id.as_deref())
                    .await
                {
                    tracing::warn!("Failed to store procedural memory: {e}");
                }
            }
        }
    }

    // Responder: validate and fix final output if schema is configured
    if let Some(schema) = output_schema {
        let last_output = state.messages.last()
            .map(|m| m.content.clone())
            .unwrap_or_default();
        if !last_output.is_empty() {
            let responder = Responder::new(llm_client.clone(), state.model.clone())
                .with_router(state.router.clone());
            match responder.validate_and_fix(&last_output, schema).await {
                Ok(fixed) => {
                    if fixed != last_output {
                        tracing::info!("Responder fixed agent output to match schema");
                        state.add_message(agenticos_core::state::Message::new(
                            "system",
                            &format!("[Responder corrected output to match schema: {}]", fixed),
                        ));
                    }
                }
                Err(e) => {
                    tracing::warn!("Responder validation failed (non-fatal): {e}");
                }
            }
        }
    }

    Ok(())
}

async fn insert_run_step(
    db: &PgPool,
    run_id_str: &Option<String>,
    step_type: &str,
    node_name: &str,
    input: Option<String>,
    output: Option<String>,
    duration_ms: i64,
    tokens_used: i64,
    metadata: serde_json::Value,
) -> anyhow::Result<()> {
    let Some(run_id) = run_id_str.as_ref().and_then(|s| Uuid::parse_str(s).ok()) else {
        return Ok(());
    };
    sqlx::query(
        "INSERT INTO run_steps (id, run_id, step_type, node_name, input, output, duration_ms, tokens_used, metadata, created_at) VALUES ($1, $2, $3::step_type, $4, $5, $6, $7, $8, $9, now())",
    )
    .bind(Uuid::new_v4())
    .bind(run_id)
    .bind(step_type)
    .bind(node_name)
    .bind(input)
    .bind(output)
    .bind(duration_ms)
    .bind(tokens_used)
    .bind(metadata)
    .execute(db)
    .await?;
    Ok(())
}

async fn record_workspace_usage(
    db: &PgPool,
    workspace_id_str: &Option<String>,
    tokens_used: i64,
    cost: f64,
    request_count: i32,
) -> anyhow::Result<()> {
    let Some(workspace_id) = workspace_id_str.as_ref().and_then(|s| Uuid::parse_str(s).ok()) else {
        return Ok(());
    };

    let today = chrono::Utc::now().date_naive();

    let existing = sqlx::query_as::<_, (i64, f64, i32)>(
        "SELECT total_tokens, total_cost::float8, request_count FROM workspace_usage WHERE workspace_id = $1 AND date = $2"
    )
    .bind(workspace_id)
    .bind(today)
    .fetch_optional(db)
    .await?;

    match existing {
        Some((existing_tokens, existing_cost, existing_requests)) => {
            let new_tokens = existing_tokens + tokens_used;
            let new_cost = existing_cost + cost;
            let new_requests = existing_requests + request_count;

            sqlx::query(
                "UPDATE workspace_usage SET total_tokens = $1, total_cost = $2, request_count = $3 WHERE workspace_id = $4 AND date = $5"
            )
            .bind(new_tokens)
            .bind(new_cost)
            .bind(new_requests)
            .bind(workspace_id)
            .bind(today)
            .execute(db)
            .await?;
        }
        None => {
            sqlx::query(
                "INSERT INTO workspace_usage (id, workspace_id, date, total_tokens, total_cost, request_count) VALUES (gen_random_uuid(), $1, $2, $3, $4, $5)"
            )
            .bind(workspace_id)
            .bind(today)
            .bind(tokens_used)
            .bind(cost)
            .bind(request_count)
            .execute(db)
            .await?;
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
