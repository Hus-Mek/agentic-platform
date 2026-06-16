use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub id: String,
    pub run_id: String,
    pub agent_id: String,
    pub tool_name: String,
    pub tool_args: serde_json::Value,
    pub status: ApprovalStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Rejected,
    Expired,
}

pub struct ApprovalStore {
    requests: Arc<Mutex<HashMap<String, ApprovalRequest>>>,
}

impl ApprovalStore {
    pub fn new() -> Self {
        Self {
            requests: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn create_request(
        &self,
        run_id: &str,
        agent_id: &str,
        tool_name: &str,
        tool_args: serde_json::Value,
    ) -> String {
        let id = format!("approval_{}", uuid::Uuid::new_v4());
        let req = ApprovalRequest {
            id: id.clone(),
            run_id: run_id.to_string(),
            agent_id: agent_id.to_string(),
            tool_name: tool_name.to_string(),
            tool_args,
            status: ApprovalStatus::Pending,
            created_at: Utc::now(),
        };
        let mut store = self.requests.lock().await;
        store.insert(id.clone(), req);
        id
    }

    pub async fn get_request(&self, id: &str) -> Option<ApprovalRequest> {
        let store = self.requests.lock().await;
        store.get(id).cloned()
    }

    pub async fn approve(&self, id: &str) -> bool {
        let mut store = self.requests.lock().await;
        if let Some(req) = store.get_mut(id) {
            req.status = ApprovalStatus::Approved;
            true
        } else {
            false
        }
    }

    pub async fn reject(&self, id: &str) -> bool {
        let mut store = self.requests.lock().await;
        if let Some(req) = store.get_mut(id) {
            req.status = ApprovalStatus::Rejected;
            true
        } else {
            false
        }
    }

    pub async fn is_approved(&self, id: &str) -> Option<bool> {
        let store = self.requests.lock().await;
        store.get(id).map(|req| req.status == ApprovalStatus::Approved)
    }

    pub async fn list_pending(&self) -> Vec<ApprovalRequest> {
        let store = self.requests.lock().await;
        store
            .values()
            .filter(|r| r.status == ApprovalStatus::Pending)
            .cloned()
            .collect()
    }
}

impl Default for ApprovalStore {
    fn default() -> Self {
        Self::new()
    }
}
