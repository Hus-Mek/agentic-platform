use axum::extract::{Path, State};
use axum::Json;
use serde_json::json;

use crate::AppState;

pub async fn list_features(State(state): State<AppState>) -> Json<serde_json::Value> {
    let flags = state.feature_flags.all_flags();
    Json(json!(flags))
}

pub async fn enable_feature(
    State(state): State<AppState>,
    Path(flag): Path<String>,
) -> Json<serde_json::Value> {
    state.feature_flags.enable(&flag);
    Json(json!({"flag": flag, "enabled": true}))
}

pub async fn disable_feature(
    State(state): State<AppState>,
    Path(flag): Path<String>,
) -> Json<serde_json::Value> {
    state.feature_flags.disable(&flag);
    Json(json!({"flag": flag, "enabled": false}))
}
