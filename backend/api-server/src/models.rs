use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AgentRow {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub instructions: String,
    pub model: String,
    pub tools: serde_json::Value,
    pub guard_config: serde_json::Value,
    pub memory_config: serde_json::Value,
    pub created_by: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AgentVersionRow {
    pub id: Uuid,
    pub agent_id: Uuid,
    pub version_number: i32,
    pub config_snapshot: serde_json::Value,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RunRow {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub agent_id: Uuid,
    pub status: String,
    pub input: Option<String>,
    pub output: Option<String>,
    pub error: Option<String>,
    pub duration_ms: Option<i64>,
    pub total_tokens: Option<i64>,
    pub total_cost: Option<f64>,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RunStepRow {
    pub id: Uuid,
    pub run_id: Uuid,
    pub step_type: String,
    pub node_name: Option<String>,
    pub input: Option<String>,
    pub output: Option<String>,
    pub duration_ms: Option<i64>,
    pub tokens_used: Option<i64>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct WorkspaceRow {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ApiKeyRow {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub key_hash: String,
    pub name: Option<String>,
    pub scope: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct KbDocumentRow {
    pub id: Uuid,
    pub knowledge_base_id: Uuid,
    pub filename: String,
    pub content_type: Option<String>,
    pub content: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct KbChunkRow {
    pub id: Uuid,
    pub document_id: Uuid,
    pub chunk_index: i32,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct KnowledgeBaseRow {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub config: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TeamAgentRow {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub supervisor_instructions: String,
    pub supervisor_model: String,
    pub sub_agents: serde_json::Value,
    pub max_delegations: i32,
    pub execution_mode: String,
    pub created_by: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TeamRunRow {
    pub id: Uuid,
    pub team_id: Uuid,
    pub workspace_id: Uuid,
    pub status: String,
    pub input: Option<String>,
    pub output: Option<String>,
    pub error: Option<String>,
    pub delegations: serde_json::Value,
    pub total_tokens: Option<i64>,
    pub total_cost: Option<f64>,
    pub duration_ms: Option<i64>,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
