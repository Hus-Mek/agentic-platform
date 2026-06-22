//! Evaluation routes — thin HTTP surface over `agenticos_core::metrics::EvalHarness`.
//!
//! NOTE: `lib.rs` referenced `routes::eval::{list_metrics, score, run_benchmark}`
//! but the module file was missing from the tree (the api-server crate did not
//! compile). This restores a faithful minimal implementation backed by the
//! existing `state.eval_harness`.

use axum::extract::State;
use axum::{Extension, Json};
use serde::Deserialize;

use agenticos_core::metrics::{BenchmarkSuite, EvalResult, MetricScore};

use crate::middleware::auth::Claims;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct ScoreRequest {
    pub hypothesis: String,
    pub reference: String,
    #[serde(default)]
    pub metrics: Option<Vec<String>>,
}

/// List the metrics registered on the harness.
pub async fn list_metrics(
    State(_state): State<AppState>,
    Extension(_claims): Extension<Claims>,
) -> Json<Vec<serde_json::Value>> {
    Json(vec![
        serde_json::json!({ "name": "bleu", "description": "BLEU-1 unigram precision with brevity penalty" }),
        serde_json::json!({ "name": "chrf", "description": "Character n-gram F-score" }),
        serde_json::json!({ "name": "exact_match", "description": "Exact string match (1.0 / 0.0)" }),
        serde_json::json!({ "name": "contains_match", "description": "Reference substring containment" }),
    ])
}

/// Score a hypothesis against a reference with the requested metrics.
pub async fn score(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Json(body): Json<ScoreRequest>,
) -> Json<Vec<MetricScore>> {
    let metrics = body.metrics.unwrap_or_else(|| {
        vec![
            "bleu".to_string(),
            "chrf".to_string(),
            "exact_match".to_string(),
            "contains_match".to_string(),
        ]
    });
    Json(state.eval_harness.score(&body.hypothesis, &body.reference, &metrics))
}

/// Run the standard benchmark suite and return per-task results.
pub async fn run_benchmark(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
) -> Json<Vec<EvalResult>> {
    Json(state.eval_harness.run_suite(&BenchmarkSuite::standard()))
}
