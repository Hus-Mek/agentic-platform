"use client";

import { Handle, Position, type NodeProps } from "@xyflow/react";
import * as Icons from "lucide-react";

import { NODE_CATALOG, nodeKind, type WorkflowNodeData } from "../../lib/workflow";

const STATUS_RING: Record<string, string> = {
  running: "ring-2 ring-amber-400 animate-pulse",
  completed: "ring-2 ring-emerald-400",
  failed: "ring-2 ring-red-400",
};

export default function WorkflowNodeView({ data, selected }: NodeProps) {
  const nodeData = data as WorkflowNodeData;
  const kind = nodeKind(nodeData.nodeType);
  const entry = NODE_CATALOG.find((e) => e.kind === kind);
  const Icon = (entry && (Icons as Record<string, unknown>)[entry.icon]) as
    | React.ComponentType<{ size?: number; className?: string }>
    | undefined;

  const status = (nodeData.__status as string | undefined) || "";
  const isInput = kind === "Input";
  const isOutput = kind === "Output";

  return (
    <div
      className={`rounded-xl border bg-surface-900 px-3 py-2.5 min-w-[150px] shadow-lg transition-colors ${
        selected ? "border-brand-500" : "border-surface-700"
      } ${STATUS_RING[status] || ""}`}
    >
      {!isInput && (
        <Handle type="target" position={Position.Left} className="!bg-surface-500 !w-2.5 !h-2.5" />
      )}
      <div className="flex items-center gap-2">
        <div className={`flex-shrink-0 ${entry?.accent || "text-surface-400"}`}>
          {Icon ? <Icon size={16} /> : null}
        </div>
        <div className="min-w-0">
          <div className="text-[10px] uppercase tracking-wide text-surface-500">{kind}</div>
          <div className="text-sm font-medium text-white truncate">{nodeData.name}</div>
        </div>
      </div>
      {!isOutput && (
        <Handle type="source" position={Position.Right} className="!bg-brand-500 !w-2.5 !h-2.5" />
      )}
    </div>
  );
}
