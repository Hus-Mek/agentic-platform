use axum::{Json, Extension};
use axum::extract::State;
use serde::{Deserialize, Serialize};
use agenticos_core::model_router::{TaskComplexity, ModelCapability};

use crate::middleware::auth::Claims;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct RouteRequest {
    pub input: String,
    pub required_capabilities: Vec<String>,
    pub has_tools: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct RouteResponse {
    pub model_id: String,
    pub estimated_cost_input: f64,
    pub estimated_cost_output: f64,
    pub complexity: String,
}

pub async fn list_models(
    State(_state): State<AppState>,
    Extension(_claims): Extension<Claims>,
) -> Json<Vec<serde_json::Value>> {
    Json(vec![
        serde_json::json!({"id": "openai/gpt-oss-120b:free", "provider": "openai", "capabilities": ["chat", "function_calling", "reasoning", "structured_output", "long_context"], "tier": "free"}),
        serde_json::json!({"id": "meta-llama/llama-3.3-70b-instruct:free", "provider": "meta", "capabilities": ["chat", "function_calling", "long_context", "code_generation"], "tier": "free"}),
        serde_json::json!({"id": "qwen/qwen3-coder:free", "provider": "qwen", "capabilities": ["chat", "function_calling", "code_generation", "long_context"], "tier": "free"}),
        serde_json::json!({"id": "google/gemma-4-31b-it:free", "provider": "google", "capabilities": ["chat", "function_calling", "vision", "long_context", "structured_output"], "tier": "free"}),
        serde_json::json!({"id": "nvidia/nemotron-3-super-120b-a12b:free", "provider": "nvidia", "capabilities": ["chat", "function_calling", "reasoning", "long_context"], "tier": "free"}),
        serde_json::json!({"id": "cohere/north-mini-code:free", "provider": "cohere", "capabilities": ["chat", "function_calling", "code_generation", "fast_inference"], "tier": "free"}),
        serde_json::json!({"id": "openrouter/owl-alpha", "provider": "openrouter", "capabilities": ["chat", "function_calling", "auto_routing"], "tier": "free"}),
    ])
}

pub async fn route_model(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Json(body): Json<RouteRequest>,
) -> Result<Json<RouteResponse>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let complexity = if body.input.len() < 500 && !body.has_tools.unwrap_or(false) {
        TaskComplexity::Simple
    } else if body.input.len() < 4000 && !body.has_tools.unwrap_or(false) {
        TaskComplexity::Medium
    } else {
        TaskComplexity::Complex
    };

    let capabilities: Vec<ModelCapability> = body.required_capabilities
        .iter()
        .filter_map(|c| match c.to_lowercase().as_str() {
            "chat" => Some(ModelCapability::Chat),
            "function_calling" | "function-calling" | "tool" => Some(ModelCapability::FunctionCalling),
            "vision" => Some(ModelCapability::Vision),
            "long_context" | "long-context" | "context" => Some(ModelCapability::LongContext),
            "structured_output" | "structured-output" => Some(ModelCapability::StructuredOutput),
            "code" | "code_generation" | "code-generation" => Some(ModelCapability::CodeGeneration),
            "reasoning" => Some(ModelCapability::Reasoning),
            "fast" | "fast_inference" | "fast-inference" => Some(ModelCapability::FastInference),
            "cheap" => Some(ModelCapability::Cheap),
            _ => None,
        })
        .collect();

    let router = state.model_router.read().await;
    let model_id = router.select_model(complexity, &capabilities, None);

    let (cost_in, cost_out) = router
        .get_model_info(&model_id)
        .map(|m| (m.cost_input_per_million, m.cost_output_per_million))
        .unwrap_or((0.0, 0.0));

    Ok(Json(RouteResponse {
        model_id,
        estimated_cost_input: cost_in,
        estimated_cost_output: cost_out,
        complexity: format!("{:?}", complexity).to_lowercase(),
    }))
}