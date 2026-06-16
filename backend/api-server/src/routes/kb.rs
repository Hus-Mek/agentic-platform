use axum::extract::{Path, State};
use axum::Extension;
use axum::Json;
use chrono::Utc;
use serde::Deserialize;
use uuid::Uuid;

use crate::middleware::auth::Claims;
use crate::models::{KbDocumentRow, KnowledgeBaseRow};
use crate::AppState;

fn internal_error<E: std::fmt::Display>(e: E) -> (axum::http::StatusCode, Json<serde_json::Value>) {
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({"error": e.to_string()})),
    )
}

fn not_found(msg: &str) -> (axum::http::StatusCode, Json<serde_json::Value>) {
    (
        axum::http::StatusCode::NOT_FOUND,
        Json(serde_json::json!({"error": msg})),
    )
}

#[derive(Debug, Deserialize)]
pub struct UploadDocumentRequest {
    pub filename: String,
    pub content: String,
    pub content_type: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct UploadDocumentResponse {
    pub document_id: Uuid,
    pub chunks_count: usize,
}

pub async fn list_knowledge_bases(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Vec<KnowledgeBaseRow>>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let workspace_id = claims
        .workspace_id
        .parse::<Uuid>()
        .map_err(|_| internal_error("invalid workspace_id"))?;

    let kbs = sqlx::query_as::<_, KnowledgeBaseRow>(
        "SELECT * FROM knowledge_bases WHERE workspace_id = $1 ORDER BY created_at DESC",
    )
    .bind(workspace_id)
    .fetch_all(&state.db)
    .await
    .map_err(internal_error)?;

    Ok(Json(kbs))
}

pub async fn get_knowledge_base(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<KnowledgeBaseRow>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let kb = sqlx::query_as::<_, KnowledgeBaseRow>(
        "SELECT * FROM knowledge_bases WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(internal_error)?
    .ok_or_else(|| not_found("Knowledge base not found"))?;

    Ok(Json(kb))
}

pub async fn list_documents(
    State(state): State<AppState>,
    Path(kb_id): Path<Uuid>,
) -> Result<Json<Vec<KbDocumentRow>>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let docs = sqlx::query_as::<_, KbDocumentRow>(
        "SELECT * FROM kb_documents WHERE knowledge_base_id = $1 ORDER BY created_at DESC",
    )
    .bind(kb_id)
    .fetch_all(&state.db)
    .await
    .map_err(internal_error)?;

    Ok(Json(docs))
}

pub async fn upload_document(
    State(state): State<AppState>,
    Path(kb_id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<UploadDocumentRequest>,
) -> Result<Json<UploadDocumentResponse>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let kb = sqlx::query_as::<_, KnowledgeBaseRow>(
        "SELECT * FROM knowledge_bases WHERE id = $1",
    )
    .bind(kb_id)
    .fetch_optional(&state.db)
    .await
    .map_err(internal_error)?
    .ok_or_else(|| not_found("Knowledge base not found"))?;

    let chunk_size: usize = kb
        .config
        .get("chunk_size")
        .and_then(|v| v.as_u64())
        .unwrap_or(512) as usize;
    let chunk_overlap: usize = kb
        .config
        .get("chunk_overlap")
        .and_then(|v| v.as_u64())
        .unwrap_or(64) as usize;

    let doc_id = Uuid::new_v4();
    let now = Utc::now();

    sqlx::query(
        "INSERT INTO kb_documents (id, knowledge_base_id, filename, content_type, content, created_by, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(doc_id)
    .bind(kb_id)
    .bind(&body.filename)
    .bind(&body.content_type)
    .bind(&body.content)
    .bind(&claims.sub)
    .bind(now)
    .execute(&state.db)
    .await
    .map_err(internal_error)?;

    let chunks = chunk_text(&body.content, chunk_size, chunk_overlap);
    let mut chunks_count = 0;

    for (idx, chunk_content) in chunks.iter().enumerate() {
        let chunk_id = Uuid::new_v4();

        let embedding = state
            .llm_client
            .embed_text(chunk_content)
            .await
            .map_err(|e| internal_error(format!("Embedding failed at chunk {idx}: {e}")))?;

        let emb_str: String = format!(
            "[{}]",
            embedding
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );

        sqlx::query(
            "INSERT INTO kb_chunks (id, document_id, chunk_index, content, embedding, created_at) VALUES ($1, $2, $3, $4, $5::vector, $6)",
        )
        .bind(chunk_id)
        .bind(doc_id)
        .bind(idx as i32)
        .bind(chunk_content)
        .bind(&emb_str)
        .bind(now)
        .execute(&state.db)
        .await
        .map_err(internal_error)?;

        chunks_count += 1;
    }

    Ok(Json(UploadDocumentResponse {
        document_id: doc_id,
        chunks_count,
    }))
}

pub async fn delete_document(
    State(state): State<AppState>,
    Path((_kb_id, doc_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let _existing = sqlx::query_as::<_, KbDocumentRow>(
        "SELECT * FROM kb_documents WHERE id = $1",
    )
    .bind(doc_id)
    .fetch_optional(&state.db)
    .await
    .map_err(internal_error)?
    .ok_or_else(|| not_found("Document not found"))?;

    sqlx::query("DELETE FROM kb_chunks WHERE document_id = $1")
        .bind(doc_id)
        .execute(&state.db)
        .await
        .map_err(internal_error)?;

    sqlx::query("DELETE FROM kb_documents WHERE id = $1")
        .bind(doc_id)
        .execute(&state.db)
        .await
        .map_err(internal_error)?;

    Ok(Json(serde_json::json!({"deleted": true, "document_id": doc_id})))
}

fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
    if text.len() <= chunk_size {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut start = 0;

    while start < text.len() {
        let end = (start + chunk_size).min(text.len());
        chunks.push(text[start..end].to_string());
        if end >= text.len() {
            break;
        }
        start = end.saturating_sub(overlap);
    }

    chunks
}
