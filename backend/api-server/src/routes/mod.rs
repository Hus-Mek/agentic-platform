pub mod agents;
pub mod runs;

use axum::Json;
use chrono::Utc;
use serde_json::json;
use std::time::Instant;

pub async fn health_check() -> Json<serde_json::Value> {
    Json(json!({
        "status": "ok",
        "service": "agenticos-api-server",
        "timestamp": Utc::now().to_rfc3339(),
        "uptime_seconds": {
            // Simple uptime tracking — reset on server restart
            "since": Instant::now().elapsed().as_secs()
        }
    }))
}
