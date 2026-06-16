use axum::extract::{Path, Query, State};
use axum::Extension;
use axum::Json;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use agenticos_agent_runtime::orchestrator::TeamOrchestrator;
use agenticos_core::state::{ExecutionMode, SubAgentConfig, TeamConfig};

use crate::middleware::auth::Claims;
use crate::models::{TeamAgentRow, TeamRunRow};
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct TeamQuery {
    pub workspace_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTeamRequest {
    pub name: String,
    pub description: Option<String>,
    pub supervisor_instructions: String,
    pub supervisor_model: Option<String>,
    pub sub_agents: Vec<SubAgentConfig>,
    pub max_delegations: Option<i32>,
    pub execution_mode: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTeamRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub supervisor_instructions: Option<String>,
    pub supervisor_model: Option<String>,
    pub sub_agents: Option<Vec<SubAgentConfig>>,
    pub max_delegations: Option<i32>,
    pub execution_mode: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TeamRunRequest {
    pub input: String,
}

#[derive(Debug, Serialize)]
pub struct TeamRunResponse {
    pub run_id: Uuid,
    pub output: String,
    pub total_tokens: i64,
    pub total_cost: f64,
    pub duration_ms: i64,
    pub status: String,
    pub delegations: serde_json::Value,
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

pub async fn list_teams(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<TeamQuery>,
) -> Result<Json<Vec<TeamAgentRow>>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let workspace_id = query
        .workspace_id
        .or_else(|| claims.workspace_id.parse::<Uuid>().ok())
        .ok_or_else(|| bad_request("workspace_id is required"))?;

    let teams = sqlx::query_as::<_, TeamAgentRow>(
        "SELECT * FROM team_agents WHERE workspace_id = $1 AND is_active = true ORDER BY created_at DESC",
    )
    .bind(workspace_id)
    .fetch_all(&state.db)
    .await
    .map_err(internal_error)?;

    Ok(Json(teams))
}

pub async fn create_team(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateTeamRequest>,
) -> Result<Json<TeamAgentRow>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    if body.name.is_empty() {
        return Err(bad_request("name is required"));
    }
    if body.supervisor_instructions.is_empty() {
        return Err(bad_request("supervisor_instructions is required"));
    }

    let workspace_id = claims
        .workspace_id
        .parse::<Uuid>()
        .map_err(|_| bad_request("invalid workspace_id in token"))?;

    let id = Uuid::new_v4();
    let now = Utc::now();
    let sub_agents = serde_json::to_value(&body.sub_agents)
        .unwrap_or_else(|_| serde_json::json!([]));
    let supervisor_model = body
        .supervisor_model
        .unwrap_or_else(|| "gpt-4o".to_string());
    let max_delegations = body.max_delegations.unwrap_or(10);
    let execution_mode = body
        .execution_mode
        .unwrap_or_else(|| "Sequential".to_string());

    sqlx::query(
        "INSERT INTO team_agents (id, workspace_id, name, description, supervisor_instructions, supervisor_model, sub_agents, max_delegations, execution_mode, created_by, is_active, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
    )
    .bind(id)
    .bind(workspace_id)
    .bind(&body.name)
    .bind(&body.description)
    .bind(&body.supervisor_instructions)
    .bind(&supervisor_model)
    .bind(&sub_agents)
    .bind(max_delegations)
    .bind(&execution_mode)
    .bind(&claims.sub)
    .bind(true)
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await
    .map_err(internal_error)?;

    let team = sqlx::query_as::<_, TeamAgentRow>("SELECT * FROM team_agents WHERE id = $1")
        .bind(id)
        .fetch_one(&state.db)
        .await
        .map_err(internal_error)?;

    Ok(Json(team))
}

pub async fn get_team(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<TeamAgentRow>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let team = sqlx::query_as::<_, TeamAgentRow>(
        "SELECT * FROM team_agents WHERE id = $1 AND is_active = true",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(internal_error)?
    .ok_or_else(|| not_found("Team not found"))?;

    Ok(Json(team))
}

pub async fn update_team(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateTeamRequest>,
) -> Result<Json<TeamAgentRow>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let existing = sqlx::query_as::<_, TeamAgentRow>(
        "SELECT * FROM team_agents WHERE id = $1 AND is_active = true",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(internal_error)?
    .ok_or_else(|| not_found("Team not found"))?;

    let name = body.name.unwrap_or(existing.name);
    let description = body.description.or(existing.description);
    let supervisor_instructions = body
        .supervisor_instructions
        .unwrap_or(existing.supervisor_instructions);
    let supervisor_model = body
        .supervisor_model
        .unwrap_or(existing.supervisor_model);
    let sub_agents = if let Some(ref sa) = body.sub_agents {
        serde_json::to_value(sa).unwrap_or(existing.sub_agents)
    } else {
        existing.sub_agents
    };
    let max_delegations = body.max_delegations.unwrap_or(existing.max_delegations);
    let execution_mode = body.execution_mode.unwrap_or(existing.execution_mode);
    let now = Utc::now();

    sqlx::query(
        "UPDATE team_agents SET name = $1, description = $2, supervisor_instructions = $3, supervisor_model = $4, sub_agents = $5, max_delegations = $6, execution_mode = $7, updated_at = $8 WHERE id = $9",
    )
    .bind(&name)
    .bind(&description)
    .bind(&supervisor_instructions)
    .bind(&supervisor_model)
    .bind(&sub_agents)
    .bind(max_delegations)
    .bind(&execution_mode)
    .bind(now)
    .bind(id)
    .execute(&state.db)
    .await
    .map_err(internal_error)?;

    let updated = sqlx::query_as::<_, TeamAgentRow>("SELECT * FROM team_agents WHERE id = $1")
        .bind(id)
        .fetch_one(&state.db)
        .await
        .map_err(internal_error)?;

    Ok(Json(updated))
}

pub async fn delete_team(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let existing = sqlx::query_as::<_, TeamAgentRow>(
        "SELECT * FROM team_agents WHERE id = $1 AND is_active = true",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(internal_error)?
    .ok_or_else(|| not_found("Team not found"))?;

    sqlx::query("UPDATE team_agents SET is_active = false, updated_at = $1 WHERE id = $2")
        .bind(Utc::now())
        .bind(existing.id)
        .execute(&state.db)
        .await
        .map_err(internal_error)?;

    Ok(Json(serde_json::json!({"deleted": true, "id": id})))
}

pub async fn run_team(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<TeamRunRequest>,
) -> Result<Json<TeamRunResponse>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let team = sqlx::query_as::<_, TeamAgentRow>(
        "SELECT * FROM team_agents WHERE id = $1 AND is_active = true",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(internal_error)?
    .ok_or_else(|| not_found("Team not found"))?;

    let workspace_id = claims
        .workspace_id
        .parse::<Uuid>()
        .map_err(|_| bad_request("invalid workspace_id in token"))?;

    let sub_agents: Vec<SubAgentConfig> = serde_json::from_value(team.sub_agents.clone())
        .map_err(|e| bad_request(&format!("Invalid sub_agents config: {}", e)))?;

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
        state.llm_client.clone(),
        state.tool_registry.clone(),
        state.guard_engine.clone(),
        state.memory_client.clone(),
        state.db.clone(),
    );

    let start = std::time::Instant::now();
    let run_result = orchestrator
        .run(
            &body.input,
            Some(workspace_id.to_string()),
            Some(claims.sub.clone()),
        )
        .await
        .map_err(internal_error)?;
    let duration_ms = start.elapsed().as_millis() as i64;

    let output = run_result
        .supervisor_state
        .last_message()
        .map(|m| m.content.clone())
        .unwrap_or_default();

    let run_id = Uuid::new_v4();
    let now = Utc::now();
    let delegations_json = serde_json::to_value(&run_result.delegations)
        .unwrap_or_else(|_| serde_json::json!([]));

    sqlx::query(
        "INSERT INTO team_runs (id, team_id, workspace_id, status, input, output, error, delegations, total_tokens, total_cost, duration_ms, created_by, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)",
    )
    .bind(run_id)
    .bind(team.id)
    .bind(workspace_id)
    .bind(&run_result.status)
    .bind(&body.input)
    .bind(&output)
    .bind(None::<String>)
    .bind(&delegations_json)
    .bind(run_result.total_tokens_used)
    .bind(run_result.total_cost)
    .bind(duration_ms)
    .bind(&claims.sub)
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await
    .map_err(internal_error)?;

    Ok(Json(TeamRunResponse {
        run_id,
        output,
        total_tokens: run_result.total_tokens_used,
        total_cost: run_result.total_cost,
        duration_ms,
        status: run_result.status,
        delegations: delegations_json,
    }))
}

pub async fn list_team_runs(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<TeamRunRow>>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let team_exists =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM team_agents WHERE id = $1")
            .bind(id)
            .fetch_one(&state.db)
            .await
            .map_err(internal_error)?;

    if team_exists == 0 {
        return Err(not_found("Team not found"));
    }

    let runs = sqlx::query_as::<_, TeamRunRow>(
        "SELECT * FROM team_runs WHERE team_id = $1 ORDER BY created_at DESC",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await
    .map_err(internal_error)?;

    Ok(Json(runs))
}

pub async fn get_team_run(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<TeamRunRow>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let run = sqlx::query_as::<_, TeamRunRow>("SELECT * FROM team_runs WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| not_found("Team run not found"))?;

    Ok(Json(run))
}
