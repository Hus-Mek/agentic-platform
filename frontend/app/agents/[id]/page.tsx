"use client";

import { useState } from "react";
import { useParams } from "next/navigation";
import Link from "next/link";
import {
  ArrowLeft,
  Bot,
  MessageSquare,
  Settings,
  Activity,
  BarChart3,
  Play,
  Clock,
  CheckCircle2,
} from "lucide-react";
import AgentChat from "@/components/AgentChat";
import AgentForm from "@/components/AgentForm";

type Tab = "chat" | "config" | "runs" | "traces";

const MOCK_AGENT = {
  id: "agent_001",
  name: "Customer Support Bot",
  description: "Handles customer inquiries, escalations, and FAQs",
  instructions:
    "You are a helpful customer support agent for a SaaS company. Be empathetic, concise, and solution-oriented. Always ask for clarification if the issue is unclear. Escalate to human agents when you detect frustration or complex billing issues.",
  model: "gpt-4o",
  tools: ["web_search", "kb_search", "calculator", "send_email"],
  guard_config: {
    enabled: true,
    pii_redaction: true,
    toxicity_filter: true,
    max_tokens_per_response: 2000,
    allowed_topics: ["support", "billing", "technical"],
    blocked_topics: ["competitors", "pricing_strategy"],
  },
  memory_config: {
    enabled: true,
    type: "conversation",
    max_history: 50,
    summarization: true,
    vector_search: true,
  },
  status: "active" as const,
  runs: 1247,
  lastRun: "2 min ago",
  avgDuration: "1.2s",
  successRate: "97.3%",
};

const TABS: { id: Tab; label: string; icon: React.ReactNode }[] = [
  { id: "chat", label: "Chat", icon: <MessageSquare size={15} /> },
  { id: "config", label: "Config", icon: <Settings size={15} /> },
  { id: "runs", label: "Runs", icon: <Activity size={15} /> },
  { id: "traces", label: "Traces", icon: <BarChart3 size={15} /> },
];

export default function AgentDetailPage() {
  const params = useParams();
  const agentId = params.id as string;
  const [activeTab, setActiveTab] = useState<Tab>("chat");

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
        <span className="text-surface-200">{MOCK_AGENT.name}</span>
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
                <h1 className="text-xl font-bold text-white">
                  {MOCK_AGENT.name}
                </h1>
                <span
                  className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium ${
                    MOCK_AGENT.status === "active"
                      ? "bg-green-500/10 text-green-400"
                      : "bg-surface-700 text-surface-400"
                  }`}
                >
                  <span
                    className={`w-1.5 h-1.5 rounded-full ${
                      MOCK_AGENT.status === "active"
                        ? "bg-green-400 animate-pulse-slow"
                        : "bg-surface-500"
                    }`}
                  />
                  {MOCK_AGENT.status}
                </span>
              </div>
              <p className="text-sm text-surface-400 mb-3">
                {MOCK_AGENT.description}
              </p>
              <div className="flex items-center gap-4 text-xs text-surface-500">
                <span className="flex items-center gap-1">
                  <Play size={12} />
                  {MOCK_AGENT.runs.toLocaleString()} runs
                </span>
                <span className="flex items-center gap-1">
                  <Clock size={12} />
                  {MOCK_AGENT.avgDuration}
                </span>
                <span className="flex items-center gap-1">
                  <CheckCircle2 size={12} />
                  {MOCK_AGENT.successRate} success
                </span>
              </div>
            </div>
          </div>
          <button className="btn-primary">
            <Play size={16} />
            Run Now
          </button>
        </div>
      </div>

      {/* Tabs */}
      <div className="flex items-center gap-6 border-b border-surface-800">
        {TABS.map((tab) => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id)}
            className={`flex items-center gap-2 px-1 py-3 text-sm transition-colors ${
              activeTab === tab.id ? "tab-active" : "tab-inactive"
            }`}
          >
            {tab.icon}
            {tab.label}
          </button>
        ))}
      </div>

      {/* Tab Content */}
      <div className="min-h-[500px]">
        {activeTab === "chat" && (
          <AgentChat agentId={agentId} onRunComplete={(id) => console.log("run complete", id)} />
        )}
        {activeTab === "config" && (
          <AgentForm
            initialData={MOCK_AGENT}
            onSubmit={(data) => console.log("save", data)}
            onCancel={() => setActiveTab("chat")}
          />
        )}
        {activeTab === "runs" && <RunsTab />}
        {activeTab === "traces" && <TracesTab />}
      </div>
    </div>
  );
}

function RunsTab() {
  return (
    <div className="card divide-y divide-surface-800">
      {[
        { id: "run_001", status: "completed", duration: "1.2s", tokens: 456, cost: "$0.009", time: "2 min ago" },
        { id: "run_002", status: "completed", duration: "2.8s", tokens: 892, cost: "$0.018", time: "15 min ago" },
        { id: "run_003", status: "failed", duration: "8.4s", tokens: 1240, cost: "$0.025", time: "1 hr ago" },
        { id: "run_004", status: "completed", duration: "0.9s", tokens: 234, cost: "$0.005", time: "2 hrs ago" },
      ].map((run) => (
        <Link
          key={run.id}
          href={`/runs/${run.id}`}
          className="flex items-center justify-between px-5 py-4 hover:bg-surface-800/50 transition-colors"
        >
          <div className="flex items-center gap-3">
            <StatusDot status={run.status} />
            <div>
              <div className="text-sm font-medium text-surface-200 font-mono">
                {run.id}
              </div>
              <div className="text-xs text-surface-500">{run.time}</div>
            </div>
          </div>
          <div className="flex items-center gap-4 text-xs text-surface-400">
            <span>{run.duration}</span>
            <span>{run.tokens} tok</span>
            <span>{run.cost}</span>
          </div>
        </Link>
      ))}
    </div>
  );
}

function TracesTab() {
  return (
    <div className="card p-6 text-center">
      <BarChart3 size={32} className="text-surface-600 mx-auto mb-3" />
      <p className="text-surface-400 text-sm">
        Select a run to view its trace timeline
      </p>
      <p className="text-xs text-surface-500 mt-1">
        Traces show each step the agent took during a run
      </p>
    </div>
  );
}

function StatusDot({ status }: { status: string }) {
  const colors: Record<string, string> = {
    completed: "bg-green-400",
    failed: "bg-red-400",
    running: "bg-blue-400 animate-pulse-slow",
  };
  return (
    <span
      className={`w-2 h-2 rounded-full ${colors[status] || "bg-surface-600"}`}
    />
  );
}
