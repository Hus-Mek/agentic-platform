use serde::{Deserialize, Serialize};

/// Result of a metric evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricScore {
    pub metric_name: String,
    pub score: f64,
    pub details: Option<String>,
}

/// Trait for evaluation metrics
pub trait Metric: Send + Sync {
    fn name(&self) -> &str;
    fn score(&self, hypothesis: &str, reference: &str) -> MetricScore;
}

/// BLEU-1 score metric (unigram precision with brevity penalty)
pub struct BleuMetric;

impl Metric for BleuMetric {
    fn name(&self) -> &str { "bleu" }

    fn score(&self, hypothesis: &str, reference: &str) -> MetricScore {
        let hyp_tokens: Vec<&str> = hypothesis.split_whitespace().collect();
        let ref_tokens: Vec<&str> = reference.split_whitespace().collect();

        if hyp_tokens.is_empty() {
            return MetricScore {
                metric_name: "bleu".into(),
                score: 0.0,
                details: Some("Empty hypothesis".into()),
            };
        }

        let matches = hyp_tokens.iter().filter(|t| ref_tokens.contains(t)).count();
        let precision = matches as f64 / hyp_tokens.len() as f64;

        let bp = if hyp_tokens.len() >= ref_tokens.len() {
            1.0
        } else {
            (1.0 - ref_tokens.len() as f64 / hyp_tokens.len() as f64).exp()
        };

        MetricScore {
            metric_name: "bleu".into(),
            score: (bp * precision).min(1.0).max(0.0),
            details: Some(format!("precision={:.3}, bp={:.3}", precision, bp)),
        }
    }
}

/// CHRF score metric (character bigram F-score)
pub struct ChrfMetric;

impl Metric for ChrfMetric {
    fn name(&self) -> &str { "chrf" }

    fn score(&self, hypothesis: &str, reference: &str) -> MetricScore {
        let hyp_bigrams: Vec<String> = hypothesis
            .chars()
            .collect::<Vec<_>>()
            .windows(2)
            .map(|w| format!("{}{}", w[0], w[1]))
            .collect();

        let ref_bigrams: Vec<String> = reference
            .chars()
            .collect::<Vec<_>>()
            .windows(2)
            .map(|w| format!("{}{}", w[0], w[1]))
            .collect();

        if hyp_bigrams.is_empty() || ref_bigrams.is_empty() {
            return MetricScore {
                metric_name: "chrf".into(),
                score: 0.0,
                details: Some("Empty input".into()),
            };
        }

        let recall = hyp_bigrams.iter().filter(|bg| ref_bigrams.contains(bg)).count() as f64
            / ref_bigrams.len() as f64;
        let precision = hyp_bigrams.iter().filter(|bg| ref_bigrams.contains(bg)).count() as f64
            / hyp_bigrams.len() as f64;

        let f_score = if precision + recall > 0.0 {
            2.0 * precision * recall / (precision + recall)
        } else {
            0.0
        };

        MetricScore {
            metric_name: "chrf".into(),
            score: f_score.min(1.0).max(0.0),
            details: Some(format!("precision={:.3}, recall={:.3}", precision, recall)),
        }
    }
}

/// Exact match metric
pub struct ExactMatchMetric;

impl Metric for ExactMatchMetric {
    fn name(&self) -> &str { "exact_match" }

    fn score(&self, hypothesis: &str, reference: &str) -> MetricScore {
        let score = if hypothesis.trim() == reference.trim() { 1.0 } else { 0.0 };
        MetricScore { metric_name: "exact_match".into(), score, details: None }
    }
}

/// Contains match metric (substring match)
pub struct ContainsMatchMetric;

impl Metric for ContainsMatchMetric {
    fn name(&self) -> &str { "contains_match" }

    fn score(&self, hypothesis: &str, reference: &str) -> MetricScore {
        let score = if hypothesis.to_lowercase().contains(&reference.to_lowercase()) { 1.0 } else { 0.0 };
        MetricScore { metric_name: "contains_match".into(), score, details: None }
    }
}

/// An evaluation task
#[derive(Debug, Clone)]
pub struct EvalTask {
    pub id: String,
    pub description: String,
    pub expected_output: String,
    pub metrics: Vec<String>,
    pub max_iterations: u32,
}

/// Result of running an eval task
#[derive(Debug, Clone)]
pub struct EvalResult {
    pub task_id: String,
    pub success: bool,
    pub scores: Vec<MetricScore>,
    pub tokens_used: i64,
    pub iterations: u32,
    pub error: Option<String>,
}

/// A benchmark suite
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSuite {
    pub name: String,
    pub tasks: Vec<EvalTaskRequest>,
    pub metrics: Vec<String>,
}

/// Request to create an eval task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalTaskRequest {
    pub id: String,
    pub description: String,
    pub expected_output: String,
    pub metrics: Vec<String>,
    pub max_iterations: u32,
}

impl BenchmarkSuite {
    /// Standard benchmark suite for agent evaluation
    pub fn standard() -> Self {
        Self {
            name: "agentic-os-standard".into(),
            tasks: vec![
                EvalTaskRequest {
                    id: "basic_echo".into(),
                    description: "Echo back the text: hello world".into(),
                    expected_output: "hello world".into(),
                    metrics: vec!["exact_match".into(), "contains_match".into()],
                    max_iterations: 3,
                },
                EvalTaskRequest {
                    id: "translation_es".into(),
                    description: "Translate to Spanish: Good morning".into(),
                    expected_output: "Buenos días".into(),
                    metrics: vec!["bleu".into(), "chrf".into(), "contains_match".into()],
                    max_iterations: 5,
                },
                EvalTaskRequest {
                    id: "contains_check".into(),
                    description: "What is the capital of France?".into(),
                    expected_output: "Paris".into(),
                    metrics: vec!["contains_match".into()],
                    max_iterations: 5,
                },
            ],
            metrics: vec![
                "bleu".into(),
                "chrf".into(),
                "exact_match".into(),
                "contains_match".into(),
            ],
        }
    }
}

/// The enhanced evaluation harness
pub struct EvalHarness {
    metrics: std::collections::HashMap<String, Box<dyn Metric>>,
}

impl EvalHarness {
    pub fn new() -> Self {
        let mut metrics: std::collections::HashMap<String, Box<dyn Metric>> = std::collections::HashMap::new();
        metrics.insert("bleu".into(), Box::new(BleuMetric));
        metrics.insert("chrf".into(), Box::new(ChrfMetric));
        metrics.insert("exact_match".into(), Box::new(ExactMatchMetric));
        metrics.insert("contains_match".into(), Box::new(ContainsMatchMetric));
        Self { metrics }
    }

    pub fn add_metric(&mut self, name: &str, metric: Box<dyn Metric>) {
        self.metrics.insert(name.into(), metric);
    }

    /// Score a hypothesis against a reference using the specified metrics
    pub fn score(&self, hypothesis: &str, reference: &str, metric_names: &[String]) -> Vec<MetricScore> {
        metric_names
            .iter()
            .filter_map(|name| self.metrics.get(name))
            .map(|metric| metric.score(hypothesis, reference))
            .collect()
    }

    /// Run a single eval task (simplified — just scores the expected output)
    pub fn run_task(&self, task: &EvalTask) -> EvalResult {
        // In a full implementation, this would run the agent loop
        // For now, we score the expected output against itself (sanity check)
        let scores = self.score(&task.expected_output, &task.expected_output, &task.metrics);
        let success = scores.iter().all(|s| s.score > 0.5);

        EvalResult {
            task_id: task.id.clone(),
            success,
            scores,
            tokens_used: 0,
            iterations: 1,
            error: None,
        }
    }

    /// Run a full benchmark suite
    pub fn run_suite(&self, suite: &BenchmarkSuite) -> Vec<EvalResult> {
        suite.tasks
            .iter()
            .map(|task| {
                let eval_task = EvalTask {
                    id: task.id.clone(),
                    description: task.description.clone(),
                    expected_output: task.expected_output.clone(),
                    metrics: task.metrics.clone(),
                    max_iterations: task.max_iterations,
                };
                self.run_task(&eval_task)
            })
            .collect()
    }
}

impl Default for EvalHarness {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bleu_perfect_match() {
        let metric = BleuMetric;
        let result = metric.score("hello world", "hello world");
        assert!(result.score > 0.9);
    }

    #[test]
    fn test_bleu_no_match() {
        let metric = BleuMetric;
        let result = metric.score("completely different words", "hello world");
        assert!(result.score < 0.1);
    }

    #[test]
    fn test_chrf_perfect_match() {
        let metric = ChrfMetric;
        let result = metric.score("hello", "hello");
        assert!(result.score > 0.9);
    }

    #[test]
    fn test_exact_match() {
        let metric = ExactMatchMetric;
        assert_eq!(metric.score("hello", "hello").score, 1.0);
        assert_eq!(metric.score("hello", "world").score, 0.0);
    }

    #[test]
    fn test_contains_match() {
        let metric = ContainsMatchMetric;
        assert_eq!(metric.score("the capital is Paris", "Paris").score, 1.0);
        assert_eq!(metric.score("the capital is London", "Paris").score, 0.0);
    }

    #[test]
    fn test_eval_harness_score() {
        let harness = EvalHarness::new();
        let scores = harness.score("hello world", "hello world", &["exact_match".into(), "contains_match".into()]);
        assert_eq!(scores.len(), 2);
        assert!(scores.iter().all(|s| s.score > 0.5));
    }

    #[test]
    fn test_benchmark_suite_standard() {
        let suite = BenchmarkSuite::standard();
        assert_eq!(suite.tasks.len(), 3);
        assert!(suite.metrics.contains(&"bleu".to_string()));
    }

    #[test]
    fn test_eval_result_success() {
        let harness = EvalHarness::new();
        let task = EvalTask {
            id: "test".into(),
            description: "test".into(),
            expected_output: "hello".into(),
            metrics: vec!["exact_match".into()],
            max_iterations: 3,
        };
        let result = harness.run_task(&task);
        assert!(result.success);
    }
}
