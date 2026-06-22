use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: serde_json::Value,
    pub result: Option<String>,
    pub error: Option<String>,
    pub duration_ms: Option<i64>,
    pub timestamp: DateTime<Utc>,
}

impl ToolCall {
    pub fn new(name: &str, args: serde_json::Value) -> Self {
        Self {
            id: format!("call_{}", Uuid::new_v4().to_string().replace('-', "")[..12].to_string()),
            name: name.to_string(),
            args,
            result: None,
            error: None,
            duration_ms: None,
            timestamp: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub role: String,
    pub content: String,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_call_id: Option<String>,
    pub name: Option<String>,
    pub timestamp: DateTime<Utc>,
}

impl Message {
    pub fn new(role: &str, content: &str) -> Self {
        Self {
            id: format!("msg_{}", Uuid::new_v4().to_string().replace('-', "")[..12].to_string()),
            role: role.to_string(),
            content: content.to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
            timestamp: Utc::now(),
        }
    }

    pub fn with_tool_calls(role: &str, content: &str, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            tool_calls: Some(tool_calls),
            ..Self::new(role, content)
        }
    }

    pub fn tool_result(name: &str, tool_call_id: &str, content: &str) -> Self {
        Self {
            role: "tool".into(),
            content: content.to_string(),
            name: Some(name.to_string()),
            tool_call_id: Some(tool_call_id.to_string()),
            ..Self::new("tool", "")
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardDecision {
    pub action: GuardAction,
    pub reason: String,
    pub edited_content: Option<String>,
    pub policy: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GuardAction {
    Continue,
    Retry,
    Abort,
    Edit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardConfig {
    pub inspector_enabled: bool,
    pub bodyguard_enabled: bool,
    pub pii_detection: bool,
    pub max_tokens_per_run: i64,
}

impl Default for GuardConfig {
    fn default() -> Self {
        Self {
            inspector_enabled: true,
            bodyguard_enabled: true,
            pii_detection: true,
            max_tokens_per_run: 100000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    pub enabled: bool,
    pub memory_type: String,
    pub max_memories: usize,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            memory_type: "episodic".into(),
            max_memories: 10,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
    pub required: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    pub messages: Vec<Message>,
    pub agent_id: Option<String>,
    pub run_id: Option<String>,
    pub workspace_id: Option<String>,
    pub user_id: Option<String>,
    pub model: String,
    pub instructions: String,
    pub tools: Vec<ToolDefinition>,
    pub guard_config: GuardConfig,
    pub memory_config: MemoryConfig,
    pub memory_contents: Vec<String>,
    pub kb_contents: Vec<String>,
    pub guard_decision: Option<GuardDecision>,
    pub token_budget: i64,
    pub total_tokens_used: i64,
    pub total_cost: f64,
    pub consecutive_errors: u32,
    pub max_consecutive_errors: u32,
    pub current_node: Option<String>,
    pub metadata: HashMap<String, serde_json::Value>,
    pub iteration: u32,
    pub max_iterations: u32,
    /// Catalog snapshot used by the Matchmaker/Router. Runtime-only (never
    /// serialized); populated from the app-level `ModelRouter` at run start.
    #[serde(skip)]
    pub router: Option<crate::model_router::ModelRouter>,
    /// When true, the router selects the model per-step from `router`'s catalog
    /// based on task complexity + remaining budget. When false, `model` is used
    /// as-is (resilience — retry/fallback — still applies).
    #[serde(default)]
    pub auto_route: bool,
}

impl AgentState {
    pub fn new(
        agent_id: Option<String>,
        run_id: Option<String>,
        workspace_id: Option<String>,
        user_id: Option<String>,
        instructions: &str,
        model: &str,
    ) -> Self {
        Self {
            messages: vec![Message::new("system", instructions)],
            agent_id,
            run_id,
            workspace_id,
            user_id,
            model: model.to_string(),
            instructions: instructions.to_string(),
            tools: Vec::new(),
            guard_config: GuardConfig::default(),
            memory_config: MemoryConfig::default(),
            memory_contents: Vec::new(),
            kb_contents: Vec::new(),
            guard_decision: None,
            token_budget: 100000,
            total_tokens_used: 0,
            total_cost: 0.0,
            consecutive_errors: 0,
            max_consecutive_errors: 3,
            current_node: None,
            metadata: HashMap::new(),
            iteration: 0,
            max_iterations: 25,
            router: None,
            auto_route: false,
        }
    }

    /// Enable Matchmaker model selection with the given catalog snapshot.
    pub fn with_router(mut self, router: crate::model_router::ModelRouter, auto_route: bool) -> Self {
        self.router = Some(router);
        self.auto_route = auto_route;
        self
    }

    pub fn add_message(&mut self, msg: Message) {
        self.messages.push(msg);
    }

    pub fn last_message(&self) -> Option<&Message> {
        self.messages.last()
    }

    pub fn has_tool_calls(&self) -> bool {
        self.messages
            .last()
            .and_then(|m| m.tool_calls.as_ref())
            .map(|c| !c.is_empty())
            .unwrap_or(false)
    }

    pub fn should_stop(&self) -> bool {
        self.guard_decision
            .as_ref()
            .map(|d| d.action == GuardAction::Abort)
            .unwrap_or(false)
            || self.consecutive_errors >= self.max_consecutive_errors
            || self.iteration >= self.max_iterations
            || self.total_tokens_used >= self.token_budget
    }

    pub fn increment_iteration(&mut self) {
        self.iteration += 1;
    }

    pub fn add_tokens(&mut self, tokens: i64, cost: f64) {
        self.total_tokens_used += tokens;
        self.total_cost += cost;
    }

    pub fn can_add_tokens(&self, tokens: i64) -> bool {
        self.total_tokens_used + tokens <= self.token_budget
    }
}

// ── Recovery types (error classification + circuit breaker) ──────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryConfig {
    pub max_retries: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub max_consecutive_failures: u32,
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay_ms: 1000,
            max_delay_ms: 32000,
            max_consecutive_failures: 5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RecoveryAction {
    Retry(u64),     // delay_ms before retry
    Fail(String),    // permanent failure message
    CircuitBreak,   // stop the agent
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryState {
    pub consecutive_failures: u32,
    pub total_retries: u32,
    pub backoff_ms: u64,
}

impl RecoveryState {
    pub fn new() -> Self {
        Self {
            consecutive_failures: 0,
            total_retries: 0,
            backoff_ms: 0,
        }
    }

    pub fn record_success(&mut self) {
        self.consecutive_failures = 0;
        self.backoff_ms = 0;
    }

    pub fn record_failure(&mut self, config: &RecoveryConfig) -> RecoveryAction {
        self.consecutive_failures += 1;

        if self.should_circuit_break(config) {
            return RecoveryAction::CircuitBreak;
        }

        if self.backoff_ms == 0 {
            self.backoff_ms = config.base_delay_ms;
        } else {
            self.backoff_ms = (self.backoff_ms * 2).min(config.max_delay_ms);
        }

        self.total_retries += 1;
        RecoveryAction::Retry(self.backoff_ms)
    }

    pub fn should_circuit_break(&self, config: &RecoveryConfig) -> bool {
        self.consecutive_failures >= config.max_consecutive_failures
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgentConfig {
    pub id: String,
    pub name: String,
    pub instructions: String,
    pub model: String,
    pub tools: Vec<ToolDefinition>,
    pub guard_config: GuardConfig,
}

impl Default for SubAgentConfig {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            instructions: String::new(),
            model: "gpt-4".into(),
            tools: Vec::new(),
            guard_config: GuardConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExecutionMode {
    Sequential,
    Parallel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamConfig {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub supervisor_instructions: String,
    pub supervisor_model: String,
    pub sub_agents: Vec<SubAgentConfig>,
    pub max_delegations: u32,
    pub execution_mode: ExecutionMode,
}

impl Default for TeamConfig {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            description: None,
            supervisor_instructions: String::new(),
            supervisor_model: "gpt-4".into(),
            sub_agents: Vec::new(),
            max_delegations: 10,
            execution_mode: ExecutionMode::Sequential,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationResult {
    pub sub_agent_id: String,
    pub sub_agent_name: String,
    pub task: String,
    pub result: String,
    pub tokens_used: i64,
    pub cost: f64,
    pub duration_ms: i64,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamRunState {
    pub team_id: String,
    pub run_id: String,
    pub workspace_id: Option<String>,
    pub user_id: Option<String>,
    pub input: String,
    pub supervisor_state: AgentState,
    pub delegations: Vec<DelegationResult>,
    pub status: String,
    pub total_tokens_used: i64,
    pub total_cost: f64,
}
