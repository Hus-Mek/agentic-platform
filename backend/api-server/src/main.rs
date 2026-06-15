use anyhow::Result;
use agenticos_api_server::db::{create_db_pool, run_migrations};
use agenticos_api_server::{create_router, AppState};
use agenticos_core::config::AppConfig;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "agenticos=debug,tower_http=debug".into()),
        )
        .json()
        .init();

    let config = AppConfig::from_env();
    let pool = create_db_pool(&config.database_url).await;
    run_migrations(&pool).await;

    let state = AppState::new(config.clone(), pool).await;

    let app = create_router(state);
    let addr = format!("{}:{}", config.host, config.port);
    tracing::info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
    Ok(())
}
