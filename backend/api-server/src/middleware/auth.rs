use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use serde::{Deserialize, Serialize};

use crate::models::ApiKeyRow;
use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub workspace_id: String,
    pub role: String,
    pub exp: usize,
}

pub async fn auth_middleware(
    State(state): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let token = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Try JWT decode first
    if let Ok(claims) = decode_jwt(token, &state.config.jwt_secret) {
        req.extensions_mut().insert(claims);
        return Ok(next.run(req).await);
    }

    // Fallback: try API key auth
    let api_key = token.to_string();
    let claims = authenticate_api_key(&state, &api_key)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    req.extensions_mut().insert(claims);
    Ok(next.run(req).await)
}

fn decode_jwt(token: &str, secret: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let decoding_key = jsonwebtoken::DecodingKey::from_secret(secret.as_bytes());
    let validation = jsonwebtoken::Validation::default();
    let token_data = jsonwebtoken::decode::<Claims>(token, &decoding_key, &validation)?;
    Ok(token_data.claims)
}

async fn authenticate_api_key(state: &AppState, api_key: &str) -> Result<Claims, ()> {
    // Look up all active API keys for this workspace
    // We hash the incoming key and compare with stored hashes
    let keys = sqlx::query_as::<_, ApiKeyRow>(
        "SELECT * FROM api_keys WHERE is_active = true",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|_| ())?;

    for row in keys {
        if bcrypt::verify(api_key, &row.key_hash).unwrap_or(false) {
            return Ok(Claims {
                sub: format!("apikey:{}", row.id),
                workspace_id: row.workspace_id.to_string(),
                role: "api".to_string(),
                exp: usize::MAX,
            });
        }
    }

    Err(())
}
