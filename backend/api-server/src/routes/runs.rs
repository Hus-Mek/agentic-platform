use axum::extract::{Path, Query, State};
use axum::Extension;
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::middleware::auth::Claims;
use crate::models::{RunRow, RunStepRow};
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct RunQuery {
    pub workspace_id: Option<Uuid>,
    pub agent_id: Option<Uuid>,
    pub status: Option<String>,
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

pub async fn list_runs(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<RunQuery>,
) -> Result<Json<Vec<RunRow>>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let workspace_id = query
        .workspace_id
        .or_else(|| claims.workspace_id.parse::<Uuid>().ok())
        .ok_or_else(|| bad_request("workspace_id is required"))?;

    let agent_id = query.agent_id;
    let status = query.status.as_deref();

    let runs = if let Some(agent_id) = agent_id {
        if let Some(status) = status {
            sqlx::query_as::<_, RunRow>(
                "SELECT * FROM runs WHERE workspace_id = $1 AND agent_id = $2 AND status = $3 ORDER BY created_at DESC",
            )
            .bind(workspace_id)
            .bind(agent_id)
            .bind(status)
            .fetch_all(&state.db)
            .await
            .map_err(internal_error)?
        } else {
            sqlx::query_as::<_, RunRow>(
                "SELECT * FROM runs WHERE workspace_id = $1 AND agent_id = $2 ORDER BY created_at DESC",
            )
            .bind(workspace_id)
            .bind(agent_id)
            .fetch_all(&state.db)
            .await
            .map_err(internal_error)?
        }
    } else if let Some(status) = status {
        sqlx::query_as::<_, RunRow>(
            "SELECT * FROM runs WHERE workspace_id = $1 AND status = $2 ORDER BY created_at DESC",
        )
        .bind(workspace_id)
        .bind(status)
        .fetch_all(&state.db)
        .await
        .map_err(internal_error)?
    } else {
        sqlx::query_as::<_, RunRow>(
            "SELECT * FROM runs WHERE workspace_id = $1 ORDER BY created_at DESC",
        )
        .bind(workspace_id)
        .fetch_all(&state.db)
        .await
        .map_err(internal_error)?
    };

    Ok(Json(runs))
}

pub async fn get_run(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<RunRow>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let run = sqlx::query_as::<_, RunRow>("SELECT * FROM runs WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| not_found("Run not found"))?;

    Ok(Json(run))
}

pub async fn get_run_steps(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<RunStepRow>>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let run_exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM runs WHERE id = $1")
        .bind(id)
        .fetch_one(&state.db)
        .await
        .map_err(internal_error)?;

    if run_exists == 0 {
        return Err(not_found("Run not found"));
    }

    let steps = sqlx::query_as::<_, RunStepRow>(
        "SELECT * FROM run_steps WHERE run_id = $1 ORDER BY created_at ASC",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await
    .map_err(internal_error)?;

    Ok(Json(steps))
}

pub async fn cancel_run(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let existing = sqlx::query_as::<_, RunRow>("SELECT * FROM runs WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| not_found("Run not found"))?;

    if existing.status == "completed" || existing.status == "failed" || existing.status == "cancelled" {
        return Err(bad_request(&format!(
            "Cannot cancel run with status '{}'",
            existing.status
        )));
    }

    sqlx::query("UPDATE runs SET status = 'cancelled' WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(internal_error)?;

    Ok(Json(serde_json::json!({"cancelled": true, "id": id})))
}

pub async fn delete_run(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let existing = sqlx::query_as::<_, RunRow>("SELECT * FROM runs WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| not_found("Run not found"))?;

    sqlx::query("DELETE FROM run_steps WHERE run_id = $1")
        .bind(existing.id)
        .execute(&state.db)
        .await
        .map_err(internal_error)?;

    sqlx::query("DELETE FROM runs WHERE id = $1")
        .bind(existing.id)
        .execute(&state.db)
        .await
        .map_err(internal_error)?;

    Ok(Json(serde_json::json!({"deleted": true, "id": id})))
}
