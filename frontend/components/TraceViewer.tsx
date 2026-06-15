"use client";

import { useState } from "react";
import { ChevronDown, ChevronRight, Clock, Cpu } from "lucide-react";

interface TraceStep {
  id: string;
  type: "plan" | "tool" | "evaluate" | "guard" | "message" | "other";
  nodeName: string;
  input: string;
  output: string;
  durationMs: number;
  tokensUsed: number;
  createdAt: string;
}

interface TraceViewerProps {
  steps: TraceStep[];
}

const TYPE_COLORS: Record<
  string,
  { bg: string; dot: string; label: string; text: string }
> = {
  plan: {
    bg: "bg-blue-500/10",
    dot: "bg-blue-500",
    label: "blue-500",
    text: "text-blue-400",
  },
  tool: {
    bg: "bg-amber-500/10",
    dot: "bg-amber-500",
    label: "amber-500",
    text: "text-amber-400",
  },
  evaluate: {
    bg: "bg-emerald-500/10",
    dot: "bg-emerald-500",
    label: "emerald-500",
    text: "text-emerald-400",
  },
  guard: {
    bg: "bg-red-500/10",
    dot: "bg-red-500",
    label: "red-500",
    text: "text-red-400",
  },
  message: {
    bg: "bg-purple-500/10",
    dot: "bg-purple-500",
    label: "purple-500",
    text: "text-purple-400",
  },
  other: {
    bg: "bg-surface-700",
    dot: "bg-surface-500",
    label: "surface-500",
    text: "text-surface-400",
  },
};

function formatMs(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

export default function TraceViewer({ steps }: TraceViewerProps) {
  const totalDuration = steps.reduce((sum, s) => sum + s.durationMs, 0);
  const totalTokens = steps.reduce((sum, s) => sum + (s.tokensUsed || 0), 0);

  return (
    <div className="space-y-4">
      {/* Summary bar */}
      <div className="flex items-center gap-4 pb-4 border-b border-surface-800">
        <div className="flex items-center gap-1.5 text-xs text-surface-400">
          <Clock size={12} />
          {steps.length} steps · {formatMs(totalDuration)} total
        </div>
        <div className="flex items-center gap-1.5 text-xs text-surface-400">
          <Cpu size={12} />
          {totalTokens.toLocaleString()} tokens
        </div>
        <div className="flex items-center gap-2 ml-auto">
          {Object.entries(TYPE_COLORS).map(([type, color]) => (
            <span
              key={type}
              className={`inline-flex items-center gap-1 text-xs ${color.text}`}
            >
              <span className={`w-1.5 h-1.5 rounded-full ${color.dot}`} />
              {type}
            </span>
          ))}
        </div>
      </div>

      {/* Timeline */}
      <div className="relative">
        {steps.map((step, i) => {
          const color = TYPE_COLORS[step.type] || TYPE_COLORS.other;
          const isLast = i === steps.length - 1;
          const widthPct = (step.durationMs / Math.max(...steps.map((s) => s.durationMs))) * 100;

          return (
            <StepItem
              key={step.id}
              step={step}
              color={color}
              isLast={isLast}
              widthPct={widthPct}
            />
          );
        })}
      </div>
    </div>
  );
}

function StepItem({
  step,
  color,
  isLast,
  widthPct,
}: {
  step: TraceStep;
  color: (typeof TYPE_COLORS)[keyof typeof TYPE_COLORS];
  isLast: boolean;
  widthPct: number;
}) {
  const [expanded, setExpanded] = useState(false);

  return (
    <div className="relative pl-8 pb-4">
      {/* Timeline line */}
      {!isLast && (
        <div className="absolute left-[11px] top-4 bottom-0 w-0.5 bg-surface-700" />
      )}

      {/* Dot */}
      <div
        className={`absolute left-[5px] top-[6px] w-[13px] h-[13px] rounded-full border-2 border-surface-900 ${color.dot}`}
      />

      {/* Content */}
      <div
        className="card overflow-hidden cursor-pointer hover:border-surface-700 transition-colors"
        onClick={() => setExpanded(!expanded)}
      >
        <div className="px-4 py-3">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2 min-w-0">
              <span
                className={`px-2 py-0.5 rounded text-xs font-medium ${color.bg} ${color.text} capitalize`}
              >
                {step.type}
              </span>
              <span className="text-sm font-medium text-surface-200 truncate">
                {step.nodeName}
              </span>
            </div>
            <div className="flex items-center gap-3 flex-shrink-0">
              {/* Duration bar */}
              <div className="hidden sm:block w-20 h-1.5 rounded-full bg-surface-800 overflow-hidden">
                <div
                  className={`h-full rounded-full ${color.dot} opacity-60`}
                  style={{ width: `${Math.max(widthPct, 5)}%` }}
                />
              </div>
              <div className="flex items-center gap-2 text-xs text-surface-400">
                <span>{formatMs(step.durationMs)}</span>
                {step.tokensUsed > 0 && (
                  <span className="hidden sm:inline">{step.tokensUsed} tok</span>
                )}
              </div>
              {expanded ? (
                <ChevronDown size={14} className="text-surface-500" />
              ) : (
                <ChevronRight size={14} className="text-surface-500" />
              )}
            </div>
          </div>
        </div>

        {/* Expanded I/O */}
        {expanded && (
          <div className="border-t border-surface-800 divide-y divide-surface-800">
            {step.input && (
              <div className="px-4 py-3">
                <div className="text-xs font-medium text-surface-500 mb-1.5">
                  Input
                </div>
                <pre className="text-xs text-surface-300 whitespace-pre-wrap font-mono bg-surface-950 rounded-lg p-3 max-h-40 overflow-y-auto">
                  {step.input}
                </pre>
              </div>
            )}
            {step.output && (
              <div className="px-4 py-3">
                <div className="text-xs font-medium text-surface-500 mb-1.5">
                  Output
                </div>
                <pre className="text-xs text-surface-300 whitespace-pre-wrap font-mono bg-surface-950 rounded-lg p-3 max-h-40 overflow-y-auto">
                  {step.output}
                </pre>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
