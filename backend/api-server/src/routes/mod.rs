pub mod agents;
pub mod auth;
pub mod kb;
pub mod runs;
pub mod teams;
pub mod workspaces;
pub mod features;
pub mod approval;

use axum::Json;
use chrono::Utc;
use serde_json::json;
use std::sync::OnceLock;
use std::time::Instant;

static SERVER_START: OnceLock<Instant> = OnceLock::new();

pub async fn health_check() -> Json<serde_json::Value> {
    let start = *SERVER_START.get_or_init(Instant::now);
    Json(json!({
        "status": "ok",
        "service": "agenticos-api-server",
        "timestamp": Utc::now().to_rfc3339(),
        "uptime_seconds": {
            "since": start.elapsed().as_secs()
        }
    }))
}
