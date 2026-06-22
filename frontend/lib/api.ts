"use client";

// Centralized API client for the Workflow Designer. Mirrors the inline
// fetch/Bearer pattern used elsewhere in the app (app/agents/page.tsx).

export const API_URL =
  process.env.NEXT_PUBLIC_API_URL || "http://localhost:8000";

export const DEMO_TOKEN = process.env.NEXT_PUBLIC_DEMO_TOKEN || "";

export function getAuthToken(): string | null {
  if (typeof window !== "undefined") {
    return localStorage.getItem("token") || DEMO_TOKEN || null;
  }
  return DEMO_TOKEN || null;
}

function authHeaders(): Record<string, string> {
  const headers: Record<string, string> = { "Content-Type": "application/json" };
  const token = getAuthToken();
  if (token) headers["Authorization"] = `Bearer ${token}`;
  return headers;
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${API_URL}${path}`, {
    ...init,
    headers: { ...authHeaders(), ...(init?.headers || {}) },
  });
  if (!res.ok) {
    const body = await res.text().catch(() => "");
    let message = `HTTP ${res.status}`;
    try {
      const parsed = JSON.parse(body);
      if (parsed.error) message = parsed.error;
    } catch {
      if (body) message = body;
    }
    throw new Error(message);
  }
  if (res.status === 204) return undefined as T;
  return res.json() as Promise<T>;
}

// --- Domain types -------------------------------------------------------

export type DataType =
  | "Text"
  | "Json"
  | "Number"
  | "Boolean"
  | "Binary"
  | "Any";

// Externally-tagged enum matching agenticos_core::pipeline::NodeType serde.
export type NodeTypeKind =
  | "Input"
  | "Output"
  | "Model"
  | "Script"
  | "Decision"
  | "Router"
  | "Metric"
  | "Agent"
  | "Team";

export type NodeTypeData =
  | "Input"
  | "Output"
  | { Model: { model_id: string; prompt_template: string } }
  | { Script: { code: string | null; function_name: string } }
  | { Decision: { condition: string } }
  | { Router: { routes: Record<string, string> } }
  | { Metric: { metric_type: string } }
  | { Agent: { agent_id: string } }
  | { Team: { team_id: string } };

export interface NodeInput {
  name: string;
  data_type: DataType;
  value: unknown | null;
  source_node: string | null;
}

export interface NodeOutput {
  name: string;
  data_type: DataType;
  value: unknown;
}

export interface PipelineNode {
  id: string;
  name: string;
  node_type: NodeTypeData;
  inputs: NodeInput[];
  outputs: NodeOutput[];
  config: Record<string, unknown>;
}

export interface PipelineEdge {
  id: string;
  source_node: string;
  source_output: string;
  target_node: string;
  target_input: string;
}

// UI-only layout persisted alongside the engine graph; the backend ignores it.
export interface NodeLayout {
  id: string;
  x: number;
  y: number;
}

export interface PipelineGraph {
  nodes: PipelineNode[];
  edges: PipelineEdge[];
  layout?: NodeLayout[];
}

export interface Pipeline {
  id: string;
  workspace_id: string;
  name: string;
  description: string | null;
  version: string;
  graph: PipelineGraph;
  is_active: boolean;
  created_at: string;
  updated_at: string;
}

export interface PipelineRun {
  id: string;
  pipeline_id: string | null;
  status: string;
  total_tokens: number;
  total_cost: number;
  duration_ms: number | null;
  error: string | null;
  created_at: string;
}

export interface ValidateResult {
  ok: boolean;
  errors: string[];
}

export interface RunStep {
  id: string;
  pipeline_run_id: string | null;
  step_type: string;
  node_name: string | null;
  input: string | null;
  output: string | null;
  duration_ms: number | null;
  tokens_used: number | null;
  created_at: string;
}

// --- Endpoints ----------------------------------------------------------

export const api = {
  listPipelines: () => request<Pipeline[]>("/api/pipelines"),
  getPipeline: (id: string) => request<Pipeline>(`/api/pipelines/${id}`),
  createPipeline: (body: { name: string; description?: string; graph: PipelineGraph }) =>
    request<Pipeline>("/api/pipelines", {
      method: "POST",
      body: JSON.stringify(body),
    }),
  updatePipeline: (
    id: string,
    body: { name?: string; description?: string; graph?: PipelineGraph },
  ) =>
    request<Pipeline>(`/api/pipelines/${id}`, {
      method: "PUT",
      body: JSON.stringify(body),
    }),
  deletePipeline: (id: string) =>
    request<{ deleted: boolean }>(`/api/pipelines/${id}`, { method: "DELETE" }),
  validatePipeline: (id: string) =>
    request<ValidateResult>(`/api/pipelines/${id}/validate`, { method: "POST" }),
  runPipeline: (id: string, data: Record<string, unknown>) =>
    request<{ run_id: string; status: string; total_tokens: number; total_cost: number; duration_ms: number }>(
      `/api/pipelines/${id}/run`,
      { method: "POST", body: JSON.stringify({ data }) },
    ),
  listPipelineRuns: (id: string) =>
    request<PipelineRun[]>(`/api/pipelines/${id}/runs`),
  getPipelineRunSteps: (runId: string) =>
    request<RunStep[]>(`/api/pipeline-runs/${runId}/steps`),
  deployPipeline: (id: string) =>
    request<{ version_number: number }>(`/api/pipelines/${id}/deploy`, {
      method: "POST",
      body: JSON.stringify({}),
    }),
  rollbackPipeline: (id: string, version: number) =>
    request<Pipeline>(`/api/pipelines/${id}/rollback/${version}`, { method: "POST" }),
  // Agents/teams for the Agent/Team node pickers.
  listAgents: () => request<{ id: string; name: string }[]>("/api/agents"),
  listTeams: () => request<{ id: string; name: string }[]>("/api/teams"),
};
