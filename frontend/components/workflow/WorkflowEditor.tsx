"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import {
  ReactFlow,
  Background,
  Controls,
  MiniMap,
  addEdge,
  useNodesState,
  useEdgesState,
  type Connection,
  type Edge,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { Save, CheckCircle2, AlertCircle, Rocket, Play, X, Loader2 } from "lucide-react";

import { api, type Pipeline, type ValidateResult } from "../../lib/api";
import {
  defaultNodeType,
  graphToRf,
  newNodeId,
  rfToGraph,
  type PaletteEntry,
  type WorkflowNode,
  type WorkflowNodeData,
} from "../../lib/workflow";
import WorkflowNodeView from "./CustomNode";
import NodePalette from "./NodePalette";
import NodeConfigPanel from "./NodeConfigPanel";
import WorkflowRunPanel from "./WorkflowRunPanel";

const nodeTypes = { workflowNode: WorkflowNodeView };

export default function WorkflowEditor({ pipeline }: { pipeline: Pipeline }) {
  const initial = useMemo(() => graphToRf(pipeline.graph), [pipeline.graph]);
  const [nodes, setNodes, onNodesChange] = useNodesState<WorkflowNode>(initial.nodes);
  const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>(initial.edges);
  const [name, setName] = useState(pipeline.name);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [message, setMessage] = useState<{ kind: "ok" | "err"; text: string } | null>(null);
  const [validation, setValidation] = useState<ValidateResult | null>(null);
  const [showRun, setShowRun] = useState(false);

  useEffect(() => setDirty(true), [nodes, edges, name]);

  const selectedNode = nodes.find((n) => n.id === selectedId) || null;

  const onConnect = useCallback(
    (conn: Connection) => setEdges((eds) => addEdge(conn, eds)),
    [setEdges],
  );

  const addNode = useCallback(
    (entry: PaletteEntry) => {
      const id = newNodeId();
      const node: WorkflowNode = {
        id,
        type: "workflowNode",
        position: { x: 200 + Math.random() * 200, y: 120 + Math.random() * 160 },
        data: { name: entry.label, nodeType: defaultNodeType(entry.kind), config: {} },
      };
      setNodes((ns) => [...ns, node]);
      setSelectedId(id);
    },
    [setNodes],
  );

  const updateNodeData = useCallback(
    (id: string, patch: Partial<WorkflowNodeData>) => {
      setNodes((ns) =>
        ns.map((n) => (n.id === id ? { ...n, data: { ...n.data, ...patch } } : n)),
      );
    },
    [setNodes],
  );

  const deleteNode = useCallback(
    (id: string) => {
      setNodes((ns) => ns.filter((n) => n.id !== id));
      setEdges((es) => es.filter((e) => e.source !== id && e.target !== id));
      setSelectedId(null);
    },
    [setNodes, setEdges],
  );

  const save = useCallback(async () => {
    setSaving(true);
    setMessage(null);
    try {
      const graph = rfToGraph(nodes, edges);
      await api.updatePipeline(pipeline.id, { name, graph });
      const result = await api.validatePipeline(pipeline.id);
      setValidation(result);
      setDirty(false);
      setMessage({ kind: "ok", text: "Saved" });
    } catch (e) {
      setMessage({ kind: "err", text: e instanceof Error ? e.message : "Save failed" });
    } finally {
      setSaving(false);
    }
  }, [nodes, edges, name, pipeline.id]);

  const deploy = useCallback(async () => {
    try {
      const res = await api.deployPipeline(pipeline.id);
      setMessage({ kind: "ok", text: `Deployed v${res.version_number}` });
    } catch (e) {
      setMessage({ kind: "err", text: e instanceof Error ? e.message : "Deploy failed" });
    }
  }, [pipeline.id]);

  return (
    <div className="flex flex-col h-[calc(100vh-7rem)]">
      <header className="flex items-center gap-3 pb-3">
        <input
          value={name}
          onChange={(e) => setName(e.target.value)}
          className="bg-transparent text-xl font-bold text-white outline-none border-b border-transparent focus:border-surface-700 min-w-0"
        />
        <div className="flex items-center gap-2 ml-auto">
          {validation && !validation.ok && (
            <span className="flex items-center gap-1 text-xs text-red-400">
              <AlertCircle size={13} /> Invalid DAG
            </span>
          )}
          {validation?.ok && (
            <span className="flex items-center gap-1 text-xs text-emerald-400">
              <CheckCircle2 size={13} /> Valid
            </span>
          )}
          {message && (
            <span className={`text-xs ${message.kind === "ok" ? "text-emerald-400" : "text-red-400"}`}>
              {message.text}
            </span>
          )}
          <button onClick={() => setShowRun((s) => !s)} className="btn-secondary">
            {showRun ? <X size={15} /> : <Play size={15} />}
            {showRun ? "Close" : "Run"}
          </button>
          <button onClick={deploy} className="btn-secondary" title="Snapshot a version">
            <Rocket size={15} /> Deploy
          </button>
          <button onClick={save} disabled={saving} className="btn-primary">
            {saving ? <Loader2 size={15} className="animate-spin" /> : <Save size={15} />}
            Save{dirty ? " *" : ""}
          </button>
        </div>
      </header>

      <div className="flex flex-1 min-h-0 rounded-xl border border-surface-800 overflow-hidden">
        <NodePalette onAdd={addNode} />

        <div className="flex-1 min-w-0 bg-surface-950">
          <ReactFlow
            nodes={nodes}
            edges={edges}
            onNodesChange={onNodesChange}
            onEdgesChange={onEdgesChange}
            onConnect={onConnect}
            onNodeClick={(_, n) => setSelectedId(n.id)}
            onPaneClick={() => setSelectedId(null)}
            nodeTypes={nodeTypes}
            fitView
            proOptions={{ hideAttribution: true }}
          >
            <Background color="#1e293b" gap={20} />
            <Controls className="!bg-surface-900 !border-surface-700" />
            <MiniMap pannable className="!bg-surface-900" maskColor="rgba(2,6,23,0.7)" />
          </ReactFlow>
        </div>

        {showRun ? (
          <div className="w-80 flex-shrink-0 border-l border-surface-800 bg-surface-950 p-4 overflow-y-auto">
            <WorkflowRunPanel pipelineId={pipeline.id} saved={!dirty} />
          </div>
        ) : (
          <NodeConfigPanel
            node={selectedNode}
            agents={[]}
            teams={[]}
            onChange={updateNodeData}
            onDelete={deleteNode}
          />
        )}
      </div>
    </div>
  );
}
