pub mod db;
pub mod llm;
pub mod middleware;
pub mod models;
pub mod routes;

use std::sync::Arc;

use axum::middleware::from_fn_with_state;
use axum::routing::{get, post};
use axum::Router;
use sqlx::PgPool;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use agenticos_agent_runtime::tool::ToolRegistry;
use agenticos_core::config::AppConfig;
use agenticos_guards::GuardEngine;
use agenticos_memory::MemoryClient;

use agenticos_core::llm::LiteLLMClient;

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub db: PgPool,
    pub llm_client: LiteLLMClient,
    pub tool_registry: Arc<ToolRegistry>,
    pub guard_engine: Arc<GuardEngine>,
    pub memory_client: Arc<MemoryClient>,
}

impl AppState {
    pub async fn new(config: AppConfig, db: PgPool) -> Self {
        let llm_client = LiteLLMClient::new(&config);
        let tool_registry = Arc::new(ToolRegistry::new());
        let guard_engine = Arc::new(GuardEngine::new());
        let memory_client = Arc::new(MemoryClient::from_config(&config).await);
        Self {
            config,
            db,
            llm_client,
            tool_registry,
            guard_engine,
            memory_client,
        }
    }
}

pub fn create_router(state: AppState) -> Router {
    let public_routes = Router::new().route("/health", get(routes::health_check));

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
        .route("/api/runs", get(routes::runs::list_runs))
        .route(
            "/api/runs/{id}",
            get(routes::runs::get_run).delete(routes::runs::delete_run),
        )
        .route("/api/runs/{id}/steps", get(routes::runs::get_run_steps))
        .route("/api/runs/{id}/cancel", post(routes::runs::cancel_run))
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
