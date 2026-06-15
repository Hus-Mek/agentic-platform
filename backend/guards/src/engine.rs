use agenticos_core::error::Result;
use agenticos_core::state::{AgentState, GuardDecision};
use serde_json::Value;

use crate::bodyguard::Bodyguard;
use crate::inspector::Inspector;

pub struct GuardEngine {
    pub inspector: Inspector,
    pub bodyguard: Bodyguard,
}

impl GuardEngine {
    pub fn new() -> Self {
        Self {
            inspector: Inspector::new(),
            bodyguard: Bodyguard::new(),
        }
    }

    pub async fn inspect(&self, state: &AgentState) -> Option<GuardDecision> {
        self.inspector.inspect(state).await
    }

    pub async fn check_tool_access(
        &self,
        tool_name: &str,
        args: &Value,
        user_id: &str,
        workspace_id: &str,
        scope: &str,
    ) -> Result<()> {
        self.bodyguard
            .check_tool_access(tool_name, args, user_id, workspace_id, scope)
    }
}

impl Default for GuardEngine {
    fn default() -> Self {
        Self::new()
    }
}
