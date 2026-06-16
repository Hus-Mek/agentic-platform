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
        let mut inspector = Inspector::new();
        inspector.add_policy(crate::inspector::PiiDetection::new());
        inspector.add_policy(crate::inspector::TokenBudgetPolicy);
        inspector.add_policy(crate::inspector::ContentModeration::new());
        inspector.add_policy(crate::inspector::IterationLimit);
        inspector.add_policy(crate::inspector::MemoryPoisoningPolicy);
        inspector.add_policy(crate::inspector::ToolMisusePolicy);
        inspector.add_policy(crate::inspector::PrivilegeEscalationPolicy);
        inspector.add_policy(crate::inspector::GoalHijackingPolicy);
        inspector.add_policy(crate::inspector::InsecureInterAgentPolicy);
        inspector.add_policy(crate::inspector::CascadingFailurePolicy);
        inspector.add_policy(crate::inspector::HumanTrustManipulationPolicy);
        inspector.add_policy(crate::inspector::RogueAgentPolicy);
        Self {
            inspector,
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
