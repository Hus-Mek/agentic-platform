pub mod db;
pub mod middleware;
pub mod models;
pub mod routes;

use std::sync::Arc;

use axum::middleware::from_fn_with_state;
use axum::routing::{delete, get, post};
use axum::Router;
use sqlx::PgPool;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use agenticos_agent_runtime::rag::KbRetriever;
use agenticos_agent_runtime::tool::ToolRegistry;
use agenticos_core::config::AppConfig;
use agenticos_core::features::FeatureFlags;
use agenticos_guards::GuardEngine;
use agenticos_memory::MemoryClient;

use agenticos_core::llm::LiteLLMClient;
use agenticos_core::{ModelRouter, EvalHarness, model_router::ModelInfo};
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AppState {
    // Added ModelRouter and EvalHarness for dynamic model selection and evaluation
    pub model_router: Arc<RwLock<ModelRouter>>, 
    pub eval_harness: Arc<EvalHarness>,
    pub config: AppConfig,
    pub db: PgPool,
    pub llm_client: LiteLLMClient,
    pub tool_registry: Arc<ToolRegistry>,
    pub guard_engine: Arc<GuardEngine>,
    pub memory_client: Arc<MemoryClient>,
    pub kb_retriever: Arc<KbRetriever>,
    pub feature_flags: Arc<FeatureFlags>,
}

impl AppState {
    pub async fn new(config: AppConfig, db: PgPool) -> Self {
        let llm_client = LiteLLMClient::new(&config);
        let tool_registry = Arc::new(ToolRegistry::new());
        let guard_engine = Arc::new(GuardEngine::new());
        let memory_client = Arc::new(MemoryClient::from_config(&config).await);
        let kb_retriever = Arc::new(KbRetriever::new(db.clone(), llm_client.clone()));
        let feature_flags = Arc::new(FeatureFlags::new());

        // Initialize ModelRouter with default model and register catalog
        let mut model_router = ModelRouter::new("gpt-4o-mini".into());
        for model in default_model_catalog() {
            model_router.register_model(model);
        }
        let model_router = Arc::new(RwLock::new(model_router));

        // Initialize EvalHarness
        let eval_harness = Arc::new(EvalHarness::new());

        Self {
            model_router,
            eval_harness,
            config,
            db,
            llm_client,
            tool_registry,
            guard_engine,
            memory_client,
            kb_retriever,
            feature_flags,
        }
    }
}

// Helper to provide a catalog of default models used by the ModelRouter.
fn default_model_catalog() -> Vec<ModelInfo> {
    use agenticos_core::model_router::{ModelInfo, ModelCapability};
    // Live free OpenRouter slugs (verified via /api/v1/models, prompt+completion="0")
    vec![
        ModelInfo {
            id: "openai/gpt-oss-120b:free".into(),
            provider: "openai".into(),
            capabilities: vec![
                ModelCapability::Chat,
                ModelCapability::FunctionCalling,
                ModelCapability::Reasoning,
                ModelCapability::StructuredOutput,
                ModelCapability::LongContext,
            ],
            cost_input_per_million: 0.0,
            cost_output_per_million: 0.0,
            max_context_tokens: 131_072,
            avg_latency_ms: 900,
        },
        ModelInfo {
            id: "meta-llama/llama-3.3-70b-instruct:free".into(),
            provider: "meta".into(),
            capabilities: vec![
                ModelCapability::Chat,
                ModelCapability::FunctionCalling,
                ModelCapability::LongContext,
                ModelCapability::CodeGeneration,
            ],
            cost_input_per_million: 0.0,
            cost_output_per_million: 0.0,
            max_context_tokens: 131_072,
            avg_latency_ms: 900,
        },
        ModelInfo {
            id: "qwen/qwen3-coder:free".into(),
            provider: "qwen".into(),
            capabilities: vec![
                ModelCapability::Chat,
                ModelCapability::FunctionCalling,
                ModelCapability::CodeGeneration,
                ModelCapability::LongContext,
            ],
            cost_input_per_million: 0.0,
            cost_output_per_million: 0.0,
            max_context_tokens: 1_048_576,
            avg_latency_ms: 700,
        },
        ModelInfo {
            id: "google/gemma-4-31b-it:free".into(),
            provider: "google".into(),
            capabilities: vec![
                ModelCapability::Chat,
                ModelCapability::FunctionCalling,
                ModelCapability::Vision,
                ModelCapability::LongContext,
                ModelCapability::StructuredOutput,
            ],
            cost_input_per_million: 0.0,
            cost_output_per_million: 0.0,
            max_context_tokens: 262_144,
            avg_latency_ms: 600,
        },
        ModelInfo {
            id: "nvidia/nemotron-3-super-120b-a12b:free".into(),
            provider: "nvidia".into(),
            capabilities: vec![
                ModelCapability::Chat,
                ModelCapability::FunctionCalling,
                ModelCapability::Reasoning,
                ModelCapability::LongContext,
            ],
            cost_input_per_million: 0.0,
            cost_output_per_million: 0.0,
            max_context_tokens: 1_000_000,
            avg_latency_ms: 1100,
        },
        ModelInfo {
            id: "openrouter/owl-alpha".into(),
            provider: "openrouter".into(),
            capabilities: vec![
                ModelCapability::Chat,
                ModelCapability::FunctionCalling,
                ModelCapability::Cheap,
            ],
            cost_input_per_million: 0.0,
            cost_output_per_million: 0.0,
            max_context_tokens: 1_048_576,
            avg_latency_ms: 1200,
        },
    ]
}

pub fn create_router(state: AppState) -> Router {
    let public_routes = Router::new()
        .route("/health", get(routes::health_check))
        .route("/api/auth/register", post(routes::auth::register))
        .route("/api/auth/login", post(routes::auth::login));

    let protected_routes = Router::new()
        .route("/api/agents", get(routes::agents::list_agents).post(routes::agents::create_agent))
        .route(
            "/api/agents/{id}",
            get(routes::agents::get_agent)
                .put(routes::agents::update_agent)
                .delete(routes::agents::delete_agent),
        )
        .route("/api/agents/{id}/run", post(routes::agents::run_agent))
        .route("/api/agents/{id}/run-stream", post(routes::agents::run_agent_stream))
        .route("/api/agents/{id}/versions", get(routes::agents::list_versions))
        .route("/api/agents/{id}/deploy", post(routes::agents::deploy_agent))
        .route("/api/agents/{id}/rollback/{version}", post(routes::agents::rollback_agent))
        .route("/api/runs", get(routes::runs::list_runs))
        .route(
            "/api/runs/{id}",
            get(routes::runs::get_run).delete(routes::runs::delete_run),
        )
        .route("/api/teams", get(routes::teams::list_teams).post(routes::teams::create_team))
        .route(
            "/api/teams/{id}",
            get(routes::teams::get_team)
                .put(routes::teams::update_team)
                .delete(routes::teams::delete_team),
        )
        .route("/api/teams/{id}/run", post(routes::teams::run_team))
        .route("/api/teams/{id}/runs", get(routes::teams::list_team_runs))
        .route("/api/team-runs/{id}", get(routes::teams::get_team_run))
        .route("/api/runs/{id}/steps", get(routes::runs::get_run_steps))
        .route("/api/runs/{id}/cancel", post(routes::runs::cancel_run))
        .route(
            "/api/pipelines",
            get(routes::pipelines::list_pipelines).post(routes::pipelines::create_pipeline),
        )
        .route(
            "/api/pipelines/{id}",
            get(routes::pipelines::get_pipeline)
                .put(routes::pipelines::update_pipeline)
                .delete(routes::pipelines::delete_pipeline),
        )
        .route("/api/pipelines/{id}/validate", post(routes::pipelines::validate_pipeline_handler))
        .route("/api/pipelines/{id}/run", post(routes::pipelines::run_pipeline))
        .route("/api/pipelines/{id}/run-stream", post(routes::pipelines::run_pipeline_stream))
        .route("/api/pipelines/{id}/runs", get(routes::pipelines::list_pipeline_runs))
        .route("/api/pipelines/{id}/versions", get(routes::pipelines::list_pipeline_versions))
        .route("/api/pipelines/{id}/deploy", post(routes::pipelines::deploy_pipeline))
        .route("/api/pipelines/{id}/rollback/{version}", post(routes::pipelines::rollback_pipeline))
        .route("/api/pipeline-runs/{id}", get(routes::pipelines::get_pipeline_run))
        .route("/api/pipeline-runs/{id}/steps", get(routes::pipelines::get_pipeline_run_steps))
        .route("/api/pipeline-runs/{id}/cancel", post(routes::pipelines::cancel_pipeline_run))
        .route("/api/workspaces", get(routes::workspaces::list_workspaces).post(routes::workspaces::create_workspace))
        .route(
            "/api/workspaces/{id}",
            get(routes::workspaces::get_workspace)
                .put(routes::workspaces::update_workspace)
                .delete(routes::workspaces::delete_workspace),
        )
        .route("/api/workspaces/{id}/members", get(routes::workspaces::list_members).post(routes::workspaces::add_member))
        .route("/api/workspaces/{id}/members/{user_id}", delete(routes::workspaces::remove_member))
        .route("/api/auth/me", get(routes::auth::me))
        .route("/api/knowledge-bases", get(routes::kb::list_knowledge_bases))
        .route(
            "/api/knowledge-bases/{id}",
            get(routes::kb::get_knowledge_base),
        )
        .route(
            "/api/knowledge-bases/{id}/documents",
            get(routes::kb::list_documents).post(routes::kb::upload_document),
        )
        .route(
            "/api/knowledge-bases/{id}/documents/{doc_id}",
            delete(routes::kb::delete_document),
        )
        .route("/api/features", get(routes::features::list_features))
        .route("/api/features/{flag}/enable", post(routes::features::enable_feature))
        .route("/api/features/{flag}/disable", post(routes::features::disable_feature))
        .route("/api/approvals", get(routes::approval::list_approvals))
                .route("/api/approvals/{id}/approve", post(routes::approval::approve_request))
                .route("/api/approvals/{id}/reject", post(routes::approval::reject_request))
        .route("/api/models", get(routes::models::list_models))
                .route("/api/models/route", post(routes::models::route_model))
                .route("/api/eval/metrics", get(routes::eval::list_metrics))
                .route("/api/eval/score", post(routes::eval::score))
                .route("/api/eval/benchmark", post(routes::eval::run_benchmark))
        .route("/api/workspaces/{id}/usage", get(routes::usage::get_usage))
        .route("/api/workspaces/{id}/usage/summary", get(routes::usage::get_usage_summary))
        .layer(from_fn_with_state(
            state.clone(),
            middleware::auth::auth_middleware,
        ));

    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}
