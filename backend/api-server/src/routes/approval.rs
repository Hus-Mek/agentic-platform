use axum::extract::{Path, State};
use axum::Json;
use serde_json::json;

use crate::AppState;

pub async fn list_approvals(State(state): State<AppState>) -> Json<serde_json::Value> {
    let pending = state
        .tool_registry
        .approval_store
        .list_pending()
        .await;
    Json(json!(pending))
}

pub async fn approve_request(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    let ok = state.tool_registry.approval_store.approve(&id).await;
    Json(json!({"approved": ok, "id": id}))
}

pub async fn reject_request(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    let ok = state.tool_registry.approval_store.reject(&id).await;
    Json(json!({"rejected": ok, "id": id}))
}
