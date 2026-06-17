use std::sync::Arc;
use tracing;
use uuid::Uuid;
use chrono::Utc;

use crate::meta_harness::{
    HarnessCandidate, HarnessSearchConfig, FilesystemRegistry, ParetoFrontier,
    proposer::{Proposer},
};

/// The core Meta-Harness outer loop.
/// Manages filesystem registry, proposer, evaluation, and Pareto frontier.
pub struct MetaHarnessRunner {
    config: HarnessSearchConfig,
    registry: FilesystemRegistry,
    proposer: Arc<dyn Proposer>,
    frontier: ParetoFrontier,
}

impl MetaHarnessRunner {
    pub fn new(config: HarnessSearchConfig, registry: FilesystemRegistry, proposer: Arc<dyn Proposer>) -> Self {
        Self { config, registry, proposer, frontier: ParetoFrontier::new() }
    }

    /// Initialize: create work directory, load seed harnesses.
    pub async fn init(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.registry.init().await?;
        tracing::info!("MetaHarness initialized at {:?}", self.config.work_dir);

        for seed_path in &self.config.seed_harnesses {
            if seed_path.exists() {
                let source = tokio::fs::read_to_string(seed_path).await?;
                let id = format!("seed_{}", Uuid::new_v4());
                let candidate = HarnessCandidate {
                    id: id.clone(),
                    iteration: 0,
                    source_path: seed_path.clone(),
                    traces_path: None,
                    scores: std::collections::HashMap::new(),
                    aggregate_score: 0.0,
                    context_tokens: 0,
                    created_at: Utc::now(),
                    description: format!("Seed: {}", seed_path.display()),
                };
                self.registry.register_candidate(&candidate).await?;
                let target = self.registry.candidate_source_path(&id);
                tokio::fs::write(&target, &source).await?;
                tracing::info!("Registered seed: {}", id);
            }
        }
        Ok(())
    }

    /// Run one iteration: propose → register → frontier update
    pub async fn run_iteration(&mut self, iteration: u32) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!("=== Iteration {} / {}", iteration, self.config.max_iterations);

        let candidates = self.registry.all_candidates().await;

        // Propose new harnesses
        tracing::info!("Proposing...");
        let proposal = self.proposer.propose(&self.config, &candidates, self.registry.work_directory())
            .await
            .map_err(|e| format!("Proposer: {e}"))?;

        // Register each candidate
        for (i, cp) in proposal.descriptions.iter().enumerate() {
            let id = format!("iter{}_{}", iteration, i);
            let source_path = self.registry.candidate_source_path(&id);

            // Create candidate dir, write source, then register
            if let Some(parent) = source_path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::write(&source_path, &cp.source_code).await?;

            // Placeholder scoring (0.5). In production, run eval harness here.
            let agg = 0.5;
            let candidate = HarnessCandidate {
                id: id.clone(),
                iteration,
                source_path,
                traces_path: None,
                scores: std::collections::HashMap::from([("placeholder".into(), agg)]),
                aggregate_score: agg,
                context_tokens: 1000,
                created_at: Utc::now(),
                description: cp.description.clone(),
            };

            self.registry.register_candidate(&candidate).await?;

            let on_frontier = self.frontier.update(agg, 1000, &id);
            tracing::info!("  {}: {} (frontier={})", id, cp.description, on_frontier);
        }

        tracing::info!("Frontier: {} candidates", self.frontier.len());
        Ok(())
    }

    /// Run the full search loop
    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.init().await?;
        for i in 1..=self.config.max_iterations {
            self.run_iteration(i).await?;
        }
        tracing::info!("Search complete. Frontier: {} candidates", self.frontier.len());
        if let Some(best) = self.frontier.best_accuracy() {
            tracing::info!("Best: {} (score={}, tokens={})", best.2, best.0, best.1);
        }
        Ok(())
    }

    pub fn frontier(&self) -> &ParetoFrontier {
        &self.frontier
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta_harness::proposer::MockProposer;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_runner_initializes() {
        let dir = std::env::temp_dir().join("meta_harness_test_runner");
        let _ = tokio::fs::remove_dir_all(&dir).await;

        let config = HarnessSearchConfig {
            work_dir: dir.clone(),
            max_iterations: 1,
            ..Default::default()
        };
        let registry = FilesystemRegistry::new(dir.clone());
        let proposer = Arc::new(MockProposer::simple(vec!["test harness"]));
        let mut runner = MetaHarnessRunner::new(config, registry, proposer);

        runner.init().await.unwrap();
        assert!(dir.exists());

        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn test_runner_one_iteration() {
        let dir = std::env::temp_dir().join("meta_harness_test_iter");
        let _ = tokio::fs::remove_dir_all(&dir).await;

        let config = HarnessSearchConfig {
            work_dir: dir.clone(),
            max_iterations: 1,
            ..Default::default()
        };
        let registry = FilesystemRegistry::new(dir.clone());
        let proposer = Arc::new(MockProposer::simple(vec!["iter0 harness"]));
        let mut runner = MetaHarnessRunner::new(config, registry, proposer);

        runner.run().await.unwrap();

        let candidates = runner.registry.all_candidates().await;
        assert!(!candidates.is_empty(), "Expected at least one candidate");

        let _ = tokio::fs::remove_dir_all(&dir).await;
    }
}