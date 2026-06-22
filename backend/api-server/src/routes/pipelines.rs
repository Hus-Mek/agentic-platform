//! Workflow Designer API — CRUD + validate + run (sync & SSE) + versioning.
//!
//! Mirrors the agent route conventions (`routes/agents.rs`): workspace-scoped via
//! `Claims`, parameterized sqlx only, `Result<Json<T>, (StatusCode, Json<Value>)>`.
//! A pipeline run is a run: execution reuses `run_steps` (keyed by
//! `pipeline_run_id`) and rolls token/cost into `pipeline_runs`.

use std::collections::HashMap;
use std::convert::Infallible;

use axum::extract::{Path, Query, State};
use axum::response::sse::{Event, Sse};
use axum::response::{IntoResponse, Response};
use axum::Extension;
use axum::Json;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use uuid::Uuid;

use agenticos_agent_runtime::pipeline_executor::{
    execute_pipeline, ExecutionContext, PipelineStreamEvent,
};
use agenticos_core::pipeline::{
    Pipeline, PipelineEdge, PipelineExecution, PipelineNode, PipelineRunInput,
};
use agenticos_core::pipeline_utils::validate_pipeline;

use crate::middleware::auth::Claims;
use crate::models::{PipelineRow, PipelineRunRow, PipelineVersionRow, RunStepRow};
use crate::AppState;

// Explicit column lists with casts (enum->text, numeric->float8, uuid->text)
// mirror the most recent `routes/runs.rs` to avoid sqlx decode mismatches.
const PIPELINE_COLS: &str = "id, workspace_id, name, description, version, graph, \
    created_by::text AS created_by, is_active, created_at, updated_at";
const PIPELINE_RUN_COLS: &str = "id, workspace_id, pipeline_id, status::text AS status, \
    input, execution, error, duration_ms, total_tokens, total_cost::float8 AS total_cost, \
    created_by::text AS created_by, created_at, updated_at";

#[derive(Debug, Deserialize)]
pub struct PipelineQuery {
    pub workspace_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePipelineRequest {
    pub name: String,
    pub description: Option<String>,
    #[serde(default = "default_graph")]
    pub graph: Value,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePipelineRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub graph: Option<Value>,
}

#[derive(Debug, Default, Deserialize)]
pub struct RunPipelineRequest {
    #[serde(default)]
    pub data: HashMap<String, Value>,
}

#[derive(Debug, Deserialize)]
pub struct DeployRequest {
    #[serde(default = "default_environment")]
    pub environment: String,
}

#[derive(Debug, Serialize)]
pub struct ValidateResponse {
    pub ok: bool,
    pub errors: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct PipelineRunResponse {
    pub run_id: Uuid,
    pub status: String,
    pub execution: Value,
    pub total_tokens: i64,
    pub total_cost: f64,
    pub duration_ms: i64,
}

/// The JSONB shape stored in `pipelines.graph`. A UI-only `layout` key may also
/// be present; the engine ignores anything beyond nodes/edges.
#[derive(Debug, Deserialize)]
struct PipelineGraph {
    #[serde(default)]
    nodes: Vec<PipelineNode>,
    #[serde(default)]
    edges: Vec<PipelineEdge>,
}

fn default_graph() -> Value {
    serde_json::json!({ "nodes": [], "edges": [] })
}

fn default_environment() -> String {
    "production".to_string()
}

// --- CRUD ---------------------------------------------------------------

pub async fn list_pipelines(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<PipelineQuery>,
) -> Result<Json<Vec<PipelineRow>>, ApiError> {
    let workspace_id = query
        .workspace_id
        .or_else(|| claims.workspace_id.parse::<Uuid>().ok())
        .ok_or_else(|| bad_request("workspace_id is required"))?;

    let pipelines = sqlx::query_as::<_, PipelineRow>(&format!(
        "SELECT {PIPELINE_COLS} FROM pipelines \
         WHERE workspace_id = $1 AND deleted_at IS NULL ORDER BY created_at DESC"
    ))
    .bind(workspace_id)
    .fetch_all(&state.db)
    .await
    .map_err(internal_error)?;

    Ok(Json(pipelines))
}

pub async fn create_pipeline(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreatePipelineRequest>,
) -> Result<Json<PipelineRow>, ApiError> {
    if body.name.trim().is_empty() {
        return Err(bad_request("name is required"));
    }
    let workspace_id = claims
        .workspace_id
        .parse::<Uuid>()
        .map_err(|_| bad_request("invalid workspace_id in token"))?;

    // Reject an invalid DAG up front.
    build_pipeline("new", &body.name, body.description.as_deref(), "1.0", &body.graph)
        .and_then(|p| validate_pipeline(&p).map_err(|e| e.to_string()))
        .map_err(|e| bad_request(&format!("invalid pipeline graph: {e}")))?;

    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO pipelines (id, workspace_id, name, description, version, graph, created_by) \
         VALUES ($1, $2, $3, $4, '1.0', $5, $6)",
    )
    .bind(id)
    .bind(workspace_id)
    .bind(&body.name)
    .bind(&body.description)
    .bind(&body.graph)
    .bind(claims.sub.parse::<Uuid>().ok())
    .execute(&state.db)
    .await
    .map_err(internal_error)?;

    fetch_pipeline(&state, id).await.map(Json)
}

pub async fn get_pipeline(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<PipelineRow>, ApiError> {
    fetch_pipeline(&state, id).await.map(Json)
}

pub async fn update_pipeline(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<UpdatePipelineRequest>,
) -> Result<Json<PipelineRow>, ApiError> {
    let existing = fetch_pipeline(&state, id).await?;

    // Snapshot the prior config before mutating (full versioning).
    snapshot_version(&state, &existing, claims.sub.parse::<Uuid>().ok()).await?;

    let name = body.name.unwrap_or(existing.name);
    let description = body.description.or(existing.description);
    let graph = body.graph.unwrap_or(existing.graph);

    build_pipeline(&id.to_string(), &name, description.as_deref(), &existing.version, &graph)
        .and_then(|p| validate_pipeline(&p).map_err(|e| e.to_string()))
        .map_err(|e| bad_request(&format!("invalid pipeline graph: {e}")))?;

    sqlx::query(
        "UPDATE pipelines SET name = $1, description = $2, graph = $3 WHERE id = $4",
    )
    .bind(&name)
    .bind(&description)
    .bind(&graph)
    .bind(id)
    .execute(&state.db)
    .await
    .map_err(internal_error)?;

    fetch_pipeline(&state, id).await.map(Json)
}

pub async fn delete_pipeline(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    let _ = fetch_pipeline(&state, id).await?;
    sqlx::query(
        "UPDATE pipelines SET is_active = false, deleted_at = now() WHERE id = $1",
    )
    .bind(id)
    .execute(&state.db)
    .await
    .map_err(internal_error)?;
    Ok(Json(serde_json::json!({ "deleted": true, "id": id })))
}

pub async fn validate_pipeline_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ValidateResponse>, ApiError> {
    let row = fetch_pipeline(&state, id).await?;
    let response = match build_pipeline(
        &id.to_string(),
        &row.name,
        row.description.as_deref(),
        &row.version,
        &row.graph,
    ) {
        Ok(p) => match validate_pipeline(&p) {
            Ok(()) => ValidateResponse { ok: true, errors: vec![] },
            Err(e) => ValidateResponse { ok: false, errors: vec![e.to_string()] },
        },
        Err(e) => ValidateResponse { ok: false, errors: vec![e] },
    };
    Ok(Json(response))
}

// --- Versioning ---------------------------------------------------------

pub async fn list_pipeline_versions(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<PipelineVersionRow>>, ApiError> {
    let _ = fetch_pipeline(&state, id).await?;
    let versions = sqlx::query_as::<_, PipelineVersionRow>(
        "SELECT id, pipeline_id, version_number, config_snapshot, \
         created_by::text AS created_by, created_at \
         FROM pipeline_versions WHERE pipeline_id = $1 ORDER BY version_number DESC",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await
    .map_err(internal_error)?;
    Ok(Json(versions))
}

pub async fn deploy_pipeline(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<DeployRequest>,
) -> Result<Json<Value>, ApiError> {
    let row = fetch_pipeline(&state, id).await?;
    let version_number = snapshot_version(&state, &row, claims.sub.parse::<Uuid>().ok()).await?;
    Ok(Json(serde_json::json!({
        "pipeline_id": id,
        "version_number": version_number,
        "environment": body.environment,
        "deployed_at": Utc::now(),
    })))
}

pub async fn rollback_pipeline(
    State(state): State<AppState>,
    Path((id, version)): Path<(Uuid, i32)>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<PipelineRow>, ApiError> {
    let current = fetch_pipeline(&state, id).await?;
    let target = sqlx::query_as::<_, PipelineVersionRow>(
        "SELECT id, pipeline_id, version_number, config_snapshot, \
         created_by::text AS created_by, created_at \
         FROM pipeline_versions WHERE pipeline_id = $1 AND version_number = $2",
    )
    .bind(id)
    .bind(version)
    .fetch_optional(&state.db)
    .await
    .map_err(internal_error)?
    .ok_or_else(|| not_found("Version not found"))?;

    // Snapshot current state, then restore the target snapshot's graph/name.
    snapshot_version(&state, &current, claims.sub.parse::<Uuid>().ok()).await?;
    let snap = &target.config_snapshot;
    let name = snap.get("name").and_then(|v| v.as_str()).unwrap_or(&current.name).to_string();
    let description = snap
        .get("description")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or(current.description);
    let graph = snap.get("graph").cloned().unwrap_or(current.graph);

    sqlx::query("UPDATE pipelines SET name = $1, description = $2, graph = $3 WHERE id = $4")
        .bind(&name)
        .bind(&description)
        .bind(&graph)
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(internal_error)?;

    fetch_pipeline(&state, id).await.map(Json)
}

// --- Run ----------------------------------------------------------------

pub async fn run_pipeline(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<RunPipelineRequest>,
) -> Result<Json<PipelineRunResponse>, ApiError> {
    let row = fetch_pipeline(&state, id).await?;
    let pipeline = build_pipeline(
        &id.to_string(),
        &row.name,
        row.description.as_deref(),
        &row.version,
        &row.graph,
    )
    .map_err(|e| bad_request(&format!("invalid pipeline graph: {e}")))?;

    let run_id = insert_pipeline_run(&state, &row, &body.data, &claims).await?;
    let run_input = PipelineRunInput { pipeline_id: id.to_string(), data: body.data };

    let router_snapshot = state.model_router.read().await.clone();
    let ctx = ExecutionContext {
        llm_client: &state.llm_client,
        tool_registry: &state.tool_registry,
        guard_engine: &state.guard_engine,
        memory_client: &state.memory_client,
        kb_retriever: &state.kb_retriever,
        db: &state.db,
        workspace_id: Some(row.workspace_id.to_string()),
        user_id: Some(claims.sub.clone()),
        pipeline_run_id: run_id,
        cancel: None,
        tx: None,
        router: Some(router_snapshot),
    };

    let start = std::time::Instant::now();
    let outcome = execute_pipeline(&pipeline, &run_input, &ctx)
        .await
        .map_err(internal_error)?;
    let duration_ms = start.elapsed().as_millis() as i64;
    let status = execution_status_str(&outcome.execution);
    let execution_json = serde_json::to_value(&outcome.execution).unwrap_or(Value::Null);

    finalize_pipeline_run(
        &state,
        run_id,
        row.workspace_id,
        &status,
        &execution_json,
        outcome.execution.error.as_deref(),
        duration_ms,
        outcome.total_tokens,
        outcome.total_cost,
    )
    .await?;

    Ok(Json(PipelineRunResponse {
        run_id,
        status,
        execution: execution_json,
        total_tokens: outcome.total_tokens,
        total_cost: outcome.total_cost,
        duration_ms,
    }))
}

pub async fn run_pipeline_stream(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<RunPipelineRequest>,
) -> Response {
    let row = match fetch_pipeline(&state, id).await {
        Ok(r) => r,
        Err(e) => return e.into_response(),
    };
    let pipeline = match build_pipeline(
        &id.to_string(),
        &row.name,
        row.description.as_deref(),
        &row.version,
        &row.graph,
    ) {
        Ok(p) => p,
        Err(e) => return bad_request(&format!("invalid pipeline graph: {e}")).into_response(),
    };
    let run_id = match insert_pipeline_run(&state, &row, &body.data, &claims).await {
        Ok(rid) => rid,
        Err(e) => return e.into_response(),
    };

    let (tx, rx) = tokio::sync::mpsc::channel::<PipelineStreamEvent>(256);
    let stream = ReceiverStream::new(rx);

    let state_task = state.clone();
    let ws_uuid = row.workspace_id;
    let workspace_id = row.workspace_id.to_string();
    let user_id = claims.sub.clone();
    let data = body.data;

    tokio::spawn(async move {
        let run_input = PipelineRunInput { pipeline_id: id.to_string(), data };
        let router_snapshot = state_task.model_router.read().await.clone();
        let ctx = ExecutionContext {
            llm_client: &state_task.llm_client,
            tool_registry: &state_task.tool_registry,
            guard_engine: &state_task.guard_engine,
            memory_client: &state_task.memory_client,
            kb_retriever: &state_task.kb_retriever,
            db: &state_task.db,
            workspace_id: Some(workspace_id),
            user_id: Some(user_id),
            pipeline_run_id: run_id,
            cancel: None,
            tx: Some(tx),
            router: Some(router_snapshot),
        };

        let start = std::time::Instant::now();
        match execute_pipeline(&pipeline, &run_input, &ctx).await {
            Ok(outcome) => {
                let duration_ms = start.elapsed().as_millis() as i64;
                let status = execution_status_str(&outcome.execution);
                let execution_json =
                    serde_json::to_value(&outcome.execution).unwrap_or(Value::Null);
                let _ = finalize_pipeline_run(
                    &state_task,
                    run_id,
                    ws_uuid,
                    &status,
                    &execution_json,
                    outcome.execution.error.as_deref(),
                    duration_ms,
                    outcome.total_tokens,
                    outcome.total_cost,
                )
                .await;
            }
            Err(e) => {
                let _ = sqlx::query(
                    "UPDATE pipeline_runs SET status = 'failed'::run_status, error = $1 WHERE id = $2",
                )
                .bind(e.to_string())
                .bind(run_id)
                .execute(&state_task.db)
                .await;
            }
        }
    });

    let sse_stream = stream.map(|event| {
        Ok::<_, Infallible>(Event::default().data(serde_json::to_string(&event).unwrap_or_default()))
    });
    Sse::new(sse_stream).into_response()
}

pub async fn list_pipeline_runs(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<PipelineRunRow>>, ApiError> {
    let runs = sqlx::query_as::<_, PipelineRunRow>(&format!(
        "SELECT {PIPELINE_RUN_COLS} FROM pipeline_runs \
         WHERE pipeline_id = $1 ORDER BY created_at DESC"
    ))
    .bind(id)
    .fetch_all(&state.db)
    .await
    .map_err(internal_error)?;
    Ok(Json(runs))
}

pub async fn get_pipeline_run(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<PipelineRunRow>, ApiError> {
    let run = sqlx::query_as::<_, PipelineRunRow>(&format!(
        "SELECT {PIPELINE_RUN_COLS} FROM pipeline_runs WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(internal_error)?
    .ok_or_else(|| not_found("Pipeline run not found"))?;
    Ok(Json(run))
}

pub async fn get_pipeline_run_steps(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<RunStepRow>>, ApiError> {
    let steps = sqlx::query_as::<_, RunStepRow>(
        "SELECT id, run_id, pipeline_run_id, step_type::text AS step_type, node_name, \
         input, output, duration_ms, tokens_used, metadata, created_at \
         FROM run_steps WHERE pipeline_run_id = $1 ORDER BY created_at ASC",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await
    .map_err(internal_error)?;
    Ok(Json(steps))
}

pub async fn cancel_pipeline_run(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    let status: Option<(String,)> =
        sqlx::query_as("SELECT status::text FROM pipeline_runs WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.db)
            .await
            .map_err(internal_error)?;
    let status = status.ok_or_else(|| not_found("Pipeline run not found"))?.0;
    if matches!(status.as_str(), "completed" | "failed" | "cancelled") {
        return Err(bad_request(&format!("Cannot cancel run with status '{status}'")));
    }
    sqlx::query("UPDATE pipeline_runs SET status = 'cancelled'::run_status WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(internal_error)?;
    Ok(Json(serde_json::json!({ "cancelled": true, "id": id })))
}

// --- internals ----------------------------------------------------------

async fn fetch_pipeline(state: &AppState, id: Uuid) -> Result<PipelineRow, ApiError> {
    sqlx::query_as::<_, PipelineRow>(&format!(
        "SELECT {PIPELINE_COLS} FROM pipelines WHERE id = $1 AND deleted_at IS NULL"
    ))
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(internal_error)?
    .ok_or_else(|| not_found("Pipeline not found"))
}

/// Insert the next-numbered snapshot of `row`; returns the version number.
async fn snapshot_version(
    state: &AppState,
    row: &PipelineRow,
    created_by: Option<Uuid>,
) -> Result<i32, ApiError> {
    let snapshot = serde_json::json!({
        "name": row.name,
        "description": row.description,
        "version": row.version,
        "graph": row.graph,
    });
    let next: (i32,) = sqlx::query_as(
        "SELECT COALESCE(MAX(version_number), 0) + 1 FROM pipeline_versions WHERE pipeline_id = $1",
    )
    .bind(row.id)
    .fetch_one(&state.db)
    .await
    .map_err(internal_error)?;
    sqlx::query(
        "INSERT INTO pipeline_versions (pipeline_id, version_number, config_snapshot, created_by) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(row.id)
    .bind(next.0)
    .bind(&snapshot)
    .bind(created_by)
    .execute(&state.db)
    .await
    .map_err(internal_error)?;
    Ok(next.0)
}

async fn insert_pipeline_run(
    state: &AppState,
    row: &PipelineRow,
    data: &HashMap<String, Value>,
    claims: &Claims,
) -> Result<Uuid, ApiError> {
    let run_id = Uuid::new_v4();
    let input = serde_json::to_value(data).unwrap_or(Value::Null);
    sqlx::query(
        "INSERT INTO pipeline_runs (id, workspace_id, pipeline_id, status, input, created_by) \
         VALUES ($1, $2, $3, 'running'::run_status, $4, $5)",
    )
    .bind(run_id)
    .bind(row.workspace_id)
    .bind(row.id)
    .bind(&input)
    .bind(claims.sub.parse::<Uuid>().ok())
    .execute(&state.db)
    .await
    .map_err(internal_error)?;
    Ok(run_id)
}

#[allow(clippy::too_many_arguments)]
async fn finalize_pipeline_run(
    state: &AppState,
    run_id: Uuid,
    workspace_id: Uuid,
    status: &str,
    execution: &Value,
    error: Option<&str>,
    duration_ms: i64,
    total_tokens: i64,
    total_cost: f64,
) -> Result<(), ApiError> {
    sqlx::query(
        "UPDATE pipeline_runs SET status = $1::run_status, execution = $2, error = $3, \
         duration_ms = $4, total_tokens = $5::int, total_cost = $6::numeric WHERE id = $7",
    )
    .bind(status)
    .bind(execution)
    .bind(error)
    .bind(duration_ms)
    .bind(total_tokens as i32)
    .bind(total_cost)
    .bind(run_id)
    .execute(&state.db)
    .await
    .map_err(internal_error)?;

    // Roll pipeline tokens/cost into the shared daily workspace_usage bucket,
    // exactly as agent runs do (agent-runtime::loop_mod::record_workspace_usage).
    record_workspace_usage(&state.db, workspace_id, total_tokens, total_cost).await;
    Ok(())
}

/// Upsert today's workspace_usage row (best-effort; mirrors the agent-run path).
async fn record_workspace_usage(db: &sqlx::PgPool, workspace_id: Uuid, tokens: i64, cost: f64) {
    let today = Utc::now().date_naive();
    let _ = sqlx::query(
        "INSERT INTO workspace_usage (id, workspace_id, date, total_tokens, total_cost, request_count) \
         VALUES (gen_random_uuid(), $1, $2, $3, $4, 1) \
         ON CONFLICT (workspace_id, date) DO UPDATE SET \
           total_tokens = workspace_usage.total_tokens + EXCLUDED.total_tokens, \
           total_cost = workspace_usage.total_cost + EXCLUDED.total_cost, \
           request_count = workspace_usage.request_count + 1",
    )
    .bind(workspace_id)
    .bind(today)
    .bind(tokens)
    .bind(cost)
    .execute(db)
    .await;
}

/// Reconstruct a runnable `Pipeline` from stored scalar columns + graph JSONB.
fn build_pipeline(
    id: &str,
    name: &str,
    description: Option<&str>,
    version: &str,
    graph: &Value,
) -> Result<Pipeline, String> {
    let parsed: PipelineGraph =
        serde_json::from_value(graph.clone()).map_err(|e| format!("graph parse error: {e}"))?;
    Ok(Pipeline {
        id: id.to_string(),
        name: name.to_string(),
        description: description.unwrap_or_default().to_string(),
        version: version.to_string(),
        nodes: parsed.nodes,
        edges: parsed.edges,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    })
}

fn execution_status_str(execution: &PipelineExecution) -> String {
    use agenticos_core::pipeline::PipelineExecutionStatus::*;
    match execution.status {
        Completed => "completed",
        Failed => "failed",
        Cancelled => "cancelled",
        Running => "running",
        Pending => "pending",
    }
    .to_string()
}

// --- error helpers (inline, matching teams.rs / workspaces.rs) -----------

/// `(StatusCode, Json<Value>)` already implements `IntoResponse`; this alias
/// keeps handler signatures terse.
type ApiError = (axum::http::StatusCode, Json<Value>);

fn bad_request(msg: &str) -> ApiError {
    (axum::http::StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": msg })))
}

fn not_found(msg: &str) -> ApiError {
    (axum::http::StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": msg })))
}

fn internal_error<E: std::fmt::Display>(e: E) -> ApiError {
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "error": e.to_string() })),
    )
}
