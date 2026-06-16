use std::fmt::Display;

use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::middleware::auth::Claims;
use crate::AppState;

fn internal_error<E: Display>(e: E) -> (StatusCode, Json<Value>) {
    (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})))
}

fn generate_token(claims: &Claims, secret: &str) -> Result<String, jsonwebtoken::errors::Error> {
    encode(&Header::default(), claims, &EncodingKey::from_secret(secret.as_bytes()))
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

pub async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let existing = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users WHERE email = $1")
        .bind(&body.email)
        .fetch_one(&state.db)
        .await
        .map_err(internal_error)?;

    if existing > 0 {
        return Err((StatusCode::CONFLICT, Json(json!({"error": "email already registered"}))));
    }

    let user_id = Uuid::new_v4();
    let password_hash = bcrypt::hash(&body.password, 12).map_err(internal_error)?;
    let now = Utc::now();

    sqlx::query(
        "INSERT INTO users (id, email, password_hash, name, created_at) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(user_id)
    .bind(&body.email)
    .bind(&password_hash)
    .bind(&body.name)
    .bind(now)
    .execute(&state.db)
    .await
    .map_err(internal_error)?;

    let slug = body.name.to_lowercase().replace(' ', "-") + "-workspace";
    let workspace_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT create_workspace_with_owner($1, $2, $3)",
    )
    .bind(format!("{}'s workspace", body.name))
    .bind(&slug)
    .bind(user_id)
    .fetch_one(&state.db)
    .await
    .map_err(internal_error)?;

    let exp = (Utc::now() + Duration::hours(24)).timestamp() as usize;
    let claims = Claims {
        sub: user_id.to_string(),
        workspace_id: workspace_id.to_string(),
        exp,
    };

    let token = generate_token(&claims, &state.config.jwt_secret).map_err(internal_error)?;

    Ok(Json(json!({
        "token": token,
        "user": {
            "id": user_id,
            "email": body.email,
            "name": body.name,
        }
    })))
}

pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let user = sqlx::query_as::<_, (Uuid, String, String, String, chrono::DateTime<Utc>)>(
        "SELECT id, email, password_hash, name, created_at FROM users WHERE email = $1",
    )
    .bind(&body.email)
    .fetch_optional(&state.db)
    .await
    .map_err(internal_error)?
    .ok_or_else(|| (StatusCode::UNAUTHORIZED, Json(json!({"error": "invalid email or password"}))))?;

    let valid = bcrypt::verify(&body.password, &user.2).map_err(internal_error)?;
    if !valid {
        return Err((StatusCode::UNAUTHORIZED, Json(json!({"error": "invalid email or password"}))));
    }

    let workspace_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT workspace_id FROM workspace_members WHERE user_id = $1 ORDER BY created_at LIMIT 1",
    )
    .bind(user.0)
    .fetch_optional(&state.db)
    .await
    .map_err(internal_error)?
    .ok_or_else(|| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "no workspace found"}))))?;

    let exp = (Utc::now() + Duration::hours(24)).timestamp() as usize;
    let claims = Claims {
        sub: user.0.to_string(),
        workspace_id: workspace_id.to_string(),
        exp,
    };

    let token = generate_token(&claims, &state.config.jwt_secret).map_err(internal_error)?;

    Ok(Json(json!({
        "token": token,
        "user": {
            "id": user.0,
            "email": user.1,
            "name": user.3,
        }
    })))
}

pub async fn me(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| (StatusCode::UNAUTHORIZED, Json(json!({"error": "invalid token"}))))?;

    let row = sqlx::query_as::<_, (Uuid, String, String)>(
        "SELECT id, email, name FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await
    .map_err(internal_error)?
    .ok_or_else(|| (StatusCode::NOT_FOUND, Json(json!({"error": "user not found"}))))?;

    Ok(Json(json!({
        "id": row.0,
        "email": row.1,
        "name": row.2,
    })))
}
