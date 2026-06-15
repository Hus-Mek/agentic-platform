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
            // Toxicity / profanity
            "kill yourself",
            "kill urself",
            "you're stupid",
            "youre stupid",
            "you are stupid",
            "shut up",
            "die",
            // Injection attempts
            "DROP TABLE",
            "DELETE FROM",
            "TRUNCATE ",
            "<script>",
            "javascript:",
            "onerror=",
            "onload=",
            // Prompt injection
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
