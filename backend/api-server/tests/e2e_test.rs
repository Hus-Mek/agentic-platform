#[tokio::test]
async fn test_health_endpoint() {
    let _ = dotenvy::dotenv();
    let config = agenticos_core::config::AppConfig::from_env();
    let db_url = match std::env::var("DATABASE_URL") {
        Ok(u) => u,
        Err(_) => {
            eprintln!("Skipping E2E test — DATABASE_URL not set");
            return;
        }
    };
    let pool = sqlx::PgPool::connect(&db_url).await.expect("Failed to connect to DB");
    sqlx::migrate!("./infra/migrations").run(&pool).await.ok();
    let state = agenticos_api_server::AppState::new(config, pool).await;
    let app = agenticos_api_server::create_router(state);
    let req = axum::http::Request::builder()
        .uri("/health")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = tower::ServiceExt::ready(&mut app.into_service())
        .await
        .unwrap()
        .call(req)
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}
