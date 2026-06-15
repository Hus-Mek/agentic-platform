use std::collections::HashMap;
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use qdrant_client::Qdrant;
use qdrant_client::qdrant::{
    CreateCollectionBuilder, Distance, VectorParamsBuilder,
    SearchPointsBuilder, Condition, PointId, Value as QdrantValue,
    UpsertPointsBuilder, DeletePointsBuilder,
    point_id, PointStruct, Filter,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use agenticos_core::error::*;
use agenticos_core::udm::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub content: String,
    pub memory_type: String,
    pub user_id: Option<String>,
    pub agent_id: Option<String>,
    pub run_id: Option<String>,
    pub score: f64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default)]
pub struct MemoryFilter {
    pub memory_type: Option<String>,
    pub user_id: Option<String>,
    pub agent_id: Option<String>,
    pub run_id: Option<String>,
}

struct FallbackEntry {
    id: String,
    content: String,
    memory_type: String,
    user_id: Option<String>,
    agent_id: Option<String>,
    run_id: Option<String>,
    created_at: DateTime<Utc>,
}

pub struct MemoryClient {
    qdrant: Qdrant,
    collection_name: String,
    http_client: reqwest::Client,
    embed_base_url: String,
    fallback_store: Mutex<HashMap<String, FallbackEntry>>,
    use_fallback: bool,
}

fn qdrant_value_from_str(s: &str) -> QdrantValue {
    QdrantValue {
        kind: Some(qdrant_client::qdrant::value::Kind::StringValue(s.to_string())),
    }
}

impl MemoryClient {
    pub async fn new(qdrant_url: &str, collection_name: &str) -> Self {
        let qdrant = match Qdrant::from_url(qdrant_url).build() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Failed to connect to Qdrant at {qdrant_url}: {e}. Using fallback in-memory store.");
                return Self {
                    qdrant: unsafe { std::mem::zeroed() },
                    collection_name: collection_name.to_string(),
                    http_client: reqwest::Client::new(),
                    embed_base_url: "http://localhost:4000".into(),
                    fallback_store: Mutex::new(HashMap::new()),
                    use_fallback: true,
                };
            }
        };

        let collections = qdrant
            .list_collections()
            .await
            .unwrap_or_else(|_| panic!("Failed to list Qdrant collections"));

        let exists = collections
            .collections
            .iter()
            .any(|c| c.name == collection_name);

        if !exists {
            if let Err(e) = qdrant
                .create_collection(
                    CreateCollectionBuilder::new(collection_name)
                        .vectors_config(VectorParamsBuilder::new(1536, Distance::Cosine)),
                )
                .await
            {
                tracing::warn!("Failed to create Qdrant collection: {e}. Using fallback.");
                return Self {
                    qdrant: unsafe { std::mem::zeroed() },
                    collection_name: collection_name.to_string(),
                    http_client: reqwest::Client::new(),
                    embed_base_url: "http://localhost:4000".into(),
                    fallback_store: Mutex::new(HashMap::new()),
                    use_fallback: true,
                };
            }
            tracing::info!(collection = %collection_name, "Created Qdrant collection");
        }

        Self {
            qdrant,
            collection_name: collection_name.to_string(),
            http_client: reqwest::Client::new(),
            embed_base_url: "http://localhost:4000".into(),
            fallback_store: Mutex::new(HashMap::new()),
            use_fallback: false,
        }
    }

    pub async fn from_config(config: &agenticos_core::config::AppConfig) -> Self {
        Self::new(&config.qdrant_url, "memories").await
    }

    pub async fn search(
        &self,
        query: &str,
        filter: MemoryFilter,
        limit: u64,
    ) -> Result<Vec<MemoryEntry>> {
        if self.use_fallback {
            return Ok(self.fallback_search(query, &filter, limit));
        }

        let embedding = match self.embed_text(query).await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("Embedding failed, using fallback search: {e}");
                return Ok(self.fallback_search(query, &filter, limit));
            }
        };

        let mut filter_conditions: Vec<Condition> = Vec::new();
        if let Some(ref mem_type) = filter.memory_type {
            filter_conditions.push(
                Condition::matches("memory_type", mem_type.clone()),
            );
        }
        if let Some(ref uid) = filter.user_id {
            filter_conditions.push(
                Condition::matches("user_id", uid.clone()),
            );
        }
        if let Some(ref aid) = filter.agent_id {
            filter_conditions.push(
                Condition::matches("agent_id", aid.clone()),
            );
        }

        let filter_obj = if filter_conditions.is_empty() {
            None
        } else {
            Some(Filter::must(filter_conditions))
        };

        let search_result = self
            .qdrant
            .search_points(
                SearchPointsBuilder::new(
                    self.collection_name.as_str(),
                    embedding,
                    limit.max(100),
                )
                .filter(filter_obj.unwrap_or_default())
                .with_payload(true),
            )
            .await
            .map_err(|e| AgentError::Memory(format!("Qdrant search failed: {e}")))?;

        let entries: Vec<MemoryEntry> = search_result
            .result
            .into_iter()
            .map(|point| {
                let payload = point.payload;
                MemoryEntry {
                    id: payload
                        .get("id")
                        .and_then(|v| v.kind.as_ref())
                        .and_then(|k| match k {
                            qdrant_client::qdrant::value::Kind::StringValue(s) => Some(s.clone()),
                            _ => None,
                        })
                        .unwrap_or_default(),
                    content: payload
                        .get("content")
                        .and_then(|v| v.kind.as_ref())
                        .and_then(|k| match k {
                            qdrant_client::qdrant::value::Kind::StringValue(s) => Some(s.clone()),
                            _ => None,
                        })
                        .unwrap_or_default(),
                    memory_type: payload
                        .get("memory_type")
                        .and_then(|v| v.kind.as_ref())
                        .and_then(|k| match k {
                            qdrant_client::qdrant::value::Kind::StringValue(s) => Some(s.clone()),
                            _ => None,
                        })
                        .unwrap_or_else(|| "unknown".into()),
                    user_id: payload
                        .get("user_id")
                        .and_then(|v| v.kind.as_ref())
                        .and_then(|k| match k {
                            qdrant_client::qdrant::value::Kind::StringValue(s) => {
                                if s.is_empty() || s == "null" { None } else { Some(s.clone()) }
                            }
                            _ => None,
                        }),
                    agent_id: payload
                        .get("agent_id")
                        .and_then(|v| v.kind.as_ref())
                        .and_then(|k| match k {
                            qdrant_client::qdrant::value::Kind::StringValue(s) => {
                                if s.is_empty() || s == "null" { None } else { Some(s.clone()) }
                            }
                            _ => None,
                        }),
                    run_id: payload
                        .get("run_id")
                        .and_then(|v| v.kind.as_ref())
                        .and_then(|k| match k {
                            qdrant_client::qdrant::value::Kind::StringValue(s) => {
                                if s.is_empty() || s == "null" { None } else { Some(s.clone()) }
                            }
                            _ => None,
                        }),
                    score: point.score as f64,
                    created_at: payload
                        .get("created_at")
                        .and_then(|v| v.kind.as_ref())
                        .and_then(|k| match k {
                            qdrant_client::qdrant::value::Kind::StringValue(s) => {
                                chrono::DateTime::parse_from_rfc3339(s).ok().map(|dt| dt.with_timezone(&Utc))
                            }
                            _ => None,
                        })
                        .unwrap_or(Utc::now()),
                }
            })
            .collect();

        let event = AgentEvent::new(
            EventType::MemoryRetrieved,
            None,
            filter.run_id.clone(),
            filter.agent_id.clone(),
            filter.user_id.clone(),
        )
        .with_result(EventResult::success())
        .with_observable(Observable::message(&format!(
            "Memory search for '{query}' returned {} results",
            entries.len()
        )));
        event.emit();

        Ok(entries)
    }

    pub async fn store(
        &self,
        content: &str,
        memory_type: &str,
        user_id: Option<&str>,
        agent_id: Option<&str>,
        run_id: Option<&str>,
    ) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();

        if self.use_fallback {
            let mut store = self.fallback_store.lock().unwrap();
            store.insert(
                id.clone(),
                FallbackEntry {
                    id: id.clone(),
                    content: content.to_string(),
                    memory_type: memory_type.to_string(),
                    user_id: user_id.map(String::from),
                    agent_id: agent_id.map(String::from),
                    run_id: run_id.map(String::from),
                    created_at: now,
                },
            );
            return Ok(id);
        }

        let embedding = match self.embed_text(content).await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("Embedding failed for store: {e}");
                let mut store = self.fallback_store.lock().unwrap();
                store.insert(
                    id.clone(),
                    FallbackEntry {
                        id: id.clone(),
                        content: content.to_string(),
                        memory_type: memory_type.to_string(),
                        user_id: user_id.map(String::from),
                        agent_id: agent_id.map(String::from),
                        run_id: run_id.map(String::from),
                        created_at: now,
                    },
                );
                return Ok(id);
            }
        };

        let mut payload = HashMap::new();
        payload.insert("id".to_string(), qdrant_value_from_str(&id));
        payload.insert("content".to_string(), qdrant_value_from_str(content));
        payload.insert("memory_type".to_string(), qdrant_value_from_str(memory_type));
        payload.insert("user_id".to_string(), qdrant_value_from_str(user_id.unwrap_or("")));
        payload.insert("agent_id".to_string(), qdrant_value_from_str(agent_id.unwrap_or("")));
        payload.insert("run_id".to_string(), qdrant_value_from_str(run_id.unwrap_or("")));
        payload.insert("created_at".to_string(), qdrant_value_from_str(&now.to_rfc3339()));

        let point_id = PointId {
            point_id_options: Some(point_id::PointIdOptions::Uuid(id.clone())),
        };

        let point = PointStruct::new(point_id, embedding, payload);

        self.qdrant
            .upsert_points(
                UpsertPointsBuilder::new(self.collection_name.as_str(), vec![point])
                    .wait(true),
            )
            .await
            .map_err(|e| AgentError::Memory(format!("Qdrant upsert failed: {e}")))?;

        Ok(id)
    }

    pub async fn retrieve_relevant(
        &self,
        query: &str,
        user_id: Option<&str>,
        agent_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<String>> {
        let limit = limit.min(10).max(1) as u64;
        let filter = MemoryFilter {
            memory_type: None,
            user_id: user_id.map(String::from),
            agent_id: agent_id.map(String::from),
            run_id: None,
        };

        let entries = self.search(query, filter, limit).await?;
        Ok(entries.into_iter().map(|e| e.content).collect())
    }

    pub async fn delete(&self, memory_id: &str) -> Result<()> {
        if self.use_fallback {
            let mut store = self.fallback_store.lock().unwrap();
            store.remove(memory_id);
            return Ok(());
        }

        let point_id = PointId {
            point_id_options: Some(point_id::PointIdOptions::Uuid(memory_id.to_string())),
        };

        self.qdrant
            .delete_points(
                DeletePointsBuilder::new(self.collection_name.as_str())
                    .points(vec![point_id])
                    .wait(true),
            )
            .await
            .map_err(|e| AgentError::Memory(format!("Qdrant delete failed: {e}")))?;

        Ok(())
    }

    pub async fn clear_agent_memories(&self, agent_id: &str) -> Result<()> {
        if self.use_fallback {
            let mut store = self.fallback_store.lock().unwrap();
            store.retain(|_, e| e.agent_id.as_deref() != Some(agent_id));
            return Ok(());
        }

        let filter = Filter::must(vec![
            Condition::matches("agent_id", agent_id.to_string()),
        ]);

        self.qdrant
            .delete_points(
                DeletePointsBuilder::new(self.collection_name.as_str())
                    .points(filter)
                    .wait(true),
            )
            .await
            .map_err(|e| AgentError::Memory(format!("Qdrant clear failed: {e}")))?;

        Ok(())
    }

    async fn embed_text(&self, text: &str) -> Result<Vec<f32>> {
        let url = format!("{}/v1/embeddings", self.embed_base_url);
        let body = serde_json::json!({
            "model": "text-embedding-3-small",
            "input": text,
        });

        let resp = self.http_client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| AgentError::Internal(format!("Embedding request failed: {e}")))?;

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AgentError::Internal(format!("Embedding parse failed: {e}")))?;

        let embedding: Vec<f64> = serde_json::from_value(
            data["data"][0]["embedding"].clone()
        ).map_err(|e| AgentError::Internal(format!("Embedding format error: {e}")))?;

        Ok(embedding.into_iter().map(|v| v as f32).collect())
    }

    fn fallback_search(
        &self,
        query: &str,
        filter: &MemoryFilter,
        limit: u64,
    ) -> Vec<MemoryEntry> {
        let store = self.fallback_store.lock().unwrap();
        let query_lower = query.to_lowercase();

        let mut matches: Vec<&FallbackEntry> = store
            .values()
            .filter(|e| {
                if let Some(ref mem_type) = filter.memory_type {
                    if e.memory_type != *mem_type { return false; }
                }
                if let Some(ref uid) = filter.user_id {
                    if e.user_id.as_deref() != Some(uid.as_str()) { return false; }
                }
                if let Some(ref aid) = filter.agent_id {
                    if e.agent_id.as_deref() != Some(aid.as_str()) { return false; }
                }
                e.content.to_lowercase().contains(&query_lower)
            })
            .collect();

        matches.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        matches.truncate(limit as usize);

        matches
            .into_iter()
            .map(|e| MemoryEntry {
                id: e.id.clone(),
                content: e.content.clone(),
                memory_type: e.memory_type.clone(),
                user_id: e.user_id.clone(),
                agent_id: e.agent_id.clone(),
                run_id: e.run_id.clone(),
                score: 0.0,
                created_at: e.created_at,
            })
            .collect()
    }
}

impl MemoryFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_memory_type(mut self, memory_type: &str) -> Self {
        self.memory_type = Some(memory_type.to_string());
        self
    }

    pub fn with_user_id(mut self, user_id: &str) -> Self {
        self.user_id = Some(user_id.to_string());
        self
    }

    pub fn with_agent_id(mut self, agent_id: &str) -> Self {
        self.agent_id = Some(agent_id.to_string());
        self
    }
}
