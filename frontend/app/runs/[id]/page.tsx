"use client";

import { useParams } from "next/navigation";
import Link from "next/link";
import {
  ArrowLeft,
  Clock,
  Cpu,
  DollarSign,
  CheckCircle2,
  XCircle,
  Loader2,
  Bot,
  User,
} from "lucide-react";
import TraceViewer from "@/components/TraceViewer";

const MOCK_RUN = {
  id: "run_abc123",
  agentName: "Customer Support Bot",
  agentId: "agent_001",
  status: "completed" as const,
  durationMs: 2460,
  totalTokens: 456,
  totalCost: 0.009,
  input:
    "I received a bill for $299 but I'm on the $99 Pro plan. Can you help me figure out what's going on?",
  output:
    "I understand your concern! Let me look into this for you.\n\nI can see that your account is on the **Pro Monthly** plan at $99/month, but there was an additional charge of $200 for **add-on storage** (100GB @ $2/GB) that was applied when you exceeded the 10GB included storage limit.\n\nHere's the breakdown:\n- Pro Plan: $99.00\n- Additional Storage (100GB): $200.00\n- **Total: $299.00**\n\nYou can reduce this by cleaning up old files or upgrading to the Pro Annual plan which includes 100GB of storage. Would you like me to help with either option?",
  steps: [
    {
      id: "step_1",
      type: "plan" as const,
      nodeName: "parse_intent",
      input: "Parse customer intent from message",
      output: "Intent: billing_inquiry, Sentiment: confused, Priority: medium",
      durationMs: 120,
      tokensUsed: 85,
      createdAt: new Date(Date.now() - 2500).toISOString(),
    },
    {
      id: "step_2",
      type: "tool" as const,
      nodeName: "lookup_account",
      input: 'query: "user billing info"',
      output:
        "Found account ACCT-87432: plan=pro_monthly, storage_used=110GB, current_bill=$299.00",
      durationMs: 340,
      tokensUsed: 0,
      createdAt: new Date(Date.now() - 2300).toISOString(),
    },
    {
      id: "step_3",
      type: "evaluate" as const,
      nodeName: "analyze_billing",
      input: "Account data + user message",
      output:
        "Base: $99.00, Overages: $200.00 (100GB @ $2/GB), Total: $299.00. User is 3mo into annual contract.",
      durationMs: 890,
      tokensUsed: 210,
      createdAt: new Date(Date.now() - 1900).toISOString(),
    },
    {
      id: "step_4",
      type: "guard" as const,
      nodeName: "response_guard",
      input: "Check generated response for PII, toxicity, allowed topics",
      output:
        "PII: none detected, Toxicity: 0.002, Topics: [billing] — ALLOWED",
      durationMs: 110,
      tokensUsed: 45,
      createdAt: new Date(Date.now() - 900).toISOString(),
    },
    {
      id: "step_5",
      type: "message" as const,
      nodeName: "generate_response",
      input: "Billing analysis + guard clear",
      output: "Response generated (truncated to 500 tokens)",
      durationMs: 1000,
      tokensUsed: 116,
      createdAt: new Date(Date.now() - 700).toISOString(),
    },
  ],
  createdAt: new Date(Date.now() - 300000).toISOString(),
};

export default function RunDetailPage() {
  const params = useParams();
  const runId = params.id as string;
  const run = MOCK_RUN;

  return (
    <div className="space-y-6 max-w-4xl">
      {/* Breadcrumb */}
      <div className="flex items-center gap-2 text-sm">
        <Link
          href="/agents"
          className="text-surface-400 hover:text-surface-200 flex items-center gap-1 transition-colors"
        >
          <ArrowLeft size={14} />
          {run.agentName}
        </Link>
        <span className="text-surface-600">/</span>
        <span className="text-surface-200 font-mono">{runId}</span>
      </div>

      {/* Header */}
      <div className="card p-6">
        <div className="flex items-start justify-between mb-4">
          <div>
            <div className="flex items-center gap-3 mb-2">
              <StatusBadge status={run.status} />
              <span className="text-xs font-mono text-surface-500 bg-surface-800 px-2 py-0.5 rounded">
                {runId}
              </span>
            </div>
            <Link
              href={`/agents/${run.agentId}`}
              className="text-sm text-brand-400 hover:text-brand-300 flex items-center gap-1"
            >
              <Bot size={14} />
              {run.agentName}
            </Link>
          </div>
        </div>

        <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
          <Metric
            icon={<Clock size={14} />}
            label="Duration"
            value={`${run.durationMs}ms`}
          />
          <Metric
            icon={<Cpu size={14} />}
            label="Tokens"
            value={run.totalTokens.toLocaleString()}
          />
          <Metric
            icon={<DollarSign size={14} />}
            label="Cost"
            value={`$${run.totalCost.toFixed(4)}`}
          />
          <Metric
            icon={
              run.status === "completed" ? (
                <CheckCircle2 size={14} />
              ) : (
                <XCircle size={14} />
              )
            }
            label="Status"
            value={run.status}
          />
        </div>
      </div>

      {/* Chat Transcript */}
      <div className="card p-6">
        <h2 className="text-sm font-semibold text-white mb-4">
          Chat Transcript
        </h2>
        <div className="space-y-4">
          <div className="flex items-start gap-3 flex-row-reverse">
            <div className="w-8 h-8 rounded-full bg-brand-600 flex items-center justify-center flex-shrink-0">
              <User size={14} className="text-white" />
            </div>
            <div className="message-bubble-user">
              <p className="text-sm leading-relaxed whitespace-pre-wrap">
                {run.input}
              </p>
            </div>
          </div>
          <div className="flex items-start gap-3">
            <div className="w-8 h-8 rounded-full bg-surface-700 flex items-center justify-center flex-shrink-0">
              <Bot size={14} className="text-surface-300" />
            </div>
            <div className="message-bubble-assistant">
              <p className="text-sm leading-relaxed whitespace-pre-wrap">
                {run.output}
              </p>
            </div>
          </div>
        </div>
      </div>

      {/* Trace Steps */}
      <div className="card p-6">
        <h2 className="text-sm font-semibold text-white mb-4">
          Execution Trace
        </h2>
        <TraceViewer steps={run.steps} />
      </div>
    </div>
  );
}

function StatusBadge({
  status,
}: {
  status: "completed" | "failed" | "running" | "pending";
}) {
  const config = {
    completed: {
      bg: "bg-green-500/10",
      text: "text-green-400",
      icon: <CheckCircle2 size={12} />,
    },
    failed: {
      bg: "bg-red-500/10",
      text: "text-red-400",
      icon: <XCircle size={12} />,
    },
    running: {
      bg: "bg-blue-500/10",
      text: "text-blue-400",
      icon: <Loader2 size={12} className="animate-spin" />,
    },
    pending: {
      bg: "bg-amber-500/10",
      text: "text-amber-400",
      icon: <Loader2 size={12} className="animate-spin" />,
    },
  };
  const c = config[status] || config.completed;
  return (
    <span
      className={`inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-medium ${c.bg} ${c.text}`}
    >
      {c.icon}
      {status}
    </span>
  );
}

function Metric({
  icon,
  label,
  value,
}: {
  icon: React.ReactNode;
  label: string;
  value: string;
}) {
  return (
    <div className="text-center">
      <div className="flex items-center justify-center gap-1.5 text-surface-400 text-xs mb-1">
        {icon}
        {label}
      </div>
      <div className="text-sm font-semibold text-white">{value}</div>
    </div>
  );
}
