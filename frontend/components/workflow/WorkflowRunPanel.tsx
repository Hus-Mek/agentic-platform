"use client";

import { useState } from "react";
import { Play, Loader2, AlertCircle, CheckCircle2, XCircle } from "lucide-react";

import { api, type RunStep } from "../../lib/api";
import { usePipelineRun, type NodeRunStatus } from "../../hooks/usePipelineRun";
import TraceViewer from "../TraceViewer";

interface WorkflowRunPanelProps {
  pipelineId: string;
  saved: boolean;
}

const STATUS_ICON: Record<NodeRunStatus, React.ReactNode> = {
  running: <Loader2 size={13} className="text-amber-400 animate-spin" />,
  completed: <CheckCircle2 size={13} className="text-emerald-400" />,
  failed: <XCircle size={13} className="text-red-400" />,
};

export default function WorkflowRunPanel({ pipelineId, saved }: WorkflowRunPanelProps) {
  const [rawData, setRawData] = useState('{\n  "input": "Hello"\n}');
  const [parseError, setParseError] = useState<string | null>(null);
  const [steps, setSteps] = useState<RunStep[] | null>(null);
  const { isRunning, nodes, order, error, done, run } = usePipelineRun(pipelineId);

  const handleRun = async () => {
    setParseError(null);
    setSteps(null);
    let data: Record<string, unknown>;
    try {
      data = JSON.parse(rawData);
    } catch (e) {
      setParseError(e instanceof Error ? e.message : "Invalid JSON");
      return;
    }
    await run(data);
    // After the stream completes, load the persisted node steps for the trace.
    try {
      const runs = await api.listPipelineRuns(pipelineId);
      if (runs[0]) setSteps(await api.getPipelineRunSteps(runs[0].id));
    } catch {
      // trace is best-effort; live node status already shown
    }
  };

  return (
    <div className="space-y-4">
      <div>
        <label className="label">Run input (JSON)</label>
        <textarea
          className="input font-mono text-xs min-h-[90px]"
          value={rawData}
          onChange={(e) => setRawData(e.target.value)}
        />
        {parseError && <p className="text-xs text-red-400 mt-1">{parseError}</p>}
      </div>

      <button onClick={handleRun} disabled={isRunning || !saved} className="btn-primary w-full justify-center">
        {isRunning ? <Loader2 size={16} className="animate-spin" /> : <Play size={16} />}
        {isRunning ? "Running…" : "Run workflow"}
      </button>
      {!saved && <p className="text-xs text-surface-500">Save the workflow before running.</p>}

      {error && (
        <div className="flex items-start gap-2 rounded-lg border border-red-500/30 bg-red-500/10 p-3 text-xs text-red-300">
          <AlertCircle size={14} className="flex-shrink-0 mt-0.5" />
          <span>{error}</span>
        </div>
      )}

      {order.length > 0 && (
        <div className="space-y-1.5">
          <div className="text-xs font-medium text-surface-500">Live progress</div>
          {order.map((id) => {
            const n = nodes[id];
            return (
              <div key={id} className="flex items-center gap-2 text-xs text-surface-300">
                {STATUS_ICON[n.status]}
                <span className="truncate">{n.nodeName || id}</span>
                {typeof n.tokens === "number" && n.tokens > 0 && (
                  <span className="ml-auto text-surface-500">{n.tokens} tok</span>
                )}
              </div>
            );
          })}
        </div>
      )}

      {done && steps && steps.length > 0 && (
        <div className="pt-2 border-t border-surface-800">
          <div className="text-xs font-medium text-surface-500 mb-3">Run trace</div>
          <TraceViewer
            steps={steps.map((s) => ({
              id: s.id,
              type: (s.step_type as "plan" | "tool" | "evaluate" | "guard" | "message" | "other") || "other",
              nodeName: s.node_name || "node",
              input: s.input || "",
              output: s.output || "",
              durationMs: s.duration_ms || 0,
              tokensUsed: s.tokens_used || 0,
              createdAt: s.created_at,
            }))}
          />
        </div>
      )}
    </div>
  );
}
