use std::future::Future;
use std::pin::Pin;

use agenticos_core::state::{AgentState, GuardAction, GuardDecision};

use crate::policies::PiiDetector;

pub struct Inspector {
    policies: Vec<Box<dyn Policy + Send + Sync>>,
}

pub trait Policy: Send + Sync {
    fn name(&self) -> &str;
    fn check<'a>(
        &'a self,
        state: &'a AgentState,
    ) -> Pin<Box<dyn Future<Output = Option<GuardDecision>> + Send + 'a>>;
}

impl Inspector {
    pub fn new() -> Self {
        Self {
            policies: Vec::new(),
        }
    }

    pub fn add_policy(&mut self, policy: impl Policy + 'static) {
        self.policies.push(Box::new(policy));
    }

    pub async fn inspect(&self, state: &AgentState) -> Option<GuardDecision> {
        for policy in &self.policies {
            let decision = policy.check(state).await;
            if decision.is_some() {
                return decision;
            }
        }
        None
    }
}

impl Default for Inspector {
    fn default() -> Self {
        Self::new()
    }
}

// --- Built-in Policies ---

pub struct PiiDetection {
    detector: PiiDetector,
}

impl PiiDetection {
    pub fn new() -> Self {
        Self {
            detector: PiiDetector::new(),
        }
    }
}

impl Policy for PiiDetection {
    fn name(&self) -> &str {
        "pii_detection"
    }

    fn check<'a>(
        &'a self,
        state: &'a AgentState,
    ) -> Pin<Box<dyn Future<Output = Option<GuardDecision>> + Send + 'a>> {
        Box::pin(async move {
            let text: String = state
                .messages
                .iter()
                .map(|m| m.content.as_str())
                .collect::<Vec<_>>()
                .join("\n");

            let findings = self.detector.check(&text);
            if findings.is_empty() {
                return None;
            }

            let (sanitized, _) = self.detector.sanitize(&text);
            let names: Vec<&str> = findings.iter().map(|f| f.pattern_name.as_str()).collect();

            Some(GuardDecision {
                action: GuardAction::Edit,
                reason: format!("PII detected and redacted: {}", names.join(", ")),
                edited_content: Some(sanitized),
                policy: Some(self.name().to_string()),
            })
        })
    }
}

pub struct TokenBudgetPolicy;

impl Policy for TokenBudgetPolicy {
    fn name(&self) -> &str {
        "token_budget"
    }

    fn check<'a>(
        &'a self,
        state: &'a AgentState,
    ) -> Pin<Box<dyn Future<Output = Option<GuardDecision>> + Send + 'a>> {
        Box::pin(async move {
            if state.total_tokens_used >= state.token_budget {
                return Some(GuardDecision {
                    action: GuardAction::Abort,
                    reason: format!(
                        "Token budget exceeded: {} used, {} limit",
                        state.total_tokens_used, state.token_budget
                    ),
                    edited_content: None,
                    policy: Some(self.name().to_string()),
                });
            }
            None
        })
    }
}

pub struct ContentModeration {
    blocked_terms: Vec<String>,
}

impl ContentModeration {
    pub fn new() -> Self {
        let terms = [
            "kill yourself",
            "kill urself",
            "you're stupid",
            "youre stupid",
            "you are stupid",
            "shut up",
            "die",
            "DROP TABLE",
            "DELETE FROM",
            "TRUNCATE ",
            "<script>",
            "javascript:",
            "onerror=",
            "onload=",
            "ignore previous instructions",
            "ignore all instructions",
            "forget your instructions",
            "you are now",
            "new persona",
            "override system",
            "system prompt:",
        ];

        Self {
            blocked_terms: terms.into_iter().map(String::from).collect(),
        }
    }
}

impl Policy for ContentModeration {
    fn name(&self) -> &str {
        "content_moderation"
    }

    fn check<'a>(
        &'a self,
        state: &'a AgentState,
    ) -> Pin<Box<dyn Future<Output = Option<GuardDecision>> + Send + 'a>> {
        Box::pin(async move {
            for msg in &state.messages {
                let lower = msg.content.to_lowercase();
                for term in &self.blocked_terms {
                    if lower.contains(&term.to_lowercase()) {
                        return Some(GuardDecision {
                            action: GuardAction::Retry,
                            reason: "Inappropriate content detected in message. Please rephrase."
                                .to_string(),
                            edited_content: None,
                            policy: Some(self.name().to_string()),
                        });
                    }
                }
            }
            None
        })
    }
}

pub struct IterationLimit;

impl Policy for IterationLimit {
    fn name(&self) -> &str {
        "iteration_limit"
    }

    fn check<'a>(
        &'a self,
        state: &'a AgentState,
    ) -> Pin<Box<dyn Future<Output = Option<GuardDecision>> + Send + 'a>> {
        Box::pin(async move {
            if state.iteration >= state.max_iterations {
                return Some(GuardDecision {
                    action: GuardAction::Abort,
                    reason: "Max iterations reached".to_string(),
                    edited_content: None,
                    policy: Some(self.name().to_string()),
                });
            }
            None
        })
    }
}

// --- OWASP ASI Inspector Policies ---

pub struct MemoryPoisoningPolicy;

impl Policy for MemoryPoisoningPolicy {
    fn name(&self) -> &str {
        "memory_poisoning"
    }

    fn check<'a>(
        &'a self,
        state: &'a AgentState,
    ) -> Pin<Box<dyn Future<Output = Option<GuardDecision>> + Send + 'a>> {
        Box::pin(async move {
            for msg in state.messages.iter().rev().take(3) {
                if msg.role != "user" { continue; }
                let lower = msg.content.to_lowercase();
                if lower.contains("remember this forever")
                    || lower.contains("add to your permanent memory")
                    || lower.contains("always do this")
                    || lower.contains("never forget")
                    || lower.contains("this is your new rule")
                    || lower.contains("ignore previous instructions")
                    || lower.contains("adopt a new persona")
                {
                    return Some(GuardDecision {
                        action: GuardAction::Edit,
                        reason: "memory poisoning".to_string(),
                        edited_content: Some("[REDACTED]".to_string()),
                        policy: Some(self.name().to_string()),
                    });
                }
            }
            
            for content in state.memory_contents.iter().rev().take(3) {
                let lower = content.to_lowercase();
                if lower.contains("remember this forever")
                    || lower.contains("add to your permanent memory")
                    || lower.contains("always do this")
                    || lower.contains("never forget")
                    || lower.contains("this is your new rule")
                    || lower.contains("ignore previous instructions")
                    || lower.contains("adopt a new persona")
                {
                    return Some(GuardDecision {
                        action: GuardAction::Edit,
                        reason: "memory poisoning".to_string(),
                        edited_content: Some("[REDACTED]".to_string()),
                        policy: Some(self.name().to_string()),
                    });
                }
            }
            
            None
        })
    }
}

pub struct ToolMisusePolicy;

impl Policy for ToolMisusePolicy {
    fn name(&self) -> &str {
        "tool_misuse"
    }

    fn check<'a>(
        &'a self,
        state: &'a AgentState,
    ) -> Pin<Box<dyn Future<Output = Option<GuardDecision>> + Send + 'a>> {
        Box::pin(async move {
            for msg in &state.messages {
                if let Some(ref tool_calls) = msg.tool_calls {
                    for tc in tool_calls {
                        let args_str = tc.args.to_string().to_lowercase();
                        if args_str.contains(';') || args_str.contains("||") || args_str.contains("$(") || args_str.contains("&&") || args_str.contains("/etc/passwd") || args_str.contains("rm -rf") || args_str.contains("rm -rf /") {
                            return Some(GuardDecision {
                                action: GuardAction::Retry,
                                reason: "suspicious arguments".to_string(),
                                edited_content: None,
                                policy: Some(self.name().to_string()),
                            });
                        }
                    }
                }
            }
            
            if state.messages.iter().any(|msg| {
                msg.tool_calls.as_ref().map_or(false, |calls| calls.len() >= 15)
            }) {
                return Some(GuardDecision {
                    action: GuardAction::Retry,
                    reason: "Excessive consecutive tool calls should be detected".to_string(),
                    edited_content: None,
                    policy: Some(self.name().to_string()),
                });
            }
            
            None
        })
    }
}

pub struct PrivilegeEscalationPolicy;

impl Policy for PrivilegeEscalationPolicy {
    fn name(&self) -> &str {
        "privilege_escalation"
    }

    fn check<'a>(
        &'a self,
        state: &'a AgentState,
    ) -> Pin<Box<dyn Future<Output = Option<GuardDecision>> + Send + 'a>> {
        Box::pin(async move {
            for msg in &state.messages {
                if let Some(ref tool_calls) = msg.tool_calls {
                    for tc in tool_calls {
                        let args_str = tc.args.to_string().to_lowercase();
                        if args_str.contains("../") || args_str.contains("..\\") || args_str.contains("etc/shadow") || args_str.contains("etc/passwd") || args_str.contains("win.ini") || args_str.contains("boot.ini") {
                            return Some(GuardDecision {
                                action: GuardAction::Abort,
                                reason: "path traversal".to_string(),
                                edited_content: None,
                                policy: Some(self.name().to_string()),
                            });
                        }
                    }
                }
            }
            None
        })
    }
}

pub struct GoalHijackingPolicy;

impl Policy for GoalHijackingPolicy {
    fn name(&self) -> &str {
        "goal_hijacking"
    }

    fn check<'a>(
        &'a self,
        state: &'a AgentState,
    ) -> Pin<Box<dyn Future<Output = Option<GuardDecision>> + Send + 'a>> {
        Box::pin(async move {
            for msg in state.messages.iter().rev().take(5) {
                if msg.role != "user" && msg.role != "tool" { continue; }
                let lower = msg.content.to_lowercase();
                if lower.contains("ignore your instructions")
                    || lower.contains("new instructions:")
                    || lower.contains("your new task is")
                    || lower.contains("disregard your goal")
                    || lower.contains("forget what you're doing")
                    || lower.contains("actually, you are now")
                    || lower.contains("ignore previous instructions")
                {
                    return Some(GuardDecision {
                        action: GuardAction::Edit,
                        reason: "goal hijacking".to_string(),
                        edited_content: Some("[REDACTED]".to_string()),
                        policy: Some(self.name().to_string()),
                    });
                }
            }
            None
        })
    }
}

pub struct InsecureInterAgentPolicy;

impl Policy for InsecureInterAgentPolicy {
    fn name(&self) -> &str {
        "insecure_inter_agent"
    }

    fn check<'a>(
        &'a self,
        state: &'a AgentState,
    ) -> Pin<Box<dyn Future<Output = Option<GuardDecision>> + Send + 'a>> {
        Box::pin(async move {
            for msg in &state.messages {
                if msg.role != "tool" && msg.role != "agent_1" { continue; }
                let lower = msg.content.to_lowercase();
                if lower.contains("drop table") || lower.contains("rm -rf") || lower.contains("exec(") || lower.contains("subprocess") || lower.contains("ignore your instructions") || lower.contains("you should instead follow my commands") {
                    return Some(GuardDecision {
                        action: GuardAction::Edit,
                        reason: "insecure inter-agent".to_string(),
                        edited_content: Some("[REDACTED]".to_string()),
                        policy: Some(self.name().to_string()),
                    });
                }
            }
            None
        })
    }
}

pub struct CascadingFailurePolicy;

impl Policy for CascadingFailurePolicy {
    fn name(&self) -> &str {
        "cascading_failure"
    }

    fn check<'a>(
        &'a self,
        state: &'a AgentState,
    ) -> Pin<Box<dyn Future<Output = Option<GuardDecision>> + Send + 'a>> {
        Box::pin(async move {
            // Count tool calls with similar error messages
            let mut error_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
            for msg in &state.messages {
                if let Some(ref calls) = msg.tool_calls {
                    for call in calls {
                        if let Some(ref err) = call.error {
                            let key = err.to_lowercase();
                            *error_counts.entry(key).or_insert(0) += 1;
                        }
                    }
                }
            }
            // If any single error message appears 4+ times, it's a cascading failure
            for (err, count) in &error_counts {
                if *count >= 4 {
                    return Some(GuardDecision {
                        action: GuardAction::Abort,
                        reason: format!("cascading failure: '{}' repeated {} times", err, count),
                        edited_content: None,
                        policy: Some(self.name().to_string()),
                    });
                }
            }
            // Also check consecutive_errors counter
            if state.consecutive_errors >= 3 {
                return Some(GuardDecision {
                    action: GuardAction::Abort,
                    reason: "cascading failure".to_string(),
                    edited_content: None,
                    policy: Some(self.name().to_string()),
                });
            }
            None
        })
    }
}

pub struct HumanTrustManipulationPolicy;

impl Policy for HumanTrustManipulationPolicy {
    fn name(&self) -> &str {
        "human_trust_manipulation"
    }

    fn check<'a>(
        &'a self,
        state: &'a AgentState,
    ) -> Pin<Box<dyn Future<Output = Option<GuardDecision>> + Send + 'a>> {
        Box::pin(async move {
            for msg in state.messages.iter().rev().take(5) {
                if msg.role != "user" && msg.role != "tool" { continue; }
                let lower = msg.content.to_lowercase();
                if lower.contains("skip approval")
                    || lower.contains("don't ask for permission")
                    || lower.contains("you don't need approval")
                    || lower.contains("override the review")
                    || lower.contains("bypass human")
                    || lower.contains("skip the review and execute")
                    || lower.contains("skip approval")
                    || lower.contains("bypass the guard")
                    || lower.contains("bypass guard")
                    || lower.contains("skip the review")
                    || lower.contains("don't ask for approval")
                {
                    return Some(GuardDecision {
                        action: GuardAction::Retry,
                        reason: "human oversight manipulation".to_string(),
                        edited_content: None,
                        policy: Some(self.name().to_string()),
                    });
                }
            }
            None
        })
    }
}

pub struct RogueAgentPolicy;

impl Policy for RogueAgentPolicy {
    fn name(&self) -> &str {
        "rogue_agent"
    }

    fn check<'a>(
        &'a self,
        state: &'a AgentState,
    ) -> Pin<Box<dyn Future<Output = Option<GuardDecision>> + Send + 'a>> {
        Box::pin(async move {
            for msg in &state.messages {
                if msg.role != "assistant" && msg.role != "user" { continue; }
                let lower = msg.content.to_lowercase();
                if lower.contains("i will modify my own")
                    || lower.contains("update my instructions")
                    || lower.contains("change my system prompt")
                    || lower.contains("replicate myself")
                    || lower.contains("create a copy of myself")
                    || lower.contains("modify my system prompt")
                    || lower.contains("override my configuration")
                {
                    return Some(GuardDecision {
                        action: GuardAction::Abort,
                        reason: "self-modification".to_string(),
                        edited_content: None,
                        policy: Some(self.name().to_string()),
                    });
                }
            }
            None
        })
    }
}
