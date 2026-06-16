use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Unique identifier for a pipeline node
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct NodeId(pub String);

impl NodeId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

impl Default for NodeId {
    fn default() -> Self {
        Self::new()
    }
}

/// Type of data flowing between nodes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DataType {
    Text,
    Json,
    Number,
    Boolean,
    Binary,
    Any,
}

/// Input to a pipeline node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInput {
    pub name: String,
    pub data_type: DataType,
    pub value: Option<serde_json::Value>,
    /// Source node ID if this input comes from another node
    pub source_node: Option<NodeId>,
}

/// Output from a pipeline node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeOutput {
    pub name: String,
    pub data_type: DataType,
    pub value: serde_json::Value,
}

/// Type of node in the pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeType {
    /// Receives external data into the pipeline
    Input,
    /// Calls an LLM with a prompt template
    Model {
        model_id: String,
        prompt_template: String,
    },
    /// Executes custom logic
    Script {
        /// Inline script code or reference to stored script
        code: Option<String>,
        /// Function name to call
        function_name: String,
    },
    /// Branches based on a condition
    Decision {
        /// Expression to evaluate (simple comparison)
        condition: String,
    },
    /// Routes data to different paths based on type
    Router {
        /// Mapping of data type to target node ID
        routes: HashMap<String, String>,
    },
    /// Evaluates output against reference data
    Metric {
        metric_type: String,
    },
    /// Collects and returns final results
    Output,
}

/// A single node in the pipeline DAG
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineNode {
    pub id: NodeId,
    pub name: String,
    pub node_type: NodeType,
    pub inputs: Vec<NodeInput>,
    pub outputs: Vec<NodeOutput>,
    /// Configuration specific to the node type
    pub config: HashMap<String, serde_json::Value>,
}

/// Edge connecting two nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineEdge {
    pub id: String,
    pub source_node: NodeId,
    pub source_output: String,
    pub target_node: NodeId,
    pub target_input: String,
}

/// A complete pipeline definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pipeline {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub nodes: Vec<PipelineNode>,
    pub edges: Vec<PipelineEdge>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Runtime state of a node during execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeStatus {
    Pending,
    Running,
    Completed,
    Failed { error: String },
    Skipped,
}

/// Runtime state of a pipeline execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineExecution {
    pub execution_id: String,
    pub pipeline_id: String,
    pub status: PipelineExecutionStatus,
    pub node_states: HashMap<NodeId, NodeStatus>,
    pub node_outputs: HashMap<NodeId, Vec<NodeOutput>>,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PipelineExecutionStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Input to execute a pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineRunInput {
    pub pipeline_id: String,
    /// Initial data for Input nodes, keyed by node ID
    pub data: HashMap<String, serde_json::Value>,
}
