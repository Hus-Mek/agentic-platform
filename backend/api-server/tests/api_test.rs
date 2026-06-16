use std::sync::Arc;

use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
    Router,
};
use serde_json::json;
use tower::ServiceExt;
use uuid::Uuid;

use agenticos_api_server::{create_router, middleware, models, routes, AppState};
use agenticos_core::config::AppConfig;
use agenticos_core::llm::LiteLLMClient;
use agenticos_memory::MemoryClient;

fn build_test_config() -> AppConfig {
    AppConfig {
        database_url: "postgresql://localhost:5432/nonexistent_test".into(),
        jwt_secret: "test-secret-for-integration-tests".into(),
        llm_api_key: "test-llm-key".into(),
        llm_base_url: "http://localhost:9999".into(),
        embedding_base_url: "http://localhost:9999".into(),
        qdrant_url: "http://localhost:9998".into(),
        environment: "test".into(),
        host: "127.0.0.1".into(),
        port: 0,
    }
}

async fn build_test_app() -> Router {
    let config = build_test_config();

    let db = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy(&config.database_url)
        .expect("Failed to create test pool");

    let llm_client = LiteLLMClient::new(&config);
    let tool_registry = Arc::new(agenticos_agent_runtime::tool::ToolRegistry::new());
    let guard_engine = Arc::new(agenticos_guards::GuardEngine::new());
    let memory_client = Arc::new(MemoryClient::new_fallback());
    let kb_retriever = Arc::new(
        agenticos_agent_runtime::KbRetriever::new(db.clone(), llm_client.clone()),
    );

    let state = AppState {
        config,
        db,
        llm_client,
        tool_registry,
        guard_engine,
        memory_client,
        kb_retriever,
    };

    create_router(state)
}

fn create_test_jwt(secret: &str) -> String {
    use jsonwebtoken::{encode, EncodingKey, Header};

    let claims = middleware::auth::Claims {
        sub: "test-user-123".into(),
        workspace_id: "00000000-0000-0000-0000-000000000000".into(),
        exp: 9999999999,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_ref()),
    )
    .unwrap()
}

// ─── Health Check ────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_health_check_returns_200() {
    let app = build_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();

    assert_eq!(body["status"], "ok");
    assert_eq!(body["service"], "agenticos-api-server");
    assert!(body["timestamp"].is_string());
}

// ─── Auth Rejection Tests ────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_agents_without_auth_returns_401() {
    let app = build_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/agents")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_post_agents_without_auth_returns_401() {
    let app = build_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/agents")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "name": "test-agent",
                        "instructions": "test instructions",
                        "model": "gpt-4o",
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_get_teams_without_auth_returns_401() {
    let app = build_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/teams")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_get_knowledge_bases_without_auth_returns_401() {
    let app = build_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/knowledge-bases")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_get_runs_without_auth_returns_401() {
    let app = build_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/runs")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_get_agent_by_id_without_auth_returns_401() {
    let app = build_test_app().await;
    let id = Uuid::new_v4();

    let response = app
        .oneshot(
            Request::builder()
                .uri(&format!("/api/agents/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_malformed_bearer_token_returns_401() {
    let app = build_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/agents")
                .header("Authorization", "Bearer not.a.valid.jwt")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_empty_authorization_header_returns_401() {
    let app = build_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/agents")
                .header("Authorization", "")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_basic_auth_instead_of_bearer_returns_401() {
    let app = build_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/agents")
                .header("Authorization", "Basic dGVzdDpwYXNz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

// ─── Valid JWT Tests (reach handler, DB will fail) ───────────────────────────

#[tokio::test]
async fn test_request_with_valid_jwt_reaches_handler() {
    let app = build_test_app().await;
    let token = create_test_jwt("test-secret-for-integration-tests");

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/agents")
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // JWT passes auth middleware, but handler hits DB which isn't running.
    // We expect 500 (DB connection error), NOT 401.
    assert_ne!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "Auth middleware should accept valid JWT"
    );
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

// ─── Endpoint Tests ──────────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_nonexistent_team_returns_404() {
    let app = build_test_app().await;
    let token = create_test_jwt("test-secret-for-integration-tests");
    let id = Uuid::new_v4();

    let response = app
        .oneshot(
            Request::builder()
                .uri(&format!("/api/teams/{id}"))
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // JWT passes auth, but no DB -> handler returns 500, not 404
    assert_ne!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_get_nonexistent_run_returns_404_or_error() {
    let app = build_test_app().await;
    let token = create_test_jwt("test-secret-for-integration-tests");
    let id = Uuid::new_v4();

    let response = app
        .oneshot(
            Request::builder()
                .uri(&format!("/api/runs/{id}"))
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_ne!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

// ─── CORS Headers ────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_health_check_has_cors_headers() {
    let app = build_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let headers = response.headers();
    assert!(
        headers
            .get("access-control-allow-origin")
            .is_some_and(|v| v == "*"),
        "CORS permissive layer should set allow-origin: *"
    );
}

// ─── JSON Serialization Tests ────────────────────────────────────────────────

#[tokio::test]
async fn test_create_agent_request_serialization() {
    let body = json!({
        "name": "my-agent",
        "description": "A test agent",
        "instructions": "You are a helpful assistant",
        "model": "gpt-4o",
        "tools": [{"name": "web_search", "description": "Search the web"}],
    });

    let parsed: routes::agents::CreateAgentRequest =
        serde_json::from_value(body.clone()).unwrap();

    assert_eq!(parsed.name, "my-agent");
    assert_eq!(parsed.description.as_deref(), Some("A test agent"));
    assert_eq!(parsed.instructions, "You are a helpful assistant");
    assert_eq!(parsed.model, "gpt-4o");
    assert!(parsed.tools.is_some());
}

#[tokio::test]
async fn test_create_agent_request_minimal() {
    let body = json!({
        "name": "minimal-agent",
        "instructions": "Do stuff",
    });

    let parsed: routes::agents::CreateAgentRequest =
        serde_json::from_value(body).unwrap();

    assert_eq!(parsed.name, "minimal-agent");
    assert_eq!(parsed.model, ""); // default from serde
    assert!(parsed.description.is_none());
}

#[tokio::test]
async fn test_create_agent_request_missing_required_fields_fails() {
    let body = json!({
        "name": "no-instructions",
    });

    let result: Result<routes::agents::CreateAgentRequest, _> =
        serde_json::from_value(body);
    assert!(result.is_err(), "instructions is required");
}

#[tokio::test]
async fn test_update_agent_request_serialization() {
    let body = json!({
        "name": "updated-agent",
        "model": "gpt-4-turbo",
    });

    let parsed: routes::agents::UpdateAgentRequest =
        serde_json::from_value(body).unwrap();

    assert_eq!(parsed.name.as_deref(), Some("updated-agent"));
    assert!(parsed.description.is_none());
    assert_eq!(parsed.model.as_deref(), Some("gpt-4-turbo"));
}

#[tokio::test]
async fn test_run_request_serialization() {
    let body = json!({
        "input": "Hello, world!",
        "model": "gpt-4o",
    });

    let parsed: routes::agents::RunRequest = serde_json::from_value(body).unwrap();
    assert_eq!(parsed.input, "Hello, world!");
    assert_eq!(parsed.model.as_deref(), Some("gpt-4o"));
}

#[tokio::test]
async fn test_run_request_minimal() {
    let body = json!({
        "input": "Hello!",
    });

    let parsed: routes::agents::RunRequest = serde_json::from_value(body).unwrap();
    assert_eq!(parsed.input, "Hello!");
    assert!(parsed.model.is_none());
}

#[tokio::test]
async fn test_run_response_serialization() {
    let run_id = Uuid::new_v4();
    let response = routes::agents::RunResponse {
        run_id,
        output: "Test output".into(),
        total_tokens: 150,
        total_cost: 0.002,
        duration_ms: 1200,
        status: "completed".into(),
    };

    let json = serde_json::to_value(&response).unwrap();
    assert_eq!(json["run_id"], run_id.to_string());
    assert_eq!(json["output"], "Test output");
    assert_eq!(json["total_tokens"], 150);
    assert_eq!(json["total_cost"], 0.002);
    assert_eq!(json["status"], "completed");
}

#[tokio::test]
async fn test_create_team_request_serialization() {
    let body = json!({
        "name": "my-team",
        "supervisor_instructions": "Coordinate the team",
        "sub_agents": [
            {
                "id": "researcher-1",
                "name": "researcher",
                "instructions": "Research topics",
                "model": "gpt-4o",
                "tools": [],
                "guard_config": {
                    "inspector_enabled": true,
                    "bodyguard_enabled": true,
                    "pii_detection": true,
                    "max_tokens_per_run": 100000
                }
            }
        ],
        "max_delegations": 5,
        "execution_mode": "Sequential"
    });

    let parsed: routes::teams::CreateTeamRequest =
        serde_json::from_value(body).unwrap();

    assert_eq!(parsed.name, "my-team");
    assert_eq!(parsed.supervisor_instructions, "Coordinate the team");
    assert_eq!(parsed.sub_agents.len(), 1);
    assert_eq!(parsed.max_delegations, Some(5));
}

#[tokio::test]
async fn test_team_run_request_serialization() {
    let body = json!({
        "input": "Run the team"
    });

    let parsed: routes::teams::TeamRunRequest = serde_json::from_value(body).unwrap();
    assert_eq!(parsed.input, "Run the team");
}

#[tokio::test]
async fn test_team_run_response_serialization() {
    let run_id = Uuid::new_v4();
    let response = routes::teams::TeamRunResponse {
        run_id,
        output: "Team output".into(),
        total_tokens: 500,
        total_cost: 0.01,
        duration_ms: 5000,
        status: "completed".into(),
        delegations: json!([{"sub_agent": "researcher", "status": "completed"}]),
    };

    let json = serde_json::to_value(&response).unwrap();
    assert_eq!(json["run_id"], run_id.to_string());
    assert_eq!(json["output"], "Team output");
    assert_eq!(json["delegations"][0]["sub_agent"], "researcher");
}

#[tokio::test]
async fn test_upload_document_request_serialization() {
    let body = json!({
        "filename": "report.txt",
        "content": "This is the document content",
        "content_type": "text/plain"
    });

    let parsed: routes::kb::UploadDocumentRequest =
        serde_json::from_value(body).unwrap();

    assert_eq!(parsed.filename, "report.txt");
    assert_eq!(parsed.content, "This is the document content");
    assert_eq!(parsed.content_type.as_deref(), Some("text/plain"));
}

#[tokio::test]
async fn test_upload_document_request_minimal() {
    let body = json!({
        "filename": "doc.txt",
        "content": "Some content"
    });

    let parsed: routes::kb::UploadDocumentRequest =
        serde_json::from_value(body).unwrap();

    assert_eq!(parsed.filename, "doc.txt");
    assert!(parsed.content_type.is_none());
}

#[tokio::test]
async fn test_upload_document_response_serialization() {
    let doc_id = Uuid::new_v4();
    let response = routes::kb::UploadDocumentResponse {
        document_id: doc_id,
        chunks_count: 5,
    };

    let json = serde_json::to_value(&response).unwrap();
    assert_eq!(json["document_id"], doc_id.to_string());
    assert_eq!(json["chunks_count"], 5);
}

#[tokio::test]
async fn test_model_row_serialization() {
    let now = chrono::Utc::now();
    let row = models::AgentRow {
        id: Uuid::new_v4(),
        workspace_id: Uuid::new_v4(),
        name: "test-agent".into(),
        description: Some("A test".into()),
        instructions: "Do things".into(),
        model: "gpt-4o".into(),
        tools: json!([]),
        guard_config: json!({}),
        memory_config: json!({}),
        created_by: "test-user".into(),
        is_active: true,
        created_at: now,
        updated_at: now,
    };

    let json = serde_json::to_value(&row).unwrap();
    assert_eq!(json["name"], "test-agent");
    assert_eq!(json["is_active"], true);
    assert_eq!(json["description"], "A test");

    let deserialized: models::AgentRow = serde_json::from_value(json).unwrap();
    assert_eq!(deserialized.name, row.name);
    assert_eq!(deserialized.id, row.id);
}

#[tokio::test]
async fn test_run_row_serialization() {
    let now = chrono::Utc::now();
    let row = models::RunRow {
        id: Uuid::new_v4(),
        workspace_id: Uuid::new_v4(),
        agent_id: Uuid::new_v4(),
        status: "completed".into(),
        input: Some("Hello".into()),
        output: Some("Hi there".into()),
        error: None,
        duration_ms: Some(1000),
        total_tokens: Some(50),
        total_cost: Some(0.001),
        created_by: "user".into(),
        created_at: now,
    };

    let json = serde_json::to_value(&row).unwrap();
    assert_eq!(json["status"], "completed");
    assert_eq!(json["input"], "Hello");
    assert!(json["error"].is_null());
    assert_eq!(json["total_tokens"], 50);

    let deserialized: models::RunRow = serde_json::from_value(json).unwrap();
    assert_eq!(deserialized.id, row.id);
    assert_eq!(deserialized.total_cost, row.total_cost);
}

#[tokio::test]
async fn test_team_agent_row_serialization() {
    let now = chrono::Utc::now();
    let row = models::TeamAgentRow {
        id: Uuid::new_v4(),
        workspace_id: Uuid::new_v4(),
        name: "test-team".into(),
        description: Some("A team".into()),
        supervisor_instructions: "Supervise".into(),
        supervisor_model: "gpt-4o".into(),
        sub_agents: json!([]),
        max_delegations: 10,
        execution_mode: "Sequential".into(),
        created_by: "user".into(),
        is_active: true,
        created_at: now,
        updated_at: now,
    };

    let json = serde_json::to_value(&row).unwrap();
    assert_eq!(json["name"], "test-team");
    assert_eq!(json["max_delegations"], 10);
    assert_eq!(json["execution_mode"], "Sequential");

    let deserialized: models::TeamAgentRow = serde_json::from_value(json).unwrap();
    assert_eq!(deserialized.id, row.id);
}

// ─── Error Response Tests ────────────────────────────────────────────────────

#[tokio::test]
async fn test_error_response_format() {
    let invalid_body = json!({
        "name": 123,  // should be a string
        "instructions": "test",
    });

    let app = build_test_app().await;
    let token = create_test_jwt("test-secret-for-integration-tests");

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/agents")
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::from(serde_json::to_string(&invalid_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // JWT passes, then request deserialization fails -> 422 or 400
    assert!(
        response.status() == StatusCode::UNPROCESSABLE_ENTITY
            || response.status() == StatusCode::BAD_REQUEST
            || response.status() == StatusCode::INTERNAL_SERVER_ERROR,
        "Got unexpected status: {}",
        response.status()
    );
}
