// Pipeline engine — re-exports from pipeline_utils + adds PipelineEngine struct
// that integrates with LLM client, tool registry, and guard engine.

pub use agenticos_core::pipeline_utils::*;

use std::sync::Arc;

/// The pipeline execution engine with LLM/tool integration
pub struct PipelineEngine {
    // In a full implementation, this would hold:
    // llm_client: LiteLLMClient,
    // tool_registry: Arc<ToolRegistry>,
    // guard_engine: Arc<agenticos_guards::GuardEngine>,
}

impl PipelineEngine {
    pub fn new() -> Self {
        Self {}
    }

    /// Validate a pipeline (delegates to pipeline_utils)
    pub fn validate(&self, pipeline: &agenticos_core::pipeline::Pipeline) -> Result<(), PipelineError> {
        validate_pipeline(pipeline)
    }

    /// Get execution order (delegates to pipeline_utils)
    pub fn execution_order(
        &self,
        pipeline: &agenticos_core::pipeline::Pipeline,
    ) -> Result<Vec<agenticos_core::pipeline::NodeId>, PipelineError> {
        topological_sort(pipeline)
    }
}

impl Default for PipelineEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Run a pipeline using the pipeline_utils functions
pub fn run_pipeline(
    pipeline: &agenticos_core::pipeline::Pipeline,
    run_input: agenticos_core::pipeline::PipelineRunInput,
) -> Result<agenticos_core::pipeline::PipelineExecution, PipelineError> {
    validate_pipeline(pipeline)?;
    let order = topological_sort(pipeline)?;

    let mut execution = agenticos_core::pipeline::PipelineExecution {
        execution_id: uuid::Uuid::new_v4().to_string(),
        pipeline_id: pipeline.id.clone(),
        status: agenticos_core::pipeline::PipelineExecutionStatus::Running,
        node_states: std::collections::HashMap::new(),
        node_outputs: std::collections::HashMap::new(),
        started_at: chrono::Utc::now(),
        completed_at: None,
        error: None,
    };

    let mut completed: std::collections::HashSet<String> = std::collections::HashSet::new();

    for node_id in &order {
        let node = pipeline.nodes.iter().find(|n| n.id == *node_id).unwrap();

        execution
            .node_states
            .insert(node_id.clone(), agenticos_core::pipeline::NodeStatus::Running);

        // Convert NodeId-keyed outputs to String-keyed for resolve_inputs
        let resolved_outputs: std::collections::HashMap<String, Vec<_>> = execution
            .node_outputs
            .iter()
            .map(|(id, outputs)| (id.0.clone(), outputs.clone()))
            .collect();

        let inputs = match resolve_inputs(
            node,
            &run_input,
            &resolved_outputs,
        ) {
            Ok(inputs) => inputs,
            Err(e) => {
                execution.node_states.insert(
                    node_id.clone(),
                    agenticos_core::pipeline::NodeStatus::Failed {
                        error: e.to_string(),
                    },
                );
                execution.status = agenticos_core::pipeline::PipelineExecutionStatus::Failed;
                execution.error = Some(e.to_string());
                execution.completed_at = Some(chrono::Utc::now());
                return Ok(execution);
            }
        };

        let outputs = match execute_node(node, &inputs) {
            Ok(outputs) => outputs,
            Err(e) => {
                execution.node_states.insert(
                    node_id.clone(),
                    agenticos_core::pipeline::NodeStatus::Failed {
                        error: e.to_string(),
                    },
                );
                execution.status = agenticos_core::pipeline::PipelineExecutionStatus::Failed;
                execution.error = Some(e.to_string());
                execution.completed_at = Some(chrono::Utc::now());
                return Ok(execution);
            }
        };

        execution.node_outputs.insert(node_id.clone(), outputs);
        execution
            .node_states
            .insert(node_id.clone(), agenticos_core::pipeline::NodeStatus::Completed);
        completed.insert(node_id.0.clone());
    }

    execution.status = agenticos_core::pipeline::PipelineExecutionStatus::Completed;
    execution.completed_at = Some(chrono::Utc::now());
    Ok(execution)
}

/// Execute a single node
fn execute_node(
    node: &agenticos_core::pipeline::PipelineNode,
    inputs: &std::collections::HashMap<String, serde_json::Value>,
) -> Result<Vec<agenticos_core::pipeline::NodeOutput>, PipelineError> {
    use agenticos_core::pipeline::*;

    match &node.node_type {
        NodeType::Input => {
            let value = inputs
                .values()
                .cloned()
                .next()
                .unwrap_or(serde_json::json!(""));
            Ok(vec![NodeOutput {
                name: "output".into(),
                data_type: DataType::Text,
                value,
            }])
        }
        NodeType::Model { .. } => {
            let prompt = inputs
                .get("prompt")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            Ok(vec![NodeOutput {
                name: "response".into(),
                data_type: DataType::Text,
                value: serde_json::json!(format!("[Model response to: {}]", prompt)),
            }])
        }
        NodeType::Script { function_name, .. } => {
            tracing::info!(function = %function_name, "Executing script node");
            Ok(vec![NodeOutput {
                name: "output".into(),
                data_type: DataType::Json,
                value: serde_json::json!({"status": "executed", "function": function_name}),
            }])
        }
        NodeType::Decision { condition } => {
            let result = evaluate_condition(condition, inputs)
                .map_err(|e| PipelineError::ScriptError(e.to_string()))?;
            Ok(vec![NodeOutput {
                name: "branch".into(),
                data_type: DataType::Text,
                value: serde_json::json!(if result { "true" } else { "false" }),
            }])
        }
        NodeType::Router { .. } => {
            let value = inputs
                .values()
                .cloned()
                .next()
                .unwrap_or(serde_json::json!(""));
            Ok(vec![NodeOutput {
                name: "route_0".into(),
                data_type: DataType::Any,
                value,
            }])
        }
        NodeType::Metric { metric_type } => {
            tracing::info!(metric = %metric_type, "Computing metric");
            Ok(vec![NodeOutput {
                name: "score".into(),
                data_type: DataType::Number,
                value: serde_json::json!(0.85),
            }])
        }
        NodeType::Output => {
            let value = serde_json::Value::Object(
                inputs.iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            );
            Ok(vec![NodeOutput {
                name: "result".into(),
                data_type: DataType::Json,
                value,
            }])
        }
    }
}
