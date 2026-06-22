//! Integration tests for the real pipeline executor.
//!
//! Uses a mock LLM HTTP server (mirroring pipeline_stress_test.rs) and a lazy,
//! unconnected Postgres pool. The executor's per-node `run_steps` writes are
//! best-effort (`let _ = insert_pipeline_step`), so they fail silently without a
//! DB while the pipeline itself runs to completion — letting us assert real
//! Model-node LLM binding and WASM Script execution end-to-end.

use std::collections::HashMap;
use std::io::{BufRead, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use agenticos_agent_runtime::pipeline_executor::{execute_pipeline, ExecutionContext};
use agenticos_agent_runtime::rag::KbRetriever;
use agenticos_agent_runtime::tool::ToolRegistry;
use agenticos_core::llm::LiteLLMClient;
use agenticos_core::pipeline::*;
use agenticos_memory::MemoryClient;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

const TEST_KEY: &str = "sk-test-mock-key";
const TEST_MODEL: &str = "openai/gpt-3.5-turbo";

static MOCK_STARTED: AtomicBool = AtomicBool::new(false);

fn ensure_mock_llm() -> String {
    if MOCK_STARTED.swap(true, Ordering::Relaxed) {
        return "http://127.0.0.1:9878".into();
    }
    std::thread::spawn(|| {
        let listener = std::net::TcpListener::bind("127.0.0.1:9878").expect("bind mock");
        for stream in listener.incoming().flatten() {
            std::thread::spawn(move || {
                let _ = handle_connection(stream);
            });
        }
    });
    std::thread::sleep(Duration::from_millis(300));
    "http://127.0.0.1:9878".into()
}

fn handle_connection(mut stream: std::net::TcpStream) -> std::io::Result<()> {
    let mut reader = std::io::BufReader::new(&mut stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut content_length = 0usize;
    loop {
        let mut header = String::new();
        reader.read_line(&mut header)?;
        if header.trim().is_empty() {
            break;
        }
        if header.to_lowercase().starts_with("content-length:") {
            content_length = header
                .split(':')
                .nth(1)
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(0);
        }
    }
    if content_length > 0 {
        let mut body = vec![0u8; content_length];
        reader.read_exact(&mut body)?;
    }
    let resp = r#"{"id":"m","object":"chat.completion","choices":[{"index":0,"message":{"role":"assistant","content":"Mock response"},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}"#;
    let http = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        resp.len(),
        resp
    );
    stream.write_all(http.as_bytes())?;
    stream.flush()
}

struct Harness {
    llm: LiteLLMClient,
    tools: Arc<ToolRegistry>,
    guards: Arc<agenticos_guards::GuardEngine>,
    memory: Arc<MemoryClient>,
    kb: Arc<KbRetriever>,
    db: sqlx::PgPool,
}

async fn harness() -> Harness {
    let url = ensure_mock_llm();
    let llm = LiteLLMClient::new_from_url(&url, TEST_KEY);
    // Lazy pool to a port with no server: step writes fail fast and are ignored.
    let db = PgPoolOptions::new()
        .acquire_timeout(Duration::from_millis(50))
        .connect_lazy("postgres://u:p@127.0.0.1:5433/none")
        .expect("lazy pool");
    Harness {
        tools: Arc::new(ToolRegistry::new()),
        guards: Arc::new(agenticos_guards::GuardEngine::new()),
        memory: Arc::new(MemoryClient::new_fallback()),
        kb: Arc::new(KbRetriever::new(db.clone(), llm.clone())),
        llm,
        db,
    }
}

fn ctx<'a>(h: &'a Harness) -> ExecutionContext<'a> {
    ExecutionContext {
        llm_client: &h.llm,
        tool_registry: &h.tools,
        guard_engine: &h.guards,
        memory_client: &h.memory,
        kb_retriever: &h.kb,
        db: &h.db,
        workspace_id: None,
        user_id: None,
        pipeline_run_id: Uuid::new_v4(),
        cancel: None,
        tx: None,
    }
}

fn input(source: Option<&str>, name: &str) -> NodeInput {
    NodeInput {
        name: name.into(),
        data_type: DataType::Any,
        value: None,
        source_node: source.map(|s| NodeId(s.into())),
    }
}

fn node(id: &str, name: &str, ty: NodeType, inputs: Vec<NodeInput>) -> PipelineNode {
    PipelineNode {
        id: NodeId(id.into()),
        name: name.into(),
        node_type: ty,
        inputs,
        outputs: vec![],
        config: HashMap::new(),
    }
}

fn edge(src: &str, tgt: &str) -> PipelineEdge {
    PipelineEdge {
        id: format!("{src}->{tgt}"),
        source_node: NodeId(src.into()),
        source_output: "output".into(),
        target_node: NodeId(tgt.into()),
        target_input: "input".into(),
    }
}

fn pipeline(nodes: Vec<PipelineNode>, edges: Vec<PipelineEdge>) -> Pipeline {
    Pipeline {
        id: "p1".into(),
        name: "test".into(),
        description: String::new(),
        version: "1.0".into(),
        nodes,
        edges,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

#[tokio::test]
async fn model_node_calls_real_llm_and_accumulates_tokens() {
    let h = harness().await;
    let p = pipeline(
        vec![
            node("in", "Input", NodeType::Input, vec![input(None, "input")]),
            node(
                "model",
                "Model",
                NodeType::Model { model_id: TEST_MODEL.into(), prompt_template: "Echo: {{input}}".into() },
                vec![input(Some("in"), "input")],
            ),
            node("out", "Output", NodeType::Output, vec![input(Some("model"), "response")]),
        ],
        vec![edge("in", "model"), edge("model", "out")],
    );

    let run_input = PipelineRunInput {
        pipeline_id: "p1".into(),
        data: HashMap::from([("input".to_string(), serde_json::json!("hello"))]),
    };

    let outcome = execute_pipeline(&p, &run_input, &ctx(&h)).await.expect("run ok");
    assert_eq!(outcome.execution.status, PipelineExecutionStatus::Completed);
    assert_eq!(outcome.total_tokens, 15, "mock LLM reports 15 tokens");

    let model_out = outcome.execution.node_outputs.get(&NodeId("model".into())).unwrap();
    assert_eq!(model_out[0].value, serde_json::json!("Mock response"));
}

#[tokio::test]
async fn script_node_executes_in_wasm_sandbox() {
    // Echo module: copies the JSON input straight back to the output region.
    const ECHO_WAT: &str = r#"
    (module
      (memory (export "memory") 17)
      (func (export "run")
        (local $len i32)
        (local.set $len (i32.load (i32.const 0)))
        (memory.copy (i32.const 0x100004) (i32.const 4) (local.get $len))
        (i32.store (i32.const 0x100000) (local.get $len))))
    "#;

    let h = harness().await;
    let p = pipeline(
        vec![
            node("in", "Input", NodeType::Input, vec![input(None, "input")]),
            node(
                "script",
                "Script",
                NodeType::Script { code: Some(ECHO_WAT.into()), function_name: "run".into() },
                vec![input(Some("in"), "input")],
            ),
            node("out", "Output", NodeType::Output, vec![input(Some("script"), "response")]),
        ],
        vec![edge("in", "script"), edge("script", "out")],
    );

    let run_input = PipelineRunInput {
        pipeline_id: "p1".into(),
        data: HashMap::from([("input".to_string(), serde_json::json!("hi"))]),
    };

    let outcome = execute_pipeline(&p, &run_input, &ctx(&h)).await.expect("run ok");
    assert_eq!(outcome.execution.status, PipelineExecutionStatus::Completed);
    let script_out = outcome.execution.node_outputs.get(&NodeId("script".into())).unwrap();
    // Echoed object carries the resolved input under "input".
    assert_eq!(script_out[0].value.get("input").unwrap(), &serde_json::json!("hi"));
}

#[tokio::test]
async fn cyclic_pipeline_is_rejected() {
    let h = harness().await;
    let p = pipeline(
        vec![
            node("a", "A", NodeType::Input, vec![]),
            node("b", "B", NodeType::Output, vec![]),
        ],
        vec![edge("a", "b"), edge("b", "a")],
    );
    let run_input = PipelineRunInput { pipeline_id: "p1".into(), data: HashMap::new() };
    let result = execute_pipeline(&p, &run_input, &ctx(&h)).await;
    assert!(result.is_err(), "cyclic DAG must be rejected by validation");
}
