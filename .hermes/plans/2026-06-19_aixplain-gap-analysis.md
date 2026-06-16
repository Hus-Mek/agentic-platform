# AgenticOS → aiXplain Gap Analysis & Implementation Plan

> **For Hermes:** Use subagent-driven-development skill. Dispatch coding to OpenCode. Verify `cargo check --workspace` after each batch. Commit after each task.

**Goal:** Deep-research aiXplain.com architecture, compare with AgenticOS codebase, identify all gaps, and implement improvements in priority order.

**Research Sources:**
- https://aixplain.com/ — main product page
- https://docs.aixplain.com/core-concepts/agents/ — agent & team agent docs
- https://docs.aixplain.com/concepts/assets/pipelines/ — pipeline docs
- https://deepwiki.com/aixplain/aiXplain/1.1-architecture — SDK architecture
- https://github.com/aixplain/aiXplain — source code
- https://aixplain.com/blog/evolver-meta-agent-self-improving-ai/ — Evolver meta-agent
- https://aixplain.com/blog/aixplain-agentic-os/ — Agentic OS blog
- https://arxiv.org/html/2412.17149v1 — Evolver research paper

---

## Part 1: aiXplain Architecture Summary

### Agent Types (Micro-Agents & Meta-Agents)

| Agent | Role | Our Equivalent | Gap |
|-------|------|---------------|-----|
| **Mentalist** | Understands goals, creates execution plan | `planner.rs` (basic) | PARTIAL — our planner is simple JSON, no adaptive replanning |
| **Orchestrator** | Routes tasks, coordinates subagents | `orchestrator.rs` (TeamOrchestrator) | PARTIAL — we have sequential + parallel delegation, but no dynamic routing |
| **Bodyguard** | Role-based access, data privacy | `guards/bodyguard.rs` + `engine.rs` | DONE — 12 OWASP policies |
| **Inspector** | Quality, feasibility, compliance validation | `guards/inspector.rs` | DONE — PII, content moderation, etc. |
| **Responder** | Response validation against schema | N/A | GAP — no output schema validation |
| **Evolver** | Self-improving meta-agent, optimizes prompts/tools | N/A | GAP — no meta-agent for continuous improvement |
| **Matchmaker** | Selects best model/tool for task | N/A | GAP — no dynamic model selection |
| **Architect** | Designs agentic solution from description | N/A | GAP — no solution architect agent |

### Pipeline System

| Feature | aiXplain | Our Equivalent | Gap |
|---------|----------|---------------|-----|
| DAG-based workflows | Pipeline designer with nodes | N/A | GAP — no DAG pipeline |
| Node types: Input, Script, Decision, Router, Metric | Rich node system | N/A | GAP |
| Metric nodes (BLEU, CHRF, etc.) | Built-in evaluation metrics | `eval.rs` (basic) | PARTIAL — no standard metrics |
| Pipeline execution engine | Async pipeline runner | N/A | GAP |
| Pipeline factory | PipelineFactory | N/A | GAP |

### Governance & Runtime

| Feature | aiXplain | Our Equivalent | Gap |
|---------|----------|---------------|-----|
| Runtime policy enforcement | Inspector + Bodyguard on every run | GuardEngine (inspector + bodyguard) | DONE |
| Human approval checkpoints | HITL with escalation | `approval.rs` + `HITL` | DONE |
| Audit trails | Full event logging | OTel + run steps | PARTIAL |
| Cost tracking | Per-agent token/cost tracking | `usage.rs` + cost fields | DONE |
| SOC 2 compliance | Type II certified | N/A (self-hosted) | N/A |

### Memory & Context

| Feature | aiXplain | Our Equivalent | Gap |
|---------|----------|---------------|-----|
| Short-term memory | Session context | `context.rs` + messages | DONE |
| Long-term memory | Semantic + procedural store | `memory/store.rs` + Mem0 | DONE |
| Memory agent | Decides what to store | `memory_extractor.rs` | DONE |
| Context window management | Automatic pruning | `context.rs` (basic) | PARTIAL — simple truncation, no summarization |

---

## Part 2: Gap Analysis (Priority Order)

### CRITICAL GAPS (Missing core aiXplain features)

#### Gap 1: No DAG Pipeline System
**What aiXplain has:** Full DAG-based pipeline designer with Input, Script, Decision, Router, Metric nodes. Pipelines are first-class assets that can be created, versioned, and executed.
**What we have:** Only the agent loop (plan→act→observe). No DAG workflow engine.
**Impact:** Users cannot create reusable, multi-step AI workflows with branching logic.

#### Gap 2: No Evolver Meta-Agent (Self-Improving)
**What aiXplain has:** Evolver is a meta-agent that monitors agent performance, iterates on prompts/tools/configurations, and benchmarks over time. It treats optimization as a search problem.
**What we have:** Static prompts. No feedback loop for improvement.
**Impact:** Agents don't improve from production usage. Prompt engineering is manual.

#### Gap 3: No Output Schema Validation (Responder)
**What aiXplain has:** Responder agent validates outputs against a schema before returning to user.
**What we have:** Raw text output from LLM. No structured output validation.
**Impact:** Unreliable outputs. No guarantee of format compliance.

#### Gap 4: No Dynamic Model Selection (Matchmaker)
**What aiXplain has:** Matchmaker selects the best model for each task based on capabilities, cost, and performance.
**What we have:** Static model assignment per agent. No runtime model routing.
**Impact:** Suboptimal model selection. No cost-performance tradeoff.

### HIGH PRIORITY GAPS (Significant improvements)

#### Gap 5: Limited Evaluation System
**What aiXplain has:** Metric nodes in pipelines, BLEU/CHRF/standard metrics, LLM-as-judge evaluation, benchmarking.
**What we have:** Basic `eval.rs` with simple expected-output matching.
**Impact:** Cannot properly evaluate agent quality or compare models.

#### Gap 6: No Solution Architect Agent
**What aiXplain has:** Architect agent designs agentic solutions from natural language descriptions.
**What we have:** Users must manually configure agents, tools, and workflows.
**Impact:** High barrier to entry. No guided agent creation.

#### Gap 7: Pipeline Execution Model Missing
**What aiXplain has:** Pipelines execute as async workflows with data flowing between nodes. Each node processes input and passes output to connected nodes.
**What we have:** Only agent loop execution. No workflow engine.
**Impact:** Cannot build complex multi-step AI workflows.

### MEDIUM PRIORITY GAPS (Nice to have)

#### Gap 8: No Standard Metric Library
**What aiXplain has:** BLEU, CHRF, WER, sentiment accuracy, etc. as built-in metric nodes.
**What we have:** No standard metrics.
**Impact:** Must build custom evaluation for each use case.

#### Gap 9: Limited Context Summarization
**What aiXplain has:** Intelligent context management with automatic summarization.
**What we have:** Simple truncation at 8K tokens with basic pruning.
**Impact:** Loses important context in long conversations.

#### Gap 10: No Agent Marketplace / Asset Sharing
**What aiXplain has:** Marketplace for models, tools, agents, pipelines.
**What we have:** Self-contained system.
**Impact:** Users must bring their own models/tools.

---

## Part 3: Implementation Plan

### Phase A: DAG Pipeline System (Gap 1, 7)

**Objective:** Build a DAG-based pipeline engine that supports Input, Script, Decision, Router, and Metric nodes.

**Files to create/modify:**
- `backend/core/src/pipeline.rs` — Pipeline types (Pipeline, PipelineNode, NodeInput, NodeOutput, PipelineExecution)
- `backend/agent-runtime/src/pipeline_engine.rs` — Pipeline execution engine
- `backend/api-server/src/routes/pipelines.rs` — Pipeline CRUD + execution API
- `backend/core/src/lib.rs` — Export pipeline types

**Node types:**
1. `InputNode` — receives external data (text, JSON, file)
2. `ModelNode` — calls an LLM with a prompt template
3. `ScriptNode` — executes custom logic (Rust function or WASM)
4. `DecisionNode` — branches based on a condition
5. `RouterNode` — routes to different paths based on data type
6. `MetricNode` — evaluates output against reference data
7. `OutputNode` — collects and returns final results

**Execution model:**
- DAG validation (no cycles, all inputs connected)
- Topological sort for execution order
- Parallel execution where dependencies allow
- Streaming intermediate results

**Acceptance criteria:**
- [ ] Pipeline can be created via API with node definitions
- [ ] Pipeline executes in correct dependency order
- [ ] Decision/Router nodes branch correctly
- [ ] Metric nodes produce scores
- [ ] Pipeline execution is traceable (OTel spans)
- [ ] `cargo check --workspace` passes

---

### Phase B: Evolver Meta-Agent (Gap 2)

**Objective:** Build a meta-agent that monitors agent performance and iteratively improves prompts, tool configurations, and model choices.

**Files to create/modify:**
- `backend/agent-runtime/src/evolver.rs` — Evolver meta-agent
- `backend/core/src/evolver_types.rs` — Evolver types (OptimizationTarget, BenchmarkResult, OptimizationIteration)
- `backend/api-server/src/routes/evolver.rs` — Evolver API

**Architecture:**
1. **Monitor** — observes agent runs, collects metrics (success rate, token usage, cost, latency)
2. **Evaluate** — benchmarks against standard tasks
3. **Hypothesize** — generates improvement candidates (prompt tweaks, tool changes, model swaps)
4. **Test** — runs candidates against benchmark suite
5. **Apply** — if improvement > threshold, applies change
6. **Repeat** — continuous optimization loop

**Data model:**
```rust
struct OptimizationTarget {
    agent_id: String,
    current_prompt: String,
    current_model: String,
    current_tools: Vec<String>,
    benchmark_results: Vec<BenchmarkResult>,
}

struct OptimizationIteration {
    target: OptimizationTarget,
    candidate: OptimizationTarget,
    improvement: f64,  // percentage improvement
    applied: bool,
}
```

**Acceptance criteria:**
- [ ] Evolver can observe agent runs and collect metrics
- [ ] Evolver can generate prompt improvement candidates
- [ ] Evolver can benchmark candidates against current config
- [ ] Evolver applies improvements that pass threshold
- [ ] Full audit trail of all optimization iterations
- [ ] `cargo check --workspace` passes

---

### Phase C: Output Schema Validation (Gap 3)

**Objective:** Add structured output validation to agent responses.

**Files to create/modify:**
- `backend/core/src/output_schema.rs` — OutputSchema types
- `backend/agent-runtime/src/responder.rs` — Responder agent that validates output
- `backend/agent-runtime/src/loop_mod.rs` — Integrate responder into agent loop

**Design:**
- Agents can declare an output schema (JSON Schema format)
- After LLM produces output, responder validates against schema
- On validation failure: retry with error feedback (up to 3 attempts)
- On success: return validated structured output

**Acceptance criteria:**
- [ ] Agent config supports optional output_schema field
- [ ] Responder validates JSON output against schema
- [ ] Failed validation triggers retry with error context
- [ ] Schema validation errors are descriptive
- [ ] `cargo check --workspace` passes

---

### Phase D: Dynamic Model Selection (Gap 4)

**Objective:** Add runtime model routing that selects the best model for each task.

**Files to create/modify:**
- `backend/core/src/model_router.rs` — Model routing logic
- `backend/agent-runtime/src/loop_mod.rs` — Integrate model routing

**Design:**
- `ModelRouter` maintains a registry of available models with capabilities, cost, latency
- For each LLM call, router selects model based on:
  - Task complexity (simple → cheap model, complex → capable model)
  - Token budget remaining
  - Required capabilities (function calling, vision, etc.)
  - Historical performance on similar tasks
- Fallback chain: primary → secondary → default

**Acceptance criteria:**
- [ ] ModelRouter can register models with capabilities
- [ ] Router selects appropriate model based on task
- [ ] Fallback chain works when primary model fails
- [ ] Model selection is logged for cost tracking
- [ ] `cargo check --workspace` passes

---

### Phase E: Enhanced Evaluation System (Gap 5, 8)

**Objective:** Build a comprehensive evaluation system with standard metrics and LLM-as-judge.

**Files to create/modify:**
- `backend/agent-runtime/src/eval.rs` — Enhanced eval harness
- `backend/core/src/metrics.rs` — Standard metric types
- `backend/agent-runtime/src/metrics/` — Metric implementations
  - `bleu.rs` — BLEU score
  - `chrf.rs` — CHRF score
  - `llm_judge.rs` — LLM-as-judge scoring
  - `semantic_similarity.rs` — Embedding-based similarity
- `backend/api-server/src/routes/benchmarks.rs` — Benchmark API

**Design:**
- `EvalSuite` — a collection of eval tasks with metrics
- `EvalRunner` — runs agent against suite, collects scores
- `Metric` trait — `fn score(hypothesis: &str, reference: &str) -> f64`
- `LLMJudge` — uses an LLM to score open-ended responses
- Results stored per-agent for tracking over time

**Acceptance criteria:**
- [ ] EvalSuite can be created with tasks and metrics
- [ ] BLEU, CHRF metrics produce correct scores
- [ ] LLM-as-judge produces reasonable scores
- [ ] Eval results are stored and trackable over time
- [ ] Benchmark API returns results
- [ ] `cargo check --workspace` passes

---

### Phase F: Solution Architect Agent (Gap 6)

**Objective:** Build an Architect agent that designs agentic solutions from natural language.

**Files to create/modify:**
- `backend/agent-runtime/src/architect.rs` — Architect agent
- `backend/api-server/src/routes/architect.rs` — Architect API

**Design:**
- Takes natural language description of a business need
- Outputs a complete agent configuration:
  - Agent name, description, instructions
  - Required tools and integrations
  - Sub-agent team structure (if needed)
  - Pipeline design (if needed)
  - Guard configuration
- Uses a structured LLM prompt to generate the design
- Returns JSON that can be used to create agents via API

**Acceptance criteria:**
- [ ] Architect generates valid agent config from description
- [ ] Generated config includes instructions, tools, guards
- [ ] Can generate team agent configs with sub-agents
- [ ] Output is directly usable via agent creation API
- [ ] `cargo check --workspace` passes

---

### Phase G: Enhanced Context Summarization (Gap 9)

**Objective:** Replace simple truncation with intelligent context summarization.

**Files to create/modify:**
- `backend/agent-runtime/src/context.rs` — Enhanced context manager
- `backend/agent-runtime/src/summarizer.rs` — LLM-based summarization

**Design:**
- When context approaches limit:
  1. Identify old messages (keep recent N)
  2. Use LLM to summarize old messages into key points
  3. Replace old messages with summary
  4. Preserve system prompt and recent messages intact
- Configurable: summarization threshold, keep_recent count
- Summary includes: key decisions, important facts, action items

**Acceptance criteria:**
- [ ] Context summarization triggers at configurable threshold
- [ ] LLM-generated summary preserves key information
- [ ] Recent messages are preserved verbatim
- [ ] Summarization is logged for debugging
- [ ] `cargo check --workspace` passes

---

## Part 4: Execution Order

| Phase | Gap | Priority | Est. Tasks | Dependencies |
|-------|-----|----------|------------|--------------|
| A | DAG Pipeline | CRITICAL | 5 | None |
| B | Evolver Meta-Agent | CRITICAL | 4 | E (eval system) |
| C | Output Schema | CRITICAL | 3 | None |
| D | Model Selection | CRITICAL | 3 | None |
| E | Eval System | HIGH | 5 | None |
| F | Architect Agent | HIGH | 3 | C (schema validation) |
| G | Context Summarization | MEDIUM | 2 | None |

**Recommended execution order (respecting dependencies):**
1. **Phase A** (DAG Pipeline) — foundation for workflows
2. **Phase C** (Output Schema) — needed by Architect
3. **Phase D** (Model Selection) — independent
4. **Phase E** (Eval System) — needed by Evolver
5. **Phase B** (Evolver) — depends on E
6. **Phase F** (Architect) — depends on C
7. **Phase G** (Context Summarization) — independent, last

---

## Part 5: Research Notes

### aiXplain's Adaptive Orchestration Engine
- Goal-driven agents handle execution without fixed flowcharts
- Micro-agents (Mentalist, Orchestrator, Inspector, Bodyguard) run on every agent execution
- Meta-agents (Evolver) run asynchronously to optimize over time
- Governance is enforced at runtime, not just at configuration time
- Single PAYG API key for all models/tools

### Evolver Meta-Agent (from arxiv paper)
- Treats agent optimization as a search problem
- Layers: Monitor → Evaluate → Hypothesize → Test → Apply
- Uses LLM-driven feedback loops
- Iterative refinement with benchmark validation
- Can optimize prompts, tool configurations, and model choices

### Pipeline Execution Model
- DAG-based with topological sort for execution order
- Nodes connected by typed edges (data type matching)
- Metric nodes evaluate outputs against reference data
- Pipelines are versioned assets that can be shared
- PipelineFactory for creation and retrieval

### Key Design Patterns
- Factory pattern for all asset types (Agent, Pipeline, Model, Metric)
- Runtime governance on every execution (not just config-time)
- Separation of platform concerns from business logic
- Vendor-agnostic with dynamic routing
- Built-in evaluation and benchmarking
