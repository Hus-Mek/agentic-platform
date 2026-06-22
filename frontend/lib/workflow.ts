"use client";

// Mapping between the React Flow canvas model and the backend pipeline graph
// (agenticos_core::pipeline). Keeps the engine contract (externally-tagged
// node_type, inputs wired by source_node) and a UI-only layout.

import type { Edge, Node } from "@xyflow/react";

import type {
  NodeInput,
  NodeTypeData,
  NodeTypeKind,
  PipelineEdge,
  PipelineGraph,
  PipelineNode,
} from "./api";

export interface WorkflowNodeData extends Record<string, unknown> {
  name: string;
  nodeType: NodeTypeData;
  config: Record<string, unknown>;
}

export type WorkflowNode = Node<WorkflowNodeData>;

export interface PaletteEntry {
  kind: NodeTypeKind;
  label: string;
  description: string;
  /** lucide-react icon name */
  icon: string;
  accent: string;
}

export const NODE_CATALOG: PaletteEntry[] = [
  { kind: "Input", label: "Input", description: "Entry point for run data", icon: "LogIn", accent: "text-sky-400" },
  { kind: "Model", label: "Model", description: "Call an LLM via LiteLLM", icon: "Sparkles", accent: "text-purple-400" },
  { kind: "Agent", label: "Agent", description: "Invoke a saved agent", icon: "Bot", accent: "text-brand-400" },
  { kind: "Team", label: "Team", description: "Invoke a saved team", icon: "Users", accent: "text-indigo-400" },
  { kind: "Script", label: "Script", description: "Run WASM code in the sandbox", icon: "Code", accent: "text-amber-400" },
  { kind: "Decision", label: "Decision", description: "Branch on a condition", icon: "GitBranch", accent: "text-emerald-400" },
  { kind: "Router", label: "Router", description: "Route by data type", icon: "Split", accent: "text-teal-400" },
  { kind: "Metric", label: "Metric", description: "Score against a reference", icon: "Gauge", accent: "text-rose-400" },
  { kind: "Output", label: "Output", description: "Collect final results", icon: "LogOut", accent: "text-sky-400" },
];

/** Default node_type payload for a freshly dropped node of `kind`. */
export function defaultNodeType(kind: NodeTypeKind): NodeTypeData {
  switch (kind) {
    case "Input":
      return "Input";
    case "Output":
      return "Output";
    case "Model":
      return { Model: { model_id: "gpt-4o", prompt_template: "{{input}}" } };
    case "Script":
      return { Script: { code: "", function_name: "run" } };
    case "Decision":
      return { Decision: { condition: "score > 50" } };
    case "Router":
      return { Router: { routes: {} } };
    case "Metric":
      return { Metric: { metric_type: "exact_match" } };
    case "Agent":
      return { Agent: { agent_id: "" } };
    case "Team":
      return { Team: { team_id: "" } };
  }
}

export function nodeKind(nodeType: NodeTypeData): NodeTypeKind {
  if (typeof nodeType === "string") return nodeType;
  return Object.keys(nodeType)[0] as NodeTypeKind;
}

export function slug(name: string): string {
  return name.trim().toLowerCase().replace(/[^a-z0-9]+/g, "_").replace(/^_|_$/g, "") || "input";
}

/** Convert the canvas to the backend graph (engine contract + UI layout). */
export function rfToGraph(nodes: WorkflowNode[], edges: Edge[]): PipelineGraph {
  const byId = new Map(nodes.map((n) => [n.id, n]));

  const pipelineNodes: PipelineNode[] = nodes.map((node) => {
    const incoming = edges.filter((e) => e.target === node.id);
    const inputs: NodeInput[] = incoming.map((e) => {
      const source = byId.get(e.source);
      return {
        name: slug(source?.data.name || e.source),
        data_type: "Any",
        value: null,
        source_node: e.source,
      };
    });
    // Source-less nodes still need an input so run data can flow in.
    if (inputs.length === 0 && nodeKind(node.data.nodeType) === "Input") {
      inputs.push({ name: "input", data_type: "Any", value: null, source_node: null });
    }
    return {
      id: node.id,
      name: node.data.name,
      node_type: node.data.nodeType,
      inputs,
      outputs: [],
      config: node.data.config || {},
    };
  });

  const pipelineEdges: PipelineEdge[] = edges.map((e) => ({
    id: e.id,
    source_node: e.source,
    source_output: "output",
    target_node: e.target,
    target_input: slug(byId.get(e.source)?.data.name || e.source),
  }));

  const layout = nodes.map((n) => ({ id: n.id, x: n.position.x, y: n.position.y }));
  return { nodes: pipelineNodes, edges: pipelineEdges, layout };
}

/** Convert a stored backend graph back to canvas nodes/edges. */
export function graphToRf(graph: PipelineGraph): { nodes: WorkflowNode[]; edges: Edge[] } {
  const layoutById = new Map((graph.layout || []).map((l) => [l.id, l]));
  let auto = 0;
  const nodes: WorkflowNode[] = (graph.nodes || []).map((n) => {
    const pos = layoutById.get(n.id);
    const position = pos ? { x: pos.x, y: pos.y } : { x: 120 + (auto % 3) * 240, y: 80 + Math.floor(auto / 3) * 140 };
    auto += 1;
    return {
      id: n.id,
      type: "workflowNode",
      position,
      data: { name: n.name, nodeType: n.node_type, config: n.config || {} },
    };
  });
  const edges: Edge[] = (graph.edges || []).map((e) => ({
    id: e.id,
    source: e.source_node,
    target: e.target_node,
  }));
  return { nodes, edges };
}

export function newNodeId(): string {
  return `node_${Math.random().toString(36).slice(2, 10)}`;
}
