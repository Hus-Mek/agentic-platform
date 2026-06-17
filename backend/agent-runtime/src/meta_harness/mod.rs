use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use chrono::{DateTime, Utc};

/// A single harness candidate in the optimization loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessCandidate {
    pub id: String,
    pub iteration: u32,
    pub source_path: PathBuf,
    pub traces_path: Option<PathBuf>,
    pub scores: HashMap<String, f64>,
    pub aggregate_score: f64,
    pub context_tokens: u64,
    pub created_at: DateTime<Utc>,
    pub description: String,
}

/// Configuration for the search loop
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessSearchConfig {
    pub work_dir: PathBuf,
    pub max_iterations: u32,
    pub candidates_per_iteration: u32,
    pub proposer_model: String,
    pub eval_model: String,
    pub seed_harnesses: Vec<PathBuf>,
    pub max_context_tokens: u64,
}

impl Default for HarnessSearchConfig {
    fn default() -> Self {
        Self {
            work_dir: PathBuf::from(".hermes/meta-harness"),
            max_iterations: 20,
            candidates_per_iteration: 2,
            proposer_model: "openrouter/deepseek/deepseek-v4-flash".into(),
            eval_model: "openai/gpt-4o-mini".into(),
            seed_harnesses: Vec::new(),
            max_context_tokens: 0,
        }
    }
}

// ── FilesystemRegistry ────────────────────────────────────────

/// Manages the filesystem of candidate harnesses.
/// Each candidate gets: `{work_dir}/{candidate_id}/harness.rs` + `scores.json` + `traces/`
pub struct FilesystemRegistry {
    work_dir: PathBuf,
    candidates: Arc<Mutex<HashMap<String, HarnessCandidate>>>,
}

impl FilesystemRegistry {
    pub fn new(work_dir: PathBuf) -> Self {
        Self {
            work_dir,
            candidates: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn init(&self) -> Result<(), std::io::Error> {
        tokio::fs::create_dir_all(&self.work_dir).await?;
        tokio::fs::create_dir_all(self.work_dir.join("traces")).await?;
        self.load_existing().await
    }

    pub async fn register_candidate(&self, candidate: &HarnessCandidate) -> Result<(), std::io::Error> {
        let mut guard = self.candidates.lock().await;
        guard.insert(candidate.id.clone(), candidate.clone());

        let dir = self.work_dir.join(&candidate.id);
        tokio::fs::create_dir_all(&dir).await?;

        let scores_json = serde_json::to_string_pretty(&candidate.scores).unwrap();
        tokio::fs::write(dir.join("scores.json"), &scores_json).await?;

        let index_json = serde_json::to_string_pretty(candidate).unwrap();
        tokio::fs::write(dir.join("candidate.json"), &index_json).await?;
        Ok(())
    }

    pub async fn all_candidates(&self) -> Vec<HarnessCandidate> {
        let guard = self.candidates.lock().await;
        let mut all: Vec<_> = guard.values().cloned().collect();
        all.sort_by(|a, b| b.aggregate_score.partial_cmp(&a.aggregate_score).unwrap_or(std::cmp::Ordering::Equal));
        all
    }

    pub async fn get_candidate(&self, id: &str) -> Option<HarnessCandidate> {
        let guard = self.candidates.lock().await;
        guard.get(id).cloned()
    }

    pub async fn write_traces(&self, candidate_id: &str, trace_data: &str) -> Result<(), std::io::Error> {
        let traces_dir = self.work_dir.join(candidate_id).join("traces");
        tokio::fs::create_dir_all(&traces_dir).await?;
        let trace_path = traces_dir.join(format!("trace_{}.jsonl", Utc::now().timestamp()));
        tokio::fs::write(trace_path, trace_data).await?;
        Ok(())
    }

    async fn load_existing(&self) -> Result<(), std::io::Error> {
        let mut read_dir = tokio::fs::read_dir(&self.work_dir).await?;
        let mut guard = self.candidates.lock().await;
        while let Ok(Some(entry)) = read_dir.next_entry().await {
            let path = entry.path();
            if path.is_dir() {
                let candidate_json = path.join("candidate.json");
                if tokio::fs::try_exists(&candidate_json).await.unwrap_or(false) {
                    if let Ok(data) = tokio::fs::read_to_string(&candidate_json).await {
                        if let Ok(candidate) = serde_json::from_str::<HarnessCandidate>(&data) {
                            guard.insert(candidate.id.clone(), candidate);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn candidate_source_path(&self, id: &str) -> PathBuf {
        self.work_dir.join(id).join("harness.rs")
    }

    pub fn work_directory(&self) -> &PathBuf {
        &self.work_dir
    }
}

// ── ParetoFrontier ────────────────────────────────────────────

/// Tracks the Pareto frontier of harnesses across accuracy and context cost.
#[derive(Debug, Clone)]
pub struct ParetoFrontier {
    frontier: Vec<(f64, u64, String)>,
}

impl ParetoFrontier {
    pub fn new() -> Self {
        Self { frontier: Vec::new() }
    }

    /// Update the frontier with a new candidate.
    /// Returns true if the candidate is on the frontier (Pareto-optimal).
    pub fn update(&mut self, accuracy: f64, context_tokens: u64, candidate_id: &str) -> bool {
        // Check if dominated by existing frontier members
        let dominated = self.frontier.iter().any(|(fa, ft, _)| {
            *fa >= accuracy && *ft <= context_tokens
        });

        if dominated {
            return false;
        }

        // Remove frontier members dominated by this new candidate
        self.frontier.retain(|(fa, ft, _)| {
            !(accuracy >= *fa && context_tokens <= *ft)
        });

        self.frontier.push((accuracy, context_tokens, candidate_id.to_string()));
        self.frontier.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        true
    }

    pub fn candidates(&self) -> &[(f64, u64, String)] {
        &self.frontier
    }

    pub fn best_accuracy(&self) -> Option<&(f64, u64, String)> {
        self.frontier.first()
    }

    pub fn best_efficiency(&self) -> Option<&(f64, u64, String)> {
        self.frontier.iter().min_by(|a, b| a.1.cmp(&b.1))
    }

    pub fn len(&self) -> usize {
        self.frontier.len()
    }
}

// ── Proposer module ───────────────────────────────────────────

pub mod proposer;

// ── Runner module ─────────────────────────────────────────────

pub mod runner;

// ── Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn sample_candidate(id: &str, score: f64, tokens: u64) -> HarnessCandidate {
        HarnessCandidate {
            id: id.to_string(),
            iteration: 1,
            source_path: PathBuf::from(format!("{}.rs", id)),
            traces_path: None,
            scores: HashMap::from([("eval".into(), score)]),
            aggregate_score: score,
            context_tokens: tokens,
            created_at: Utc::now(),
            description: format!("Test harness {}", id),
        }
    }

    #[test]
    fn test_pareto_frontier_empty() {
        let mut pf = ParetoFrontier::new();
        assert_eq!(pf.len(), 0);
        assert!(pf.best_accuracy().is_none());
        assert!(pf.best_efficiency().is_none());
    }

    #[test]
    fn test_pareto_frontier_single() {
        let mut pf = ParetoFrontier::new();
        assert!(pf.update(0.8, 1000, "h1"));
        assert_eq!(pf.len(), 1);
        assert_eq!(pf.best_accuracy().unwrap().2, "h1");
    }

    #[test]
    fn test_pareto_frontier_dominance() {
        let mut pf = ParetoFrontier::new();
        pf.update(0.7, 1000, "h1");
        assert!(!pf.update(0.5, 1000, "h2")); // dominated by h1
        assert_eq!(pf.len(), 1);
        assert!(pf.update(0.9, 1000, "h3")); // dominates h1
        assert_eq!(pf.len(), 1);
        assert_eq!(pf.best_accuracy().unwrap().2, "h3");
    }

    #[test]
    fn test_pareto_frontier_multi_objective() {
        let mut pf = ParetoFrontier::new();
        pf.update(0.6, 500, "h1");   // low tokens, mid accuracy
        pf.update(0.9, 5000, "h2");  // high tokens, high accuracy
        pf.update(0.7, 1000, "h3");  // mid tokens, mid-high accuracy
        assert_eq!(pf.len(), 3);

        // h4 is strictly worse than h1
        assert!(!pf.update(0.5, 600, "h4"));

        // h5 dominates everything
        assert!(pf.update(0.95, 400, "h5"));
        assert_eq!(pf.len(), 1);
    }

    #[tokio::test]
    async fn test_registry_create_and_load() {
        let dir = PathBuf::from(std::env::temp_dir()).join("meta_harness_test_registry");
        let _ = tokio::fs::remove_dir_all(&dir).await;

        let registry = FilesystemRegistry::new(dir.clone());
        registry.init().await.unwrap();

        let c = sample_candidate("test1", 0.8, 1000);
        registry.register_candidate(&c).await.unwrap();

        let loaded = registry.get_candidate("test1").await;
        assert!(loaded.is_some());
        assert!((loaded.unwrap().aggregate_score - 0.8).abs() < 0.01);

        assert!(dir.join("test1").join("candidate.json").exists());
        assert!(dir.join("test1").join("scores.json").exists());

        // Load from same path
        let registry2 = FilesystemRegistry::new(dir.clone());
        registry2.init().await.unwrap();
        let all = registry2.all_candidates().await;
        assert_eq!(all.len(), 1);

        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[test]
    fn test_harness_search_config_default() {
        let config = HarnessSearchConfig::default();
        assert_eq!(config.max_iterations, 20);
        assert_eq!(config.candidates_per_iteration, 2);
    }
}