use axum::extract::{Path, State};
use axum::Extension;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::middleware::auth::Claims;
use crate::models::WorkspaceRow;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct CreateWorkspaceRequest {
    pub name: String,
    pub slug: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateWorkspaceRequest {
    pub name: Option<String>,
    pub slug: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddMemberRequest {
    pub user_id: Uuid,
    pub role: String,
}

#[derive(Debug, Serialize)]
pub struct WorkspaceListItem {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct WorkspaceMemberResponse {
    pub user_id: Uuid,
    pub role: String,
    pub created_at: DateTime<Utc>,
}

fn internal_error<E: std::fmt::Display>(e: E) -> (axum::http::StatusCode, Json<serde_json::Value>) {
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({"error": e.to_string()})),
    )
}

fn forbidden(msg: &str) -> (axum::http::StatusCode, Json<serde_json::Value>) {
    (
        axum::http::StatusCode::FORBIDDEN,
        Json(serde_json::json!({"error": msg})),
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

async fn verify_access(
    db: &sqlx::PgPool,
    workspace_id: Uuid,
    user_id: Uuid,
    min_role: &str,
) -> Result<bool, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let has_access = sqlx::query_scalar::<_, bool>(
        "SELECT check_workspace_access($1, $2, $3::member_role)",
    )
    .bind(workspace_id)
    .bind(user_id)
    .bind(min_role)
    .fetch_one(db)
    .await
    .map_err(internal_error)?;
    Ok(has_access)
}

pub async fn list_workspaces(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Vec<WorkspaceListItem>>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let user_id = claims
        .sub
        .parse::<Uuid>()
        .map_err(|_| bad_request("invalid user_id in token"))?;

    let rows = sqlx::query_as::<_, (Uuid, String, String, String, DateTime<Utc>)>(
        r#"SELECT w.id, w.name, w.slug, wm.role::text, wm.created_at
           FROM workspaces w
           JOIN workspace_members wm ON wm.workspace_id = w.id
           WHERE wm.user_id = $1
           ORDER BY wm.created_at DESC"#,
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await
    .map_err(internal_error)?;

    let workspaces = rows
        .into_iter()
        .map(|(id, name, slug, role, created_at)| WorkspaceListItem {
            id,
            name,
            slug,
            role,
            created_at,
        })
        .collect();

    Ok(Json(workspaces))
}

pub async fn create_workspace(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateWorkspaceRequest>,
) -> Result<Json<WorkspaceRow>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    if body.name.is_empty() {
        return Err(bad_request("name is required"));
    }
    if body.slug.is_empty() {
        return Err(bad_request("slug is required"));
    }

    let user_id = claims
        .sub
        .parse::<Uuid>()
        .map_err(|_| bad_request("invalid user_id in token"))?;

    let workspace_id = Uuid::new_v4();
    let now = Utc::now();

    sqlx::query(
        "INSERT INTO workspaces (id, name, slug, created_at, updated_at) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(workspace_id)
    .bind(&body.name)
    .bind(&body.slug)
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await
    .map_err(internal_error)?;

    sqlx::query(
        "INSERT INTO workspace_members (workspace_id, user_id, role) VALUES ($1, $2, 'owner')",
    )
    .bind(workspace_id)
    .bind(user_id)
    .execute(&state.db)
    .await
    .map_err(internal_error)?;

    let workspace = sqlx::query_as::<_, WorkspaceRow>("SELECT * FROM workspaces WHERE id = $1")
        .bind(workspace_id)
        .fetch_one(&state.db)
        .await
        .map_err(internal_error)?;

    Ok(Json(workspace))
}

pub async fn get_workspace(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let user_id = claims
        .sub
        .parse::<Uuid>()
        .map_err(|_| bad_request("invalid user_id in token"))?;

    if !verify_access(&state.db, id, user_id, "viewer").await? {
        return Err(forbidden("no access to this workspace"));
    }

    let workspace = sqlx::query_as::<_, WorkspaceRow>("SELECT * FROM workspaces WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| not_found("Workspace not found"))?;

    let member_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM workspace_members WHERE workspace_id = $1",
    )
    .bind(id)
    .fetch_one(&state.db)
    .await
    .map_err(internal_error)?;

    Ok(Json(serde_json::json!({
        "id": workspace.id,
        "name": workspace.name,
        "slug": workspace.slug,
        "created_at": workspace.created_at,
        "member_count": member_count,
    })))
}

pub async fn update_workspace(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateWorkspaceRequest>,
) -> Result<Json<WorkspaceRow>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let user_id = claims
        .sub
        .parse::<Uuid>()
        .map_err(|_| bad_request("invalid user_id in token"))?;

    if !verify_access(&state.db, id, user_id, "admin").await? {
        return Err(forbidden("only owners and admins can update this workspace"));
    }

    let existing = sqlx::query_as::<_, WorkspaceRow>("SELECT * FROM workspaces WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| not_found("Workspace not found"))?;

    let name = body.name.unwrap_or(existing.name);
    let slug = body.slug.unwrap_or(existing.slug);

    sqlx::query("UPDATE workspaces SET name = $1, slug = $2 WHERE id = $3")
        .bind(&name)
        .bind(&slug)
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(internal_error)?;

    let updated = sqlx::query_as::<_, WorkspaceRow>("SELECT * FROM workspaces WHERE id = $1")
        .bind(id)
        .fetch_one(&state.db)
        .await
        .map_err(internal_error)?;

    Ok(Json(updated))
}

pub async fn delete_workspace(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let user_id = claims
        .sub
        .parse::<Uuid>()
        .map_err(|_| bad_request("invalid user_id in token"))?;

    if !verify_access(&state.db, id, user_id, "owner").await? {
        return Err(forbidden("only the owner can delete this workspace"));
    }

    let existing = sqlx::query_as::<_, WorkspaceRow>("SELECT * FROM workspaces WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| not_found("Workspace not found"))?;

    sqlx::query("DELETE FROM workspaces WHERE id = $1")
        .bind(existing.id)
        .execute(&state.db)
        .await
        .map_err(internal_error)?;

    Ok(Json(serde_json::json!({"deleted": true, "id": id})))
}

pub async fn list_members(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<WorkspaceMemberResponse>>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let user_id = claims
        .sub
        .parse::<Uuid>()
        .map_err(|_| bad_request("invalid user_id in token"))?;

    if !verify_access(&state.db, id, user_id, "viewer").await? {
        return Err(forbidden("no access to this workspace"));
    }

    let rows = sqlx::query_as::<_, (Uuid, String, DateTime<Utc>)>(
        r#"SELECT user_id, role::text, created_at FROM workspace_members WHERE workspace_id = $1 ORDER BY created_at"#,
    )
    .bind(id)
    .fetch_all(&state.db)
    .await
    .map_err(internal_error)?;

    let members = rows
        .into_iter()
        .map(|(user_id, role, created_at)| WorkspaceMemberResponse {
            user_id,
            role,
            created_at,
        })
        .collect();

    Ok(Json(members))
}

pub async fn add_member(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(body): Json<AddMemberRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let user_id = claims
        .sub
        .parse::<Uuid>()
        .map_err(|_| bad_request("invalid user_id in token"))?;

    if !verify_access(&state.db, id, user_id, "admin").await? {
        return Err(forbidden("only owners and admins can add members"));
    }

    if body.role.is_empty() {
        return Err(bad_request("role is required"));
    }

    sqlx::query(
        "INSERT INTO workspace_members (workspace_id, user_id, role) VALUES ($1, $2, $3::member_role) ON CONFLICT (workspace_id, user_id) DO UPDATE SET role = $3::member_role",
    )
    .bind(id)
    .bind(body.user_id)
    .bind(&body.role)
    .execute(&state.db)
    .await
    .map_err(internal_error)?;

    Ok(Json(serde_json::json!({"added": true, "workspace_id": id, "user_id": body.user_id, "role": body.role})))
}

pub async fn remove_member(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((workspace_id, member_user_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let current_user_id = claims
        .sub
        .parse::<Uuid>()
        .map_err(|_| bad_request("invalid user_id in token"))?;

    if !verify_access(&state.db, workspace_id, current_user_id, "admin").await? {
        return Err(forbidden("only owners and admins can remove members"));
    }

    let result = sqlx::query(
        "DELETE FROM workspace_members WHERE workspace_id = $1 AND user_id = $2",
    )
    .bind(workspace_id)
    .bind(member_user_id)
    .execute(&state.db)
    .await
    .map_err(internal_error)?;

    if result.rows_affected() == 0 {
        return Err(not_found("Member not found"));
    }

    Ok(Json(serde_json::json!({"removed": true, "workspace_id": workspace_id, "user_id": member_user_id})))
}
