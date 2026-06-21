"use client";

import { useState, useEffect } from "react";
import { useParams } from "next/navigation";
import Link from "next/link";
import {
  ArrowLeft,
  Bot,
  MessageSquare,
  Activity,
  Play,
  Clock,
  AlertCircle,
  Loader2,
  Cpu,
  Shield,
  Database,
  Wrench,
  FileText,
} from "lucide-react";
import AgentChat from "@/components/AgentChat";

interface Agent {
  id: string;
  name: string;
  description: string;
  instructions: string;
  model: string;
  tools:
    | string[]
    | { type: string; function: { name: string } }[];
  guard_config: {
    enabled: boolean;
    pii_redaction: boolean;
    toxicity_filter: boolean;
    max_tokens_per_response: number;
    allowed_topics: string[];
    blocked_topics: string[];
  };
  memory_config: {
    enabled: boolean;
    type: string;
    max_history: number;
    summarization: boolean;
    vector_search: boolean;
  };
  status: string;
  created_at: string;
}

interface Run {
  id: string;
  status: string;
  duration: number;
  total_tokens: number;
  created_at: string;
}

export default function AgentDetailPage() {
  const params = useParams();
  const agentId = params.id as string;
  const [agent, setAgent] = useState<Agent | null>(null);
  const [runs, setRuns] = useState<Run[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<"chat" | "runs">("chat");

  useEffect(() => {
    const baseUrl =
      process.env.NEXT_PUBLIC_API_URL || "http://localhost:8000";

    Promise.all([
      fetch(`${baseUrl}/api/agents/${agentId}`).then((r) => {
        if (!r.ok) throw new Error(`Failed to fetch agent: ${r.status}`);
        return r.json();
      }),
      fetch(`${baseUrl}/api/runs?agent_id=${agentId}`).then((r) => {
        if (!r.ok) return [];
        return r.json();
      }),
    ])
      .then(([agentData, runsData]) => {
        setAgent(agentData);
        setRuns(Array.isArray(runsData) ? runsData : []);
        setLoading(false);
      })
      .catch((err) => {
        setError(err.message);
        setLoading(false);
      });
  }, [agentId]);

  if (loading) {
    return (
      <div className="flex items-center justify-center min-h-[400px]">
        <Loader2 size={24} className="text-brand-400 animate-spin" />
      </div>
    );
  }

  if (error || !agent) {
    return (
      <div className="card p-12 text-center">
        <AlertCircle size={32} className="text-red-400 mx-auto mb-3" />
        <p className="text-red-400 font-medium mb-1">Failed to load agent</p>
        <p className="text-surface-500 text-sm">{error || "Agent not found"}</p>
        <Link href="/agents" className="btn-secondary mt-4 inline-flex items-center gap-1">
          <ArrowLeft size={14} />
          Back to Agents
        </Link>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Breadcrumb */}
      <div className="flex items-center gap-2 text-sm">
        <Link
          href="/agents"
          className="text-surface-400 hover:text-surface-200 flex items-center gap-1 transition-colors"
        >
          <ArrowLeft size={14} />
          Agents
        </Link>
        <span className="text-surface-600">/</span>
        <span className="text-surface-200">{agent.name}</span>
      </div>

      {/* Agent Header */}
      <div className="card p-6">
        <div className="flex items-start justify-between">
          <div className="flex items-start gap-4">
            <div className="w-14 h-14 rounded-2xl bg-brand-500/10 flex items-center justify-center text-brand-400">
              <Bot size={28} />
            </div>
            <div>
              <div className="flex items-center gap-3 mb-1">
                <h1 className="text-xl font-bold text-white">{agent.name}</h1>
                <span
                  className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium ${
                    agent.status === "active"
                      ? "bg-green-500/10 text-green-400"
                      : "bg-surface-700 text-surface-400"
                  }`}
                >
                  <span
                    className={`w-1.5 h-1.5 rounded-full ${
                      agent.status === "active"
                        ? "bg-green-400 animate-pulse-slow"
                        : "bg-surface-500"
                    }`}
                  />
                  {agent.status}
                </span>
              </div>
              <p className="text-sm text-surface-400 mb-3">{agent.description}</p>
              <span className="px-2 py-0.5 rounded bg-surface-800 font-mono text-xs text-surface-500">
                {agent.model}
              </span>
            </div>
          </div>
        </div>
      </div>

      {/* Agent Details Grid */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        <div className="lg:col-span-2 space-y-6">
          {/* Instructions */}
          <div className="card p-5">
            <div className="flex items-center gap-2 mb-3">
              <FileText size={16} className="text-brand-400" />
              <h2 className="text-sm font-semibold text-white">Instructions</h2>
            </div>
            <p className="text-sm text-surface-300 whitespace-pre-wrap">
              {agent.instructions}
            </p>
          </div>

          {/* Tools */}
          <div className="card p-5">
            <div className="flex items-center gap-2 mb-3">
              <Wrench size={16} className="text-brand-400" />
              <h2 className="text-sm font-semibold text-white">Tools</h2>
            </div>
            {agent.tools && agent.tools.length > 0 ? (
              <div className="flex flex-wrap gap-2">
                {agent.tools.map((tool, idx) => {
                  // Defensive: backend may store either plain strings (new format)
                  // or OpenAI-shape wrappers (legacy once the form was wrapping them).
                  const label =
                    typeof tool === "string"
                      ? tool
                      : tool?.function?.name || tool?.name || JSON.stringify(tool);
                  return (
                    <span
                      key={typeof tool === "string" ? tool : idx}
                      className="px-2.5 py-1 rounded-lg bg-surface-800 text-xs font-mono text-surface-300"
                    >
                      {label}
                    </span>
                  );
                })}
              </div>
            ) : (
              <p className="text-sm text-surface-500">No tools configured</p>
            )}
          </div>

          {/* Chat Tab */}
          <div className="card overflow-hidden">
            <div className="flex items-center gap-4 px-5 pt-4 pb-0 border-b border-surface-800">
              <button
                onClick={() => setActiveTab("chat")}
                className={`flex items-center gap-2 px-1 py-3 text-sm transition-colors ${
                  activeTab === "chat" ? "tab-active" : "tab-inactive"
                }`}
              >
                <MessageSquare size={15} />
                Chat
              </button>
              <button
                onClick={() => setActiveTab("runs")}
                className={`flex items-center gap-2 px-1 py-3 text-sm transition-colors ${
                  activeTab === "runs" ? "tab-active" : "tab-inactive"
                }`}
              >
                <Activity size={15} />
                Run History
              </button>
            </div>
            <div className="p-5">
              {activeTab === "chat" ? (
                <AgentChat
                  agentId={agentId}
                  onRunComplete={(id) => console.log("run complete", id)}
                />
              ) : (
                <RunsList runs={runs} />
              )}
            </div>
          </div>
        </div>

        {/* Sidebar */}
        <div className="space-y-6">
          <div className="card p-5">
            <div className="flex items-center gap-2 mb-3">
              <Cpu size={16} className="text-brand-400" />
              <h2 className="text-sm font-semibold text-white">Model</h2>
            </div>
            <p className="text-sm font-mono text-surface-200">{agent.model}</p>
          </div>

          <div className="card p-5">
            <div className="flex items-center gap-2 mb-3">
              <Shield size={16} className="text-brand-400" />
              <h2 className="text-sm font-semibold text-white">Guardrails</h2>
            </div>
            {agent.guard_config ? (
              <div className="space-y-2 text-sm">
                <div className="flex items-center justify-between">
                  <span className="text-surface-400">Enabled</span>
                  <span
                    className={
                      agent.guard_config.enabled
                        ? "text-green-400"
                        : "text-surface-500"
                    }
                  >
                    {agent.guard_config.enabled ? "Yes" : "No"}
                  </span>
                </div>
                <div className="flex items-center justify-between">
                  <span className="text-surface-400">PII Redaction</span>
                  <span
                    className={
                      agent.guard_config.pii_redaction
                        ? "text-green-400"
                        : "text-surface-500"
                    }
                  >
                    {agent.guard_config.pii_redaction ? "Yes" : "No"}
                  </span>
                </div>
                <div className="flex items-center justify-between">
                  <span className="text-surface-400">Toxicity Filter</span>
                  <span
                    className={
                      agent.guard_config.toxicity_filter
                        ? "text-green-400"
                        : "text-surface-500"
                    }
                  >
                    {agent.guard_config.toxicity_filter ? "Yes" : "No"}
                  </span>
                </div>
                <div className="flex items-center justify-between">
                  <span className="text-surface-400">Max Tokens</span>
                  <span className="text-surface-200">
                    {agent.guard_config.max_tokens_per_response}
                  </span>
                </div>
              </div>
            ) : (
              <p className="text-sm text-surface-500">No guardrails configured</p>
            )}
          </div>

          <div className="card p-5">
            <div className="flex items-center gap-2 mb-3">
              <Database size={16} className="text-brand-400" />
              <h2 className="text-sm font-semibold text-white">Memory</h2>
            </div>
            {agent.memory_config ? (
              <div className="space-y-2 text-sm">
                <div className="flex items-center justify-between">
                  <span className="text-surface-400">Enabled</span>
                  <span
                    className={
                      agent.memory_config.enabled
                        ? "text-green-400"
                        : "text-surface-500"
                    }
                  >
                    {agent.memory_config.enabled ? "Yes" : "No"}
                  </span>
                </div>
                <div className="flex items-center justify-between">
                  <span className="text-surface-400">Type</span>
                  <span className="text-surface-200 font-mono text-xs">
                    {agent.memory_config.type}
                  </span>
                </div>
                <div className="flex items-center justify-between">
                  <span className="text-surface-400">Max History</span>
                  <span className="text-surface-200">
                    {agent.memory_config.max_history}
                  </span>
                </div>
                <div className="flex items-center justify-between">
                  <span className="text-surface-400">Summarization</span>
                  <span
                    className={
                      agent.memory_config.summarization
                        ? "text-green-400"
                        : "text-surface-500"
                    }
                  >
                    {agent.memory_config.summarization ? "Yes" : "No"}
                  </span>
                </div>
                <div className="flex items-center justify-between">
                  <span className="text-surface-400">Vector Search</span>
                  <span
                    className={
                      agent.memory_config.vector_search
                        ? "text-green-400"
                        : "text-surface-500"
                    }
                  >
                    {agent.memory_config.vector_search ? "Yes" : "No"}
                  </span>
                </div>
              </div>
            ) : (
              <p className="text-sm text-surface-500">No memory configured</p>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

const STATUS_STYLES: Record<string, string> = {
  completed: "bg-green-500/10 text-green-400 border border-green-500/20",
  failed: "bg-red-500/10 text-red-400 border border-red-500/20",
  running: "bg-blue-500/10 text-blue-400 border border-blue-500/20",
  cancelled: "bg-surface-700 text-surface-400 border border-surface-600",
};

function RunsList({ runs }: { runs: Run[] }) {
  if (runs.length === 0) {
    return (
      <div className="text-center py-8">
        <Activity size={24} className="text-surface-600 mx-auto mb-2" />
        <p className="text-surface-500 text-sm">No runs yet</p>
      </div>
    );
  }

  return (
    <div className="divide-y divide-surface-800 -mx-5 -mb-5">
      {runs.map((run) => (
        <Link
          key={run.id}
          href={`/runs/${run.id}`}
          className="flex items-center justify-between px-5 py-4 hover:bg-surface-800/50 transition-colors"
        >
          <div className="flex items-center gap-3">
            <span
              className={`w-2 h-2 rounded-full ${
                run.status === "completed"
                  ? "bg-green-400"
                  : run.status === "failed"
                  ? "bg-red-400"
                  : run.status === "running"
                  ? "bg-blue-400 animate-pulse-slow"
                  : "bg-surface-600"
              }`}
            />
            <div>
              <div className="text-sm font-medium text-surface-200 font-mono">
                {run.id}
              </div>
              <div className="text-xs text-surface-500">
                {run.created_at
                  ? new Date(run.created_at).toLocaleString()
                  : "N/A"}
              </div>
            </div>
          </div>
          <div className="flex items-center gap-4 text-xs text-surface-400">
            <span>
              {run.duration ? `${(run.duration / 1000).toFixed(1)}s` : "—"}
            </span>
            <span>{run.total_tokens ?? "—"} tok</span>
            <span
              className={`inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium ${
                STATUS_STYLES[run.status] || ""
              }`}
            >
              {run.status}
            </span>
          </div>
        </Link>
      ))}
    </div>
  );
}
