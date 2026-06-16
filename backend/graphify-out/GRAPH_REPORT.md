# Graph Report - .  (2026-06-16)

## Corpus Check
- Corpus is ~28,425 words - fits in a single context window. You may not need a graph.

## Summary
- 1047 nodes · 2438 edges · 54 communities (53 shown, 1 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS · INFERRED: 12 edges (avg confidence: 0.8)
- Token cost: 0 input · 0 output

## Community Hubs (Navigation)
- [[_COMMUNITY_Community 0|Community 0]]
- [[_COMMUNITY_Community 1|Community 1]]
- [[_COMMUNITY_Community 2|Community 2]]
- [[_COMMUNITY_Community 3|Community 3]]
- [[_COMMUNITY_Community 4|Community 4]]
- [[_COMMUNITY_Community 5|Community 5]]
- [[_COMMUNITY_Community 6|Community 6]]
- [[_COMMUNITY_Community 7|Community 7]]
- [[_COMMUNITY_Community 8|Community 8]]
- [[_COMMUNITY_Community 9|Community 9]]
- [[_COMMUNITY_Community 10|Community 10]]
- [[_COMMUNITY_Community 11|Community 11]]
- [[_COMMUNITY_Community 12|Community 12]]
- [[_COMMUNITY_Community 13|Community 13]]
- [[_COMMUNITY_Community 14|Community 14]]
- [[_COMMUNITY_Community 15|Community 15]]
- [[_COMMUNITY_Community 16|Community 16]]
- [[_COMMUNITY_Community 17|Community 17]]
- [[_COMMUNITY_Community 18|Community 18]]
- [[_COMMUNITY_Community 19|Community 19]]
- [[_COMMUNITY_Community 20|Community 20]]
- [[_COMMUNITY_Community 21|Community 21]]
- [[_COMMUNITY_Community 22|Community 22]]
- [[_COMMUNITY_Community 23|Community 23]]
- [[_COMMUNITY_Community 24|Community 24]]
- [[_COMMUNITY_Community 25|Community 25]]
- [[_COMMUNITY_Community 26|Community 26]]
- [[_COMMUNITY_Community 27|Community 27]]
- [[_COMMUNITY_Community 28|Community 28]]
- [[_COMMUNITY_Community 29|Community 29]]
- [[_COMMUNITY_Community 30|Community 30]]
- [[_COMMUNITY_Community 31|Community 31]]
- [[_COMMUNITY_Community 32|Community 32]]
- [[_COMMUNITY_Community 33|Community 33]]
- [[_COMMUNITY_Community 34|Community 34]]
- [[_COMMUNITY_Community 35|Community 35]]
- [[_COMMUNITY_Community 36|Community 36]]
- [[_COMMUNITY_Community 37|Community 37]]
- [[_COMMUNITY_Community 38|Community 38]]
- [[_COMMUNITY_Community 39|Community 39]]
- [[_COMMUNITY_Community 40|Community 40]]
- [[_COMMUNITY_Community 41|Community 41]]
- [[_COMMUNITY_Community 42|Community 42]]
- [[_COMMUNITY_Community 43|Community 43]]
- [[_COMMUNITY_Community 44|Community 44]]
- [[_COMMUNITY_Community 45|Community 45]]
- [[_COMMUNITY_Community 47|Community 47]]

## God Nodes (most connected - your core abstractions)
1. `test_state()` - 41 edges
2. `MemoryClient` - 26 edges
3. `AgentState` - 20 edges
4. `ToolRegistry` - 19 edges
5. `build_test_app()` - 19 edges
6. `run_inner()` - 18 edges
7. `AgentEvent` - 18 edges
8. `update_workspace()` - 17 edges
9. `run_inner()` - 16 edges
10. `upload_document()` - 16 edges

## Surprising Connections (you probably didn't know these)
- `run_agent_stream()` --calls--> `run_agent_streaming()`  [INFERRED]
  api-server/src/routes/agents.rs → agent-runtime/src/streaming.rs
- `run_inner()` --calls--> `evaluate_step()`  [INFERRED]
  agent-runtime/src/streaming.rs → agent-runtime/src/evaluate.rs
- `run_inner()` --calls--> `plan_step()`  [INFERRED]
  agent-runtime/src/loop_mod.rs → agent-runtime/src/plan.rs
- `build_test_app()` --calls--> `create_router()`  [INFERRED]
  api-server/tests/api_test.rs → api-server/src/lib.rs
- `run_inner()` --calls--> `evaluate_step()`  [INFERRED]
  agent-runtime/src/loop_mod.rs → agent-runtime/src/evaluate.rs

## Import Cycles
- 1-file cycle: `agent-runtime/src/approval.rs -> agent-runtime/src/approval.rs`
- 1-file cycle: `agent-runtime/src/compaction.rs -> agent-runtime/src/compaction.rs`
- 1-file cycle: `agent-runtime/src/context.rs -> agent-runtime/src/context.rs`
- 1-file cycle: `agent-runtime/src/filesystem.rs -> agent-runtime/src/filesystem.rs`
- 1-file cycle: `agent-runtime/src/loop_mod.rs -> agent-runtime/src/loop_mod.rs`
- 1-file cycle: `agent-runtime/src/mcp.rs -> agent-runtime/src/mcp.rs`
- 1-file cycle: `agent-runtime/src/memory_extractor.rs -> agent-runtime/src/memory_extractor.rs`
- 1-file cycle: `agent-runtime/src/orchestrator.rs -> agent-runtime/src/orchestrator.rs`
- 1-file cycle: `guards/src/inspector.rs -> guards/src/inspector.rs`
- 1-file cycle: `agent-runtime/src/plan.rs -> agent-runtime/src/plan.rs`
- 1-file cycle: `agent-runtime/src/planner.rs -> agent-runtime/src/planner.rs`
- 1-file cycle: `agent-runtime/src/rag.rs -> agent-runtime/src/rag.rs`
- 1-file cycle: `agent-runtime/src/responder.rs -> agent-runtime/src/responder.rs`
- 1-file cycle: `agent-runtime/src/streaming.rs -> agent-runtime/src/streaming.rs`
- 1-file cycle: `agent-runtime/src/tool.rs -> agent-runtime/src/tool.rs`
- 1-file cycle: `agent-runtime/tests/pipeline_stress_test.rs -> agent-runtime/tests/pipeline_stress_test.rs`
- 1-file cycle: `guards/src/engine.rs -> guards/src/engine.rs`
- 1-file cycle: `api-server/src/lib.rs -> api-server/src/lib.rs`
- 1-file cycle: `api-server/src/main.rs -> api-server/src/main.rs`
- 1-file cycle: `api-server/src/middleware/auth.rs -> api-server/src/middleware/auth.rs`

## Communities (54 total, 1 thin omitted)

### Community 0 - "Community 0"
Cohesion: 0.06
Nodes (51): AgentState, Bodyguard, AgentState, Default, GuardDecision, Option, Result, Self (+43 more)

### Community 1 - "Community 1"
Cohesion: 0.11
Nodes (29): Box, Future, AgentState, Default, GuardDecision, Option, Self, Send (+21 more)

### Community 2 - "Community 2"
Cohesion: 0.12
Nodes (31): Arc, Default, HashMap, Option, Result, Self, String, ToolCall (+23 more)

### Community 3 - "Community 3"
Cohesion: 0.08
Nodes (40): AgentError, ChatFunctionCall, ChatMessage, ChatTool, ChatToolCall, ChatToolFunction, AppConfig, Client (+32 more)

### Community 4 - "Community 4"
Cohesion: 0.10
Nodes (26): DateTime, Default, GuardAction, HashMap, Option, Self, String, Utc (+18 more)

### Community 5 - "Community 5"
Cohesion: 0.13
Nodes (18): AppConfig, Client, DateTime, HashMap, Mutex, Option, Result, Self (+10 more)

### Community 6 - "Community 6"
Cohesion: 0.23
Nodes (37): AgentVersionRow, AgentRow, AppState, Claims, E, Extension, Json, Option (+29 more)

### Community 7 - "Community 7"
Cohesion: 0.08
Nodes (21): AppConfig, Router, String, build_test_app(), build_test_config(), create_test_jwt(), test_basic_auth_instead_of_bearer_returns_401(), test_empty_authorization_header_returns_401() (+13 more)

### Community 8 - "Community 8"
Cohesion: 0.25
Nodes (36): AppState, Claims, DateTime, E, Extension, Json, Option, Path (+28 more)

### Community 9 - "Community 9"
Cohesion: 0.11
Nodes (18): DateTime, Option, Self, String, Utc, Value, Vec, Entity (+10 more)

### Community 10 - "Community 10"
Cohesion: 0.23
Nodes (34): AppState, Claims, E, Extension, Json, Option, Path, Query (+26 more)

### Community 11 - "Community 11"
Cohesion: 0.14
Nodes (31): AgentState, KbRetriever, LiteLLMClient, MemoryClient, Option, Result, String, AgentState (+23 more)

### Community 12 - "Community 12"
Cohesion: 0.13
Nodes (22): Arc, GuardEngine, LiteLLMClient, MemoryClient, Option, PgPool, Result, Self (+14 more)

### Community 13 - "Community 13"
Cohesion: 0.23
Nodes (26): AppState, Claims, E, Extension, Json, Option, Path, Result (+18 more)

### Community 14 - "Community 14"
Cohesion: 0.23
Nodes (26): AppState, Claims, E, Extension, Json, Option, Path, Query (+18 more)

### Community 15 - "Community 15"
Cohesion: 0.13
Nodes (15): Arc, DateTime, Default, HashMap, Mutex, Option, Self, String (+7 more)

### Community 16 - "Community 16"
Cohesion: 0.17
Nodes (22): AgentState, GuardAction, GuardDecision, Option, Result, AgentState, GuardEngine, KbRetriever (+14 more)

### Community 17 - "Community 17"
Cohesion: 0.14
Nodes (23): Error, LiteLLMClient, MemoryClient, Result, String, TcpStream, build_llm_client(), build_memory_client() (+15 more)

### Community 18 - "Community 18"
Cohesion: 0.16
Nodes (18): PgPool, AppConfig, Arc, GuardEngine, KbRetriever, LiteLLMClient, MemoryClient, PgPool (+10 more)

### Community 19 - "Community 19"
Cohesion: 0.21
Nodes (13): Default, Message, Self, Vec, ContextConfig, ContextManager, maybe_prune_returns_false_when_not_needed(), maybe_prune_returns_true_when_pruned() (+5 more)

### Community 20 - "Community 20"
Cohesion: 0.17
Nodes (12): DashMap, Arc, Default, HashMap, Result, Self, String, Value (+4 more)

### Community 21 - "Community 21"
Cohesion: 0.20
Nodes (19): AppState, Claims, E, Extension, Json, Option, Path, Query (+11 more)

### Community 22 - "Community 22"
Cohesion: 0.20
Nodes (15): Client, Result, Self, String, ToolRegistry, Value, Vec, McpContent (+7 more)

### Community 23 - "Community 23"
Cohesion: 0.26
Nodes (18): AppState, Claims, E, Error, Extension, Json, Result, State (+10 more)

### Community 24 - "Community 24"
Cohesion: 0.21
Nodes (15): Mem0Result, Client, Option, Result, Self, String, Value, Vec (+7 more)

### Community 25 - "Community 25"
Cohesion: 0.47
Nodes (17): DateTime, Option, String, Utc, Uuid, Value, AgentRow, AgentVersionRow (+9 more)

### Community 26 - "Community 26"
Cohesion: 0.21
Nodes (13): AppState, Json, Response, Result, State, StatusCode, String, Value (+5 more)

### Community 27 - "Community 27"
Cohesion: 0.29
Nodes (9): Default, Self, String, Vec, Regex, CompiledPattern, PiiDetector, PiiFinding (+1 more)

### Community 28 - "Community 28"
Cohesion: 0.26
Nodes (12): Error, Option, String, ClassifiedError, classify_error(), ErrorKind, test_classify_context_overflow(), test_classify_guard_denied() (+4 more)

### Community 29 - "Community 29"
Cohesion: 0.18
Nodes (7): Arc, HashMap, Self, String, Vec, RwLock, FeatureFlags

### Community 30 - "Community 30"
Cohesion: 0.33
Nodes (11): LiteLLMClient, Message, Option, String, Vec, extract_facts(), extract_procedure(), ExtractedFact (+3 more)

### Community 31 - "Community 31"
Cohesion: 0.30
Nodes (11): RecoveryAction, RecoveryConfig, RecoveryState, apply_recovery(), test_apply_recovery_non_retryable(), test_apply_recovery_retryable(), test_circuit_breaker_after_max_failures(), test_config() (+3 more)

### Community 32 - "Community 32"
Cohesion: 0.29
Nodes (9): Option, String, Vec, EvalResult, EvalTask, evaluate_output(), get_standard_tasks(), print_eval_report() (+1 more)

### Community 33 - "Community 33"
Cohesion: 0.27
Nodes (7): LiteLLMClient, PgPool, Result, Self, String, Vec, KbRetriever

### Community 34 - "Community 34"
Cohesion: 0.47
Nodes (9): AppState, Json, Path, State, String, Value, approve_request(), list_approvals() (+1 more)

### Community 35 - "Community 35"
Cohesion: 0.47
Nodes (9): AppState, Json, Path, State, String, Value, disable_feature(), enable_feature() (+1 more)

### Community 36 - "Community 36"
Cohesion: 0.56
Nodes (8): Result, String, PathBuf, ensure_workspace(), fs_list_dir(), fs_read_file(), fs_write_file(), workspace_dir()

### Community 37 - "Community 37"
Cohesion: 0.32
Nodes (6): AgentState, LiteLLMClient, Message, Result, compact_context(), estimate_context_tokens()

### Community 38 - "Community 38"
Cohesion: 0.43
Nodes (7): LiteLLMClient, Result, String, Vec, create_plan(), Plan, PlanStep

### Community 39 - "Community 39"
Cohesion: 0.39
Nodes (4): LiteLLMClient, Self, String, Responder

### Community 40 - "Community 40"
Cohesion: 0.29
Nodes (4): Result, Self, String, WasmSandbox

### Community 41 - "Community 41"
Cohesion: 0.38
Nodes (6): Result, Span, agent_run_span(), init_otel(), plan_step_span(), tool_call_span()

### Community 42 - "Community 42"
Cohesion: 0.47
Nodes (5): AgentRow, AgentState, Claims, Option, build_agent_state()

### Community 43 - "Community 43"
Cohesion: 0.40
Nodes (5): Send, Sync, detect_provider(), LlmProvider, ProviderKind

### Community 44 - "Community 44"
Cohesion: 0.40
Nodes (3): Self, String, AppConfig

### Community 45 - "Community 45"
Cohesion: 0.67
Nodes (3): Json, Value, health_check()

## Knowledge Gaps
- **135 isolated node(s):** `ApprovalStatus`, `DateTime`, `Utc`, `ApprovalStatus`, `Vec` (+130 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **1 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.