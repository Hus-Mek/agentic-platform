/// Agent Evaluation Harness
/// Runs agents against standardized tasks and measures:
/// - Success rate (did the agent produce the expected output?)
/// - Token usage
/// - Number of steps/tool calls
/// - Latency
use std::time::Instant;

#[derive(Debug)]
pub struct EvalTask {
    pub name: String,
    pub input: String,
    pub expected_contains: Vec<String>,  // Output should contain these strings
    pub max_steps: u32,
    pub max_tokens: i64,
}

#[derive(Debug)]
pub struct EvalResult {
    pub task_name: String,
    pub success: bool,
    pub tokens_used: i64,
    pub steps_taken: u32,
    pub duration_ms: i64,
    pub output: String,
    pub failure_reason: Option<String>,
}

pub fn get_standard_tasks() -> Vec<EvalTask> {
    vec![
        EvalTask {
            name: "simple_math".to_string(),
            input: "What is 2 + 2?".to_string(),
            expected_contains: vec!["4".to_string()],
            max_steps: 3,
            max_tokens: 1000,
        },
        EvalTask {
            name: "echo_test".to_string(),
            input: "Echo the word 'hello'".to_string(),
            expected_contains: vec!["hello".to_string()],
            max_steps: 3,
            max_tokens: 500,
        },
        EvalTask {
            name: "calculator_addition".to_string(),
            input: "Use the calculator to compute 15 + 27".to_string(),
            expected_contains: vec!["42".to_string()],
            max_steps: 5,
            max_tokens: 2000,
        },
        EvalTask {
            name: "current_datetime".to_string(),
            input: "What is the current date and time?".to_string(),
            expected_contains: vec![],  // Just check it doesn't error
            max_steps: 3,
            max_tokens: 1000,
        },
        EvalTask {
            name: "multi_step_math".to_string(),
            input: "First compute 10 + 5, then multiply the result by 3".to_string(),
            expected_contains: vec!["45".to_string()],
            max_steps: 6,
            max_tokens: 3000,
        },
    ]
}

pub fn evaluate_output(output: &str, expected: &[String]) -> bool {
    if expected.is_empty() {
        return !output.is_empty();
    }
    expected.iter().all(|exp| output.to_lowercase().contains(&exp.to_lowercase()))
}

pub fn print_eval_report(results: &[EvalResult]) {
    println!("\n=== Agent Evaluation Report ===\n");
    let total = results.len();
    let passed = results.iter().filter(|r| r.success).count();
    let total_tokens: i64 = results.iter().map(|r| r.tokens_used).sum();
    let total_duration: i64 = results.iter().map(|r| r.duration_ms).sum();
    
    println!("Tasks: {}/{} passed ({:.1}%)", passed, total, (passed as f64 / total as f64) * 100.0);
    println!("Total tokens: {}", total_tokens);
    println!("Total duration: {}ms", total_duration);
    println!("Avg tokens/task: {}", total_tokens / total as i64);
    println!("Avg duration/task: {}ms", total_duration / total as i64);
    
    println!("\n--- Per-Task Results ---");
    for result in results {
        let status = if result.success { "PASS" } else { "FAIL" };
        println!("  [{}] {} — {} tokens, {} steps, {}ms {}",
            status,
            result.task_name,
            result.tokens_used,
            result.steps_taken,
            result.duration_ms,
            result.failure_reason.as_ref().map(|r| format!("({})", r)).unwrap_or_default()
        );
    }
}

#[cfg(test)]
mod eval_tests {
    use super::*;

    #[test]
    fn test_eval_output_matching() {
        assert!(evaluate_output("The answer is 42", &["42".to_string()]));
        assert!(evaluate_output("Hello World", &["hello".to_string()]));
        assert!(!evaluate_output("The answer is 7", &["42".to_string()]));
        assert!(evaluate_output("anything", &[]));
    }

    #[test]
    fn test_standard_tasks_load() {
        let tasks = get_standard_tasks();
        assert!(tasks.len() >= 3);
    }
}
