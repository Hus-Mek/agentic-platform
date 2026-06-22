# AgenticOS Platform — API Reference

Base URL: `http://localhost:8000`

## Authentication

All endpoints except `/health` require authentication via one of:

- **Authorization header:** `Authorization: Bearer <jwt_token>`
- **Authorization header (API key):** `Authorization: Bearer <api_key>`

Auth is attempted as JWT first, falling back to API key verification (bcrypt against `api_keys` table).

## Common Patterns

### Error Response

```json
{"error": "Human-readable error message"}
```

### Status Codes

| Code | Meaning                |
|------|------------------------|
| 200  | Success                |
| 201  | Created                |
| 400  | Bad Request            |
| 401  | Unauthorized           |
| 404  | Not Found              |
| 500  | Internal Server Error  |

### Pagination

Query parameters for list endpoints:

| Param      | Type | Default | Description         |
|------------|------|---------|---------------------|
| `page`     | int  | 1       | Page number         |
| `per_page` | int  | 20      | Items per page      |

**Note:** Pagination support may vary per endpoint. Current implementation applies workspace-scoped queries without explicit pagination; these params are reserved for future use.

---

## Agents

### `GET /api/agents` — List agents

**Auth:** Required  
**Query Params:**

| Param          | Type | Required | Description                        |
|----------------|------|----------|------------------------------------|
| `workspace_id` | UUID | No       | Filter by workspace (defaults to token's workspace) |

**Response `200`:**

```json
[
  {
    "id": "uuid",
    "workspace_id": "uuid",
    "name": "My Agent",
    "description": null,
    "instructions": "You are a helpful assistant...",
    "model": "gpt-4o",
    "tools": [],
    "guard_config": {},
    "memory_config": {},
    "created_by": "user-uuid",
    "is_active": true,
    "created_at": "2025-01-01T00:00:00Z",
    "updated_at": "2025-01-01T00:00:00Z"
  }
]
```

---

### `POST /api/agents` — Create agent

**Auth:** Required

**Request Body:**

```json
{
  "name": "My Agent",
  "description": "Optional description",
  "instructions": "You are a helpful assistant...",
  "model": "gpt-4o",
  "tools": [],
  "guard_config": {},
  "memory_config": {}
}
```

| Field           | Type   | Required | Default    | Description              |
|-----------------|--------|----------|------------|--------------------------|
| `name`          | string | yes      | —          | Agent name               |
| `description`   | string | no       | null       | Optional description     |
| `instructions`  | string | yes      | —          | System prompt            |
| `model`         | string | no       | `"gpt-4o"` | LLM model identifier     |
| `tools`         | array  | no       | `[]`       | Tool definitions         |
| `guard_config`  | object | no       | default    | Guard configuration      |
| `memory_config` | object | no       | default    | Memory configuration     |

**Response `200`:** Full [`AgentRow`](#agent-row)

---

### `GET /api/agents/{id}` — Get agent

**Auth:** Required  
**Path:** `id` — Agent UUID

**Response `200`:** Full [`AgentRow`](#agent-row)  
**Response `404`:** `{"error": "Agent not found"}`

---

### `PUT /api/agents/{id}` — Update agent

**Auth:** Required  
**Path:** `id` — Agent UUID

**Request Body** (all fields optional):

```json
{
  "name": "Updated Name",
  "description": "Updated description",
  "instructions": "Updated instructions",
  "model": "claude-sonnet-4-20250514",
  "tools": [],
  "guard_config": {},
  "memory_config": {}
}
```

**Response `200`:** Full [`AgentRow`](#agent-row)  
**Response `404`:** `{"error": "Agent not found"}`

---

### `DELETE /api/agents/{id}` — Delete agent (soft-delete)

**Auth:** Required  
**Path:** `id` — Agent UUID

**Response `200`:**

```json
{"deleted": true, "id": "uuid"}
```

**Response `404`:** `{"error": "Agent not found"}`

---

### `POST /api/agents/{id}/run` — Run agent

**Auth:** Required  
**Path:** `id` — Agent UUID

**Request Body:**

```json
{
  "input": "What is the capital of France?",
  "model": "claude-sonnet-4-20250514"
}
```

| Field   | Type   | Required | Description                          |
|---------|--------|----------|--------------------------------------|
| `input` | string | yes      | User message to send to agent        |
| `model` | string | no       | Override agent's default model       |

**Response `200`:**

```json
{
  "run_id": "uuid",
  "output": "The capital of France is Paris.",
  "total_tokens": 150,
  "total_cost": 0.003,
  "duration_ms": 2345,
  "status": "completed"
}
```

**Response `500`:**

```json
{
  "error": "Error message",
  "run_id": "uuid"
}
```

---

### `POST /api/agents/{id}/run-stream` — Stream agent run (SSE)

**Auth:** Required  
**Path:** `id` — Agent UUID

**Request Body:**

```json
{
  "input": "Explain quantum computing in simple terms",
  "model": null
}
```

**Response `200`:** Server-Sent Events stream (`text/event-stream`)

### SSE Event Format

Each event's `data` field contains a JSON-encoded `StreamEvent`:

```
data: {"Token":"Quantum computing is a..."}
data: {"ToolCall":{"name":"web_search","args":{"query":"quantum computing basics"}}}
data: {"ToolResult":{"name":"web_search","result":"Search results..."}}
data: "Done"
data: {"Error":"Something went wrong"}
```

| Event Type    | Data Format                                       | Description                  |
|---------------|---------------------------------------------------|------------------------------|
| `Token`       | `{"Token": "text fragment"}`                      | Streamed output token batch  |
| `ToolCall`    | `{"ToolCall": {"name": "...", "args": {...}}}`    | Agent invoked a tool         |
| `ToolResult`  | `{"ToolResult": {"name": "...", "result": "..."}}`| Tool execution result        |
| `Done`        | `"Done"`                                          | Stream complete (success)    |
| `Error`       | `{"Error": "message"}`                            | Stream terminated with error |

---

### `GET /api/agents/{id}/versions` — List versions

**Auth:** Required  
**Path:** `id` — Agent UUID

**Response `200`:**

```json
[
  {
    "version_number": 1,
    "agent_id": "uuid",
    "name": "My Agent",
    "description": null,
    "instructions": "You are a helpful assistant...",
    "model": "gpt-4o",
    "tools": [],
    "guard_config": {},
    "memory_config": {},
    "created_at": "2025-01-01T00:00:00Z",
    "updated_at": "2025-01-01T00:00:00Z"
  }
]
```

---

### `POST /api/agents/{id}/deploy` — Deploy agent (snapshot + version)

**Auth:** Required  
**Path:** `id` — Agent UUID

Creates a version snapshot of the current agent config and marks it deployable.

**Request Body:**

```json
{
  "environment": "production"
}
```

| Field         | Type   | Required | Description                     |
|---------------|--------|----------|----------------------------------|
| `environment` | string | yes      | Target environment (e.g. `production`, `staging`) |

**Response `200`:**

```json
{
  "version_number": 3,
  "agent_id": "uuid",
  "environment": "production",
  "deployed_at": "2025-01-01T00:00:00Z"
}
```

**Response `404`:** `{"error": "Agent not found"}`

---

### `POST /api/agents/{id}/rollback/{version}` — Rollback agent

**Auth:** Required  
**Path:** `id` — Agent UUID, `version` — Version number (integer)

Restores the agent's config to the specified version and creates a new version snapshot.

**Response `200`:** Full [`AgentRow`](#agent-row)  
**Response `404`:** `{"error": "Agent not found"}` or `{"error": "Version not found"}`

---

## Runs

### `GET /api/runs` — List runs

**Auth:** Required  
**Query Params:**

| Param          | Type   | Required | Description                                  |
|----------------|--------|----------|----------------------------------------------|
| `workspace_id` | UUID   | No       | Filter by workspace (defaults to token's)    |
| `agent_id`     | UUID   | No       | Filter by agent                              |
| `status`       | string | No       | Filter by status (`running`, `completed`, `failed`, `cancelled`) |

**Response `200`:**

```json
[
  {
    "id": "uuid",
    "workspace_id": "uuid",
    "agent_id": "uuid",
    "status": "completed",
    "input": "What is the capital of France?",
    "output": "Paris.",
    "error": null,
    "duration_ms": 1234,
    "total_tokens": 150,
    "total_cost": 0.003,
    "created_by": "user-uuid",
    "created_at": "2025-01-01T00:00:00Z"
  }
]
```

---

### `GET /api/runs/{id}` — Get run

**Auth:** Required  
**Path:** `id` — Run UUID

**Response `200`:** Full [`RunRow`](#run-row)  
**Response `404`:** `{"error": "Run not found"}`

---

### `POST /api/runs/{id}/cancel` — Cancel run

**Auth:** Required  
**Path:** `id` — Run UUID

**Response `200`:**

```json
{"cancelled": true, "id": "uuid"}
```

**Response `400`:** `{"error": "Cannot cancel run with status 'completed'"}`  
**Response `404`:** `{"error": "Run not found"}`

---

### `DELETE /api/runs/{id}` — Delete run

**Auth:** Required  
**Path:** `id` — Run UUID

**Response `200`:**

```json
{"deleted": true, "id": "uuid"}
```

**Response `404`:** `{"error": "Run not found"}`

---

### `GET /api/runs/{id}/steps` — Get run steps

**Auth:** Required  
**Path:** `id` — Run UUID

**Response `200`:**

```json
[
  {
    "id": "uuid",
    "run_id": "uuid",
    "step_type": "llm_call",
    "node_name": null,
    "input": null,
    "output": null,
    "duration_ms": 500,
    "tokens_used": 100,
    "metadata": {},
    "created_at": "2025-01-01T00:00:00Z"
  }
]
```

**Response `404`:** `{"error": "Run not found"}`

---

## Teams

### `GET /api/teams` — List teams

**Auth:** Required  
**Query Params:**

| Param          | Type | Required | Description                        |
|----------------|------|----------|------------------------------------|
| `workspace_id` | UUID | No       | Filter by workspace (defaults to token's) |

**Response `200`:**

```json
[
  {
    "id": "uuid",
    "workspace_id": "uuid",
    "name": "Research Team",
    "description": null,
    "supervisor_instructions": "Coordinate the sub-agents...",
    "supervisor_model": "gpt-4o",
    "sub_agents": [],
    "max_delegations": 10,
    "execution_mode": "Sequential",
    "created_by": "user-uuid",
    "is_active": true,
    "created_at": "2025-01-01T00:00:00Z",
    "updated_at": "2025-01-01T00:00:00Z"
  }
]
```

---

### `POST /api/teams` — Create team

**Auth:** Required

**Request Body:**

```json
{
  "name": "Research Team",
  "description": "A team for research tasks",
  "supervisor_instructions": "Coordinate the sub-agents to complete research tasks",
  "supervisor_model": "gpt-4o",
  "sub_agents": [
    {
      "agent_id": "uuid",
      "name": "Web Searcher",
      "instructions": "Search the web for information",
      "tools": ["web_search"],
      "model": "gpt-4o-mini"
    }
  ],
  "max_delegations": 10,
  "execution_mode": "Sequential"
}
```

| Field                   | Type   | Required | Default        | Description                       |
|-------------------------|--------|----------|----------------|-----------------------------------|
| `name`                  | string | yes      | —              | Team name                         |
| `description`           | string | no       | null           | Optional description              |
| `supervisor_instructions` | string | yes    | —              | Supervisor system prompt          |
| `supervisor_model`      | string | no       | `"gpt-4o"`     | Supervisor LLM model              |
| `sub_agents`            | array  | yes      | —              | Array of `SubAgentConfig`         |
| `max_delegations`       | int    | no       | `10`           | Max delegation rounds             |
| `execution_mode`        | string | no       | `"Sequential"` | `"Sequential"` or `"Parallel"`    |

**Response `200`:** Full [`TeamAgentRow`](#team-agent-row)

---

### `GET /api/teams/{id}` — Get team

**Auth:** Required  
**Path:** `id` — Team UUID

**Response `200`:** Full [`TeamAgentRow`](#team-agent-row)  
**Response `404`:** `{"error": "Team not found"}`

---

### `PUT /api/teams/{id}` — Update team

**Auth:** Required  
**Path:** `id` — Team UUID

**Request Body** (all fields optional):

```json
{
  "name": "Updated Team",
  "description": "Updated description",
  "supervisor_instructions": "Updated instructions",
  "supervisor_model": "claude-sonnet-4-20250514",
  "sub_agents": [],
  "max_delegations": 20,
  "execution_mode": "Parallel"
}
```

**Response `200`:** Full [`TeamAgentRow`](#team-agent-row)  
**Response `404`:** `{"error": "Team not found"}`

---

### `DELETE /api/teams/{id}` — Delete team (soft-delete)

**Auth:** Required  
**Path:** `id` — Team UUID

**Response `200`:**

```json
{"deleted": true, "id": "uuid"}
```

**Response `404`:** `{"error": "Team not found"}`

---

### `POST /api/teams/{id}/run` — Run team

**Auth:** Required  
**Path:** `id` — Team UUID

**Request Body:**

```json
{
  "input": "Research the latest AI breakthroughs"
}
```

**Response `200`:**

```json
{
  "run_id": "uuid",
  "output": "Here are the latest AI breakthroughs...",
  "total_tokens": 2500,
  "total_cost": 0.05,
  "duration_ms": 15000,
  "status": "completed",
  "delegations": [
    {
      "agent_id": "uuid",
      "sub_agent_name": "Web Searcher",
      "input": "Search for...",
      "output": "Results...",
      "tokens_used": 500
    }
  ]
}
```

---

### `GET /api/teams/{id}/runs` — List team runs

**Auth:** Required  
**Path:** `id` — Team UUID

**Response `200`:**

```json
[
  {
    "id": "uuid",
    "team_id": "uuid",
    "workspace_id": "uuid",
    "status": "completed",
    "input": "...",
    "output": "...",
    "error": null,
    "delegations": [],
    "total_tokens": 2500,
    "total_cost": 0.05,
    "duration_ms": 15000,
    "created_by": "user-uuid",
    "created_at": "2025-01-01T00:00:00Z",
    "updated_at": "2025-01-01T00:00:00Z"
  }
]
```

---

### `GET /api/team-runs/{id}` — Get team run

**Auth:** Required  
**Path:** `id` — Team run UUID

**Response `200`:** Full [`TeamRunRow`](#team-run-row)  
**Response `404`:** `{"error": "Team run not found"}`

---

## Workflows (Pipelines)

The Workflow Designer persists and runs multi-step DAGs ("pipelines"). A pipeline
is a graph of typed nodes; running one is a *run* — node-level trace is written to
the shared `run_steps` table (keyed by `pipeline_run_id`) and token/cost roll into
`workspace_usage` exactly like agent runs.

### Node types

| Type       | Binding                                                        |
|------------|---------------------------------------------------------------|
| `Input`    | Entry point; receives values from the run `data` map          |
| `Output`   | Collects all incoming values into the final result            |
| `Model`    | Calls the LLM via the LiteLLM client (never the metered API)  |
| `Agent`    | Invokes a saved agent (`agent_id`) with its tools/guards      |
| `Team`     | Invokes a saved team (`team_id`) supervisor + sub-agents      |
| `Script`   | Runs a WASM module in the wasmtime sandbox (fuel/mem/epoch limited) |
| `Decision` | Evaluates a simple condition (`"score > 50"`)                 |
| `Router`   | Passes the incoming value through to connected nodes          |
| `Metric`   | Scores output vs. an `expected` value (full eval is a roadmap item) |

The `graph` JSONB stores `{ nodes, edges }` plus a UI-only `layout` key the engine
ignores. `node_type` is an externally-tagged enum, e.g.
`{"Model": {"model_id": "gpt-4o", "prompt_template": "{{input}}"}}`.

**Script node:** `code` carries a WAT or wasm-text module executed in a wasmtime
sandbox with **no WASI, no network, no host imports**. Per-node `config` keys
`fuel`, `memory_bytes`, and `wall_ms` bound CPU, memory, and wall-clock. (Authoring
scripts in JavaScript via a QuickJS-on-wasm front-end is a planned follow-up;
sub-pipeline nesting and per-node guard policies are out of scope for v1.)

### Endpoints

| Method & path                                  | Description                          |
|------------------------------------------------|--------------------------------------|
| `GET /api/pipelines`                           | List pipelines (workspace-scoped)    |
| `POST /api/pipelines`                          | Create (validates the DAG first)     |
| `GET /api/pipelines/{id}`                      | Get a pipeline                       |
| `PUT /api/pipelines/{id}`                      | Update (snapshots the prior version) |
| `DELETE /api/pipelines/{id}`                   | Soft delete                          |
| `POST /api/pipelines/{id}/validate`            | `{ ok, errors[] }`                   |
| `POST /api/pipelines/{id}/run`                 | Run synchronously → run summary      |
| `POST /api/pipelines/{id}/run-stream`          | Run with SSE node-level events       |
| `GET /api/pipelines/{id}/runs`                 | List runs for a pipeline             |
| `GET /api/pipelines/{id}/versions`             | List config-snapshot versions        |
| `POST /api/pipelines/{id}/deploy`              | Snapshot a version                   |
| `POST /api/pipelines/{id}/rollback/{version}`  | Restore a prior version              |
| `GET /api/pipeline-runs/{id}`                  | Get a run + execution snapshot       |
| `GET /api/pipeline-runs/{id}/steps`            | Per-node trace steps                 |
| `POST /api/pipeline-runs/{id}/cancel`          | Cancel a pending/running run         |

**Run request body:** `{ "data": { "input": "..." } }` — keys map to `Input`
nodes by node id or input name.

**SSE events** (`run-stream`): `{type:"node_started"|"node_completed"|"node_failed"|"done"|"error", ...}`.

**Create example:**

```json
POST /api/pipelines
{
  "name": "Summarize",
  "graph": {
    "nodes": [
      {"id":"in","name":"Input","node_type":"Input","inputs":[{"name":"input","data_type":"Any","value":null,"source_node":null}],"outputs":[],"config":{}},
      {"id":"m","name":"Model","node_type":{"Model":{"model_id":"gpt-4o","prompt_template":"Summarize: {{input}}"}},"inputs":[{"name":"input","data_type":"Any","value":null,"source_node":"in"}],"outputs":[],"config":{}},
      {"id":"out","name":"Output","node_type":"Output","inputs":[{"name":"response","data_type":"Any","value":null,"source_node":"m"}],"outputs":[],"config":{}}
    ],
    "edges": [
      {"id":"e1","source_node":"in","source_output":"output","target_node":"m","target_input":"input"},
      {"id":"e2","source_node":"m","source_output":"output","target_node":"out","target_input":"response"}
    ]
  }
}
```

---

## Knowledge Bases

### `GET /api/knowledge-bases` — List knowledge bases

**Auth:** Required

Scoped to user's workspace from auth token.

**Response `200`:**

```json
[
  {
    "id": "uuid",
    "workspace_id": "uuid",
    "name": "Company Wiki",
    "description": "Internal knowledge base",
    "config": {
      "chunk_size": 512,
      "chunk_overlap": 64
    },
    "created_at": "2025-01-01T00:00:00Z"
  }
]
```

---

### `GET /api/knowledge-bases/{id}` — Get knowledge base

**Auth:** Required  
**Path:** `id` — KB UUID

**Response `200`:** Full [`KnowledgeBaseRow`](#knowledge-base-row)  
**Response `404`:** `{"error": "Knowledge base not found"}`

---

### `GET /api/knowledge-bases/{id}/documents` — List documents

**Auth:** Required  
**Path:** `id` — KB UUID

**Response `200`:**

```json
[
  {
    "id": "uuid",
    "knowledge_base_id": "uuid",
    "filename": "research.pdf",
    "content_type": "application/pdf",
    "content": null,
    "metadata": null,
    "created_by": "user-uuid",
    "created_at": "2025-01-01T00:00:00Z"
  }
]
```

---

### `POST /api/knowledge-bases/{id}/documents` — Upload document

**Auth:** Required  
**Path:** `id` — KB UUID

**Request Body:**

```json
{
  "filename": "research.txt",
  "content": "Full text content of the document...",
  "content_type": "text/plain"
}
```

| Field          | Type   | Required | Description              |
|----------------|--------|----------|--------------------------|
| `filename`     | string | yes      | Document filename        |
| `content`      | string | yes      | Full text content        |
| `content_type` | string | no       | MIME type hint           |

The server will:
1. Insert the document record
2. Chunk the text by `chunk_size` (default 512) with `chunk_overlap` (default 64) from KB config
3. Embed each chunk via LiteLLM proxy
4. Store chunks with embeddings in `kb_chunks` table

**Response `200`:**

```json
{
  "document_id": "uuid",
  "chunks_count": 12
}
```

---

### `DELETE /api/knowledge-bases/{id}/documents/{doc_id}` — Delete document

**Auth:** Required  
**Path:** `kb_id` — KB UUID, `doc_id` — Document UUID

Cascades to delete associated chunks.

**Response `200`:**

```json
{"deleted": true, "document_id": "uuid"}
```

**Response `404`:** `{"error": "Document not found"}`

---

## System

### `GET /health` — Health check

**Auth:** Not required

**Response `200`:**

```json
{
  "status": "ok",
  "service": "agenticos-api-server",
  "timestamp": "2025-01-01T00:00:00Z",
  "uptime_seconds": {
    "since": 3600
  }
}
```

---

## Schema Reference

### Agent Row

| Field            | Type               | Description                     |
|------------------|--------------------|---------------------------------|
| `id`             | UUID               | Agent unique identifier         |
| `workspace_id`   | UUID               | Owning workspace                |
| `name`           | string             | Agent name                      |
| `description`    | string or null     | Optional description            |
| `instructions`   | string             | System prompt / instructions    |
| `model`          | string             | LLM model identifier            |
| `tools`          | JSON array         | Tool definitions                |
| `guard_config`   | JSON object        | Guard configuration             |
| `memory_config`  | JSON object        | Memory configuration            |
| `created_by`     | string             | Creator user ID                 |
| `is_active`      | boolean            | Soft-delete flag                |
| `created_at`     | datetime (ISO8601) | Creation timestamp              |
| `updated_at`     | datetime (ISO8601) | Last update timestamp           |

### Run Row

| Field         | Type               | Description                     |
|---------------|--------------------|---------------------------------|
| `id`          | UUID               | Run unique identifier           |
| `workspace_id`| UUID               | Owning workspace                |
| `agent_id`    | UUID               | Associated agent                |
| `status`      | string             | `running`, `completed`, `failed`, `cancelled` |
| `input`       | string or null     | User input message              |
| `output`      | string or null     | Final output                    |
| `error`       | string or null     | Error message if failed         |
| `duration_ms` | int or null        | Execution duration              |
| `total_tokens`| int or null        | Total tokens consumed           |
| `total_cost`  | float or null      | Total cost in USD               |
| `created_by`  | string             | Creator user ID                 |
| `created_at`  | datetime (ISO8601) | Creation timestamp              |

### Team Agent Row

| Field                     | Type               | Description                     |
|---------------------------|--------------------|---------------------------------|
| `id`                      | UUID               | Team unique identifier          |
| `workspace_id`            | UUID               | Owning workspace                |
| `name`                    | string             | Team name                       |
| `description`             | string or null     | Optional description            |
| `supervisor_instructions` | string             | Supervisor system prompt        |
| `supervisor_model`        | string             | Supervisor LLM model            |
| `sub_agents`              | JSON array         | Sub-agent configurations        |
| `max_delegations`         | int                | Max delegation rounds           |
| `execution_mode`          | string             | `Sequential` or `Parallel`      |
| `created_by`              | string             | Creator user ID                 |
| `is_active`               | boolean            | Soft-delete flag                |
| `created_at`              | datetime (ISO8601) | Creation timestamp              |
| `updated_at`              | datetime (ISO8601) | Last update timestamp           |

### Team Run Row

| Field         | Type               | Description                     |
|---------------|--------------------|---------------------------------|
| `id`          | UUID               | Run unique identifier           |
| `team_id`     | UUID               | Associated team                 |
| `workspace_id`| UUID               | Owning workspace                |
| `status`      | string             | Run status                      |
| `input`       | string or null     | User input                      |
| `output`      | string or null     | Final output                    |
| `error`       | string or null     | Error message                   |
| `delegations` | JSON array         | Delegation results              |
| `total_tokens`| int or null        | Total tokens consumed           |
| `total_cost`  | float or null      | Total cost in USD               |
| `duration_ms` | int or null        | Execution duration              |
| `created_by`  | string             | Creator user ID                 |
| `created_at`  | datetime (ISO8601) | Creation timestamp              |
| `updated_at`  | datetime (ISO8601) | Last update timestamp           |

### Knowledge Base Row

| Field          | Type               | Description                     |
|----------------|--------------------|---------------------------------|
| `id`           | UUID               | KB unique identifier            |
| `workspace_id` | UUID               | Owning workspace                |
| `name`         | string             | KB name                         |
| `description`  | string or null     | Optional description            |
| `config`       | JSON object        | Chunk size, overlap, etc.       |
| `created_at`   | datetime (ISO8601) | Creation timestamp              |
