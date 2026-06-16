# AgenticOS Harness Integration — Remaining Work Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Complete Phase 1 (Core Hardening) by finishing the tool middleware pipeline, then implement Phase 2-7 from the harness integration plan to build a production-grade agent runtime.

**Architecture:** Layered approach: (1) complete the partial middleware pipeline, (2) add context window management, (3) build multi-provider LLM abstraction, (4) implement parallel execution + HITL + planning, (5) overhaul memory, (6) expand tools via MCP, (7) add observability + cost control, (8) build evaluation harness.

**Tech Stack:** Rust (tokio, reqwest, serde, sqlx, tracing),(axum for API), Qdrant (via agenticos-memory), OpenTelemetry, wasmtime (for sandboxing), Next.js frontend.

---

## Current State Assessment

### Done
- `error.rs` — ErrorKind enum, classify_error(), 6 tests
- `recovery.rs` — RecoveryConfig, RecoveryState, RecoveryAction, circuit breaker, 6 tests
- `tool.rs` — ToolMetadata struct (requires_approval, retryable, timeout_secs), metadata field on ToolImpl
- `orchestrator.rs` — create_delegate_tool() includes metadata field

### In Progress
- **Task 1.2 (Tool Middleware Pipeline)** — ToolMetadata added but pipeline logic missing:
  - No `validate_tool_args()` — args are passed directly to handlers
  - No timeout wrapping — `ToolRegistry::execute()` uses raw `(tool.handler)(args).await`
  - No retry logic — errors from handlers bubble up immediately
  - `register()` takes 5 args but callers that want custom metadata need a `register_with_metadata()` method

### Not Started
- Task 1.1 (Context Window Management)
- Phases 2-7

### Key Observations from Code Review

1. `tool.rs` line 58: `register()` always sets `metadata: ToolMetadata::default()` — no way for callers to customize. The original `register()` is used for all built-in tools (calculator, current_datetime, echo) plus orchestrator.rs registers read/write/list_dir/create_plan tools externally. There's no `register_with_metadata()` variant.

2. `registry.execute()` at line 98: `let result = (tool.handler)(tool_call.args.clone()).await;` — no validation, no timeout, no retry. This is the core gap.

3. `loop_mod.rs` lines 209-231: Tool execution calls `tool_registry.execute()` directly, then on error does `state.consecutive_errors += 1` (raw counter, not using the recovery module).

4. `streaming.rs` lines 209-231: Same pattern — raw execute call, no pipeline.

5. `responder.rs` has `summarize()` method — this is what ContextManager will use.

6. `AgentState` in `state.rs` already has `max_iterations`, `token_budget`, `consecutive_errors`, `max_consecutive_errors` — recovery.rs builds on these.

7. The `ToolRegistry` `register()` method is used in two places:
   - `tool.rs` `register_builtins()` — registers 3 tools (calculator, current_datetime, echo)
   - `orchestrator.rs` line 78-84 — BUT actually the orchestrator pushes ToolDefinition to state, not ToolImpl to ToolRegistry. The ToolRegistry is separate.

---

## Phase 1: Core Runtime Hardening (COMPLETE THE REMAINING 1.5 TASKS)

### Task 1.2a: Add register_with_metadata() to ToolRegistry

**Objective:** Allow external callers to register tools with custom metadata (requires_approval=true for write_file, etc.)

**Files:**
- Modify: `backend/agent-runtime/src/tool.rs:44-61`
- Add tests to bottom of `tool.rs`

**Step 1: Add a `register_with_metadata()` method**

```rust
pub fn register_with_metadata(
    &mut self,
    name: &str,
    description: &str,
    schema: serde_json::Value,
    handler: ToolHandler,
    metadata: ToolMetadata,
) {
    self.tools.insert(
        name.to_string(),
        ToolImpl {
            name: name.to_string(),
            description: description.to_string(),
            schema,
            handler,
            metadata,
        },
    );
}
```

**Step 2: Keep existing `register()` as a convenience wrapper**

The existing `register()` already exists at line 44. It should remain unchanged (calls with default metadata).

**Step 3: Write test**

```rust
#[test]
fn test_register_with_metadata() {
    let mut reg = ToolRegistry::empty();
    reg.register_with_metadata(
        "write_file",
        "Write to a file",
        serde_json::json!({"type": "object", "properties": {}}),
        Arc::new(|_args| Box::pin(async { Ok("ok".into()) })),
        ToolMetadata {
            requires_approval: true,
            retryable: false,
            timeout_secs: 30,
        },
    );
    let tool = reg.get("write_file").unwrap();
    assert!(tool.metadata.requires_approval);
    assert!(!tool.metadata.retryable);
    assert_eq!(tool.metadata.timeout_secs, 30);
}
```

Note: add an `empty()` constructor for testing (no builtins):

```rust
#[cfg(test)]
pub fn empty() -> Self {
    Self {
        tools: HashMap::new(),
    }
}
```

**Step 4: Run test**

Run: `cargo test -p agenticos-agent-runtime -- test_register_with_metadata`
Expected: PASS

**Step 5: Commit**

```bash
git add backend/agent-runtime/src/tool.rs
git commit -m "feat: add register_with_metadata() to ToolRegistry"
```

---

### Task 1.2b: Add Tool Argument Validation

**Objective:** Validate tool call arguments against the tool's JSON Schema before execution.

**Files:**
- Modify: `backend/agent-runtime/src/tool.rs`
- Add tests

**Step 1: Add a `validate_tool_args()` method on ToolRegistry**

```rust
pub fn validate_tool_args(&self, tool_name: &str, args: &serde_json::Value) -> Result<(), String> {
    let tool = self.tools.get(tool_name)
        .ok_or_else(|| format!("Unknown tool: {}", tool_name))?;
    let schema = &tool.schema;

    // Validate required fields exist in args
    if let Some(required) = schema.get("required").and_then(|v| v.as_array()) {
        for field in required {
            let field_name = field.as_str()
                .ok_or("Schema 'required' contains non-string value")?;
            if args.get(field_name).is_none() {
                return Err(format!("Missing required argument: '{}'", field_name));
            }
        }
    }

    // Validate argument types against schema properties
    if let Some(properties) = schema.get("properties").and_then(|v| v.as_object()) {
        for (key, val) in args.as_object().unwrap_or(&serde_json::Map::new()) {
            if let Some(prop_schema) = properties.get(key) {
                if let Some(expected_type) = prop_schema.get("type").and_then(|t| t.as_str()) {
                    let type_matches = match expected_type {
                        "string" => val.is_string(),
                        "number" | "integer" => val.is_number(),
                        "boolean" => val.is_boolean(),
                        "array" => val.is_array(),
                        "object" => val.is_object(),
                        _ => true, // unknown types pass
                    };
                    if !type_matches {
                        return Err(format!(
                            "Argument '{}' expected {} but got {}",
                            key, expected_type,
                            match val {
                                serde_json::Value::Null => "null",
                                serde_json::Value::Bool(_) => "boolean",
                                serde_json::Value::Number(_) => "number",
                                serde_json::Value::String(_) => "string",
                                serde_json::Value::Array(_) => "array",
                                serde_json::Value::Object(_) => "object",
                            }
                        ));
                    }
                }
            }
        }
    }

    Ok(())
}
```

**Step 2: Call validation in execute()**

Insert after line 85 (after getting the tool from the map):

```rust
// Validate arguments against schema
if let Err(validation_err) = self.validate_tool_args(&tool_call.name, &tool_call.args) {
    tracing::error!(
        tool = %tool_call.name,
        error = %validation_err,
        "Tool argument validation failed"
    );
    return Err(anyhow::anyhow!("Tool validation error: {}", validation_err));
}
```

**Step 3: Write test**

```rust
#[tokio::test]
async fn test_validate_missing_required_arg() {
    let reg = ToolRegistry::new();
    let args = serde_json::json!({}); // missing "expression"
    let result = reg.validate_tool_args("calculator", &args);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Missing required argument: 'expression'"));
}

#[tokio::test]
async fn test_validate_wrong_type() {
    let reg = ToolRegistry::new();
    let args = serde_json::json!({"expression": 123}); // should be string
    let result = reg.validate_tool_args("calculator", &args);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("expected string"));
}

#[tokio::test]
async fn test_validate_valid_args() {
    let reg = ToolRegistry::new();
    let args = serde_json::json!({"expression": "2 + 2"});
    let result = reg.validate_tool_args("calculator", &args);
    assert!(result.is_ok());
}
```

**Step 4: Run tests**

Run: `cargo test -p agenticos-agent-runtime -- validate`
Expected: PASS

**Step 5: Commit**

```bash
git add backend/agent-runtime/src/tool.rs
git commit -m "feat: add tool argument validation against JSON Schema"
```

---

### Task 1.2c: Add Timeout Wrapping to Tool Execution

**Objective:** Wrap tool execution with tokio::time::timeout using the tool's metadata.timeout_secs.

**Files:**
- Modify: `backend/agent-runtime/src/tool.rs:98`

**Step 1: Replace raw handler call with timeout-wrapped version**

In `ToolRegistry::execute()`, replace line 98:

```rust
let start = Instant::now();
let timeout_duration = std::time::Duration::from_secs(tool.metadata.timeout_secs);
let result = match tokio::time::timeout(timeout_duration, (tool.handler)(tool_call.args.clone())).await {
    Ok(inner) => inner,
    Err(_) => anyhow::bail!("Tool '{}' timed out after {}s", tool_call.name, tool.metadata.timeout_secs),
};
let duration_ms = start.elapsed().as_millis() as i64;
```

**Step 2: Write test**

```rust
#[tokio::test]
async fn test_tool_timeout() {
    let mut reg = ToolRegistry::empty();
    reg.register_with_metadata(
        "slow_tool",
        "A tool that takes too long",
        serde_json::json!({"type": "object", "properties": {}}),
        Arc::new(|_args| Box::pin(async {
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            Ok("done".into())
        })),
        ToolMetadata {
            requires_approval: false,
            retryable: false,
            timeout_secs: 1, // 1 second timeout
        },
    );

    let mut tc = agenticos_core::state::ToolCall::new("slow_tool", serde_json::json!({}));
    let result = reg.execute(&mut tc, None, None, None).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("timed out"));
}
```

**Step 3: Run test**

Run: `cargo test -p agenticos-agent-runtime -- timeout`
Expected: PASS

**Step 4: Commit**

```bash
git add backend/agent-runtime/src/tool.rs
git commit -m "feat: add tool execution timeout via tokio::time::timeout"
```

---

### Task 1.2d: Add Retry Logic with Exponential Backoff

**Objective:** On transient errors, retry tool execution up to max_retries with exponential backoff using recovery module.

**Files:**
- Modify: `backend/agent-runtime/src/tool.rs`
- Modify: `backend/agent-runtime/src/loop_mod.rs` (use recovery for tool retries)

**Step 1: Add a `execute_with_retry()` method on ToolRegistry**

This is an alternative to `execute()` that wraps the retry logic. The key insight: retry should happen at the tool level for transient errors, not require the orchestrator to manage it.

```rust
use crate::error::classify_error;
use crate::recovery::{apply_recovery, RecoveryConfig};

pub async fn execute_with_retry(
    &self,
    tool_call: &mut agenticos_core::state::ToolCall,
    workspace_id: Option<&str>,
    run_id: Option<&str>,
    agent_id: Option<&str>,
) -> anyhow::Result<String> {
    let tool = self
        .tools
        .get(&tool_call.name)
        .ok_or_else(|| anyhow::anyhow!("Unknown tool: {}", tool_call.name))?;

    if !tool.metadata.retryable {
        return self.execute(tool_call, workspace_id, run_id, agent_id).await;
    }

    let recovery_config = RecoveryConfig::default();
    let mut recovery_state = RecoveryState::new();

    loop {
        let mut tc_attempt = tool_call.clone();
        match self.execute(&mut tc_attempt, workspace_id, run_id, agent_id).await {
            Ok(output) => {
                // Copy results back to original tool_call
                tool_call.result = tc_attempt.result;
                tool_call.duration_ms = tc_attempt.duration_ms;
                return Ok(output);
            }
            Err(e) => {
                let classified = classify_error(&e);
                if !classified.retryable || !tool.metadata.retryable {
                    // Copy error back
                    tool_call.error = tc_attempt.error;
                    tool_call.duration_ms = tc_attempt.duration_ms;
                    return Err(e);
                }

                let action = apply_recovery(&mut recovery_state, &recovery_config, true);
                match action {
                    crate::recovery::Retry(delay_ms) => {
                        tracing::warn!(
                            tool = %tool_call.name,
                            attempt = recovery_state.total_retries,
                            delay_ms = delay_ms,
                            "Tool retry with backoff"
                        );
                        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                        continue;
                    }
                    crate::recovery::CircuitBreak => {
                        tool_call.error = Some("Circuit breaker opened for tool".into());
                        return Err(anyhow::anyhow!("Circuit breaker opened after {} retries", recovery_state.total_retries));
                    }
                    crate::recovery::Fail(msg) => {
                        tool_call.error = Some(msg.clone());
                        return Err(anyhow::anyhow!(msg));
                    }
                }
            }
        }
    }
}
```

**Step 2: Update ToolMetadata default for timeout_secs**

Change `Default` impl so `timeout_secs` is 30 (not 0):

```rust
impl Default for ToolMetadata {
    fn default() -> Self {
        Self {
            requires_approval: false,
            retryable: true,
            timeout_secs: 30,
        }
    }
}
```

**Step 3: Write tests**

```rust
#[tokio::test]
async fn test_retry_on_transient_failure() {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    let call_count = Arc::new(AtomicU32::new(0));
    let count_clone = call_count.clone();

    let mut reg = ToolRegistry::empty();
    reg.register_with_metadata(
        "flaky_tool",
        "Fails first 2 times, succeeds on 3rd",
        serde_json::json!({"type": "object", "properties": {}}),
        Arc::new(move |_args| {
            let c = count_clone.clone();
            Box::pin(async move {
                let n = c.fetch_add(1, Ordering::SeqCst);
                if n < 2 {
                    Err(anyhow::anyhow!("HTTP 503 Service Unavailable"))
                } else {
                    Ok("success".into())
                }
            })
        }),
        ToolMetadata {
            requires_approval: false,
            retryable: true,
            timeout_secs: 30,
        },
    );

    let mut tc = agenticos_core::state::ToolCall::new("flaky_tool", serde_json::json!({}));
    let result = reg.execute_with_retry(&mut tc, None, None, None).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "success");
    assert_eq!(call_count.load(Ordering::SeqCst), 3);
}
```

**Step 4: Run tests**

Run: `cargo test -p agenticos-agent-runtime -- retry`
Expected: PASS

**Step 5: Commit**

```bash
git add backend/agent-runtime/src/tool.rs backend/agent-runtime/src/lib.rs
git commit -m "feat: add tool retry with exponential backoff using recovery module"
```

---

### Task 1.2e: Update loop_mod.rs and streaming.rs to Use execute_with_retry

**Objective:** Replace direct `tool_registry.execute()` calls with `tool_registry.execute_with_retry()`.

**Files:**
- Modify: `backend/agent-runtime/src/loop_mod.rs:199-206`
- Modify: `backend/agent-runtime/src/streaming.rs:209-216`

**Step 1: In loop_mod.rs, replace execute call**

Replace lines ~199-206:

```rust
let exec_result = tool_registry
    .execute_with_retry(
        &mut core_tc,
        state.workspace_id.as_deref(),
        state.run_id.as_deref(),
        state.agent_id.as_deref(),
    )
    .await;
```

**Step 2: In streaming.rs, do the same replacement**

Replace lines ~209-216 with the same `execute_with_retry()` call.

**Step 3: Verify**

Run: `cargo check --workspace`
Expected: compiles clean

**Step 4: Commit**

```bash
git add backend/agent-runtime/src/loop_mod.rs backend/agent-runtime/src/streaming.rs
git commit -m "feat: use execute_with_retry in loop and streaming paths"
```

---

### Task 1.1: Context Window Management

**Objective:** Prevent unbounded message growth by summarizing old messages when approaching context limits.

**Design:** 
- `ContextManager` struct in `context.rs` tracks token count estimates (chars/4 approximation)
- When approaching 80% of model capacity: summarize older messages, replace with summary
- Preserves: system prompt, last N messages, summary of earlier conversation
- Uses `Responder::summarize()` from `responder.rs`

**Files:**
- New: `backend/agent-runtime/src/context.rs`
- Modify: `backend/agent-runtime/src/lib.rs` (add `pub mod context`)
- Modify: `backend/agent-runtime/src/loop_mod.rs` (call context manager each iteration)
- Modify: `backend/agent-runtime/src/streaming.rs` (same for streaming path)
- New: `backend/agent-runtime/tests/context_tests.rs`

---

#### Task 1.1a: Create ContextManager

**Files:**
- Create: `backend/agent-runtime/src/context.rs`

**Implement:**

```rust
use agenticos_core::state::Message;

#[derive(Debug, Clone)]
pub struct ContextConfig {
    pub max_tokens: i64,
    pub summarize_threshold: f64, // 0.8 = summarize at 80% capacity
    pub keep_recent: usize, // number of recent messages to preserve
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            max_tokens: 8000,
            summarize_threshold: 0.8,
            keep_recent: 5,
        }
    }
}

pub struct ContextManager {
    config: ContextConfig,
}

impl ContextManager {
    pub fn new(config: ContextConfig) -> Self {
        Self { config }
    }

    /// Estimate token count from messages (rough: chars / 4)
    pub fn estimate_tokens(messages: &[Message]) -> i64 {
        let total_chars: usize = messages.iter().map(|m| m.content.len()).sum();
        (total_chars / 4) as i64
    }

    /// Check if messages exceed the summarization threshold
    pub fn should_summarize(&self, messages: &[Message]) -> bool {
        let tokens = Self::estimate_tokens(messages);
        (tokens as f64) >= (self.config.max_tokens as f64 * self.config.summarize_threshold)
    }

    /// Summarize old messages (all except system + last N), return pruned messages
    ///
    /// Strategy:
    /// 1. Keep system messages (first message)
    /// 2. Keep last N messages
    /// 3. Replace middle messages with a placeholder summary
    /// 4. The actual LLM summarization is done externally (we just prune here)
    pub fn prune_messages(&self, messages: &[Message]) -> Vec<Message> {
        if messages.len() <= self.config.keep_recent + 1 {
            // +1 for system message — already small enough
            return messages.to_vec();
        }

        let mut result = Vec::new();

        // Keep system message (first)
        if !messages.is_empty() {
            result.push(messages[0].clone());
        }

        // Insert summary placeholder
        let middle_count = messages.len() - 1 - self.config.keep_recent;
        result.push(Message::new(
            "system",
            &format!(
                "[Summary of earlier conversation: {} messages condensed during context management]",
                middle_count
            ),
        ));

        // Keep last N messages
        let start = messages.len() - self.config.keep_recent;
        for msg in &messages[start..] {
            result.push(msg.clone());
        }

        result
    }

    /// Full summarization pass: checks threshold, prunes if needed
    /// Returns (pruned_messages, did_summarize)
    pub fn maybe_prune(&self, messages: &mut Vec<Message>) -> bool {
        if self.should_summarize(messages) {
            let pruned = self.prune_messages(messages);
            *messages = pruned;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_msg(role: &str, content: &str) -> Message {
        Message::new(role, content)
    }

    #[test]
    fn test_estimate_tokens() {
        let messages = vec![
            make_msg("system", "You are a helpful assistant"), // ~32 chars = 8 tokens
            make_msg("user", "Hello"),                          // ~5 chars = 1 token
        ];
        let tokens = ContextManager::estimate_tokens(&messages);
        assert!(tokens > 0);
        assert_eq!(tokens, (32 + 5) / 4); // floor division
    }

    #[test]
    fn test_should_summarize_below_threshold() {
        let config = ContextConfig::default();
        let manager = ContextManager::new(config);
        let messages = vec![
            make_msg("system", "Short system prompt"),
            make_msg("user", "Hi"),
        ];
        assert!(!manager.should_summarize(&messages));
    }

    #[test]
    fn test_should_summarize_above_threshold() {
        let config = ContextConfig {
            max_tokens: 10,
            summarize_threshold: 0.8,
            keep_recent: 5,
        };
        let manager = ContextManager::new(config);
        // Create messages that exceed 8 tokens (10 * 0.8)
        let messages = vec![
            make_msg("system", "This is a longer system prompt that has many chars"),
            make_msg("user", "This message also has enough characters to push us past threshold"),
        ];
        assert!(manager.should_summarize(&messages));
    }

    #[test]
    fn test_prune_messages_keeps_system_and_recent() {
        let config = ContextConfig {
            max_tokens: 1000,
            summarize_threshold: 0.8,
            keep_recent: 2,
        };
        let manager = ContextManager::new(config);

        let messages = vec![
            make_msg("system", "system prompt"),
            make_msg("user", "old msg 1"),
            make_msg("assistant", "old msg 2"),
            make_msg("user", "old msg 3"),
            make_msg("user", "recent msg 1"),
            make_msg("assistant", "recent msg 2"),
        ];

        let pruned = manager.prune_messages(&messages);
        assert_eq!(pruned.len(), 4); // system + summary + 2 recent
        assert_eq!(pruned[0].role, "system");
        assert_eq!(pruned[0].content, "system prompt");
        assert!(pruned[1].content.contains("Summary")); // summary
        assert_eq!(pruned[2].content, "recent msg 1");
        assert_eq!(pruned[3].content, "recent msg 2");
    }

    #[test]
    fn test_prune_messages_short_enough() {
        let config = ContextConfig {
            max_tokens: 1000,
            ..Default::default()
        };
        let manager = ContextManager::new(config);
        let messages = vec![
            make_msg("system", "sys"),
            make_msg("user", "hello"),
            make_msg("assistant", "world"),
        ];
        let pruned = manager.prune_messages(&messages);
        assert_eq!(pruned.len(), 3); // unchanged
    }

    #[test]
    fn test_maybe_prune_returns_bool() {
        let config = ContextConfig {
            max_tokens: 10,
            summarize_threshold: 0.5, // very low threshold
            keep_recent: 2,
        };
        let manager = ContextManager::new(config);
        let mut messages = vec![
            make_msg("system", "This is a deliberately long system prompt for testing purposes"),
            make_msg("user", "This is also a deliberately long user message to exceed our very low token threshold"),
        ];
        let did_prune = manager.maybe_prune(&mut messages);
        assert!(did_prune);
        assert!(messages.len() < 4); // system + summary placeholder
    }
}
```

**Step 2: Add to lib.rs**

In `backend/agent-runtime/src/lib.rs`, add:

```rust
pub mod context;
pub use context::{ContextConfig, ContextManager};
```

**Step 3: Run tests**

Run: `cargo test -p agenticos-agent-runtime -- context`
Expected: PASS

**Step 4: Commit**

```bash
git add backend/agent-runtime/src/context.rs backend/agent-runtime/src/lib.rs
git commit -m "feat: add ContextManager for token-bounded message pruning"
```

---

#### Task 1.1b: Integrate ContextManager into loop_mod.rs

**Objective:** Call `ContextManager::maybe_prune()` at the start of each agent loop iteration.

**Files:**
- Modify: `backend/agent-runtime/src/loop_mod.rs`

**Step 1: Add import at top**

```rust
use crate::context::{ContextConfig, ContextManager};
```

**Step 2: After `while !state.should_stop()` at line 80, add context management**

After the opening brace of the while loop (after line 80), add:

```rust
        // Context window management: prune messages if approaching limit
        let context_config = ContextConfig {
            max_tokens: 16000, // conservative default
            summarize_threshold: 0.8,
            keep_recent: 5,
        };
        let context_manager = ContextManager::new(context_config);
        context_manager.maybe_prune(&mut state.messages);
```

**Step 3: Verify**

Run: `cargo check --workspace`
Expected: compiles clean

**Step 4: Run all tests to ensure nothing broke**

Run: `cargo test -p agenticos-agent-runtime`
Expected: PASS (all existing tests + new context tests)

**Step 5: Commit**

```bash
git add backend/agent-runtime/src/loop_mod.rs
git commit -m "feat: integrate ContextManager into agent loop"
```

---

#### Task 1.1c: Integrate ContextManager into streaming.rs

**Objective:** Same integration for the streaming path.

**Files:**
- Modify: `backend/agent-runtime/src/streaming.rs`

**Step 1: Add import**

```rust
use crate::context::{ContextConfig, ContextManager};
```

**Step 2: Add context pruning at start of while loop (after line 98)**

```rust
        let context_config = ContextConfig {
            max_tokens: 16000,
            summarize_threshold: 0.8,
            keep_recent: 5,
        };
        let context_manager = ContextManager::new(context_config);
        context_manager.maybe_prune(&mut state.messages);
```

**Step 3: Verify**

Run: `cargo test -p agenticos-agent-runtime`
Expected: PASS

**Step 4: Commit**

```bash
git add backend/agent-runtime/src/streaming.rs
git commit -m "feat: integrate ContextManager into streaming path"
```

---

## Phase 2: LLM Provider Abstraction

### Task 2.1: Multi-Provider LLM Client

**Objective:** Create a provider abstraction so the agent can use Anthropic, Google, and OpenAI natively.

**Design:**
- `LlmProvider` trait: `chat_completion()`, `chat_completion_stream()`, `embed_text()`
- `LlmClient` facade that routes to the correct provider based on model name prefix
- Provider auto-detection: "claude-*" → Anthropic, "gemini-*" → Google, others → OpenAI-compatible

**Files:**
- New: `backend/core/src/llm_provider.rs`
- Modify: `backend/core/src/llm.rs` (refactor LiteLLMClient into OpenAIProvider)
- Modify: `backend/core/src/lib.rs` (export new types)
- New: `backend/core/tests/llm_provider_tests.rs`

---

#### Task 2.1a: Define the LlmProvider trait

**Files:**
- Create: `backend/core/src/llm_provider.rs`

```rust
use crate::state::{Message, ToolCall, ToolDefinition};
use async_trait::async_trait;

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn chat_completion(
        &self,
        model: &str,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> crate::error::Result<crate::llm::LlmResponse>;

    async fn chat_completion_stream(
        &self,
        model: &str,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> crate::error::Result<tokio::sync::mpsc::Receiver<String>>;
}

/// Provider selector based on model name prefix
pub enum ProviderKind {
    OpenAI,
    Anthropic,
    Google,
}

pub fn detect_provider(model: &str) -> ProviderKind {
    if model.starts_with("claude") || model.starts_with("anthropic") {
        ProviderKind::Anthropic
    } else if model.starts_with("gemini") || model.starts_with("google") {
        ProviderKind::Google
    } else {
        ProviderKind::OpenAI
    }
}
```

**Step 1: Add async-trait to core Cargo.toml**

In `backend/core/Cargo.toml`, add:
```toml
async-trait = "0.1"
```

---

#### Task 2.1b: Refactor LiteLLMClient into OpenAIProvider

**Files:**
- Modify: `backend/core/src/llm.rs` (rename/extract the OpenAI-specific logic)

**Step 1: Extract OpenAIProvider**

Move the existing LiteLLMClient into an `OpenAIProvider` struct that implements `LlmProvider`:

```rust
pub struct OpenAIProvider {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
}

#[async_trait]
impl LlmProvider for OpenAIProvider {
    async fn chat_completion(
        &self,
        model: &str,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> crate::error::Result<LlmResponse> {
        // Existing chat_completion logic from LiteLLMClient
        // ...
    }

    async fn chat_completion_stream(
        &self,
        model: &str,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> crate::error::Result<tokio::sync::mpsc::Receiver<String>> {
        // Existing stream logic
        // ...
    }
}
```

**Step 2: Create LlmClient facade**

```rust
pub struct LlmClient {
    openai: OpenAIProvider,
    // anthropic: Option<AnthropicProvider>,  // Phase 2.1c
    // google: Option<GoogleProvider>,        // Phase 2.1c
}

impl LlmClient {
    pub fn new(config: &crate::config::AppConfig) -> crate::error::Result<Self> {
        Ok(Self {
            openai: OpenAIProvider::new(config)?,
        })
    }

    pub fn from_openai(provider: OpenAIProvider) -> Self {
        Self { openai: provider }
    }

    pub async fn chat_completion(
        &self,
        model: &str,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> crate::error::Result<LlmResponse> {
        match detect_provider(model) {
            ProviderKind::OpenAI => self.openai.chat_completion(model, messages, tools).await,
            _ => {
                // For now, fall back to OpenAI for unsupported providers
                // Anthropic/Google implementations come in later tasks
                self.openai.chat_completion(model, messages, tools).await
            }
        }
    }
}

// Backward compatibility: LiteLLMClient is now a type alias
pub type LiteLLMClient = LlmClient;
```

---

#### Task 2.1c: Add Provider Detection Tests

**Files:**
- Modify (create if not exists): `backend/core/tests/llm_provider_tests.rs`

```rust
#[cfg(test)]
mod tests {
    use super::super::llm_provider::detect_provider;

    #[test]
    fn test_detect_openai() {
        assert!(matches!(detect_provider("gpt-4"), ProviderKind::OpenAI));
        assert!(matches!(detect_provider("gpt-3.5-turbo"), ProviderKind::OpenAI));
    }

    #[test]
    fn test_detect_anthropic() {
        assert!(matches!(detect_provider("claude-3-opus"), ProviderKind::Anthropic));
        assert!(matches!(detect_provider("anthropic/claude-3"), ProviderKind::Anthropic));
    }

    #[test]
    fn test_detect_google() {
        assert!(matches!(detect_provider("gemini-1.5-pro"), ProviderKind::Google));
        assert!(matches!(detect_provider("google/gemini-pro"), ProviderKind::Google));
    }
}
```

**Verify:** `cargo test -p agenticos-core -- llm_provider`

**Commit:** `feat: add multi-provider LLM abstraction with trait + detection`

---

## Phase 3: Advanced Agent Patterns

### Task 3.1: Parallel Sub-Agent Execution

**Objective:** Implement true parallel execution when `ExecutionMode::Parallel` is set.

**Design:**
- Use `FuturesUnordered` from futures crate for concurrent sub-agent execution
- Each sub-agent gets its own AgentState and tool registry
- Results collected and merged by supervisor
- Config: `max_parallel_agents` (default: 4)

**Files:**
- New: `backend/agent-runtime/src/parallel.rs`
- Modify: `backend/agent-runtime/src/orchestrator.rs` (Parallel mode)
- Tests: `backend/agent-runtime/tests/parallel_tests.rs`

---

#### Task 3.1a: Implement Parallel Execution Engine

**Files:**
- Create: `backend/agent-runtime/src/parallel.rs`

Key implementation:
- `ParallelExecutor` struct with `FuturesUnordered`
- Spawn sub-agents as independent futures
- Collect results as they complete

---

#### Task 3.1b: Integrate into orchestrator.rs

In `TeamOrchestrator::run()`, when `config.execution_mode == ExecutionMode::Parallel`:
1. Parse tool_calls from plan step
2. Spawn independent sub-agents for non-delegate tool calls
3. Use `join_all` or `FuturesUnordered` to wait for all results
4. Merge results into supervisor state

---

### Task 3.2: Human-in-the-Loop Approval System

**Objective:** Add explicit approval flow for tools marked `requires_approval = true`.

**Files:**
- New: `backend/agent-runtime/src/approval.rs`
- New: `backend/api-server/src/routes/approvals.rs`
- New: `backend/migrations/0005_approvals.sql`
- Modify: `backend/agent-runtime/src/tool.rs` (set `requires_approval: true` for write_file)

---

### Task 3.3: Multi-Step Planning with Replanning

**Objective:** Execute plans step-by-step with failure replanning.

**Files:**
- Modify: `backend/agent-runtime/src/plan.rs`
- New: `backend/agent-runtime/src/plan_executor.rs`
- Modify: `backend/agent-runtime/src/planner.rs`

---

## Phase 4: Memory System Overhaul

### Task 4.1: Tiered Memory Architecture

**Objective:** Three-tier memory (working/recall/archival) with consolidation.

**Files:**
- New: `backend/memory/src/tiers.rs`
- New: `backend/memory/src/entities.rs`
- New: `backend/memory/src/consolidation.rs`
- Modify: `backend/memory/src/store.rs`
- Modify: `backend/memory/src/lib.rs`

### Task 4.2: Memory-Enhanced System Prompt

**Objective:** Dynamic system prompt that injects relevant memories.

**Files:**
- New: `backend/agent-runtime/src/prompt_builder.rs`
- Modify: `backend/agent-runtime/src/plan.rs`

---

## Phase 5: Tool Ecosystem Expansion

### Task 5.1: MCP Client

**Objective:** Connect to external tool servers via MCP protocol (stdio + HTTP).

**Files:**
- New: `backend/agent-runtime/src/mcp/mod.rs`
- New: `backend/agent-runtime/src/mcp/stdio.rs`
- New: `backend/agent-runtime/src/mcp/http.rs`
- New: `backend/agent-runtime/src/mcp/types.rs`
- Modify: `backend/agent-runtime/src/tool.rs` (register_mcp_tools)

### Task 5.2: Built-in Tool Expansion (web_search, web_fetch, http_request, memory tools)

**Files:**
- New: `backend/agent-runtime/src/tools/` directory

### Task 5.3: WASM Sandbox for Code Execution

**Files:**
- New: `backend/agent-runtime/src/sandbox.rs`
- Modify: `backend/agent-runtime/Cargo.toml` (add wasmtime)

---

## Phase 6: Observability & Cost Control

### Task 6.1: OpenTelemetry Integration

**Files:**
- New: `backend/core/src/otel.rs`
- Modify: `backend/core/Cargo.toml` (add opentelemetry, tracing-opentelemetry)

### Task 6.2: Cost Tracking & Budget Enforcement

**Files:**
- New: `backend/migrations/0006_workspace_usage.sql`
- New: `backend/api-server/src/routes/usage.rs`
- Modify: `backend/agent-runtime/src/loop_mod.rs`

---

## Phase 7: Testing & Evaluation

### Task 7.1: Agent Evaluation Harness

**Files:**
- New: `backend/agent-runtime/src/eval.rs`
- New: `backend/agent-runtime/tests/eval_harness.rs`
- New: `backend/agent-runtime/tests/eval_tasks/`

### Task 7.2: Security Test Suite

**Files:**
- New: `backend/agent-runtime/tests/security_tests.rs`
- New: `backend/guards/tests/security_tests.rs`

---

## Execution Order Summary

| Order | Task | Est. Complexity |
|-------|------|----------------|
| 1 | 1.2a: register_with_metadata() | Trivial (20min) |
| 2 | 1.2b: validate_tool_args() | Low (30min) |
| 3 | 1.2c: timeout wrapping | Trivial (15min) |
| 4 | 1.2d: retry with backoff | Medium (45min) |
| 5 | 1.2e: update loop + streaming | Trivial (15min) |
| 6 | 1.1a: ContextManager | Medium (45min) |
| 7 | 1.1b: integrate into loop_mod | Trivial (10min) |
| 8 | 1.1c: integrate into streaming | Trivial (10min) |
| 9 | 2.1a-c: LLM provider abstraction | High (90min) |
| 10 | 3.1a-b: parallel execution | High (90min) |
| 11 | 3.2: HITL approval system | High (120min) |
| 12 | 3.3: multi-step planning | High (90min) |
| 13 | 4.1: tiered memory | High (120min) |
| 14 | 4.2: memory-enhanced prompt | Medium (60min) |
| 15 | 5.1: MCP client | High (120min) |
| 16 | 5.2: built-in tool expansion | Medium (60min) |
| 17 | 5.3: WASM sandbox | Very High (180min) |
| 18 | 6.1: OpenTelemetry | Medium (45min) |
| 19 | 6.2: cost tracking + budgets | Medium (60min) |
| 20 | 7.1: eval harness | Medium (90min) |
| 21 | 7.2: security tests | Medium (60min) |

**Total estimated work: ~24 person-hours (3 days focused)**

All Phase 1 tasks (1-8) should be done first as they are foundational. Phase 2 (task 9) is a dependency for Phase 3+.

---

## Verification Strategy

After each phase:
1. `cargo test -p agenticos-agent-runtime` — all runtime tests pass
2. `cargo test -p agenticos-core` — all core tests pass
3. `cargo test -p agenticos-guards` — all guard tests pass
4. `cargo check --workspace` — workspace compiles clean
5. `cargo clippy --workspace` — no warnings (optional)

After all phases:
1. `cargo test --workspace` — full test suite passes
2. `cargo build --release` — release build succeeds
3. Manual: start server, send agent request via SSE endpoint, verify response

---

## Risks & Tradeoffs

1. **Rust compile times**: Each code change requires a full cargo check for verification. Budget ~2-3 min per iteration.
2. **Tool migration**: The `register_with_metadata()` approach means existing `register_builtin` tools all get default metadata (no approval needed, retryable, 30s timeout). This is fine for calculator/echo but needs explicit approval for file-write tools.
3. **ContextManager pruning**: The current design uses a simple prune (mark old messages as summarized placeholder). A more sophisticated approach would call `Responder::summarize()` on old messages, but that requires an LLM call per summarization — adds latency and cost. The current approach is YAGNI-compliant.
4. **Parallel execution**: Requires careful handling of shared state. The current `ToolRegistry` is `Arc`-based but not `Clone`. Parallel sub-agents will each need their own registry clone or shared access.
5. **MTP client**: The MCP spec is complex. Start with stdio transport only, HTTP transport is stretch.

---

## Open Questions

1. Should `ContextManager` call `Responder::summarize()` for actual summarization, or is the placeholder approach sufficient for v1?
2. Should the `LiteLLMClient → LlmClient` refactor preserve exact API compatibility (all callers use `.chat_completion()` the same way)?
3. For the eval harness, should results be written as JSON files (for CI) or queried via API endpoint?
4. Is the `async-trait` crate acceptable for the provider abstraction, or should we avoid macros?
