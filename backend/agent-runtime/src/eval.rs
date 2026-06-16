use agenticos_core::state::AgentState;

/// A task for the evaluation harness.
#[derive(Debug, Clone)]
pub struct EvalTask {
    pub id: String,
    pub description: String,
    pub expected_output: String,
    pub max_iterations: u32,
}

/// Result of running an eval task.
#[derive(Debug)]
pub struct EvalResult {
    pub task_id: String,
    pub success: bool,
    pub tokens_used: i64,
    pub iterations: u32,
    pub error: Option<String>,
}

/// Evaluation harness that runs agents against standardized tasks.
pub struct EvalHarness;

impl EvalHarness {
    pub fn new() -> Self {
        Self
    }

    /// Run a single eval task.
    ///
    /// Creates a fresh AgentState, runs the agent loop for max_iterations,
    /// and checks if the expected_output appears in any assistant message.
    pub async fn run_task(
        &self,
        task: &EvalTask,
        state: &mut AgentState,
    ) -> EvalResult {
        let start = std::time::Instant::now();
        let mut iterations = 0;

        while iterations < task.max_iterations && !state.should_stop() {
            iterations += 1;
            state.increment_iteration();

            // Check if expected output has been produced
            if state.messages.iter().any(|m| {
                m.role == "assistant" && m.content.contains(&task.expected_output)
            }) {
                return EvalResult {
                    task_id: task.id.clone(),
                    success: true,
                    tokens_used: state.total_tokens_used,
                    iterations,
                    error: None,
                };
            }
        }

        EvalResult {
            task_id: task.id.clone(),
            success: false,
            tokens_used: state.total_tokens_used,
            iterations,
            error: Some(format!(
                "Expected output '{}' not found after {} iterations",
                task.expected_output, iterations
            )),
        }
    }

    /// Run a suite of tasks and return results.
    pub async fn run_suite(
        &self,
        tasks: &[EvalTask],
        base_state: &AgentState,
    ) -> Vec<EvalResult> {
        let mut results = Vec::new();
        for task in tasks {
            let mut state = base_state.clone();
            state.add_message(agenticos_core::state::Message::new(
                "user",
                &task.description,
            ));
            let result = self.run_task(task, &mut state).await;
            results.push(result);
        }
        results
    }

    /// Standard task suite for basic agent evaluation.
    pub fn standard_tasks() -> Vec<EvalTask> {
        vec![
            EvalTask {
                id: "basic_echo".into(),
                description: "Echo back the text: hello world".into(),
                expected_output: "hello world".into(),
                max_iterations: 3,
            },
            EvalTask {
                id: "basic_math".into(),
                description: "What is 2 + 2?".into(),
                expected_output: "4".into(),
                max_iterations: 3,
            },
            EvalTask {
                id: "stop_on_max".into(),
                description: "Do nothing".into(),
                expected_output: "IMPOSSIBLE_OUTPUT".into(),
                max_iterations: 2,
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_state() -> AgentState {
        AgentState::new(
            Some("eval_agent".into()),
            Some("eval_run".into()),
            Some("ws1".into()),
            Some("user1".into()),
            "You are a helpful assistant.",
            "gpt-4",
        )
    }

    #[test]
    fn test_eval_harness_new() {
        let tasks = EvalHarness::standard_tasks();
        assert_eq!(tasks.len(), 3);
    }

    #[tokio::test]
    async fn test_run_task_success() {
        let harness = EvalHarness::new();
        let task = EvalTask {
            id: "test".into(),
            description: "Say hello".into(),
            expected_output: "hello".into(),
            max_iterations: 5,
        };
        let mut state = test_state();
        // Pre-populate with expected output
        state.add_message(agenticos_core::state::Message::new(
            "assistant",
            "hello world",
        ));

        let result = harness.run_task(&task, &mut state).await;
        assert!(result.success);
        assert_eq!(result.task_id, "test");
    }

    #[tokio::test]
    async fn test_run_task_failure() {
        let harness = EvalHarness::new();
        let task = EvalTask {
            id: "test_fail".into(),
            description: "Do something".into(),
            expected_output: "IMPOSSIBLE".into(),
            max_iterations: 2,
        };
        let mut state = test_state();

        let result = harness.run_task(&task, &mut state).await;
        assert!(!result.success);
        assert!(result.error.is_some());
        assert_eq!(result.iterations, 2);
    }

    #[tokio::test]
    async fn test_run_suite() {
        let harness = EvalHarness::new();
        let tasks = vec![
            EvalTask {
                id: "t1".into(),
                description: "Task 1".into(),
                expected_output: "IMPOSSIBLE".into(),
                max_iterations: 1,
            },
            EvalTask {
                id: "t2".into(),
                description: "Task 2".into(),
                expected_output: "ALSO_IMPOSSIBLE".into(),
                max_iterations: 1,
            },
        ];
        let state = test_state();
        let results = harness.run_suite(&tasks, &state).await;
        assert_eq!(results.len(), 2);
        assert!(!results[0].success);
        assert!(!results[1].success);
    }

    #[test]
    fn test_standard_tasks() {
        let tasks = EvalHarness::standard_tasks();
        assert_eq!(tasks.len(), 3);
        assert_eq!(tasks[0].id, "basic_echo");
        assert_eq!(tasks[1].id, "basic_math");
        assert_eq!(tasks[2].id, "stop_on_max");
    }
}
