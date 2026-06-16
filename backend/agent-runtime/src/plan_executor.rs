use agenticos_core::state::AgentState;
use agenticos_core::udm::{AgentEvent, EventResult, EventType};

use crate::plan::{PlanExecution, PlanStep, PlanStepStatus};
use crate::tool::ToolRegistry;

/// Execute a plan step-by-step with replanning on failure.
///
/// For each step:
/// 1. Mark step as InProgress
/// 2. Execute the tool call via tool_registry
/// 3. On success: mark Completed, continue to next step
/// 4. On failure: mark Failed, replan from current position (max 3 replans)
pub async fn execute_plan(
    execution: &mut PlanExecution,
    state: &mut AgentState,
    tool_registry: &ToolRegistry,
) -> anyhow::Result<()> {
    while !execution.is_complete() {
        let step_idx = execution.current_index;
        let step_desc = execution
            .current_step()
            .map(|s| s.description.clone())
            .unwrap_or_default();
        let step_id = execution
            .current_step()
            .map(|s| s.id.clone())
            .unwrap_or_default();

        if let Some(step) = execution.current_step_mut() {
            step.status = PlanStepStatus::InProgress;
        }

        AgentEvent::new(
            EventType::PlanStepStarted,
            state.workspace_id.clone(),
            state.run_id.clone(),
            state.agent_id.clone(),
            state.user_id.clone(),
        )
        .with_observable(agenticos_core::udm::Observable::message(&format!(
            "Executing plan step {}: {}",
            execution.current_index + 1,
            step_desc
        )))
        .emit();

        // Execute the step's tool call if present
        let tool_call = execution
            .current_step()
            .and_then(|s| s.tool_call.clone());

        let result = if let Some(mut tc) = tool_call {
            tool_registry
                .execute_with_retry(
                    &mut tc,
                    state.workspace_id.as_deref(),
                    state.run_id.as_deref(),
                    state.agent_id.as_deref(),
                )
                .await
        } else {
            // No tool call — just a text step, mark completed
            Ok(step_desc.clone())
        };

        match result {
            Ok(output) => {
                execution.mark_current_completed(output.clone());
                state.add_message(agenticos_core::state::Message::tool_result(
                    "execute_plan",
                    &step_id,
                    &output,
                ));

                AgentEvent::new(
                    EventType::PlanStepCompleted,
                    state.workspace_id.clone(),
                    state.run_id.clone(),
                    state.agent_id.clone(),
                    state.user_id.clone(),
                )
                .with_result(EventResult::success())
                .emit();
            }
            Err(e) => {
                let err_msg = format!("Step failed: {}", e);
                execution.mark_current_failed(err_msg.clone());

                AgentEvent::new(
                    EventType::PlanStepCompleted,
                    state.workspace_id.clone(),
                    state.run_id.clone(),
                    state.agent_id.clone(),
                    state.user_id.clone(),
                )
                .with_result(EventResult::failure(&err_msg))
                .emit();

                // Try to replan
                if execution.replan_from_current() {
                    tracing::warn!(
                        replan_count = execution.replan_count,
                        "Replanning from step {}",
                        execution.current_index + 1
                    );
                    // Add error context to conversation
                    state.add_message(agenticos_core::state::Message::new(
                        "system",
                        &format!(
                            "Step {} failed with error: {}. Replanning attempt {}/3.",
                            execution.current_index + 1,
                            err_msg,
                            execution.replan_count
                        ),
                    ));
                    continue;
                } else {
                    tracing::error!("Max replans reached, aborting plan execution");
                    return Err(anyhow::anyhow!(
                        "Plan execution failed after {} replans: {}",
                        execution.replan_count,
                        err_msg
                    ));
                }
            }
        }
    }

    tracing::info!(
        steps = execution.steps.len(),
        replans = execution.replan_count,
        "Plan execution completed"
    );
    Ok(())
}

/// Create a PlanExecution from a list of step descriptions.
/// Each step gets a simple text description (no tool call).
pub fn create_simple_plan(descriptions: Vec<String>) -> PlanExecution {
    let steps = descriptions
        .into_iter()
        .enumerate()
        .map(|(i, desc)| PlanStep {
            id: format!("plan_step_{}", i),
            description: desc,
            status: PlanStepStatus::Pending,
            tool_call: None,
            result: None,
        })
        .collect();
    PlanExecution::new(steps)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_execution_new() {
        let steps = vec![
            PlanStep {
                id: "s1".into(),
                description: "Step 1".into(),
                status: PlanStepStatus::Pending,
                tool_call: None,
                result: None,
            },
            PlanStep {
                id: "s2".into(),
                description: "Step 2".into(),
                status: PlanStepStatus::Pending,
                tool_call: None,
                result: None,
            },
        ];
        let exec = PlanExecution::new(steps);
        assert_eq!(exec.current_index, 0);
        assert_eq!(exec.replan_count, 0);
        assert!(!exec.is_complete());
    }

    #[test]
    fn test_plan_execution_mark_completed() {
        let mut exec = create_simple_plan(vec!["a".into(), "b".into()]);
        exec.mark_current_completed("result_a".into());
        assert_eq!(exec.current_index, 1);
        assert_eq!(exec.steps[0].status, PlanStepStatus::Completed);
        assert_eq!(exec.steps[0].result, Some("result_a".into()));
    }

    #[test]
    fn test_plan_execution_mark_failed() {
        let mut exec = create_simple_plan(vec!["a".into(), "b".into()]);
        exec.mark_current_failed("error_a".into());
        assert_eq!(exec.steps[0].status, PlanStepStatus::Failed);
        assert_eq!(exec.steps[0].result, Some("error_a".into()));
    }

    #[test]
    fn test_plan_execution_replan() {
        let mut exec = create_simple_plan(vec!["a".into(), "b".into(), "c".into()]);
        exec.mark_current_failed("error".into());
        assert!(exec.replan_from_current());
        assert_eq!(exec.replan_count, 1);
        // After replan, current step should be pending
        assert_eq!(exec.steps[0].status, PlanStepStatus::Pending);
    }

    #[test]
    fn test_plan_execution_max_replans() {
        let mut exec = create_simple_plan(vec!["a".into()]);
        for _ in 0..3 {
            exec.mark_current_failed("error".into());
            assert!(exec.replan_from_current());
        }
        // 4th replan should fail
        exec.mark_current_failed("error".into());
        assert!(!exec.replan_from_current());
        assert_eq!(exec.replan_count, 3);
    }

    #[test]
    fn test_plan_execution_is_complete() {
        let mut exec = create_simple_plan(vec!["a".into()]);
        assert!(!exec.is_complete());
        exec.mark_current_completed("done".into());
        assert!(exec.is_complete());
    }

    #[test]
    fn test_create_simple_plan() {
        let exec = create_simple_plan(vec!["step1".into(), "step2".into()]);
        assert_eq!(exec.steps.len(), 2);
        assert_eq!(exec.steps[0].id, "plan_step_0");
        assert_eq!(exec.steps[1].id, "plan_step_1");
    }
}
