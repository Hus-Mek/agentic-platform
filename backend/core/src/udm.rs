// Unified Data Model (UDM) for agent platform telemetry
// Inspired by Google Chronicle UDM — normalized event schema
// Every component emits UDM events: agent steps, tool calls, guard actions, memory ops

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Top-level UDM event envelope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEvent {
    pub event_id: String,
    pub event_type: EventType,
    pub event_time: DateTime<Utc>,
    pub metadata: EventMetadata,
    pub principal: Option<Entity>,
    pub target: Option<Entity>,
    pub observer: Option<Entity>,
    pub result: Option<EventResult>,
    pub observables: Vec<Observable>,
    pub extensions: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetadata {
    pub product_name: String,
    pub product_version: String,
    pub workspace_id: Option<String>,
    pub run_id: Option<String>,
    pub agent_id: Option<String>,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub correlation_id: Option<String>,
    pub ingested_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum EventType {
    // Agent lifecycle
    AgentRunStarted,
    AgentRunCompleted,
    AgentRunFailed,
    AgentRunCancelled,

    // Execution steps
    PlanStepStarted,
    PlanStepCompleted,
    ToolCallInitiated,
    ToolCallCompleted,
    ToolCallFailed,
    EvaluateStepStarted,
    EvaluateStepCompleted,

    // Guard actions
    GuardInspectorCheck,
    GuardBodyguardCheck,
    GuardViolation,
    GuardPiiRedaction,

    // Memory operations
    MemoryRetrieved,
    MemoryStored,
    MemoryDeleted,

    // System events
    SystemWarning,
    SystemError,
    TokenBudgetWarning,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub id: String,
    pub entity_type: String,
    pub name: Option<String>,
    pub attributes: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventResult {
    pub status: String,
    pub code: Option<String>,
    pub message: Option<String>,
    pub duration_ms: Option<i64>,
    pub tokens_used: Option<i64>,
    pub token_cost: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observable {
    pub observable_type: String,
    pub value: String,
    pub labels: Vec<String>,
}

impl AgentEvent {
    pub fn new(
        event_type: EventType,
        workspace_id: Option<String>,
        run_id: Option<String>,
        agent_id: Option<String>,
        user_id: Option<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            event_id: Uuid::new_v4().to_string(),
            event_type,
            event_time: now,
            metadata: EventMetadata {
                product_name: "AgenticOS".into(),
                product_version: "0.1.0".into(),
                workspace_id,
                run_id,
                agent_id,
                user_id,
                session_id: None,
                correlation_id: None,
                ingested_at: now,
            },
            principal: None,
            target: None,
            observer: None,
            result: None,
            observables: Vec::new(),
            extensions: serde_json::Value::Null,
        }
    }

    pub fn with_result(mut self, result: EventResult) -> Self {
        self.result = Some(result);
        self
    }

    pub fn with_principal(mut self, entity: Entity) -> Self {
        self.principal = Some(entity);
        self
    }

    pub fn with_target(mut self, entity: Entity) -> Self {
        self.target = Some(entity);
        self
    }

    pub fn with_observable(mut self, observable: Observable) -> Self {
        self.observables.push(observable);
        self
    }

    pub fn emit(self) {
        // In production: send to trace ingestion pipeline (Kafka/nats/ClickHouse)
        // For now: log as structured event
        tracing::info!(
            event_type = ?self.event_type,
            event_id = %self.event_id,
            run_id = ?self.metadata.run_id,
            workspace_id = ?self.metadata.workspace_id,
            "UDM event: {}",
            serde_json::to_string(&self).unwrap_or_default()
        );
    }
}

impl Entity {
    pub fn user(id: &str, name: Option<&str>) -> Self {
        Self {
            id: id.to_string(),
            entity_type: "user".into(),
            name: name.map(String::from),
            attributes: serde_json::json!({}),
        }
    }

    pub fn agent(id: &str, name: Option<&str>) -> Self {
        Self {
            id: id.to_string(),
            entity_type: "agent".into(),
            name: name.map(String::from),
            attributes: serde_json::json!({}),
        }
    }

    pub fn tool(name: &str) -> Self {
        Self {
            id: name.to_string(),
            entity_type: "tool".into(),
            name: Some(name.to_string()),
            attributes: serde_json::json!({}),
        }
    }

    pub fn model(name: &str) -> Self {
        Self {
            id: name.to_string(),
            entity_type: "model".into(),
            name: Some(name.to_string()),
            attributes: serde_json::json!({}),
        }
    }
}

impl EventResult {
    pub fn success() -> Self {
        Self {
            status: "SUCCESS".into(),
            code: None,
            message: None,
            duration_ms: None,
            tokens_used: None,
            token_cost: None,
        }
    }

    pub fn failure(message: &str) -> Self {
        Self {
            status: "FAILURE".into(),
            code: None,
            message: Some(message.to_string()),
            duration_ms: None,
            tokens_used: None,
            token_cost: None,
        }
    }

    pub fn with_duration(mut self, ms: i64) -> Self {
        self.duration_ms = Some(ms);
        self
    }

    pub fn with_tokens(mut self, tokens: i64, cost: f64) -> Self {
        self.tokens_used = Some(tokens);
        self.token_cost = Some(cost);
        self
    }
}

impl Observable {
    pub fn message(content: &str) -> Self {
        Self {
            observable_type: "message".into(),
            value: content.to_string(),
            labels: vec!["text".into()],
        }
    }

    pub fn tool_call(name: &str, args: &serde_json::Value) -> Self {
        Self {
            observable_type: "tool_call".into(),
            value: serde_json::to_string(args).unwrap_or_default(),
            labels: vec![format!("tool:{}", name)],
        }
    }

    pub fn token_usage(tokens: i64, cost: f64) -> Self {
        Self {
            observable_type: "token_usage".into(),
            value: serde_json::json!({ "tokens": tokens, "cost": cost }).to_string(),
            labels: vec!["metrics".into()],
        }
    }
}
