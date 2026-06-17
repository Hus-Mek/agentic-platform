use serde::{Deserialize, Serialize};
use tracing;
use uuid::Uuid;

use agenticos_core::metrics::{EvalHarness, BenchmarkSuite, EvalResult, MetricScore};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationTarget {
    pub agent_id: String,
    pub current_prompt: String,
    pub current_model: String,
    pub current_tools: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub task_id: String,
    pub score: f64,
    pub passed: bool,
}

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

pub struct Evolver {
    harness: EvalHarness,
    iterations: Vec<OptimizationIteration>,
    improvement_threshold: f64,
}

impl Evolver {
    pub fn new(harness: EvalHarness) -> Self {
        Self {
            harness,
            iterations: Vec::new(),
            improvement_threshold: 0.05,
        }
    }

    pub fn optimize(
        &mut self,
        target: &OptimizationTarget,
        suite: &BenchmarkSuite,
    ) -> Result<OptimizationIteration, EvolverError> {
        let before = self.benchmark_with_prompt(&target.agent_id, &target.current_prompt, &target.current_model, suite)?;
        let (candidate_prompt, candidate_model) = self.generate_candidate(target, &before)?;
        let after = self.benchmark_with_prompt(&target.agent_id, &candidate_prompt, &candidate_model, suite)?;

        let before_avg = average_score(&before);
        let after_avg = average_score(&after);
        let improvement = after_avg - before_avg;
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

    fn benchmark_with_prompt(
        &self,
        _agent_id: &str,
        _prompt: &str,
        _model: &str,
        suite: &BenchmarkSuite,
    ) -> Result<Vec<BenchmarkResult>, EvolverError> {
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

    fn generate_candidate(
        &self,
        target: &OptimizationTarget,
        results: &[BenchmarkResult],
    ) -> Result<(String, String), EvolverError> {
        let failed_tasks: Vec<&BenchmarkResult> = results.iter().filter(|r| !r.passed).collect();

        if failed_tasks.is_empty() {
            return Ok((target.current_prompt.clone(), "gpt-4o-mini".into()));
        }

        let failed_ids: Vec<&str> = failed_tasks.iter().map(|r| r.task_id.as_str()).collect();
        let improved_prompt = format!(
            "{}\n\n## Focus Areas\nPay special attention to: {}\nEnsure your responses are complete and accurate.",
            target.current_prompt,
            failed_ids.join(", ")
        );

        Ok((improved_prompt, target.current_model.clone()))
    }

    pub fn iterations(&self) -> &[OptimizationIteration] {
        &self.iterations
    }

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
#[error("Evolver error: {0}")]
pub struct EvolverError(pub String);

#[cfg(test)]
mod tests {
    use super::*;

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
}