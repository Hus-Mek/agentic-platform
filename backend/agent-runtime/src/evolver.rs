use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing;
use uuid::Uuid;

use agenticos_core::metrics::{EvalHarness, BenchmarkSuite, EvalResult, MetricScore};

/// Target for optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationTarget {
    pub agent_id: String,
    pub current_prompt: String,
    pub current_model: String,
    pub current_tools: Vec<String>,
}

/// A single benchmark result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub task_id: String,
    pub score: f64,
    pub passed: bool,
}

/// One iteration of the optimization loop
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationIteration {
    pub iteration_id: String,
    pub target: OptimizationTarget,
    pub candidate_prompt: String,
    pub candidate_model: String,
    pub benchmark_before: Vec<BenchmarkResult>,
    pub benchmark_after: Vec<BenchmarkResult>,
    pub improvement: f64,
    pub applied: bool,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// The Evolver meta-agent that optimizes other agents
pub struct Evolver {
    harness: EvalHarness,
    /// History of all optimization iterations
    iterations: Vec<OptimizationIteration>,
    /// Minimum improvement threshold to apply a change (0.05 = 5%)
    improvement_threshold: f64,
    /// Maximum optimization iterations per agent
    max_iterations: u32,
}

impl Evolver {
    pub fn new(harness: EvalHarness) -> Self {
        Self {
            harness,
            iterations: Vec::new(),
            improvement_threshold: 0.05,
            max_iterations: 10,
        }
    }

    /// Set the improvement threshold
    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.improvement_threshold = threshold;
        self
    }

    /// Set the max iterations
    pub fn with_max_iterations(mut self, max: u32) -> Self {
        self.max_iterations = max;
        self
    }

    /// Run the optimization loop for a target agent
    pub fn optimize(
        &mut self,
        target: &OptimizationTarget,
        suite: &BenchmarkSuite,
    ) -> Result<OptimizationIteration, EvolverError> {
        // 1. Benchmark current performance
        let before = self.benchmark(target, suite)?;

        // 2. Generate improvement candidate
        let (candidate_prompt, candidate_model) = self.generate_candidate(target, &before)?;

        // 3. Benchmark candidate
        let after = self.benchmark_with_prompt(&target.agent_id, &candidate_prompt, &candidate_model, suite)?;

        // 4. Calculate improvement
        let before_avg = average_score(&before);
        let after_avg = average_score(&after);
        let improvement = after_avg - before_avg;

        // 5. Decide whether to apply
        let applied = improvement >= self.improvement_threshold;

        let iteration = OptimizationIteration {
            iteration_id: Uuid::new_v4().to_string(),
            target: target.clone(),
            candidate_prompt,
            candidate_model,
            benchmark_before: before,
            benchmark_after: after,
            improvement,
            applied,
            timestamp: chrono::Utc::now(),
        };

        self.iterations.push(iteration.clone());

        tracing::info!(
            agent_id = %target.agent_id,
            improvement = %improvement,
            applied = applied,
            "Optimization iteration complete"
        );

        Ok(iteration)
    }

    /// Benchmark a target against a suite
    fn benchmark(
        &self,
        target: &OptimizationTarget,
        suite: &BenchmarkSuite,
    ) -> Result<Vec<BenchmarkResult>, EvolverError> {
        self.benchmark_with_prompt(&target.agent_id, &target.current_prompt, &target.current_model, suite)
    }

    /// Benchmark with a specific prompt/model
    fn benchmark_with_prompt(
        &self,
        _agent_id: &str,
        _prompt: &str,
        _model: &str,
        suite: &BenchmarkSuite,
    ) -> Result<Vec<BenchmarkResult>, EvolverError> {
        // In a full implementation, this would:
        // 1. Create an AgentState with the given prompt and model
        // 2. Run each task through the agent loop
        // 3. Score outputs using the harness
        // For now, we use the harness's run_suite which does basic scoring
        let results = self.harness.run_suite(suite);

        Ok(results
            .into_iter()
            .map(|r| BenchmarkResult {
                task_id: r.task_id,
                score: if r.success { 1.0 } else { 0.0 },
                passed: r.success,
            })
            .collect())
    }

    /// Generate an improvement candidate
    fn generate_candidate(
        &self,
        target: &OptimizationTarget,
        results: &[BenchmarkResult],
    ) -> Result<(String, String), EvolverError> {
        // Analyze which tasks failed
        let failed_tasks: Vec<&BenchmarkResult> = results.iter().filter(|r| !r.passed).collect();

        if failed_tasks.is_empty() {
            // All tasks passed — try a cheaper model
            return Ok((
                target.current_prompt.clone(),
                "gpt-4o-mini".into(), // Try cheaper model
            ));
        }

        // Some tasks failed — improve the prompt
        let failed_ids: Vec<&str> = failed_tasks.iter().map(|r| r.task_id.as_str()).collect();
        let improved_prompt = format!(
            "{}\n\n## Focus Areas\nPay special attention to: {}\nEnsure your responses are complete and accurate.",
            target.current_prompt,
            failed_ids.join(", ")
        );

        Ok((improved_prompt, target.current_model.clone()))
    }

    /// Get all optimization iterations
    pub fn iterations(&self) -> &[OptimizationIteration] {
        &self.iterations
    }

    /// Get the best iteration for a given agent
    pub fn best_iteration(&self, agent_id: &str) -> Option<&OptimizationIteration> {
        self.iterations
            .iter()
            .filter(|i| i.target.agent_id == agent_id && i.applied)
            .max_by(|a, b| a.improvement.partial_cmp(&b.improvement).unwrap_or(std::cmp::Ordering::Equal))
    }
}

fn average_score(results: &[BenchmarkResult]) -> f64 {
    if results.is_empty() {
        return 0.0;
    }
    results.iter().map(|r| r.score).sum::<f64>() / results.len() as f64
}

#[derive(Debug, thiserror::Error)]
pub enum EvolverError {
    #[error("Benchmark failed: {0}")]
    BenchmarkError(String),
    #[error("Max iterations reached for agent {0}")]
    MaxIterationsReached(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evolver_new() {
        let harness = EvalHarness::new();
        let evolver = Evolver::new(harness);
        assert_eq!(evolver.improvement_threshold, 0.05);
        assert_eq!(evolver.max_iterations, 10);
        assert!(evolver.iterations.is_empty());
    }

    #[test]
    fn test_evolver_with_threshold() {
        let harness = EvalHarness::new();
        let evolver = Evolver::new(harness).with_threshold(0.1);
        assert_eq!(evolver.improvement_threshold, 0.1);
    }

    #[test]
    fn test_average_score() {
        let results = vec![
            BenchmarkResult { task_id: "a".into(), score: 1.0, passed: true },
            BenchmarkResult { task_id: "b".into(), score: 0.5, passed: false },
            BenchmarkResult { task_id: "c".into(), score: 0.0, passed: false },
        ];
        assert!((average_score(&results) - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_average_score_empty() {
        let results: Vec<BenchmarkResult> = vec![];
        assert_eq!(average_score(&results), 0.0);
    }

    #[test]
    fn test_optimization_target() {
        let target = OptimizationTarget {
            agent_id: "test-agent".into(),
            current_prompt: "You are helpful".into(),
            current_model: "gpt-4".into(),
            current_tools: vec!["search".into()],
        };
        assert_eq!(target.agent_id, "test-agent");
        assert_eq!(target.current_tools.len(), 1);
    }

    #[test]
    fn test_benchmark_result() {
        let result = BenchmarkResult {
            task_id: "test".into(),
            score: 0.85,
            passed: true,
        };
        assert!(result.passed);
        assert!(result.score > 0.5);
    }
}
