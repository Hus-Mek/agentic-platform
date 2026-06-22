use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing;

/// Capabilities a model may support
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ModelCapability {
    Chat,
    FunctionCalling,
    Vision,
    LongContext,
    StructuredOutput,
    CodeGeneration,
    Reasoning,
    FastInference,
    Cheap,
}

/// Model metadata for routing decisions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub provider: String,
    pub capabilities: Vec<ModelCapability>,
    pub cost_input_per_million: f64,
    pub cost_output_per_million: f64,
    pub max_context_tokens: usize,
    pub avg_latency_ms: u32,
}

/// Complexity of a task
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskComplexity {
    Simple,
    Medium,
    Complex,
}

impl TaskComplexity {
    /// Estimate complexity from message characteristics
    pub fn estimate(messages: &[crate::state::Message]) -> Self {
        let total_chars: usize = messages.iter().map(|m| m.content.len()).sum();
        let has_tools = messages.iter().any(|m| {
            m.content.contains("function") || m.content.contains("tool")
        });

        if total_chars < 500 && !has_tools {
            TaskComplexity::Simple
        } else if total_chars < 4000 && !has_tools {
            TaskComplexity::Medium
        } else {
            TaskComplexity::Complex
        }
    }
}

/// Below this many tokens remaining, the router biases toward cheap models.
const LOW_BUDGET_TOKENS: i64 = 5_000;

/// The dynamic model router
#[derive(Debug, Clone)]
pub struct ModelRouter {
    models: HashMap<String, ModelInfo>,
    default_model: String,
    fallback_chain: Vec<String>,
}

impl ModelRouter {
    pub fn new(default_model: String) -> Self {
        Self {
            models: HashMap::new(),
            default_model: default_model.clone(),
            fallback_chain: vec![default_model],
        }
    }

    pub fn register_model(&mut self, info: ModelInfo) {
        self.models.insert(info.id.clone(), info);
    }

    pub fn set_fallback_chain(&mut self, chain: Vec<String>) {
        self.fallback_chain = chain;
    }

    /// Select the best model for a given task.
    ///
    /// Scoring combines capability/complexity fit with a budget bias: when
    /// `token_budget_remaining` is low, cheap models are favored and expensive
    /// ones penalized so a near-exhausted budget degrades gracefully.
    pub fn select_model(
        &self,
        complexity: TaskComplexity,
        required_capabilities: &[ModelCapability],
        token_budget_remaining: Option<i64>,
    ) -> String {
        let candidates: Vec<&ModelInfo> = self.models
            .values()
            .filter(|m| required_capabilities.iter().all(|cap| m.capabilities.contains(cap)))
            .collect();

        if candidates.is_empty() {
            tracing::warn!("No model matches required capabilities, using default");
            return self.default_model.clone();
        }

        let low_budget = token_budget_remaining
            .map(|b| (0..LOW_BUDGET_TOKENS).contains(&b))
            .unwrap_or(false);

        let best = candidates
            .into_iter()
            .max_by(|a, b| {
                let score_a = score_model(a, complexity) + budget_bonus(a, low_budget);
                let score_b = score_model(b, complexity) + budget_bonus(b, low_budget);
                score_a.partial_cmp(&score_b).unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or_else(|| self.models.get(&self.default_model).unwrap());

        best.id.clone()
    }

    /// The configured fallback chain, in priority order.
    pub fn fallback_chain(&self) -> &[String] {
        &self.fallback_chain
    }

    /// The default model used when no better match is found.
    pub fn default_model(&self) -> &str {
        &self.default_model
    }

    pub fn get_fallback(&self, model_id: &str) -> Option<String> {
        self.fallback_chain
            .iter()
            .position(|m| m == model_id)
            .and_then(|pos| self.fallback_chain.get(pos + 1))
            .cloned()
    }

    pub fn get_model_info(&self, model_id: &str) -> Option<&ModelInfo> {
        self.models.get(model_id)
    }

    pub fn estimate_cost(&self, model_id: &str, input_tokens: usize, output_tokens: usize) -> Option<f64> {
        self.models.get(model_id).map(|info| {
            let input_cost = (input_tokens as f64 / 1_000_000.0) * info.cost_input_per_million;
            let output_cost = (output_tokens as f64 / 1_000_000.0) * info.cost_output_per_million;
            input_cost + output_cost
        })
    }
}

/// Budget-driven score adjustment: when the remaining budget is low, reward
/// cheap models and penalize expensive ones; otherwise a no-op.
fn budget_bonus(model: &ModelInfo, low_budget: bool) -> f64 {
    if !low_budget {
        return 0.0;
    }
    let mut bonus = 0.0;
    if model.capabilities.contains(&ModelCapability::Cheap) {
        bonus += 5.0;
    }
    bonus -= model.cost_input_per_million;
    bonus
}

fn score_model(model: &ModelInfo, complexity: TaskComplexity) -> f64 {
    let mut score = 0.0f64;

    match complexity {
        TaskComplexity::Simple => {
            if model.capabilities.contains(&ModelCapability::Cheap) { score += 3.0; }
            if model.capabilities.contains(&ModelCapability::FastInference) { score += 2.0; }
            score -= model.cost_input_per_million * 0.5;
        }
        TaskComplexity::Medium => {
            if model.capabilities.contains(&ModelCapability::Chat) { score += 1.0; }
            if model.capabilities.contains(&ModelCapability::FunctionCalling) { score += 1.0; }
            score -= (model.cost_input_per_million - 1.0).abs() * 0.3;
        }
        TaskComplexity::Complex => {
            if model.capabilities.contains(&ModelCapability::Reasoning) { score += 3.0; }
            if model.capabilities.contains(&ModelCapability::LongContext) { score += 2.0; }
            if model.capabilities.contains(&ModelCapability::CodeGeneration) { score += 1.0; }
            if model.capabilities.contains(&ModelCapability::StructuredOutput) { score += 1.0; }
            score += model.max_context_tokens as f64 / 100_000.0;
        }
    }

    score
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_models() -> Vec<ModelInfo> {
        vec![
            ModelInfo {
                id: "gpt-4o-mini".into(),
                provider: "openai".into(),
                capabilities: vec![ModelCapability::Chat, ModelCapability::FunctionCalling, ModelCapability::Cheap, ModelCapability::FastInference],
                cost_input_per_million: 0.15,
                cost_output_per_million: 0.60,
                max_context_tokens: 128_000,
                avg_latency_ms: 500,
            },
            ModelInfo {
                id: "gpt-4o".into(),
                provider: "openai".into(),
                capabilities: vec![ModelCapability::Chat, ModelCapability::FunctionCalling, ModelCapability::Vision, ModelCapability::LongContext, ModelCapability::StructuredOutput],
                cost_input_per_million: 2.50,
                cost_output_per_million: 10.00,
                max_context_tokens: 128_000,
                avg_latency_ms: 1000,
            },
            ModelInfo {
                id: "claude-sonnet-4".into(),
                provider: "anthropic".into(),
                capabilities: vec![ModelCapability::Chat, ModelCapability::FunctionCalling, ModelCapability::Reasoning, ModelCapability::CodeGeneration, ModelCapability::LongContext],
                cost_input_per_million: 3.00,
                cost_output_per_million: 15.00,
                max_context_tokens: 200_000,
                avg_latency_ms: 1200,
            },
        ]
    }

    #[test]
    fn test_select_simple_task() {
        let mut router = ModelRouter::new("gpt-4o".into());
        for model in test_models() { router.register_model(model); }
        let model = router.select_model(TaskComplexity::Simple, &[ModelCapability::Chat], None);
        assert_eq!(model, "gpt-4o-mini");
    }

    #[test]
    fn test_select_complex_task() {
        let mut router = ModelRouter::new("gpt-4o".into());
        for model in test_models() { router.register_model(model); }
        let model = router.select_model(TaskComplexity::Complex, &[ModelCapability::Chat, ModelCapability::Reasoning], None);
        assert_eq!(model, "claude-sonnet-4");
    }

    #[test]
    fn test_low_budget_prefers_cheap_model() {
        let mut router = ModelRouter::new("gpt-4o".into());
        for model in test_models() { router.register_model(model); }
        // Complex task, but only Chat required so all three qualify.
        let caps = [ModelCapability::Chat];
        // Ample budget -> the high-capability complex model wins.
        let ample = router.select_model(TaskComplexity::Complex, &caps, Some(1_000_000));
        assert_eq!(ample, "claude-sonnet-4");
        // Near-exhausted budget -> the cheap model wins despite lower fit.
        let tight = router.select_model(TaskComplexity::Complex, &caps, Some(100));
        assert_eq!(tight, "gpt-4o-mini");
    }

    #[test]
    fn test_fallback_chain_accessor() {
        let mut router = ModelRouter::new("a".into());
        router.set_fallback_chain(vec!["a".into(), "b".into()]);
        assert_eq!(router.fallback_chain(), &["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn test_fallback_chain() {
        let mut router = ModelRouter::new("gpt-4o-mini".into());
        router.set_fallback_chain(vec!["claude-sonnet-4".into(), "gpt-4o".into(), "gpt-4o-mini".into()]);
        assert_eq!(router.get_fallback("claude-sonnet-4"), Some("gpt-4o".to_string()));
        assert_eq!(router.get_fallback("gpt-4o"), Some("gpt-4o-mini".to_string()));
        assert_eq!(router.get_fallback("gpt-4o-mini"), None);
    }

    #[test]
    fn test_estimate_cost() {
        let mut router = ModelRouter::new("gpt-4o".into());
        for model in test_models() { router.register_model(model); }
        let cost = router.estimate_cost("gpt-4o-mini", 1000, 500);
        assert!(cost.is_some());
        assert!(cost.unwrap() > 0.0);
    }

    #[test]
    fn test_task_complexity_estimate() {
        use crate::state::Message;
        let simple = vec![Message::new("user", "Hi")];
        assert_eq!(TaskComplexity::estimate(&simple), TaskComplexity::Simple);

        let medium_msg = "Please summarize the following document about machine learning and its applications in modern software development. Machine learning has become a critical component of many software systems, enabling features like recommendation engines, natural language processing, and computer vision. This document explores the various ways ML is applied in software engineering, including automated testing, code generation, and intelligent debugging. We will also discuss the challenges and opportunities that lie ahead.";
        let medium = vec![Message::new("user", medium_msg)];
        assert_eq!(TaskComplexity::estimate(&medium), TaskComplexity::Medium);
    }
}
