use std::fmt::Display;

use axum::extract::{Path, Query, State};
use axum::Extension;
use axum::Json;
use chrono::{NaiveDate, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::middleware::auth::Claims;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct UsageQuery {
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
}

pub async fn get_usage(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(_claims): Extension<Claims>,
    Query(query): Query<UsageQuery>,
) -> Result<Json<Vec<Value>>, (axum::http::StatusCode, Json<Value>)> {
    let rows = sqlx::query_as::<_, (NaiveDate, i64, f64, i32)>(
        "SELECT date, total_tokens, total_cost::float8, request_count FROM workspace_usage WHERE workspace_id = $1 AND ($2::DATE IS NULL OR date >= $2::DATE) AND ($3::DATE IS NULL OR date <= $3::DATE) ORDER BY date DESC",
    )
    .bind(id)
    .bind(query.start_date)
    .bind(query.end_date)
    .fetch_all(&state.db)
    .await
    .map_err(internal_error)?;

    let usage: Vec<Value> = rows
        .into_iter()
        .map(|(date, total_tokens, total_cost, request_count)| {
            json!({
                "date": date,
                "total_tokens": total_tokens,
                "total_cost": total_cost,
                "request_count": request_count,
            })
        })
        .collect();

    Ok(Json(usage))
}

pub async fn get_usage_summary(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(_claims): Extension<Claims>,
) -> Result<Json<Value>, (axum::http::StatusCode, Json<Value>)> {
    let today = Utc::now().date_naive();
    let row = sqlx::query_as::<_, (NaiveDate, i64, f64, i32)>(
        "SELECT date, total_tokens, total_cost::float8, request_count FROM workspace_usage WHERE workspace_id = $1 AND date = $2"
    )
    .bind(id)
    .bind(today)
    .fetch_optional(&state.db)
    .await
    .map_err(internal_error)?;

    let usage = match row {
        Some((date, total_tokens, total_cost, request_count)) => json!({
            "date": date,
            "total_tokens": total_tokens,
            "total_cost": total_cost,
            "request_count": request_count,
        }),
        None => json!({
            "date": today,
            "total_tokens": 0,
            "total_cost": 0.0,
            "request_count": 0,
        }),
    };

    Ok(Json(usage))
}

fn internal_error<E: Display>(e: E) -> (axum::http::StatusCode, Json<Value>) {
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": e.to_string()})),
    )
}
