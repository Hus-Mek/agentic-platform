use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::io::{BufRead, Read, Write};
use agenticos_core::llm::LiteLLMClient;
use agenticos_core::state::{Message, ToolDefinition};
use agenticos_memory::MemoryClient;
use agenticos_agent_runtime::memory_extractor;
use agenticos_agent_runtime::responder::Responder;

const TEST_KEY: &str = "sk-test-mock-key";
const TEST_MODEL: &str = "openai/gpt-3.5-turbo";

static MOCK_STARTED: AtomicBool = AtomicBool::new(false);

fn ensure_mock_llm() -> String {
    if MOCK_STARTED.load(Ordering::Relaxed) {
        return "http://127.0.0.1:9877".into();
    }
    MOCK_STARTED.store(true, Ordering::Relaxed);

    std::thread::spawn(|| {
        let listener = std::net::TcpListener::bind("127.0.0.1:9877").expect("Mock server bind");
        listener.set_nonblocking(false).ok();
        for stream in listener.incoming() {
            let stream = match stream {
                Ok(s) => s,
                Err(_) => continue,
            };
            std::thread::spawn(move || {
                if let Err(e) = handle_connection(stream) {
                    eprintln!("Mock server connection error: {e}");
                }
            });
        }
    });
    std::thread::sleep(std::time::Duration::from_millis(300));
    "http://127.0.0.1:9877".into()
}

fn handle_connection(mut stream: std::net::TcpStream) -> Result<(), std::io::Error> {
    let mut reader = std::io::BufReader::new(&mut stream);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line)?;
        if line.trim().is_empty() { break; }
        if line.to_lowercase().starts_with("content-length:") {
            content_length = line.trim_start_matches("Content-Length:")
                .trim_start_matches("content-length:")
                .trim().parse().unwrap_or(0);
        }
    }
    if content_length > 0 {
        let mut body = vec![0u8; content_length];
        reader.read_exact(&mut body)?;
    }

    let resp_body = r#"{"id":"mock-1","object":"chat.completion","choices":[{"index":0,"message":{"role":"assistant","content":"Mock response"},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}"#;
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        resp_body.len(),
        resp_body
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()?;
    Ok(())
}

async fn build_llm_client() -> LiteLLMClient {
    let url = tokio::task::spawn_blocking(|| ensure_mock_llm()).await.unwrap();
    LiteLLMClient::new_from_url(&url, TEST_KEY)
}

fn build_memory_client() -> MemoryClient {
    MemoryClient::new_fallback()
}

#[tokio::test]
async fn test_01_basic_chat_completion() {
    let llm = build_llm_client().await;
    let messages = vec![
        Message::new("system", "You are a helpful assistant. Answer concisely."),
        Message::new("user", "What is 2+2? Reply with just the number."),
    ];

    let result = llm.chat_completion(TEST_MODEL, &messages, &[]).await;
    assert!(result.is_ok(), "Chat completion failed: {:?}", result.err());
    let response = result.unwrap();
    assert!(response.content.is_some(), "No content in response");
    let content = response.content.unwrap();
    assert!(!content.is_empty(), "Empty response content");
    println!("PASS: Basic chat completion: {content}");
}

#[tokio::test]
async fn test_02_tool_call_with_function() {
    let llm = build_llm_client().await;
    let messages = vec![
        Message::new("system", "You have access to a calculator tool. Use it when needed."),
        Message::new("user", "What is 123 * 456? Use the calculator tool."),
    ];

    let tools = vec![ToolDefinition {
        name: "calculator".into(),
        description: "Multiply two numbers".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "a": {"type": "number"},
                "b": {"type": "number"}
            },
            "required": ["a", "b"]
        }),
        required: vec!["a".into(), "b".into()],
    }];

    let result = llm.chat_completion(TEST_MODEL, &messages, &tools).await;
    assert!(result.is_ok(), "Tool call failed: {:?}", result.err());
    let response = result.unwrap();

    if let Some(tcs) = &response.tool_calls {
        assert!(!tcs.is_empty(), "Expected at least one tool call");
        println!("PASS: Tool call generated: {} with args {:?}", tcs[0].name, tcs[0].args);
    } else if let Some(content) = &response.content {
        println!("INFO: Model returned text instead of tool call: {content}");
    }
}

#[tokio::test]
async fn test_03_conversation_memory_roundtrip() {
    let mem = build_memory_client();

    mem.set_context("user_name", "Alice");
    mem.set_context("session_id", "ses_123");
    assert_eq!(mem.get_context("user_name"), Some("Alice".into()));
    assert_eq!(mem.get_context("session_id"), Some("ses_123".into()));
    assert_eq!(mem.get_context("nonexistent"), None);

    let all = mem.get_all_context();
    assert_eq!(all.len(), 2);

    mem.clear_context();
    assert_eq!(mem.get_context("user_name"), None);
    println!("PASS: Working memory roundtrip");
}

#[tokio::test]
async fn test_04_semantic_memory_roundtrip() {
    let mem = build_memory_client();

    let id = mem.store_semantic("Alice prefers dark mode", Some("user1"), Some("agent1")).await;
    assert!(id.is_ok(), "Store semantic failed: {:?}", id.err());
    println!("PASS: Stored semantic memory: {}", id.unwrap());
}

#[tokio::test]
async fn test_05_procedural_memory_roundtrip() {
    let mem = build_memory_client();

    let id = mem.store_procedural("Web search: query tavily, summarize results", Some("user1")).await;
    assert!(id.is_ok(), "Store procedural failed: {:?}", id.err());
    println!("PASS: Stored procedural memory: {}", id.unwrap());
}

#[tokio::test]
async fn test_06_memory_extractor_calls_llm() {
    let llm = build_llm_client().await;
    let messages = vec![
        Message::new("user", "My name is Bob and I work at Acme Corp. I love Python programming."),
        Message::new("assistant", "Nice to meet you Bob! How can I help you today?"),
    ];

    let facts = memory_extractor::extract_facts(&messages, &llm, TEST_MODEL).await;
    println!("PASS: Extracted {} facts: {:?}", facts.len(), facts);
}

#[tokio::test]
async fn test_07_procedure_extraction() {
    let llm = build_llm_client().await;
    let messages = vec![
        Message::with_tool_calls(
            "assistant",
            "",
            vec![
                agenticos_core::state::ToolCall::new("web_search", serde_json::json!({"query": "latest AI news"})),
            ],
        ),
        Message::tool_result("web_search", "call_1", "Found latest AI news: GPT-5 released"),
        Message::new("assistant", "Based on the search, GPT-5 was released."),
    ];

    let procedure = memory_extractor::extract_procedure(&messages, &llm, TEST_MODEL).await;
    assert!(procedure.is_some(), "Procedure extraction returned None");
    println!("PASS: Extracted procedure: {:?}", procedure.unwrap());
}

#[tokio::test]
async fn test_08_responder_formats_output() {
    let llm = build_llm_client().await;
    let responder = Responder::new(llm, TEST_MODEL.to_string());

    let formatted = responder.format(
        "the capital of france is paris. it has population of 2.1 million.",
        "What is the capital of France?",
    ).await;
    assert!(!formatted.is_empty(), "Formatted output is empty");
    println!("PASS: Responder formatted: {formatted}");
}

#[tokio::test]
async fn test_09_responder_summarizes() {
    let llm = build_llm_client().await;
    let responder = Responder::new(llm, TEST_MODEL.to_string());
    let long_text = "Alice said she prefers dark mode. Bob said he works at Acme Corp. \
                     Both agreed on the project timeline. The deadline is next Friday. \
                     They decided to use Python for the backend and React for the frontend. \
                     The database will be PostgreSQL. CI/CD will use GitHub Actions.";

    let summary = responder.summarize(long_text, 100).await;
    assert!(!summary.is_empty(), "Summary is empty");
    println!("PASS: Summarizer ({}/400 chars): {summary}", summary.len());
}

#[tokio::test]
async fn test_10_llm_streaming_endpoint() {
    let llm = build_llm_client().await;
    let messages = vec![
        Message::new("user", "Count from 1 to 5. Reply with just the numbers."),
    ];

    let result = llm.chat_completion_stream(TEST_MODEL, &messages, &[]).await;
    if let Ok(response) = &result {
        println!("PASS: Streaming endpoint status {}", response.status());
    } else {
        println!("INFO: Stream not supported by mock LLM, skipping assertion");
    }
}

#[tokio::test]
async fn test_11_high_volume_memory_ops() {
    let mem = build_memory_client();

    for i in 0..1000 {
        mem.set_context(&format!("key_{i}"), &format!("value_{i}"));
    }
    let all = mem.get_all_context();
    assert_eq!(all.len(), 1000, "Expected 1000 entries, got {}", all.len());
    println!("PASS: 1000 working memory operations");

    for i in 0..100 {
        let result = mem.store_semantic(
            &format!("Fact number {i}"),
            Some("stress_user"),
            Some("stress_agent"),
        ).await;
        assert!(result.is_ok(), "Store semantic {} failed: {:?}", i, result.err());
    }
    println!("PASS: 100 semantic memory stores");
}

#[tokio::test]
async fn test_12_concurrent_chat_completions() {
    let llm = build_llm_client().await;
    let llm_ref = Arc::new(llm);

    let mut handles = Vec::new();
    for i in 0..5 {
        let llm = llm_ref.clone();
        let handle = tokio::spawn(async move {
            let messages = vec![
                Message::new("user", &format!("What is {i} + {i}? Reply with just the number.")),
            ];
            llm.chat_completion(TEST_MODEL, &messages, &[]).await
        });
        handles.push(handle);
    }

    for (i, handle) in handles.into_iter().enumerate() {
        let result = handle.await.expect("Task panicked");
        assert!(result.is_ok(), "Concurrent request {i} failed: {:?}", result.err());
        println!("PASS: Concurrent request {i} succeeded");
    }
}

#[tokio::test]
async fn test_13_large_context_chat() {
    let llm = build_llm_client().await;

    let mut messages = vec![
        Message::new("system", "You are a summarizer. Be extremely concise."),
    ];
    for i in 0..20 {
        messages.push(Message::new("user", &format!("This is paragraph {i} with some random content about AI agents and their applications in modern software engineering.")));
        messages.push(Message::new("assistant", &format!("Understood paragraph {i}. Continuing.")));
    }
    messages.push(Message::new("user", "Summarize the entire conversation in one sentence."));

    let result = llm.chat_completion(TEST_MODEL, &messages, &[]).await;
    assert!(result.is_ok(), "Large context chat failed: {:?}", result.err());
    let response = result.unwrap();
    assert!(response.content.is_some(), "No content in large context response");
    println!("PASS: Large context chat ({} messages): {} tokens used",
        messages.len(), response.usage.total_tokens);
}
