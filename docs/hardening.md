# AgenticOS Platform — Production Hardening Guide

## Current Security State

| Area                | Status                      | Priority |
|---------------------|-----------------------------|----------|
| JWT auth middleware | ✅ Implemented              | —        |
| API key auth        | ✅ Implemented              | —        |
| Row-Level Security  | ✅ Implemented (workspaces) | —        |
| CORS                | ❌ Permissive (all origins) | High     |
| Rate limiting       | ❌ None on API layer        | High     |
| Secrets management  | ❌ Defaults in code         | High     |
| Input validation    | ⚠️ Partial                 | Medium   |
| Audit logging       | ⚠️ Partial                 | Medium   |
| HTTPS               | ❌ Not configured           | High     |

---

## 1. CORS — Restrict Allowed Origins

### Current (`backend/api-server/src/lib.rs:107`)

```rust
.layer(CorsLayer::permissive())
```

### Recommended

```rust
use std::env;
use tower_http::cors::{Any, CorsLayer};
use axum::http::{Method, header::{AUTHORIZATION, CONTENT_TYPE}};

let allowed_origins = env::var("ALLOWED_ORIGINS")
    .unwrap_or_else(|_| "http://localhost:3001".into());

let cors = CorsLayer::new()
    .allow_origins(
        allowed_origins
            .split(',')
            .map(|s| s.trim().parse().unwrap())
            .collect::<Vec<_>>(),
    )
    .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
    .allow_headers([AUTHORIZATION, CONTENT_TYPE]);

// Replace:
// .layer(CorsLayer::permissive())
// With:
// .layer(cors)
```

### Env Var

```
ALLOWED_ORIGINS=http://localhost:3001,https://app.agenticos.com
```

---

## 2. Rate Limiting — API Layer

### Current

No rate limiting on HTTP routes. (The `guards` crate has a tool-level rate limiter in `Bodyguard`, but it only applies within agent execution, not to API endpoints.)

### Recommended: Add `tower-governor`

Add to `backend/api-server/Cargo.toml`:

```toml
tower-governor = "0.5"
```

Then in `backend/api-server/src/lib.rs`:

```rust
use tower_governor::{GovernorLayer, governor::GovernorConfigBuilder, key_extractor::SmartKeyExtractor};

let governor_config = Box::new(
    GovernorConfigBuilder::default()
        .per_second(100)
        .burst_size(200)
        .key_extractor(SmartKeyExtractor)
        .finish()
        .unwrap(),
);

Router::new()
    .merge(public_routes)
    .merge(protected_routes)
    .layer(GovernorLayer { config: governor_config })
    .layer(TraceLayer::new_for_http())
    .layer(cors)
    .with_state(state)
```

### Alternative: Simple DashMap Rate Limiter

If you prefer not to add a dependency, implement directly using `DashMap` (already in `guards`):

```rust
use std::sync::Arc;
use dashmap::DashMap;
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct RateLimiter {
    buckets: Arc<DashMap<String, Vec<Instant>>>,
    max_requests: usize,
    window: Duration,
}

impl RateLimiter {
    pub fn new(max_requests: usize, window_secs: u64) -> Self {
        Self {
            buckets: Arc::new(DashMap::new()),
            max_requests,
            window: Duration::from_secs(window_secs),
        }
    }

    pub fn check(&self, key: &str) -> Result<(), StatusCode> {
        let mut entry = self.buckets.entry(key.to_string()).or_insert_with(Vec::new);
        let cutoff = Instant::now() - self.window;
        entry.retain(|&t| t > cutoff);
        if entry.len() >= self.max_requests {
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
        entry.push(Instant::now());
        Ok(())
    }
}
```

Apply as axum middleware using `from_fn_with_state`. Default: **100 requests/minute per API key**.

---

## 3. Secrets Rotation & Validation

### Current Risk

Default secrets in `backend/core/src/config.rs`:

```rust
jwt_secret: env::var("JWT_SECRET")
    .unwrap_or_else(|_| "dev-secret-change-in-production".into()),
```

### Rotating JWT_SECRET

1. Generate new secret:
   ```bash
   openssl rand -base64 32
   ```

2. Update `JWT_SECRET` in environment / `.env`

3. **All existing tokens are immediately invalidated.** Users must re-authenticate.

**Production best practice:** Use a secrets manager (HashiCorp Vault, AWS Secrets Manager, or Azure Key Vault) instead of env vars. Rotate every 90 days.

### Env Var Validation at Startup

Add to `backend/api-server/src/main.rs`:

```rust
fn validate_env() -> Result<(), Vec<String>> {
    let required = [
        "DATABASE_URL",
        "JWT_SECRET",
        "LITELLM_PROXY_URL",
        "QDRANT_URL",
    ];
    let mut errors = Vec::new();
    for var in required {
        if std::env::var(var).is_err() {
            errors.push(format!("{} is not set", var));
        }
    }
    if let Ok(secret) = std::env::var("JWT_SECRET") {
        if secret == "dev-secret-change-in-production" || secret.len() < 16 {
            errors.push("JWT_SECRET must be at least 16 characters and not the default value".into());
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}
```

Call at the start of `main()` before `AppConfig::from_env()`.

### Add `REQUIRED_ENV_VARS` constant

```rust
/// Required environment variables validated at startup.
const REQUIRED_ENV_VARS: &[&str] = &[
    "DATABASE_URL",
    "JWT_SECRET",
    "LITELLM_PROXY_URL",
    "QDRANT_URL",
];
```

---

## 4. Input Validation

### Request Body Size Limits

Add body size limit in `backend/api-server/src/lib.rs`:

```rust
use tower_http::validate_request::ValidateRequestHeaderLayer;
use axum::body::Bytes;

// Limit request bodies to 2MB
.layer(ValidateRequestHeaderLayer::accept("application/json"))
```

Using the `tower-http` `RequestBodyLimit` layer (already available via the `validate-request` feature in workspace deps):

```rust
use tower_http::limit::RequestBodyLimitLayer;

Router::new()
    // ...
    .layer(RequestBodyLimitLayer::new(2 * 1024 * 1024)) // 2MB
```

### UUID Parameter Validation

UUID params are already typed as `Path<Uuid>` in route handlers, which provides automatic validation. If the UUID format is invalid, axum returns a 422 or 400 automatically.

### String Length Sanitization

Add length checks in `middleware/auth.rs` or a shared validation module:

```rust
pub fn validate_string_length(value: &str, field: &str, max: usize) -> Result<(), StatusCode> {
    if value.len() > max {
        tracing::warn!("Input too long: {} ({} chars, max {})", field, value.len(), max);
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}
```

Recommended limits:

| Field                | Max Length |
|----------------------|------------|
| Agent name           | 255 chars  |
| Agent instructions   | 100_000    |
| Team name            | 255 chars  |
| Document filename    | 512 chars  |
| Document content     | 10 MB      |
| API key name         | 255 chars  |

For agent run input (`RunRequest.input`), add a cap:

```rust
if body.input.len() > 100_000 {
    return Err(bad_request("Input exceeds maximum length (100,000 characters)"));
}
```

---

## 5. Audit Logging

### Auth Failures

In `backend/api-server/src/middleware/auth.rs`, add `tracing::warn!` calls:

```rust
// Before returning UNAUTHORIZED in auth_middleware:
tracing::warn!(
    target = "agenticos.auth",
    "Authentication failed: no valid Bearer token provided"
);

// After JWT decode failure in the fallback branch:
tracing::warn!(
    target = "agenticos.auth",
    "JWT decode failed, attempting API key auth"
);

// After API key lookup failure:
tracing::warn!(
    target = "agenticos.auth",
    "API key authentication failed: no matching key found"
);
```

### Resource Mutations (UDM Events)

The system already has a `Unified Data Model` in `agenticos_core::udm` with `AgentEvent`, `EventType` (20+ variants), and `EventResult`. Ensure UDM events are emitted on all mutations:

| Mutation                    | EventType                          |
|-----------------------------|------------------------------------|
| Agent created               | `AgentCreated`                     |
| Agent updated               | `AgentUpdated`                     |
| Agent deleted               | `AgentDeleted`                     |
| Run started                 | `AgentRunStarted`                  |
| Run completed               | `AgentRunCompleted`                |
| Run failed                  | `AgentRunFailed`                   |
| Team created                | `TeamCreated` (add to EventType)   |
| Team run started            | `TeamRunStarted` (add to EventType)|
| Document uploaded           | `DocumentUploaded` (add to EventType)|
| Document deleted            | `DocumentDeleted` (add to EventType)|

### Add UDM event types to `core/src/udm.rs`:

```rust
pub enum EventType {
    // Existing...
    TeamCreated,
    TeamUpdated,
    TeamDeleted,
    TeamRunStarted,
    TeamRunCompleted,
    TeamRunFailed,
    DocumentUploaded,
    DocumentDeleted,
    KnowledgeBaseCreated,
    // ...
}
```

Then in each route handler, emit events:

```rust
AgentEvent::new(EventType::AgentCreated, workspace_id, run_id, agent_id, user_id)
    .with_result(EventResult::success())
    .emit();
```

---

## Production Checklist

### Pre-Launch

- [ ] CORS configured to restrict origins to specific domains
- [ ] Rate limiting enabled (100 req/min per key on API, stricter on auth)
- [ ] JWT_SECRET rotated from default, minimum 32 characters
- [ ] Env var validation passes at startup
- [ ] HTTPS termination configured (reverse proxy: Nginx, Caddy, or cloud LB)
- [ ] Database connection pooling tuned (max connections)
- [ ] Log level set to `info` or `warn` in production (not `debug`)
- [ ] Monitoring/alerting configured (LangFuse or Datadog/Grafana)
- [ ] Backup strategy for PostgreSQL + Qdrant

### Post-Launch

- [ ] Secrets rotation every 90 days
- [ ] Penetration testing on auth endpoints
- [ ] Review rate limit thresholds based on usage patterns
- [ ] Enable SQL audit logging for all mutation queries
- [ ] Add admin audit dashboard for viewing auth failures and mutations
