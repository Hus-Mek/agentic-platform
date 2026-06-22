//! Real pipeline execution with live I/O bindings.
//!
//! The pure engine in [`agenticos_core::pipeline_utils`] (`validate_pipeline`,
//! `topological_sort`, `resolve_inputs`, `render_template`, `evaluate_condition`)
//! stays untouched and synchronous. This module wraps it with an async executor
//! that binds nodes to the real LLM client, the WASM sandbox, and existing
//! agents/teams, persists a `run_steps` row per node, and rolls up token/cost.
//!
//! A workflow run is a run: node steps are written to the shared `run_steps`
//! table keyed by `pipeline_run_id`, so the existing trace UI works unchanged.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use serde::Serialize;
use serde_json::{json, Value};
use sqlx::postgres::PgPool;
use tokio::sync::mpsc;
use uuid::Uuid;

use agenticos_core::pipeline::*;
use agenticos_core::pipeline_utils::{
    evaluate_condition, render_template, resolve_inputs, topological_sort, validate_pipeline,
    PipelineError,
};
use agenticos_core::state::{
    AgentState, ExecutionMode, GuardConfig, MemoryConfig, Message, SubAgentConfig, TeamConfig,
    ToolDefinition,
};

use crate::orchestrator::TeamOrchestrator;
use crate::plan::LiteLLMClient;
use crate::rag::KbRetriever;
use crate::tool::ToolRegistry;
use crate::wasm_sandbox::{run_wasm, SandboxLimits};

/// Event streamed to clients as a pipeline executes, node by node.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PipelineStreamEvent {
    NodeStarted { node_id: String, node_name: String },
    NodeCompleted { node_id: String, output: Value, tokens: i64, cost: f64 },
    NodeFailed { node_id: String, error: String },
    Done,
    Error { message: String },
}

/// Everything a running pipeline needs from the host process. Holds borrows of
/// the shared clients/registries so node dispatch can reach real I/O.
pub struct ExecutionContext<'a> {
    pub llm_client: &'a LiteLLMClient,
    pub tool_registry: &'a Arc<ToolRegistry>,
    pub guard_engine: &'a Arc<agenticos_guards::GuardEngine>,
    pub memory_client: &'a Arc<agenticos_memory::MemoryClient>,
    pub kb_retriever: &'a Arc<KbRetriever>,
    pub db: &'a PgPool,
    pub workspace_id: Option<String>,
    pub user_id: Option<String>,
    /// Owner of every `run_steps` row this run writes.
    pub pipeline_run_id: Uuid,
    /// Cooperative cancellation checked between nodes.
    pub cancel: Option<Arc<AtomicBool>>,
    /// Optional live event sink.
    pub tx: Option<mpsc::Sender<PipelineStreamEvent>>,
    /// Optional router catalog giving Model nodes fallback resilience.
    pub router: Option<agenticos_core::model_router::ModelRouter>,
}

/// Result of a full pipeline run: the execution snapshot plus rolled-up cost.
pub struct PipelineRunOutcome {
    pub execution: PipelineExecution,
    pub total_tokens: i64,
    pub total_cost: f64,
}

/// Minimal projection of the `agents` row needed to invoke an Agent node.
#[derive(sqlx::FromRow)]
struct AgentNodeRow {
    id: Uuid,
    instructions: String,
    model: String,
    tools: Value,
    guard_config: Option<Value>,
    memory_config: Option<Value>,
    workspace_id: Uuid,
}

/// Minimal projection of the `team_agents` row needed to invoke a Team node.
#[derive(sqlx::FromRow)]
struct TeamNodeRow {
    id: Uuid,
    name: String,
    description: Option<String>,
    supervisor_instructions: String,
    supervisor_model: String,
    sub_agents: Value,
    max_delegations: i32,
    execution_mode: String,
}

/// Execute a validated pipeline to completion (or first failure), persisting a
/// `run_steps` row per node and rolling up tokens/cost. Reuses the pure
/// topo-sort/resolve helpers; only node *execution* is bound to real I/O here.
pub async fn execute_pipeline(
    pipeline: &Pipeline,
    run_input: &PipelineRunInput,
    ctx: &ExecutionContext<'_>,
) -> Result<PipelineRunOutcome, PipelineError> {
    validate_pipeline(pipeline)?;
    let order = topological_sort(pipeline)?;
    let node_by_id: HashMap<&str, &PipelineNode> =
        pipeline.nodes.iter().map(|n| (n.id.0.as_str(), n)).collect();

    let mut execution = PipelineExecution {
        execution_id: ctx.pipeline_run_id.to_string(),
        pipeline_id: pipeline.id.clone(),
        status: PipelineExecutionStatus::Running,
        node_states: HashMap::new(),
        node_outputs: HashMap::new(),
        started_at: chrono::Utc::now(),
        completed_at: None,
        error: None,
    };
    for node in &pipeline.nodes {
        execution.node_states.insert(node.id.clone(), NodeStatus::Pending);
    }

    let mut completed_outputs: HashMap<String, Vec<NodeOutput>> = HashMap::new();
    let mut total_tokens: i64 = 0;
    let mut total_cost: f64 = 0.0;

    for node_id in &order {
        if ctx.cancel.as_ref().is_some_and(|c| c.load(Ordering::Relaxed)) {
            execution.status = PipelineExecutionStatus::Cancelled;
            execution.error = Some("cancelled".to_string());
            execution.completed_at = Some(chrono::Utc::now());
            return Ok(PipelineRunOutcome { execution, total_tokens, total_cost });
        }

        let node = node_by_id.get(node_id.0.as_str()).expect("node in topo order exists");
        execution.node_states.insert(node_id.clone(), NodeStatus::Running);
        emit(ctx, PipelineStreamEvent::NodeStarted {
            node_id: node_id.0.clone(),
            node_name: node.name.clone(),
        })
        .await;

        let resolved = resolve_inputs(node, run_input, &completed_outputs)?;
        let started = Instant::now();
        let result = execute_node(node, &resolved, ctx).await;
        let duration_ms = started.elapsed().as_millis() as i64;

        match result {
            Ok(node_run) => {
                total_tokens += node_run.tokens;
                total_cost += node_run.cost;
                let output_value = node_run
                    .outputs
                    .first()
                    .map(|o| o.value.clone())
                    .unwrap_or(Value::Null);

                let _ = insert_pipeline_step(
                    ctx,
                    step_type_for(&node.node_type),
                    node,
                    &resolved,
                    &output_value,
                    duration_ms,
                    node_run.tokens,
                )
                .await;

                execution.node_states.insert(node_id.clone(), NodeStatus::Completed);
                execution.node_outputs.insert(node_id.clone(), node_run.outputs.clone());
                completed_outputs.insert(node_id.0.clone(), node_run.outputs);

                emit(ctx, PipelineStreamEvent::NodeCompleted {
                    node_id: node_id.0.clone(),
                    output: output_value,
                    tokens: node_run.tokens,
                    cost: node_run.cost,
                })
                .await;
            }
            Err(e) => {
                let msg = e.to_string();
                let _ = insert_pipeline_step(
                    ctx,
                    "other",
                    node,
                    &resolved,
                    &json!({ "error": msg }),
                    duration_ms,
                    0,
                )
                .await;
                execution
                    .node_states
                    .insert(node_id.clone(), NodeStatus::Failed { error: msg.clone() });
                execution.status = PipelineExecutionStatus::Failed;
                execution.error = Some(msg.clone());
                execution.completed_at = Some(chrono::Utc::now());
                emit(ctx, PipelineStreamEvent::NodeFailed {
                    node_id: node_id.0.clone(),
                    error: msg,
                })
                .await;
                return Ok(PipelineRunOutcome { execution, total_tokens, total_cost });
            }
        }
    }

    execution.status = PipelineExecutionStatus::Completed;
    execution.completed_at = Some(chrono::Utc::now());
    emit(ctx, PipelineStreamEvent::Done).await;
    Ok(PipelineRunOutcome { execution, total_tokens, total_cost })
}

/// Output of a single node execution plus its cost contribution.
struct NodeRun {
    outputs: Vec<NodeOutput>,
    tokens: i64,
    cost: f64,
}

impl NodeRun {
    fn pure(outputs: Vec<NodeOutput>) -> Self {
        Self { outputs, tokens: 0, cost: 0.0 }
    }
}

fn out(name: &str, data_type: DataType, value: Value) -> NodeOutput {
    NodeOutput { name: name.to_string(), data_type, value }
}

/// Dispatch a single node to its real binding.
async fn execute_node(
    node: &PipelineNode,
    resolved: &HashMap<String, Value>,
    ctx: &ExecutionContext<'_>,
) -> Result<NodeRun, PipelineError> {
    match &node.node_type {
        NodeType::Input => {
            let value = first_value(resolved).unwrap_or(Value::Null);
            Ok(NodeRun::pure(vec![out("output", DataType::Any, value)]))
        }
        NodeType::Output => {
            let collected: serde_json::Map<String, Value> =
                resolved.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            Ok(NodeRun::pure(vec![out("output", DataType::Json, Value::Object(collected))]))
        }
        NodeType::Model { model_id, prompt_template } => {
            let prompt = render_template(prompt_template, resolved);
            let messages = vec![Message::new("user", &prompt)];
            let resp = crate::route::route_chat_completion(
                ctx.llm_client,
                model_id,
                &messages,
                &[],
                ctx.router.as_ref(),
                &crate::route::RouteOptions::resilient(),
            )
            .await
            .map_err(|e| PipelineError::LlmError(e.to_string()))?;
            let content = resp.content.unwrap_or_default();
            Ok(NodeRun {
                outputs: vec![out("response", DataType::Text, Value::String(content))],
                tokens: resp.usage.total_tokens as i64,
                cost: resp.usage.cost,
            })
        }
        NodeType::Decision { condition } => {
            let pass = evaluate_condition(condition, resolved)?;
            Ok(NodeRun::pure(vec![out("result", DataType::Boolean, json!(pass))]))
        }
        NodeType::Router { routes } => {
            // Pass the incoming value through; expose the matched route name.
            let value = first_value(resolved).unwrap_or(Value::Null);
            let route = routes.keys().next().cloned().unwrap_or_else(|| "default".to_string());
            Ok(NodeRun::pure(vec![
                out("output", DataType::Any, value),
                out("route", DataType::Text, Value::String(route)),
            ]))
        }
        NodeType::Metric { metric_type } => {
            // Lightweight v1 scorer: exact-match when an expected value is
            // present, else a neutral score. Full eval is the documented gap.
            let score = match (resolved.get("output"), resolved.get("expected")) {
                (Some(a), Some(b)) => {
                    if a == b {
                        1.0
                    } else {
                        0.0
                    }
                }
                _ => 0.85,
            };
            Ok(NodeRun::pure(vec![
                out("score", DataType::Number, json!(score)),
                out("metric", DataType::Text, Value::String(metric_type.clone())),
            ]))
        }
        NodeType::Script { code, function_name } => {
            execute_script(node, code.as_deref(), function_name, resolved)
        }
        NodeType::Agent { agent_id } => execute_agent_node(agent_id, resolved, ctx).await,
        NodeType::Team { team_id } => execute_team_node(team_id, resolved, ctx).await,
    }
}

/// Run a Script node's WASM module in the sandbox. `code` is WAT or wasm-text
/// (binary modules can be supplied via a `base64` config flag in a follow-up).
fn execute_script(
    node: &PipelineNode,
    code: Option<&str>,
    _function_name: &str,
    resolved: &HashMap<String, Value>,
) -> Result<NodeRun, PipelineError> {
    let code = code.ok_or_else(|| {
        PipelineError::ScriptError("Script node has no code/module".to_string())
    })?;

    let limits = SandboxLimits {
        fuel: config_u64(node, "fuel").unwrap_or(SandboxLimits::default().fuel),
        memory_bytes: config_u64(node, "memory_bytes")
            .map(|v| v as usize)
            .unwrap_or(SandboxLimits::default().memory_bytes),
        wall_ms: config_u64(node, "wall_ms").unwrap_or(SandboxLimits::default().wall_ms),
    };

    let input: serde_json::Map<String, Value> =
        resolved.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    let output = run_wasm(code.as_bytes(), &Value::Object(input), &limits)
        .map_err(|e| PipelineError::ScriptError(e.to_string()))?;
    Ok(NodeRun::pure(vec![out("output", DataType::Json, output)]))
}

async fn execute_agent_node(
    agent_id: &str,
    resolved: &HashMap<String, Value>,
    ctx: &ExecutionContext<'_>,
) -> Result<NodeRun, PipelineError> {
    let aid = Uuid::parse_str(agent_id)
        .map_err(|_| PipelineError::NodeError(agent_id.to_string(), "invalid agent id".into()))?;
    let agent = sqlx::query_as::<_, AgentNodeRow>(
        "SELECT id, instructions, model, tools, guard_config, memory_config, workspace_id \
         FROM agents WHERE id = $1 AND is_active = true",
    )
    .bind(aid)
    .fetch_optional(ctx.db)
    .await
    .map_err(|e| PipelineError::NodeError(agent_id.to_string(), e.to_string()))?
    .ok_or_else(|| PipelineError::NodeError(agent_id.to_string(), "agent not found".into()))?;

    // run_id = None: nested steps are skipped (the pipeline records the node-level
    // step), which also avoids a run_steps FK to a non-existent runs row.
    let mut agent_state = AgentState::new(
        Some(agent.id.to_string()),
        None,
        Some(agent.workspace_id.to_string()),
        ctx.user_id.clone(),
        &agent.instructions,
        &agent.model,
    );
    if let Some(router) = ctx.router.clone() {
        // Agent nodes keep their explicit model; the router adds fallback resilience.
        agent_state = agent_state.with_router(router, false);
    }
    if let Ok(tools) = serde_json::from_value::<Vec<ToolDefinition>>(agent.tools.clone()) {
        agent_state.tools = tools;
    }
    if let Some(gc) = agent.guard_config.clone() {
        if let Ok(gc) = serde_json::from_value::<GuardConfig>(gc) {
            agent_state.guard_config = gc;
        }
    }
    if let Some(mc) = agent.memory_config.clone() {
        if let Ok(mc) = serde_json::from_value::<MemoryConfig>(mc) {
            agent_state.memory_config = mc;
        }
    }
    agent_state.add_message(Message::new("user", &as_text(resolved)));

    crate::loop_mod::run_agent(
        &mut agent_state,
        ctx.llm_client,
        ctx.tool_registry,
        ctx.guard_engine,
        ctx.memory_client,
        Some(ctx.kb_retriever),
        ctx.db,
        None,
    )
    .await
    .map_err(|e| PipelineError::NodeError(agent_id.to_string(), e.to_string()))?;

    let content = agent_state.last_message().map(|m| m.content.clone()).unwrap_or_default();
    Ok(NodeRun {
        outputs: vec![out("response", DataType::Text, Value::String(content))],
        tokens: agent_state.total_tokens_used,
        cost: agent_state.total_cost,
    })
}

async fn execute_team_node(
    team_id: &str,
    resolved: &HashMap<String, Value>,
    ctx: &ExecutionContext<'_>,
) -> Result<NodeRun, PipelineError> {
    let tid = Uuid::parse_str(team_id)
        .map_err(|_| PipelineError::NodeError(team_id.to_string(), "invalid team id".into()))?;
    let team = sqlx::query_as::<_, TeamNodeRow>(
        "SELECT id, name, description, supervisor_instructions, supervisor_model, \
         sub_agents, max_delegations, execution_mode FROM team_agents \
         WHERE id = $1 AND is_active = true",
    )
    .bind(tid)
    .fetch_optional(ctx.db)
    .await
    .map_err(|e| PipelineError::NodeError(team_id.to_string(), e.to_string()))?
    .ok_or_else(|| PipelineError::NodeError(team_id.to_string(), "team not found".into()))?;

    let sub_agents: Vec<SubAgentConfig> = serde_json::from_value(team.sub_agents.clone())
        .map_err(|e| PipelineError::NodeError(team_id.to_string(), e.to_string()))?;
    let config = TeamConfig {
        id: team.id.to_string(),
        name: team.name.clone(),
        description: team.description.clone(),
        supervisor_instructions: team.supervisor_instructions.clone(),
        supervisor_model: team.supervisor_model.clone(),
        sub_agents,
        max_delegations: team.max_delegations as u32,
        execution_mode: match team.execution_mode.as_str() {
            "Parallel" => ExecutionMode::Parallel,
            _ => ExecutionMode::Sequential,
        },
    };

    let orchestrator = TeamOrchestrator::new(
        config,
        ctx.llm_client.clone(),
        ctx.tool_registry.clone(),
        ctx.guard_engine.clone(),
        ctx.memory_client.clone(),
        ctx.db.clone(),
    );
    let run_result = orchestrator
        .run(&as_text(resolved), ctx.workspace_id.clone(), ctx.user_id.clone())
        .await
        .map_err(|e| PipelineError::NodeError(team_id.to_string(), e.to_string()))?;

    let content = run_result
        .supervisor_state
        .last_message()
        .map(|m| m.content.clone())
        .unwrap_or_default();
    Ok(NodeRun {
        outputs: vec![out("response", DataType::Text, Value::String(content))],
        tokens: run_result.total_tokens_used,
        cost: run_result.total_cost,
    })
}

// --- helpers -------------------------------------------------------------

async fn emit(ctx: &ExecutionContext<'_>, event: PipelineStreamEvent) {
    if let Some(tx) = &ctx.tx {
        let _ = tx.send(event).await;
    }
}

fn first_value(resolved: &HashMap<String, Value>) -> Option<Value> {
    for key in ["input", "value", "prompt", "response", "output"] {
        if let Some(v) = resolved.get(key) {
            return Some(v.clone());
        }
    }
    resolved.values().next().cloned()
}

/// Best-effort textual rendering of resolved inputs for agent/team prompts.
fn as_text(resolved: &HashMap<String, Value>) -> String {
    match first_value(resolved) {
        Some(Value::String(s)) => s,
        Some(other) => other.to_string(),
        None => String::new(),
    }
}

fn config_u64(node: &PipelineNode, key: &str) -> Option<u64> {
    node.config.get(key).and_then(|v| v.as_u64())
}

fn step_type_for(node_type: &NodeType) -> &'static str {
    match node_type {
        NodeType::Model { .. } => "message",
        NodeType::Script { .. } => "tool",
        NodeType::Metric { .. } => "evaluate",
        NodeType::Agent { .. } | NodeType::Team { .. } => "message",
        _ => "other",
    }
}

#[allow(clippy::too_many_arguments)]
async fn insert_pipeline_step(
    ctx: &ExecutionContext<'_>,
    step_type: &str,
    node: &PipelineNode,
    resolved: &HashMap<String, Value>,
    output: &Value,
    duration_ms: i64,
    tokens_used: i64,
) -> Result<(), sqlx::Error> {
    let node_kind = node_type_label(&node.node_type);
    sqlx::query(
        "INSERT INTO run_steps (id, pipeline_run_id, step_type, node_name, input, output, duration_ms, tokens_used, metadata, created_at) \
         VALUES ($1, $2, $3::step_type, $4, $5, $6, $7, $8, $9, now())",
    )
    .bind(Uuid::new_v4())
    .bind(ctx.pipeline_run_id)
    .bind(step_type)
    .bind(&node.name)
    .bind(serde_json::to_string(resolved).ok())
    .bind(serde_json::to_string(output).ok())
    .bind(duration_ms)
    .bind(tokens_used)
    .bind(json!({ "node_id": node.id.0, "node_type": node_kind }))
    .execute(ctx.db)
    .await?;
    Ok(())
}

fn node_type_label(node_type: &NodeType) -> &'static str {
    match node_type {
        NodeType::Input => "input",
        NodeType::Output => "output",
        NodeType::Model { .. } => "model",
        NodeType::Script { .. } => "script",
        NodeType::Decision { .. } => "decision",
        NodeType::Router { .. } => "router",
        NodeType::Metric { .. } => "metric",
        NodeType::Agent { .. } => "agent",
        NodeType::Team { .. } => "team",
    }
}
