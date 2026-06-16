# AgenticOS Phase 7c-12b Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.
> Dispatch all coding to OpenCode with `opencode run '$(cat /c/Users/USER/agentic-platform/.hermes/prompts/<file>.txt)' --model opencode/deepseek-v4-flash-free --format json` and verify `cargo check --workspace` after each batch.

**Goal:** Complete 7 remaining tasks for AgenticOS: parallel sub-agent execution, OTel integration, OWASP ASI guardrails, HITL approval, WASM sandboxing, feature flags, and E2E test.

**Architecture:** Rust workspace (5 crates: core, agent-runtime, api-server, guards, memory).
- `agent-runtime` = agent loop, orchestrator, tools, guards integration
- `core` = shared types, state, OTel helpers, LLM client
- `guards` = bodyguard (tool access) + inspector (content policies)
- `api-server` = Axum HTTP layer, routes, AppState

**Tech Stack:** Rust 2024, axum 0.8, sqlx 08, tokio, wasmtime 25, opentelemetry 0.27, tracing

---

## Phase 7c: Parallel Sub-Agent Execution — Verify & Fix

### Task 7c-1: Verify orchestrator compiles (sequential baseline)

**Objective:** Ensure `orchestrator.rs` compiles and the sequential delegation logic is correct.

**Files:**
- Check: `backend/agent-runtime/src/orchestrator.rs:39-237`

**Current state:** The `run()` method processes tool calls in a `for tc in &tool_calls` loop (line 103). Delegate calls (line 104) are handled sequentially — each `self.run_sub_agent().await` blocks before the next. The handover says parallel code was partially written but doesn't exist in the current file.

**Step 1:** Write a prompt file:
```bash
cat > .hermes/prompts/7c_orchestrator_parallel.txt << 'PROMPT'
In backend/agent-runtime/src/orchestrator.rs, modify the TeamOrchestrator::run() method to support parallel sub-agent execution.

Context:
- The current code (lines 102-204) iterates over tool_calls in a for loop. When a tool call is "delegate", it calls self.run_sub_agent().await sequentially.
- ToolMetadata has requires_approval field already.
- There is NO ExecutionMode enum yet — you need to add one.

Changes needed:
1. Add this import at the top (after line 9): `use futures::future::join_all;`
   Also add this import if not present: `use std::collections::HashMap;`

2. In the run() method, after getting tool_calls (line 102), split them into two groups:
   - delegate_calls: Vec of (usize, &ToolCall) where tc.name == "delegate"
   - regular_calls: Vec of (usize, &ToolCall) where tc.name != "delegate"
   Use the index to track original ordering.

3. For delegate_calls, spawn them as tokio::spawn tasks:
   - Clone what you need for each task (sub_agent_id, task, run_id, workspace_id, user_id, config)
   - Use tokio::spawn(async move { ... }) to run each delegation in parallel
   - Collect results with join_all
   - Map results back to the original indices

4. For regular_calls, execute them sequentially as before (current code).

5. After all delegates and regular tools complete, add messages to supervisor_state in the original tool_calls order (use the tracked indices).

6. Ensure the compilation works: cargo check -p agenticos-agent-runtime

Pitfalls:
- You need to clone/Arc fields that move into spawned tasks
- The TeamOrchestrator fields (llm_client, tool_registry, etc.) are not Clone, so use Arc-wrapping or restructure
- Simplest approach: pre-extract all sub-agent configs before spawning, then join_all, then process results sequentially

Read the file first, understand the full structure, then make all edits in one pass.
Do NOT add "work fast". Focus on correctness. Handle edge cases.
PROMPT
```

**Step 2:** Run:
```bash
cd /c/Users/USER/agentic-platform/backend
opencode run '$(cat /c/Users/USER/agentic-platform/.hermes/prompts/7c_orchestrator_parallel.txt)' --model opencode/deepseek-v4-flash-free --format json
```

**Step 3:** Verify: `cargo check -p agenticos-agent-runtime` — expected: compiles with only pre-existing warnings.

---

## Phase 9a: OpenTelemetry Integration into Agent Loop

### Task 9a-1: Wire OTel spans into `loop_mod.rs`

**Objective:** Wrap agent loop body and tool executions with OTel spans from `core/src/otel.rs`.

**Files:**
- Modify: `backend/agent-runtime/src/loop_mod.rs:70-318` (run_inner function)
- Check: `backend/core/src/otel.rs:1-54` (span helpers already exist)

**Current state:** `core/src/otel.rs` defines `agent_run_span()`, `tool_call_span()`, `plan_step_span()`. The `init_otel()` function exists. But `loop_mod.rs` doesn't use any of them. The `#[tracing::instrument]` attribute is on `run_agent` and `run_inner` but these are basic tracing spans, not OTel.

**Span helpers available:**
- `agent_run_span(agent_id: &str, run_id: &str, workspace_id: &str) -> tracing::Span`
- `tool_call_span(tool_name: &str, run_id: &str) -> tracing::Span`
- `plan_step_span(iteration: u32, model: &str) -> tracing::Span`

**Step 1:** Write prompt file:
```bash
cat > .hermes/prompts/9a_otel_loop_mod.txt << 'PROMPT'
In backend/agent-runtime/src/loop_mod.rs, integrate OpenTelemetry spans from agenticos_core::otel.

The span helpers are at core/src/otel.rs and available via the core crate:
- init_otel(service_name: &str) -> Result<()>
- agent_run_span(agent_id, run_id, workspace_id) -> tracing::Span
- tool_call_span(tool_name, run_id) -> tracing::Span
- plan_step_span(iteration, model) -> tracing::Span

Import agenticos_core::otel in loop_mod.rs.

In the run_inner function (line 71):

1. At the TOP of the while loop (around line 80), before any work begins:
   Create an agent_run_span using state.agent_id, state.run_id, state.workspace_id.
   Use .enter() or let _span = ... to keep it alive for the iteration.

2. Around the plan_step call (line 106):
   Wrap with plan_step_span(state.iteration, &state.model).

3. Around EACH tool execution (the tool_registry.execute call, lines 198-206):
   Wrap with tool_call_span(&tc.name, state.run_id.as_deref().unwrap_or("")).

4. The spans should use tracing::info_span! and be entered/guarded properly.

Use .instrument(span) pattern on async blocks or use let _guard = span.enter(); for synchronous sections.

Focus on correctness. The spans should nest: agent_run_span > { plan_step_span, tool_call_span }.
Handle the Option<String> fields properly (they're already Option<String>, so .as_deref() or .as_ref() works).
PROMPT
```

**Step 2:** Run OpenCode with the prompt.

**Step 3:** Verify: `cargo check -p agenticos-agent-runtime`.

### Task 9a-2: Wire OTel spans into `streaming.rs`

**Objective:** Add OTel streaming.rs (same pattern as loop_mod.rs).

**Files:**
- Modify: `backend/agent-runtime/src/streaming.rs:85-338` (run_inner function)

**Step 1:** Write prompt file:
```bash
cat > .hermes/prompts/9a_otel_streaming.txt << 'PROMPT'
In backend/agent-runtime/src/streaming.rs, integrate OpenTelemetry spans from agenticos_core::otel.
This is nearly identical to what was done for loop_mod.rs:

Import agenticos_core::otel.

In the run_inner function (line 85):

1. At the TOP of the while loop, create an agent_run_span:

2. Around the plan_step call (line 101):
   Wrap with plan_step_span.

3. Around each tool execution (the tool_registry.execute call, lines 208-216):
   Wrap with tool_call_span.

Read the file first, follow the same pattern as loop_mod.rs.
Verify compilation after changes. Focus on correctness.
PROMPT
```

**Step 2:** Run OpenCode.

**Step 3:** Verify: `cargo check -p agenticos-agent-runtime`.

### Task 9a-3: Add init_otel() call in main.rs

**Objective:** Initialize OTel when the API server starts.

**Files:**
- Modify: `backend/api-server/src/main.rs:7-15`

**Current state:** `main.rs` initializes tracing_subscriber with fmt layer OTel is not initialized.

**Step 1:** Write prompt file:
```bash
cat > .hermes/prompts/9a_otel_main.txt << 'PROMPT'
In backend/api-server/src/main.rs, add a call to init_otel() after the tracing_subscriber is initialized.

1. Add import: `use agenticos_core::otel::init_otel;`
2. After line 15 (the .init() call), add:
   if let Err(e) = init_otel("agenticos-api-server") {
       tracing::warn!(error = %e, "Failed to initialize OpenTelemetry");
   }

This should go AFTER tracing_subscriber::fmt().init() because init_otel() also calls try_init() on the subscriber and we don't want to double-init. The init_otel internals use .try_init().ok() so it won't panic if already initialized.

Focus on correctness. Verify cargo check -p agenticos-api-server after.
PROMPT
```

**Step 2:** Run OpenCode.

**Step 3:** Verify: `cargo check -p agenticos-api-server`.

### Task 9a-4: Add tracing-opentelemetry to api-server Cargo.toml

**Objective:** Ensure api-server depends on tracing-opentelemetry (it already has OTel deps from workspace).

**Files:**
- Check: `backend/api-server/src/main.rs`
- Modify if needed: `backend/api-server/Cargo.toml`

**Step 1:** Verify the workspace already has the deps. api-server uses agenticos-core which re-exports core. Check if tracing-opentelemetry is transitively available. If cargo check fails with missing imports, add `tracing-opentelemetry = { workspace = true }` to api-server Cargo.toml dependencies.

---

## Phase 10a: OWASP ASI Guardrails (8 new inspector policies)

### Task 10a-1: Add 8 new policies to inspector.rs

**Objective:** Add 8 new OWASP ASI inspector policies to `guards/src/inspector.rs`.

**Files:**
- Modify: `backend/guards/src/inspector.rs:193-216` (after IterationLimit)
- Modify: `backend/guards/src/engine.rs:14-23` (register new policies)

**Current state:** 4 policies exist: PiiDetection, TokenBudgetPolicy, ContentModeration, IterationLimit.
`Inspector` has an `add_policy()` method. `GuardEngine::new()` registers all policies.

**New policies to add (after IterationLimit at line 216):**

1. **MemoryPoisoningPolicy** (ASI01) — Detects attempts to inject false memories/persistent context
2. **ToolMisusePolicy** (ASI02) — Detects dangerous tool argument combinations
3. **PrivilegeEscalationPolicy** (ASI03) — Detects attempts to gain unauthorized tool access
4. **GoalHijackingPolicy** (ASI04) — Detects attempts to modify agent's goal/instructions mid-run
5. **InsecureInterAgentPolicy** (ASI07) — Detects unvalidated inter-agent message passing
6. **CascadingFailurePolicy** (ASI08) — Detects excessive consecutive tool failures (potential cascade)
7. **HumanTrustManipulationPolicy** (ASI09) — Detects attempts to skip human approval steps
8. **RogueAgentPolicy** (ASI10) — Detects agent attempting to replicate/modify its own config

**Step 1:** Write prompt file:
```bash
cat > .hermes/prompts/10a_owasp_policies.txt << 'PROMPT'
In backend/guards/src/inspector.rs, add 8 new OWASP ASI inspector policies after the existing IterationLimit policy (line 216).

All policies implement the Policy trait (check method returns Pin<Box<dyn Future<Output = Option<GuardDecision>> + Send + 'a>>).

Each policy checks the AgentState (messages, iteration, tools, consecutive_errors, etc.) and returns Some(GuardDecision) if triggered, None otherwise.

Here are the policies:

1. MemoryPoisoningPolicy (ASI01):
   - Check: Look in the last 3 user messages for phrases that try to establish persistent context across runs
   - Triggers on: "remember this forever", "add to your permanent memory", "always do this", "never forget", "this is your new rule"
   - Action: GuardAction::Retry, reason: "Potential memory poisoning attempt detected"

2. ToolMisusePolicy (ASI02):
   - Check: Look at tool call arguments for dangerous patterns
   - Triggers on: calculator tool with expression containing ";" or "||" or backticks, or any tool with args containing OS command patterns like "|", ";", "$(", "&&" in string values
   - Action: GuardAction::Abort, reason: "Potential tool misuse — dangerous argument pattern"

3. PrivilegeEscalationPolicy (ASI03):
   - Check: Look at tool_calls in assistant messages for attempts to access tools not in the agent's tool list
   - Compare tool names against state.tools names
   - Triggers: if a tool call requests a tool not in state.tools[*].name
   - Action: GuardAction::Abort, reason: "Privilege escalation attempt — tool not in agent's allowed list"

4. GoalHijackingPolicy (ASI04):
   - Check: User messages containing phrases that try to override agent behavior
   - Triggers on: "ignore your instructions", "new instructions:", "your new task is", "disregard your goal", "forget what you're doing", "actually, you are now"
   - Action: GuardAction::Retry, reason: "Goal hijacking attempt detected"

5. InsecureInterAgentPolicy (ASI07):
   - Check: Messages containing unvalidated delegation patterns
   - Triggers on: tool results or tool calls containing raw SQL, shell commands, or code injection patterns in agent context
   - Look for: messages with role "tool" that contain suspicious executable content
   - Action: GuardAction::Retry, reason: "Potentially unsafe inter-agent content"

6. CascadingFailurePolicy (ASI08):
   - Check: state.consecutive_errors count
   - Triggers when: state.consecutive_errors >= 2 (lower threshold than max_consecutive_errors)
   - Action: GuardAction::Retry, reason: "Cascading failure detected — multiple consecutive tool errors"

7. HumanTrustManipulationPolicy (ASI09):
   - Check: User messages trying to bypass approval/oversight
   - Triggers on: "skip approval", "don't ask for permission", "you don't need approval", "override the review", "bypass human", "skip the review and execute"
   - Action: GuardAction::Abort, reason: "Attempt to skip human-in-the-loop approval"

8. RogueAgentPolicy (ASI10):
   - Check: agent messages trying to self-modify or replicate
   - Triggers on: assistant messages containing "I will modify my own", "update my instructions", "change my system prompt", "replicate myself", "create a copy of myself"
   - Action: GuardAction::Abort, reason: "Rogue agent behavior — self-modification/replication attempt"

For ALL policies: return None (not triggered) by default. The check should be efficient — short-circuit on first match.

Pattern matching: use to_lowercase().contains() for case-insensitive matching.

After adding all 8 policies, update GuardEngine::new() in backend/guards/src/engine.rs to register them:
inspector.add_policy(crate::inspector::MemoryPoisoningPolicy);
inspector.add_policy(crate::inspector::ToolMisusePolicy);
inspector.add_policy(crate::inspector::PrivilegeEscalationPolicy);
inspector.add_policy(crate::inspector::GoalHijackingPolicy);
inspector.add_policy(crate::inspector::InsecureInterAgentPolicy);
inspector.add_policy(crate::inspector::CascadingFailurePolicy);
inspector.add_policy(crate::inspector::HumanTrustManipulationPolicy);
inspector.add_policy(crate::inspector::RogueAgentPolicy);

Read both files first. Make ALL edits in one pass per file.
Verify: cargo check --workspace after both files are edited.
Focus on correctness. Handle edge cases.
PROMPT
```

**Step 2:** Run OpenCode.

**Step 3:** Verify: `cargo check --workspace`.

---

## Phase 10b: HITL Approval Integration

### Task 10b-1: Integrate approval into ToolRegistry.execute()

**Objective:** Add HITL approval check to tool execution path.

**Files:**
- Modify: `backend/agent-runtime/src/tool.rs:75-138` (execute method)
- Create: approval error type

**Current state:** `ToolMetadata` has `requires_approval: bool`. `ApprovalStore` exists in `approval.rs` but is NOT integrated into the agent loop or tool execution. The `ToolRegistry::execute()` method does not check for approval.

**Step 1:** Write prompt file:
```bash
cat > .hermes/prompts/10b_hitl_tool.txt << 'PROMPT'
In backend/agent-runtime/src/tool.rs, modify the ToolRegistry and its execute() method to support HITL approval.

Context:
- ToolImpl has a `metadata: ToolMetadata` field which has `requires_approval: bool`.
- approval.rs (in this same crate) has ApprovalStore with methods: create_request(), get_request(), approve(), reject(), is_approved(), list_pending().
- The execute() method (lines 75-138) currently just calls the tool handler directly.

Changes:

1. Add field to ToolRegistry struct:
   pub approval_store: std::sync::Arc<crate::approval::ApprovalStore>,
   
   Update ToolRegistry::new() to initialize it:
   approval_store: std::sync::Arc::new(crate::approval::ApprovalStore::new()),

2. In execute() method, BEFORE calling the tool handler (around line 85):
   - Check if tool.metadata.requires_approval is true
   - If yes, call self.approval_store.create_request() with run_id, agent_id, tool_name, tool_args
   - Return an Err with a clear message containing the approval ID, like:
     anyhow::anyhow!("APPROVAL_REQUIRED:{} — Tool '{}' requires human approval", approval_id, tool_call.name)
   - If no, proceed as before

3. Add a new method to ToolRegistry:
   pub async fn check_approval(&self, approval_id: &str) -> Option<bool> {
       self.approval_store.is_approved(approval_id).await
   }

4. Add a method to retry an approved tool call:
   pub async fn execute_approved(
       &self,
       tool_call: &mut agenticos_core::state::ToolCall,
       workspace_id: Option<&str>,
       run_id: Option<&str>,
       agent_id: Option<&str>,
       approval_id: &str,
   ) -> anyhow::Result<String> {
       match self.approval_store.is_approved(approval_id).await {
           Some(true) => {
               // Proceed with normal execution (call handler directly, skip approval check)
               // Reuse the handler call logic
           }
           _ => anyhow::bail!("Approval not granted or not found"),
       }
   }

Read the file first. Make all edits in one pass.
Verify: cargo check -p agenticos-agent-runtime.
Focus on correctness.
PROMPT
```

**Step 2:** Run OpenCode.

**Step 3:** Verify: `cargo check -p agenticos-agent-runtime`.

### Task 10b-2: Integrate approval into agent loop (loop_mod.rs)

**Objective:** Catch approval errors in the agent loop and pause execution.

**Files:**
- Modify: `backend/agent-runtime/src/loop_mod.rs:196-306` (tool execution section)

**Step 1:** Write prompt file:
```bash
cat > .hermes/prompts/10b_hitl_loop.txt << 'PROMPT'
In backend/agent-runtime/src/loop_mod.rs, modify the tool execution section to handle HITL approval errors.

Context:
- The tool execution happens around lines 196-306.
- When a tool requires approval, ToolRegistry::execute() now returns an Err with message starting "APPROVAL_REQUIRED:{id}".
- The agent loop should catch this, set the run status to "waiting_for_approval", and return.

Changes:

1. In the match exec_result block (around line 209), add a new arm:
   Err(ref e) if e.to_string().starts_with("APPROVAL_REQUIRED:") => {
       let approval_id = e.to_string()
           .trim_start_matches("APPROVAL_REQUIRED:")
           .split_whitespace()
           .next()
           .unwrap_or("")
           .to_string();
       
       tracing::info!(
           approval_id = %approval_id,
           tool = %tc.name,
           "Tool execution paused — awaiting human approval"
       );
       
       // Insert a run step recording the approval request
       insert_run_step(db, &state.run_id, "approval", "approval",
           Some(serde_json::to_string(&tc.args).unwrap_or_default()),
           Some(format!("Approval required: {}", approval_id)),
           exec_duration, 0,
           serde_json::json!({"event_type": "ApprovalRequired", "approval_id": approval_id, "tool_name": tc.name}),
       ).await?;
       
       // Add a message to state indicating approval is needed
       state.add_message(agenticos_core::state::Message::tool_result(
           &tc.name, &core_tc.id,
           &format!("APPROVAL_REQUIRED:{}", approval_id),
       ));
       
       // Return early — the run is now in "waiting_for_approval" state
       // The caller (route handler) will check for this and update the run status
       return Ok(());
   }

2. The existing Err(e) arm (line 277) should remain as the catch-all for non-approval errors.

Read the file first. Make edits in one pass.
Verify: cargo check -p agenticos-agent-runtime.
Focus on correctness.
PROMPT
```

**Step 2:** Run OpenCode.

**Step 3:** Verify: `cargo check -p agenticos-agent-runtime`.

### Task 10b-3: Add approval routes to api-server

**Objective:** Add HTTP endpoints for listing and resolving approval requests.

**Files:**
- Create: `backend/api-server/src/routes/approval.rs`
- Modify: `backend/api-server/src/routes/mod.rs` (add `pub mod approval;`)
- Modify: `backend/api-server/src/lib.rs` (add routes)

**Step 1:** Write prompt file:
```bash
cat > .hermes/prompts/10b_hitl_routes.txt << 'PROMPT'
Create backend/api-server/src/routes/approval.rs with HITL approval HTTP endpoints.

Context:
- The AppState in lib.rs has tool_registry: Arc<ToolRegistry> which has approval_store: Arc<ApprovalStore>
- ApprovalStore has: list_pending(), get_request(id), approve(id), reject(id)
- ApprovalRequest has fields: id, run_id, agent_id, tool_name, tool_name, tool_args, status, created_at

Create these endpoints:

1. GET /api/approvals — List all pending approval requests
   - Query: SELECT from the approval store's list_pending()
   - Returns: Json<Vec<ApprovalRequest>>

2. POST /api/approvals/{id}/approve — Approve a request
   - Calls approval_store.approve(id)
   - Returns: Json with {"approved": true, "id": id}

3. POST /api/approvals/{id}/reject — Reject a request
   - Calls approval_store.reject(id)
   - Returns: Json with {"rejected": true, "id": id}

All routes require auth (use the same auth_middleware as other protected routes).
Use the same error handling patterns as other routes in this project (internal_error, bad_request, not_found).

Then:
- In backend/api-server/src/routes/mod.rs, add: pub mod approval;
- In backend/api-server/src/lib.rs, add these routes to the protected_routes Router:
  .route("/api/approvals", get(routes::approval::list_approvals))
  .route("/api/approvals/{id}/approve", post(routes::approval::approve_request))
  .route("/api/approvals/{id}/reject", post(routes::approval::reject_request))

Read existing route files (e.g., runs.rs) for the exact patterns used.
Make all edits in one pass. Verify: cargo check --workspace.
Focus on correctness.
PROMPT
```

**Step 2:** Run OpenCode.

**Step 3:** Verify: `cargo check --workspace`.

### Task 10b-4: Mark filesystem tools as requiring approval

**Objective:** Set `requires_approval: true` on write_file and list_dir tools.

**Files:**
- Find where filesystem tools are registered (search for where ToolRegistry gets the fs tools)

**Note:** The filesystem tools (fs_read_file, fs_write_file, fs_list_dir) are in `filesystem.rs` but I couldn't find where they're registered as ToolImpl. They may be registered in the agent setup code in the route handlers. Search for where they're added to the tool registry and set `requires_approval: true` on write_file and list_dir.

**Step 1:** Search for where filesystem tools are registered:
```bash
cd /c/Users/USER/agentic-platform/backend
rg -n "read_file|write_file|list_dir" --type rust -g '!filesystem.rs' -g '!security_tests.rs'
```

**Step 2:** Based on results, write a prompt to set `requires_approval: true` on the write_file and list_dir tool registrations.

---

## Phase 10c: WASM Sandboxing

### Task 10c-1: Create sandbox.rs with WasmSandbox

**Objective:** Create a WASM sandbox for tool execution using wasmtime.

**Files:**
- Create: `backend/agent-runtime/src/sandbox.rs`
- Modify: `backend/agent-runtime/src/lib.rs` (add `pub mod sandbox;`)
- Modify: `backend/agent-runtime/Cargo.toml` (ensure wasmtime dep)

**Step 1:** Write prompt file:
```bash
cat > .hermes/prompts/10c_wasm_sandbox.txt << 'PROMPT'
Create backend/agent-runtime/src/sandbox.rs with a WasmSandbox struct that uses wasmtime to sandbox tool execution.

Context:
- wasmtime = "25" is already in workspace dependencies
- The sandbox should provide a way to execute untrusted code (e.g., from calculator tool or user-provided code) in a WASM sandbox
- This is a foundational component — it doesn't need to be fully integrated into the tool execution path yet

Create:

1. WasmSandbox struct with:
   - engine: wasmtime::Engine
   - store: wasmtime::Store (with a simple wasmtime::Store<()>)
   - A constructor `new() -> Self`

2. Methods:
   - `fn execute(&mut self, wasm_bytes: &[u8], func_name: &str, args: &[wasmtime::Val]) -> anyhow::Result<Vec<wasmtime::Val>>`
     - Compiles and instantiates the WASM module
     - Calls the named function with the given args
     - Returns the results
   - `fn execute_expression(&mut self, expr: &str) -> anyhow::Result<String>`
     - A convenience method that takes a string expression
     - For now, just return Ok(expr.to_string()) as a placeholder
     - This will be extended later with actual WASM compilation

3. Safety: The sandbox should use wasmtime's default limiting (no fuel limits needed for now, but add a comment about future work)

4. Add `pub mod sandbox;` to backend/agent-runtime/src/lib.rs

5. Ensure agent-runtime/Cargo.toml has wasmtime:
   wasmtime = { workspace = true }

Read the existing lib.rs and Cargo.toml first.
Verify: cargo check -p agenticos-agent-runtime.
Focus on correctness. This is a foundation — it doesn't need to be perfect.
PROMPT
```

**Step 2:** Run OpenCode.

**Step 3:** Verify: `cargo check -p agenticos-agent-runtime`.

---

## Phase 11b: Feature Flags

### Task 11b-1: Create core/src/features.rs

**Objective:** Create in-memory feature flags system.

**Files:**
- Create: `backend/core/src/features.rs`
- Modify: `backend/core/src/lib.rs` (add `pub mod features;`)

**Step 1:** Write prompt file:
```bash
cat > .hermes/prompts/11b_features_core.txt << 'PROMPT'
Create backend/core/src/features.rs with an in-memory feature flags system.

Requirements:

1. FeatureFlags struct:
   - Fields (all bool, default false):
     parallel_subagents: bool (default: true)
     planning_tool: bool (default: true)
     filesystem_tools: bool (default: true)
     mcp_tools: bool (default: true)
     hitl_approval: bool (default: false)
     wasm_sandbox: bool (default: false)
     cost_dashboard: bool (default: true)
   - Wrap in Arc<tokio::sync::RwLock<HashMap<String, bool>>> for thread-safe access
   - Or use individual Arc<std::sync::atomic::AtomicBool> fields for simplicity

2. Methods:
   - fn new() -> Self (with defaults as specified above)
   - fn is_enabled(&self, flag: &str) -> bool
   - fn enable(&mut self, flag: &str)
   - fn disable(&mut self, flag: &str)
   - fn all_flags(&self) -> Vec<(String, bool)> — returns all flags as key-value pairs

3. Make it Clone-friendly (derive Clone or use Arc internally).

4. Add `pub mod features;` to backend/core/src/lib.rs

Read core/src/lib.rs first to see existing modules.
Verify: cargo check -p agenticos-core.
Focus on correctness.
PROMPT
```

**Step 2:** Run OpenCode.

**Step 3:** Verify: `cargo check -p agenticos-core`.

### Task 11b-2: Add feature flag routes to api-server

**Objective:** Add HTTP endpoints for managing feature flags.

**Files:**
- Create: `backend/api-server/src/routes/features.rs`
- Modify: `backend/api-server/src/routes/mod.rs` (add `pub mod features;`)
- Modify: `backend/api-server/src/lib.rs` (add routes, add FeatureFlags to AppState)

**Step 1:** Write prompt file:
```bash
cat > .hermes/prompts/11b_features_routes.txt << 'PROMPT'
Create backend/api-server/src/routes/features.rs with feature flag HTTP endpoints.

Context:
- agenticos_core::features::FeatureFlags has: new(), is_enabled(flag), enable(flag), disable(flag), all_flags()
- AppState needs to hold FeatureFlags (add field: pub feature_flags: std::sync::Arc<agenticos_core::features::FeatureFlags>)
- Update AppState::new() to initialize feature_flags

Create these endpoints:

1. GET /api/features — List all feature flags
   - Returns: Json<serde_json::Value> with all flags as key-value pairs
   - Uses feature_flags.all_flags()

2. POST /api/features/{flag}/enable — Enable a feature flag
   - Calls feature_flags.enable(flag)
   - Returns: Json with {"flag": flag, "enabled": true}

3. POST /api/features/{flag}/disable — Disable a feature flag
   - Calls feature_flags.disable(flag)
   - Returns: Json with {"flag": flag, "enabled": false}

All routes require auth (same as other protected routes).
Use same error handling patterns as other routes.

Then:
- In backend/api-server/src/routes/mod.rs, add: pub mod features;
- In backend/api-server/src/lib.rs:
  - Add feature_flags: Arc<FeatureFlags> to AppState struct
  - Initialize it in AppState::new()
  - Add routes to protected_routes Router:
    .route("/api/features", get(routes::features::list_features))
    .route("/api/features/{flag}/enable", post(routes::features::enable_feature))
    .route("/api/features/{flag}/disable", post(routes::features::disable_feature))

Read existing route files and lib.rs for exact patterns.
Make all edits in one pass. Verify: cargo check --workspace.
Focus on correctness.
PROMPT
```

**Step 2:** Run OpenCode.

**Step 3:** Verify: `cargo check --workspace`.

---

## Phase 12b: E2E Integration Test

### Task 12b-1: Create E2E test

**Objective:** Create a full-flow integration test for the API server.

**Files:**
- Create: `backend/api-server/tests/e2e_test.rs`

**Step 1:** Write prompt file:
```bash
cat > .hermes/prompts/12b_e2e_test.txt << 'PROMPT'
Create backend/api-server/tests/e2e_test.rs with an end-to-end integration test.

Context:
- The api-server uses axum, sqlx with PostgreSQL
- Dev dependencies already include: reqwest, tokio, serde_json, tower, tower-http
- The test needs a running database — use the test pattern of starting the server and hitting it

Create a test that:

1. Sets up a test database connection (use dotenvy to load .env or set DATABASE_URL to a test DB)
2. Creates the AppState and router
3. Uses reqwest or tower::ServiceExt to make requests against the router

Test flow (test_full_flow):
1. POST /api/auth/register — Register a new user
   - Body: {"email": "test_e2e@example.com", "password": "testpass123", "workspace_id": "<generate uuid>"}
   - Assert: 200 OK, returns token

2. POST /api/auth/login — Login
   - Body: {"email": "test_e2e@example.com", "password": "testpass123"}
   - Assert: 200 OK, extract token

3. POST /api/agents — Create an agent (use the token from step 2)
   - Body: {"name": "Test Agent", "instructions": "You are a helpful assistant", "model": "gpt-4o"}
   - Assert: 200 OK, extract agent_id

4. GET /api/agents/{id} — Get the agent
   - Assert: 200 OK, name matches

5. POST /api/agents/{id}/run — Run the agent
   - Body: {"input": "Say hello"}
   - Assert: 200 OK, has run_id

6. GET /api/runs/{id} — Get the run
   - Assert: 200 OK

7. GET /api/runs — List runs
   - Assert: 200 OK, at least 1 run

8. GET /api/workspaces — List workspaces
   - Assert: 200 OK

Use tokio::test for async tests.
Use reqwest::Client for HTTP requests.
Start the server on a random port (e.g., 0 for OS-assigned) in a tokio::spawn.
Wait briefly for server startup.

IMPORTANT: This test requires a running PostgreSQL database. Use a test-specific database or skip if DATABASE_URL is not set. Use the pattern:
```
let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
    eprintln!("Skipping E2E test — DATABASE_URL not set");
    return;
});
```

Read existing test files (security_tests.rs, eval_harness.rs) for patterns.
Verify: cargo test -p api-server --test e2e_test (if DB is available) or at least cargo check -p api-server.
Focus on correctness.
PROMPT
```

**Step 2:** Run OpenCode.

**Step 3:** Verify: `cargo check -p api-server` (full test requires DB).

---

## Execution Order

**Batch 1 (Sequential — foundation):**
1. Task 7c-1: Parallel orchestrator
2. Task 9a-1: OTel in loop_mod.rs
3. Task 9a-2: OTel in streaming.rs
4. Task 9a-3: init_otel in main.rs

**Batch 2 (Parallel — independent files):**
5. Task 10a-1: OWASP ASI policies (inspector.rs + engine.rs)
6. Task 10c-1: WASM sandbox (sandbox.rs)
7. Task 11b-1: Feature flags core (features.rs)

**Batch 3 (Sequential — depends on Batch 2):**
8. Task 10b-1: HITL in tool.rs
9. Task 10b-2: HITL in loop_mod.rs
10. Task 10b-3: HITL routes (approval.rs + mod.rs + lib.rs)
11. Task 10b-4: Mark fs tools for approval

**Batch 4 (Parallel — independent):**
12. Task 11b-2: Feature flag routes
13. Task 12b-1: E2E test

**Final verification:** `cargo check --workspace` — must pass with only pre-existing warnings.

---

## Verification Steps

After ALL tasks complete:
1. `cd backend && cargo check --workspace` — must pass
2. `cd backend && cargo test -p agenticos-agent-runtime` — existing tests pass
3. `cd backend && cargo test -p agenticos-guards` — existing tests pass
4. `cd backend && cargo test -p agenticos-api-server` — existing tests pass
5. `cd frontend && npm run build` — frontend still compiles

---

## Risks & Notes

- **Parallel orchestrator**: The main risk is ownership/borrowing with tokio::spawn. TeamOrchestrator fields aren't Clone, so the spawned tasks need to extract data before spawning. Consider restructuring to pre-extecute sub-agent configs.
- **OTel**: `tracing_subscriber::init()` panics if already called — `init_otel()` already uses `.try_init().ok()` so this is handled.
- **HITL approval**: The approval store is in-memory only (no DB persistence). This is acceptable for the current phase.
- **WASM sandbox**: This is a foundation — full integration with tool execution is deferred.
- **E2E test**: Requires PostgreSQL. The test should gracefully skip if DATABASE_URL is not set.
- **OpenCode pitfalls**: Always verify `cargo check --workspace` after each batch. OpenCode may forget to update lib.rs exports or Cargo.toml dependencies.
