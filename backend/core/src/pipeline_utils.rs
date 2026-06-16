use std::collections::{HashMap, HashSet, VecDeque};
use tracing;

use crate::pipeline::*;

/// Errors that can occur during pipeline operations
#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("Validation failed: {0}")]
    ValidationError(String),
    #[error("Cycle detected in pipeline DAG")]
    CycleDetected,
    #[error("Node {0} failed: {1}")]
    NodeError(String, String),
    #[error("Missing input for node {0}: {1}")]
    MissingInput(String, String),
    #[error("LLM error: {0}")]
    LlmError(String),
    #[error("Script error: {0}")]
    ScriptError(String),
}

/// Validate a pipeline DAG: check for cycles, disconnected nodes, etc.
pub fn validate_pipeline(pipeline: &Pipeline) -> Result<(), PipelineError> {
    let node_ids: HashSet<&str> = pipeline.nodes.iter().map(|n| n.id.0.as_str()).collect();

    // Check all edges reference existing nodes
    for edge in &pipeline.edges {
        if !node_ids.contains(edge.source_node.0.as_str()) {
            return Err(PipelineError::ValidationError(format!(
                "Edge {} references non-existent source node {}",
                edge.id, edge.source_node.0
            )));
        }
        if !node_ids.contains(edge.target_node.0.as_str()) {
            return Err(PipelineError::ValidationError(format!(
                "Edge {} references non-existent target node {}",
                edge.id, edge.target_node.0
            )));
        }
    }

    // Check for cycles using DFS (Kahn's algorithm)
    let mut in_degree: HashMap<&str, usize> = HashMap::new();
    for node in &pipeline.nodes {
        in_degree.entry(&node.id.0).or_insert(0);
    }
    for edge in &pipeline.edges {
        *in_degree.entry(&edge.target_node.0).or_insert(0) += 1;
    }

    let mut queue: VecDeque<&str> = in_degree
        .iter()
        .filter(|(_, deg)| **deg == 0)
        .map(|(id, _)| *id)
        .collect();

    let mut processed = 0usize;
    while let Some(id) = queue.pop_front() {
        processed += 1;
        for edge in &pipeline.edges {
            if edge.source_node.0 == id {
                let deg = in_degree.get_mut(edge.target_node.0.as_str()).unwrap();
                *deg -= 1;
                if *deg == 0 {
                    queue.push_back(&edge.target_node.0);
                }
            }
        }
    }

    if processed != pipeline.nodes.len() {
        return Err(PipelineError::CycleDetected);
    }

    Ok(())
}

/// Topological sort of nodes using Kahn's algorithm
pub fn topological_sort(pipeline: &Pipeline) -> Result<Vec<NodeId>, PipelineError> {
    validate_pipeline(pipeline)?;

    let mut in_degree: HashMap<String, usize> = HashMap::new();
    for node in &pipeline.nodes {
        in_degree.insert(node.id.0.clone(), 0);
    }
    for edge in &pipeline.edges {
        *in_degree.entry(edge.target_node.0.clone()).or_insert(0) += 1;
    }

    let mut queue: VecDeque<String> = in_degree
        .iter()
        .filter(|(_, deg)| **deg == 0)
        .map(|(id, _)| id.clone())
        .collect();

    let mut result = Vec::new();
    while let Some(id) = queue.pop_front() {
        result.push(NodeId(id.clone()));
        for edge in &pipeline.edges {
            if edge.source_node.0 == id {
                let deg = in_degree.get_mut(&edge.target_node.0).unwrap();
                *deg = deg.saturating_sub(1);
                if *deg == 0 {
                    queue.push_back(edge.target_node.0.clone());
                }
            }
        }
    }

    Ok(result)
}

/// Resolve node inputs from upstream outputs and run data
pub fn resolve_inputs(
    node: &PipelineNode,
    run_input: &PipelineRunInput,
    completed_outputs: &HashMap<String, Vec<NodeOutput>>,
) -> Result<HashMap<String, serde_json::Value>, PipelineError> {
    let mut resolved = HashMap::new();

    for input in &node.inputs {
        if let Some(ref source_id) = input.source_node {
            let outputs = completed_outputs.get(&source_id.0).ok_or_else(|| {
                PipelineError::MissingInput(
                    node.name.clone(),
                    format!("upstream node {} not yet completed", source_id.0),
                )
            })?;

            let output = outputs
                .iter()
                .find(|o| o.name == input.name || o.name == "response" || o.name == "output")
                .ok_or_else(|| {
                    PipelineError::MissingInput(
                        node.name.clone(),
                        format!("output '{}' not found from node {}", input.name, source_id.0),
                    )
                })?;

            resolved.insert(input.name.clone(), output.value.clone());
        } else if let Some(ref val) = input.value {
            resolved.insert(input.name.clone(), val.clone());
        } else if let Some(val) = run_input.data.get(&node.id.0) {
            resolved.insert(input.name.clone(), val.clone());
        } else if let Some(val) = run_input.data.get(&input.name) {
            resolved.insert(input.name.clone(), val.clone());
        } else {
            tracing::debug!(
                node = %node.name,
                input = %input.name,
                "Optional input not provided, skipping"
            );
        }
    }

    Ok(resolved)
}

/// Simple template rendering: replace {{variable}} with value
pub fn render_template(template: &str, inputs: &HashMap<String, serde_json::Value>) -> String {
    let mut result = template.to_string();
    for (key, value) in inputs {
        let placeholder = format!("{{{{{}}}}}", key);
        let value_str = match value {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        result = result.replace(&placeholder, &value_str);
    }
    result
}

/// Evaluate a simple condition
pub fn evaluate_condition(
    condition: &str,
    inputs: &HashMap<String, serde_json::Value>,
) -> Result<bool, PipelineError> {
    let parts: Vec<&str> = condition.split_whitespace().collect();
    if parts.len() != 3 {
        return Err(PipelineError::ScriptError(format!(
            "Cannot parse condition: {}",
            condition
        )));
    }

    let left = inputs.get(parts[0]).ok_or_else(|| {
        PipelineError::ScriptError(format!("Unknown variable in condition: {}", parts[0]))
    })?;
    let op = parts[1];
    let right_raw = parts[2];

    match op {
        "==" | "eq" => {
            let right_val: serde_json::Value = match right_raw.parse::<f64>() {
                Ok(n) => serde_json::json!(n),
                Err(_) => serde_json::Value::String(right_raw.to_string()),
            };
            Ok(left == &right_val)
        }
        "!=" | "neq" => {
            let right_val: serde_json::Value = match right_raw.parse::<f64>() {
                Ok(n) => serde_json::json!(n),
                Err(_) => serde_json::Value::String(right_raw.to_string()),
            };
            Ok(left != &right_val)
        }
        ">" | "gt" => {
            let left_num = left.as_f64().ok_or_else(|| {
                PipelineError::ScriptError(format!("Cannot compare non-numeric: {:?}", left))
            })?;
            let right_num: f64 = right_raw.parse().map_err(|_| {
                PipelineError::ScriptError(format!("Cannot parse number: {}", right_raw))
            })?;
            Ok(left_num > right_num)
        }
        "<" | "lt" => {
            let left_num = left.as_f64().ok_or_else(|| {
                PipelineError::ScriptError(format!("Cannot compare non-numeric: {:?}", left))
            })?;
            let right_num: f64 = right_raw.parse().map_err(|_| {
                PipelineError::ScriptError(format!("Cannot parse number: {}", right_raw))
            })?;
            Ok(left_num < right_num)
        }
        "contains" => {
            let left_str = left.as_str().unwrap_or("");
            Ok(left_str.contains(right_raw))
        }
        _ => Err(PipelineError::ScriptError(format!("Unknown operator: {}", op))),
    }
}

/// Find which nodes are ready to execute given completed nodes
pub fn find_ready_nodes(pipeline: &Pipeline, completed: &HashSet<String>) -> Vec<NodeId> {
    let mut ready = Vec::new();

    for node in &pipeline.nodes {
        if completed.contains(&node.id.0) {
            continue;
        }

        let upstream_complete = pipeline
            .edges
            .iter()
            .filter(|e| e.target_node.0 == node.id.0)
            .all(|e| completed.contains(&e.source_node.0));

        if upstream_complete {
            ready.push(node.id.clone());
        }
    }

    ready
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_node(id: &str, name: &str) -> PipelineNode {
        PipelineNode {
            id: NodeId(id.into()),
            name: name.into(),
            node_type: NodeType::Input,
            inputs: vec![],
            outputs: vec![],
            config: HashMap::new(),
        }
    }

    fn make_edge(source: &str, target: &str) -> PipelineEdge {
        PipelineEdge {
            id: format!("edge_{}_{}", source, target),
            source_node: NodeId(source.into()),
            source_output: "response".into(),
            target_node: NodeId(target.into()),
            target_input: "input".into(),
        }
    }

    fn make_linear_pipeline() -> Pipeline {
        Pipeline {
            id: "test".into(),
            name: "Test Pipeline".into(),
            description: "A".into(),
            version: "1.0".into(),
            nodes: vec![
                make_node("a", "Node A"),
                make_node("b", "Node B"),
                make_node("c", "Node C"),
            ],
            edges: vec![make_edge("a", "b"), make_edge("b", "c")],
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn test_validate_valid_pipeline() {
        let pipeline = make_linear_pipeline();
        assert!(validate_pipeline(&pipeline).is_ok());
    }

    #[test]
    fn test_validate_cycle_detected() {
        let pipeline = Pipeline {
            id: "test".into(),
            name: "Cyclic".into(),
            description: "A".into(),
            version: "1.0".into(),
            nodes: vec![make_node("a", "Node A"), make_node("b", "Node B")],
            edges: vec![make_edge("a", "b"), make_edge("b", "a")],
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        assert!(matches!(
            validate_pipeline(&pipeline),
            Err(PipelineError::CycleDetected)
        ));
    }

    #[test]
    fn test_validate_missing_node() {
        let pipeline = Pipeline {
            id: "test".into(),
            name: "Bad".into(),
            description: "A".into(),
            version: "1.0".into(),
            nodes: vec![make_node("a", "Node A")],
            edges: vec![make_edge("a", "nonexistent")],
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        assert!(validate_pipeline(&pipeline).is_err());
    }

    #[test]
    fn test_topological_sort() {
        let pipeline = make_linear_pipeline();
        let sorted = topological_sort(&pipeline).unwrap();
        assert_eq!(sorted.len(), 3);
        let pos_a = sorted.iter().position(|id| id.0 == "a").unwrap();
        let pos_b = sorted.iter().position(|id| id.0 == "b").unwrap();
        let pos_c = sorted.iter().position(|id| id.0 == "c").unwrap();
        assert!(pos_a < pos_b);
        assert!(pos_b < pos_c);
    }

    #[test]
    fn test_render_template() {
        let template = "Hello {{name}}, your score is {{score}}";
        let mut inputs = HashMap::new();
        inputs.insert("name".into(), serde_json::json!("Alice"));
        inputs.insert("score".into(), serde_json::json!(42));
        let result = render_template(template, &inputs);
        assert_eq!(result, "Hello Alice, your score is 42");
    }

    #[test]
    fn test_evaluate_condition_eq() {
        let mut inputs = HashMap::new();
        inputs.insert("status".into(), serde_json::json!("active"));
        assert!(evaluate_condition("status == active", &inputs).unwrap());
        assert!(!evaluate_condition("status == inactive", &inputs).unwrap());
    }

    #[test]
    fn test_evaluate_condition_gt() {
        let mut inputs = HashMap::new();
        inputs.insert("score".into(), serde_json::json!(85));
        assert!(evaluate_condition("score > 50", &inputs).unwrap());
        assert!(!evaluate_condition("score > 90", &inputs).unwrap());
    }

    #[test]
    fn test_evaluate_condition_contains() {
        let mut inputs = HashMap::new();
        inputs.insert("text".into(), serde_json::json!("hello world"));
        assert!(evaluate_condition("text contains world", &inputs).unwrap());
        assert!(!evaluate_condition("text contains foo", &inputs).unwrap());
    }

    #[test]
    fn test_find_ready_nodes() {
        let pipeline = make_linear_pipeline();
        let mut completed = HashSet::new();

        let ready = find_ready_nodes(&pipeline, &completed);
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].0, "a");

        completed.insert("a".into());
        let ready = find_ready_nodes(&pipeline, &completed);
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].0, "b");

        completed.insert("b".into());
        let ready = find_ready_nodes(&pipeline, &completed);
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].0, "c");

        completed.insert("c".into());
        let ready = find_ready_nodes(&pipeline, &completed);
        assert!(ready.is_empty());
    }

    #[test]
    fn test_resolve_inputs_inline() {
        let node = PipelineNode {
            id: NodeId("n1".into()),
            name: "Test".into(),
            node_type: NodeType::Model {
                model_id: "gpt-4".into(),
                prompt_template: "{{prompt}}".into(),
            },
            inputs: vec![NodeInput {
                name: "prompt".into(),
                data_type: DataType::Text,
                value: Some(serde_json::Value::String("Hello".into())),
                source_node: None,
            }],
            outputs: vec![],
            config: HashMap::new(),
        };

        let run_input = PipelineRunInput {
            pipeline_id: "p1".into(),
            data: HashMap::new(),
        };

        let result = resolve_inputs(&node, &run_input, &HashMap::new()).unwrap();
        assert_eq!(result.get("prompt").unwrap(), "Hello");
    }
}
