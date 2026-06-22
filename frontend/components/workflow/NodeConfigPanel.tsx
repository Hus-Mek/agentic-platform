"use client";

import { Trash2 } from "lucide-react";

import type { NodeTypeData } from "../../lib/api";
import { nodeKind, type WorkflowNode, type WorkflowNodeData } from "../../lib/workflow";

interface NodeConfigPanelProps {
  node: WorkflowNode | null;
  agents: { id: string; name: string }[];
  teams: { id: string; name: string }[];
  onChange: (id: string, data: Partial<WorkflowNodeData>) => void;
  onDelete: (id: string) => void;
}

export default function NodeConfigPanel({
  node,
  agents,
  teams,
  onChange,
  onDelete,
}: NodeConfigPanelProps) {
  if (!node) {
    return (
      <div className="w-72 flex-shrink-0 border-l border-surface-800 bg-surface-950 p-4">
        <p className="text-sm text-surface-500">Select a node to configure it.</p>
      </div>
    );
  }

  const data = node.data;
  const kind = nodeKind(data.nodeType);

  const setType = (nodeType: NodeTypeData) => onChange(node.id, { nodeType });
  const setConfig = (patch: Record<string, unknown>) =>
    onChange(node.id, { config: { ...data.config, ...patch } });

  return (
    <div className="w-72 flex-shrink-0 border-l border-surface-800 bg-surface-950 p-4 space-y-4 overflow-y-auto">
      <div className="flex items-center justify-between">
        <span className="text-xs uppercase tracking-wide text-surface-500">{kind} node</span>
        <button
          onClick={() => onDelete(node.id)}
          className="text-surface-500 hover:text-red-400 transition-colors"
          title="Delete node"
        >
          <Trash2 size={15} />
        </button>
      </div>

      <Field label="Name">
        <input
          className="input"
          value={data.name}
          onChange={(e) => onChange(node.id, { name: e.target.value })}
        />
      </Field>

      {typeof data.nodeType !== "string" && "Model" in data.nodeType && (
        <>
          <Field label="Model ID">
            <input
              className="input"
              value={data.nodeType.Model.model_id}
              onChange={(e) => setType({ Model: { ...(data.nodeType as { Model: { model_id: string; prompt_template: string } }).Model, model_id: e.target.value } })}
            />
          </Field>
          <Field label="Prompt template" hint="Use {{name}} of an upstream node">
            <textarea
              className="input min-h-[120px] font-mono text-xs"
              value={data.nodeType.Model.prompt_template}
              onChange={(e) => setType({ Model: { ...(data.nodeType as { Model: { model_id: string; prompt_template: string } }).Model, prompt_template: e.target.value } })}
            />
          </Field>
        </>
      )}

      {typeof data.nodeType !== "string" && "Script" in data.nodeType && (
        <>
          <Field label="Function name">
            <input
              className="input"
              value={data.nodeType.Script.function_name}
              onChange={(e) => setType({ Script: { ...(data.nodeType as { Script: { code: string | null; function_name: string } }).Script, function_name: e.target.value } })}
            />
          </Field>
          <Field label="WASM module (WAT or wasm-text)" hint="Runs in the wasmtime sandbox">
            <textarea
              className="input min-h-[140px] font-mono text-xs"
              value={data.nodeType.Script.code || ""}
              onChange={(e) => setType({ Script: { ...(data.nodeType as { Script: { code: string | null; function_name: string } }).Script, code: e.target.value } })}
            />
          </Field>
          <div className="grid grid-cols-3 gap-2">
            <Field label="Fuel">
              <input className="input" type="number" value={(data.config.fuel as number) ?? ""} placeholder="200M"
                onChange={(e) => setConfig({ fuel: e.target.value ? Number(e.target.value) : undefined })} />
            </Field>
            <Field label="Mem (B)">
              <input className="input" type="number" value={(data.config.memory_bytes as number) ?? ""} placeholder="64M"
                onChange={(e) => setConfig({ memory_bytes: e.target.value ? Number(e.target.value) : undefined })} />
            </Field>
            <Field label="Wall ms">
              <input className="input" type="number" value={(data.config.wall_ms as number) ?? ""} placeholder="5000"
                onChange={(e) => setConfig({ wall_ms: e.target.value ? Number(e.target.value) : undefined })} />
            </Field>
          </div>
        </>
      )}

      {typeof data.nodeType !== "string" && "Decision" in data.nodeType && (
        <Field label="Condition" hint="e.g. score > 50">
          <input
            className="input font-mono text-xs"
            value={data.nodeType.Decision.condition}
            onChange={(e) => setType({ Decision: { condition: e.target.value } })}
          />
        </Field>
      )}

      {typeof data.nodeType !== "string" && "Metric" in data.nodeType && (
        <Field label="Metric type">
          <select
            className="input"
            value={data.nodeType.Metric.metric_type}
            onChange={(e) => setType({ Metric: { metric_type: e.target.value } })}
          >
            <option value="exact_match">exact_match</option>
            <option value="contains_match">contains_match</option>
            <option value="bleu">bleu</option>
            <option value="chrf">chrf</option>
          </select>
        </Field>
      )}

      {typeof data.nodeType !== "string" && "Agent" in data.nodeType && (
        <Field label="Agent">
          <select
            className="input"
            value={data.nodeType.Agent.agent_id}
            onChange={(e) => setType({ Agent: { agent_id: e.target.value } })}
          >
            <option value="">Select an agent…</option>
            {agents.map((a) => (
              <option key={a.id} value={a.id}>{a.name}</option>
            ))}
          </select>
        </Field>
      )}

      {typeof data.nodeType !== "string" && "Team" in data.nodeType && (
        <Field label="Team">
          <select
            className="input"
            value={data.nodeType.Team.team_id}
            onChange={(e) => setType({ Team: { team_id: e.target.value } })}
          >
            <option value="">Select a team…</option>
            {teams.map((t) => (
              <option key={t.id} value={t.id}>{t.name}</option>
            ))}
          </select>
        </Field>
      )}

      {(kind === "Input" || kind === "Output" || kind === "Router") && (
        <p className="text-xs text-surface-500">
          {kind === "Input"
            ? "Receives run data. Reference it downstream as {{input}}."
            : kind === "Output"
              ? "Collects all incoming values as the final result."
              : "Passes the incoming value through to connected nodes."}
        </p>
      )}
    </div>
  );
}

function Field({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <div>
      <label className="label">{label}</label>
      {children}
      {hint && <p className="text-[11px] text-surface-500 mt-1">{hint}</p>}
    </div>
  );
}
