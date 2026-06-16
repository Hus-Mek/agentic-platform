use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct Mem0Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct StoreMemoryRequest {
    pub messages: Vec<Mem0Message>,
    pub user_id: Option<String>,
    pub agent_id: Option<String>,
    pub run_id: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct StoreMemoryResponse {
    pub id: String,
    pub memory: String,
}

#[derive(Debug, Serialize)]
pub struct SearchMemoryRequest {
    pub query: String,
    pub user_id: Option<String>,
    pub agent_id: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct SearchMemoryResponse {
    pub results: Vec<Mem0Result>,
}

#[derive(Debug, Deserialize)]
pub struct Mem0Result {
    pub id: String,
    pub memory: String,
    pub score: f64,
    pub metadata: Option<serde_json::Value>,
}

pub struct Mem0Client {
    base_url: String,
    api_key: String,
    http_client: reqwest::Client,
}

impl Mem0Client {
    pub fn new(base_url: &str, api_key: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            http_client: reqwest::Client::new(),
        }
    }

    pub async fn store(&self, req: &StoreMemoryRequest) -> Result<Vec<StoreMemoryResponse>> {
        let resp = self
            .http_client
            .post(format!("{}/v1/memories/", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(req)
            .send()
            .await?;
        let results: Vec<StoreMemoryResponse> = resp.json().await?;
        Ok(results)
    }

    pub async fn search(&self, req: &SearchMemoryRequest) -> Result<Vec<Mem0Result>> {
        let resp = self
            .http_client
            .post(format!("{}/v1/memories/search/", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(req)
            .send()
            .await?;
        let results: SearchMemoryResponse = resp.json().await?;
        Ok(results.results)
    }
}
