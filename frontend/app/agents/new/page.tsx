"use client";

import { useRouter } from "next/navigation";
import Link from "next/link";
import { ArrowLeft, Bot } from "lucide-react";
import AgentForm from "@/components/AgentForm";

export default function NewAgentPage() {
  const router = useRouter();

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
        <span className="text-surface-200">New Agent</span>
      </div>

      {/* Header */}
      <div className="card p-6">
        <div className="flex items-start gap-4">
          <div className="w-14 h-14 rounded-2xl bg-brand-500/10 flex items-center justify-center text-brand-400">
            <Bot size={28} />
          </div>
          <div>
            <h1 className="text-xl font-bold text-white">New Agent</h1>
            <p className="text-sm text-surface-400 mt-1">
              Configure your AI agent with custom instructions, tools, guardrails, and memory.
            </p>
          </div>
        </div>
      </div>

      {/* Form */}
      <AgentForm
        onSubmit={(data) => console.log("create agent", data)}
        onCancel={() => router.push("/agents")}
      />
    </div>
  );
}
