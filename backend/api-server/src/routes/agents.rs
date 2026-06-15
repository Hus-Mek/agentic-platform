use std::convert::Infallible;

use axum::extract::{Path, Query, State};
use axum::response::sse::{Event, Sse};
use axum::response::{IntoResponse, Response};
use axum::Extension;
use axum::Json;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

use agenticos_agent_runtime::loop_mod::run_agent as runtime_run_agent;
use agenticos_agent_runtime::streaming::{run_agent_streaming, StreamEvent};
use agenticos_core::state::{AgentState, GuardConfig, MemoryConfig, Message, ToolDefinition};

use crate::middleware::auth::Claims;
use crate::models::AgentRow;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct AgentQuery {
    pub workspace_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAgentRequest {
    pub name: String,
    pub description: Option<String>,
    pub instructions: String,
    #[serde(default)]
    pub model: String,
    pub tools: Option<serde_json::Value>,
    pub guard_config: Option<serde_json::Value>,
    pub memory_config: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAgentRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub instructions: Option<String>,
    pub model: Option<String>,
    pub tools: Option<serde_json::Value>,
    pub guard_config: Option<serde_json::Value>,
    pub memory_config: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct RunRequest {
    pub input: String,
    pub model: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RunResponse {
    pub run_id: Uuid,
    pub output: String,
    pub total_tokens: i64,
    pub total_cost: f64,
    pub duration_ms: i64,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct AgentVersionRow {
    pub version_number: i32,
    pub agent_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub instructions: String,
    pub model: String,
    pub tools: serde_json::Value,
    pub guard_config: serde_json::Value,
    pub memory_config: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

fn internal_error<E: std::fmt::Display>(e: E) -> (axum::http::StatusCode, Json<serde_json::Value>) {
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({"error": e.to_string()})),
    )
}

fn not_found(msg: &str) -> (axum::http::StatusCode, Json<serde_json::Value>) {
    (
        axum::http::StatusCode::NOT_FOUND,
        Json(serde_json::json!({"error": msg})),
    )
}

fn bad_request(msg: &str) -> (axum::http::StatusCode, Json<serde_json::Value>) {
    (
        axum::http::StatusCode::BAD_REQUEST,
        Json(serde_json::json!({"error": msg})),
    )
}

pub async fn list_agents(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<AgentQuery>,
) -> Result<Json<Vec<AgentRow>>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let workspace_id = query
        .workspace_id
        .or_else(|| claims.workspace_id.parse::<Uuid>().ok())
        .ok_or_else(|| bad_request("workspace_id is required"))?;

    let agents = sqlx::query_as::<_, AgentRow>(
        "SELECT * FROM agents WHERE workspace_id = $1 AND is_active = true ORDER BY created_at DESC",
    )
    .bind(workspace_id)
    .fetch_all(&state.db)
    .await
    .map_err(internal_error)?;

    Ok(Json(agents))
}

pub async fn create_agent(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateAgentRequest>,
) -> Result<Json<AgentRow>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    if body.name.is_empty() {
        return Err(bad_request("name is required"));
    }
    if body.instructions.is_empty() {
        return Err(bad_request("instructions is required"));
    }

    let workspace_id = claims
        .workspace_id
        .parse::<Uuid>()
        .map_err(|_| bad_request("invalid workspace_id in token"))?;

    let id = Uuid::new_v4();
    let now = Utc::now();
    let tools = body.tools.unwrap_or_else(|| serde_json::json!([]));
    let guard_config = body
        .guard_config
        .unwrap_or_else(|| serde_json::to_value(GuardConfig::default()).unwrap_or_default());
    let memory_config = body
        .memory_config
        .unwrap_or_else(|| serde_json::to_value(MemoryConfig::default()).unwrap_or_default());
    let model = if body.model.is_empty() {
        "gpt-4o"
    } else {
        &body.model
    };

    sqlx::query(
        "INSERT INTO agents (id, workspace_id, name, description, instructions, model, tools, guard_config, memory_config, created_by, is_active, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
    )
    .bind(id)
    .bind(workspace_id)
    .bind(&body.name)
    .bind(&body.description)
    .bind(&body.instructions)
    .bind(model)
    .bind(&tools)
    .bind(&guard_config)
    .bind(&memory_config)
    .bind(&claims.sub)
    .bind(true)
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await
    .map_err(internal_error)?;

    let agent = sqlx::query_as::<_, AgentRow>("SELECT * FROM agents WHERE id = $1")
        .bind(id)
        .fetch_one(&state.db)
        .await
        .map_err(internal_error)?;

    Ok(Json(agent))
}

pub async fn get_agent(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<AgentRow>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let agent = sqlx::query_as::<_, AgentRow>(
        "SELECT * FROM agents WHERE id = $1 AND is_active = true",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(internal_error)?
    .ok_or_else(|| not_found("Agent not found"))?;

    Ok(Json(agent))
}

pub async fn update_agent(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateAgentRequest>,
) -> Result<Json<AgentRow>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let existing = sqlx::query_as::<_, AgentRow>(
        "SELECT * FROM agents WHERE id = $1 AND is_active = true",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(internal_error)?
    .ok_or_else(|| not_found("Agent not found"))?;

    let name = body.name.unwrap_or(existing.name);
    let description = body.description.or(existing.description);
    let instructions = body.instructions.unwrap_or(existing.instructions);
    let model = body.model.unwrap_or(existing.model);
    let tools = body.tools.unwrap_or(existing.tools);
    let guard_config = body.guard_config.unwrap_or(existing.guard_config);
    let memory_config = body.memory_config.unwrap_or(existing.memory_config);
    let now = Utc::now();

    sqlx::query(
        "UPDATE agents SET name = $1, description = $2, instructions = $3, model = $4, tools = $5, guard_config = $6, memory_config = $7, updated_at = $8 WHERE id = $9",
    )
    .bind(&name)
    .bind(&description)
    .bind(&instructions)
    .bind(&model)
    .bind(&tools)
    .bind(&guard_config)
    .bind(&memory_config)
    .bind(now)
    .bind(id)
    .execute(&state.db)
    .await
    .map_err(internal_error)?;

    let updated = sqlx::query_as::<_, AgentRow>("SELECT * FROM agents WHERE id = $1")
        .bind(id)
        .fetch_one(&state.db)
        .await
        .map_err(internal_error)?;

    Ok(Json(updated))
}

pub async fn delete_agent(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let existing = sqlx::query_as::<_, AgentRow>(
        "SELECT * FROM agents WHERE id = $1 AND is_active = true",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(internal_error)?
    .ok_or_else(|| not_found("Agent not found"))?;

    sqlx::query("UPDATE agents SET is_active = false, updated_at = $1 WHERE id = $2")
        .bind(Utc::now())
        .bind(existing.id)
        .execute(&state.db)
        .await
        .map_err(internal_error)?;

    Ok(Json(serde_json::json!({"deleted": true, "id": id})))
}

pub async fn list_versions(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<AgentVersionRow>>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let agent = sqlx::query_as::<_, AgentRow>(
        "SELECT * FROM agents WHERE id = $1 AND is_active = true",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(internal_error)?
    .ok_or_else(|| not_found("Agent not found"))?;

    let version = AgentVersionRow {
        version_number: 1,
        agent_id: agent.id,
        name: agent.name,
        description: agent.description,
        instructions: agent.instructions,
        model: agent.model,
        tools: agent.tools,
        guard_config: agent.guard_config,
        memory_config: agent.memory_config,
        created_at: agent.created_at,
        updated_at: agent.updated_at,
    };

    Ok(Json(vec![version]))
}

pub async fn run_agent(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<RunRequest>,
) -> Result<Json<RunResponse>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let agent = sqlx::query_as::<_, AgentRow>(
        "SELECT * FROM agents WHERE id = $1 AND is_active = true",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(internal_error)?
    .ok_or_else(|| not_found("Agent not found"))?;

    let run_id = Uuid::new_v4();
    let now = Utc::now();

    sqlx::query(
        "INSERT INTO runs (id, workspace_id, agent_id, status, input, created_by, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(run_id)
    .bind(agent.workspace_id)
    .bind(agent.id)
    .bind("running")
    .bind(&body.input)
    .bind(&claims.sub)
    .bind(now)
    .execute(&state.db)
    .await
    .map_err(internal_error)?;

    let model = body.model.as_deref().unwrap_or(&agent.model);

    let mut agent_state = AgentState::new(
        Some(agent.id.to_string()),
        Some(run_id.to_string()),
        Some(agent.workspace_id.to_string()),
        Some(claims.sub.clone()),
        &agent.instructions,
        model,
    );

    if let Ok(tools) = serde_json::from_value::<Vec<ToolDefinition>>(agent.tools.clone()) {
        agent_state.tools = tools;
    }
    if let Ok(gc) = serde_json::from_value::<GuardConfig>(agent.guard_config.clone()) {
        agent_state.guard_config = gc;
    }
    if let Ok(mc) = serde_json::from_value::<MemoryConfig>(agent.memory_config.clone()) {
        agent_state.memory_config = mc;
    }

    agent_state.add_message(Message::new("user", &body.input));

    let start = std::time::Instant::now();

    match runtime_run_agent(
        &mut agent_state,
        &state.llm_client,
        &state.tool_registry,
        &state.guard_engine,
        &state.memory_client,
    )
    .await
    {
        Ok(()) => {
            let duration_ms = start.elapsed().as_millis() as i64;
            let output = agent_state
                .last_message()
                .map(|m| m.content.clone())
                .unwrap_or_default();

            sqlx::query(
                "UPDATE runs SET status = 'completed', output = $1, duration_ms = $2, total_tokens = $3, total_cost = $4 WHERE id = $5",
            )
            .bind(&output)
            .bind(duration_ms)
            .bind(agent_state.total_tokens_used)
            .bind(agent_state.total_cost)
            .bind(run_id)
            .execute(&state.db)
            .await
            .map_err(internal_error)?;

            Ok(Json(RunResponse {
                run_id,
                output,
                total_tokens: agent_state.total_tokens_used,
                total_cost: agent_state.total_cost,
                duration_ms,
                status: "completed".to_string(),
            }))
        }
        Err(e) => {
            let duration_ms = start.elapsed().as_millis() as i64;

            sqlx::query(
                "UPDATE runs SET status = 'failed', error = $1, duration_ms = $2, total_tokens = $3, total_cost = $4 WHERE id = $5",
            )
            .bind(e.to_string())
            .bind(duration_ms)
            .bind(agent_state.total_tokens_used)
            .bind(agent_state.total_cost)
            .bind(run_id)
            .execute(&state.db)
            .await
            .map_err(internal_error)?;

            Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": e.to_string(),
                    "run_id": run_id
                })),
            ))
        }
    }
}

pub async fn run_agent_stream(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<RunRequest>,
) -> Response {
    let agent = match sqlx::query_as::<_, AgentRow>(
        "SELECT * FROM agents WHERE id = $1 AND is_active = true",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    {
        Ok(Some(a)) => a,
        Ok(None) => {
            return (
                axum::http::StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Agent not found"})),
            )
                .into_response()
        }
        Err(e) => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    let run_id = Uuid::new_v4();
    let now = Utc::now();

    if let Err(e) = sqlx::query(
        "INSERT INTO runs (id, workspace_id, agent_id, status, input, created_by, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(run_id)
    .bind(agent.workspace_id)
    .bind(agent.id)
    .bind("running")
    .bind(&body.input)
    .bind(&claims.sub)
    .bind(now)
    .execute(&state.db)
    .await
    {
        return (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response();
    }

    let (tx, rx) = tokio::sync::mpsc::channel::<StreamEvent>(256);
    let stream = ReceiverStream::new(rx);

    let state_for_task = state.clone();
    let agent_clone = agent;
    let claims_clone = claims;
    let input = body.input;
    let run_id_str = run_id.to_string();

    tokio::spawn(async move {
        let model = &agent_clone.model;

        let mut agent_state = AgentState::new(
            Some(agent_clone.id.to_string()),
            Some(run_id_str.clone()),
            Some(agent_clone.workspace_id.to_string()),
            Some(claims_clone.sub.clone()),
            &agent_clone.instructions,
            model,
        );

        if let Ok(tools) =
            serde_json::from_value::<Vec<ToolDefinition>>(agent_clone.tools.clone())
        {
            agent_state.tools = tools;
        }
        if let Ok(gc) =
            serde_json::from_value::<GuardConfig>(agent_clone.guard_config.clone())
        {
            agent_state.guard_config = gc;
        }
        if let Ok(mc) =
            serde_json::from_value::<MemoryConfig>(agent_clone.memory_config.clone())
        {
            agent_state.memory_config = mc;
        }

        agent_state.add_message(Message::new("user", &input));

        let db = state_for_task.db.clone();

        match run_agent_streaming(
            &mut agent_state,
            &state_for_task.llm_client,
            &state_for_task.tool_registry,
            &state_for_task.guard_engine,
            &state_for_task.memory_client,
            tx.clone(),
        )
        .await
        {
            Ok(()) => {
                let _ = tx
                    .send(StreamEvent::Done)
                    .await;

                let _ = sqlx::query(
                    "UPDATE runs SET status = 'completed', output = $1, total_tokens = $2, total_cost = $3 WHERE id = $4",
                )
                .bind(
                    agent_state
                        .last_message()
                        .map(|m| m.content.clone())
                        .unwrap_or_default(),
                )
                .bind(agent_state.total_tokens_used)
                .bind(agent_state.total_cost)
                .bind(&run_id_str)
                .execute(&db)
                .await;
            }
            Err(e) => {
                let _ = tx
                    .send(StreamEvent::Error(e.to_string()))
                    .await;
            }
        }
    });

    let sse_stream = stream.map(|event| {
        Ok::<_, Infallible>(
            Event::default().data(serde_json::to_string(&event).unwrap()),
        )
    });

    Sse::new(sse_stream).into_response()
}
