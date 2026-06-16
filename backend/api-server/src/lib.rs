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

#[derive(Clone)]
pub struct AppState {
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
        Self {
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
