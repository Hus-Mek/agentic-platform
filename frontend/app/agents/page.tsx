"use client";

import { useState } from "react";
import Link from "next/link";
import {
  Search,
  Plus,
  Bot,
  Clock,
  Play,
  Settings,
  MoreHorizontal,
  Sparkles,
} from "lucide-react";

interface Agent {
  id: string;
  name: string;
  description: string;
  model: string;
  status: "active" | "inactive";
  runs: number;
  lastRun: string;
  tools: number;
}

const FILTERS = ["All", "Active", "Inactive", "GPT-4o", "Claude"];

const MOCK_AGENTS: Agent[] = [
  {
    id: "agent_001",
    name: "Customer Support Bot",
    description: "Handles customer inquiries, escalations, and FAQs",
    model: "gpt-4o",
    status: "active",
    runs: 1247,
    lastRun: "2 min ago",
    tools: 5,
  },
  {
    id: "agent_002",
    name: "Code Review Assistant",
    description: "Reviews PRs for bugs, style, and security issues",
    model: "claude-sonnet-4",
    status: "active",
    runs: 832,
    lastRun: "15 min ago",
    tools: 4,
  },
  {
    id: "agent_003",
    name: "Data Analyzer",
    description: "Analyzes CSV/JSON data and generates reports",
    model: "gpt-4o-mini",
    status: "active",
    runs: 453,
    lastRun: "1 hr ago",
    tools: 3,
  },
  {
    id: "agent_004",
    name: "Email Composer",
    description: "Drafts and replies to emails with context awareness",
    model: "gpt-4o",
    status: "inactive",
    runs: 89,
    lastRun: "3 days ago",
    tools: 2,
  },
  {
    id: "agent_005",
    name: "Research Agent",
    description: "Deep web research with source aggregation",
    model: "claude-sonnet-4",
    status: "active",
    runs: 312,
    lastRun: "2 hrs ago",
    tools: 6,
  },
  {
    id: "agent_006",
    name: "Meeting Scheduler",
    description: "Calendar management and meeting coordination",
    model: "gpt-4o-mini",
    status: "inactive",
    runs: 156,
    lastRun: "1 week ago",
    tools: 3,
  },
];

export default function AgentsPage() {
  const [search, setSearch] = useState("");
  const [activeFilter, setActiveFilter] = useState("All");

  const filtered = MOCK_AGENTS.filter((agent) => {
    const matchesSearch =
      agent.name.toLowerCase().includes(search.toLowerCase()) ||
      agent.description.toLowerCase().includes(search.toLowerCase());
    const matchesFilter =
      activeFilter === "All" ||
      (activeFilter === "Active" && agent.status === "active") ||
      (activeFilter === "Inactive" && agent.status === "inactive") ||
      (activeFilter === "GPT-4o" && agent.model === "gpt-4o") ||
      (activeFilter === "Claude" && agent.model.includes("claude"));
    return matchesSearch && matchesFilter;
  });

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-white">Agents</h1>
          <p className="text-surface-400 text-sm mt-1">
            Manage your AI agents and configurations
          </p>
        </div>
        <Link href="/agents/new" className="btn-primary">
          <Plus size={16} />
          New Agent
        </Link>
      </div>

      {/* Search + Filters */}
      <div className="flex flex-col sm:flex-row gap-3">
        <div className="relative flex-1">
          <Search
            size={16}
            className="absolute left-3 top-1/2 -translate-y-1/2 text-surface-500"
          />
          <input
            type="text"
            placeholder="Search agents..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="input pl-10"
          />
        </div>
      </div>

      <div className="flex items-center gap-2 flex-wrap">
        {FILTERS.map((f) => (
          <button
            key={f}
            onClick={() => setActiveFilter(f)}
            className={`px-3 py-1.5 rounded-full text-xs font-medium transition-colors ${
              activeFilter === f
                ? "bg-brand-500/20 text-brand-300 border border-brand-500/30"
                : "bg-surface-800 text-surface-400 border border-surface-700 hover:border-surface-600"
            }`}
          >
            {f}
          </button>
        ))}
      </div>

      {/* Agent Grid */}
      {filtered.length > 0 ? (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {filtered.map((agent) => (
            <AgentCard key={agent.id} agent={agent} />
          ))}
        </div>
      ) : (
        <EmptyState
          hasSearch={search.length > 0 || activeFilter !== "All"}
        />
      )}
    </div>
  );
}

function AgentCard({ agent }: { agent: Agent }) {
  return (
    <Link
      href={`/agents/${agent.id}`}
      className="card-hover p-5 group block"
    >
      <div className="flex items-start justify-between mb-3">
        <div className="w-10 h-10 rounded-xl bg-brand-500/10 flex items-center justify-center text-brand-400 group-hover:bg-brand-500/20 transition-colors">
          <Bot size={20} />
        </div>
        <button
          onClick={(e) => {
            e.preventDefault();
          }}
          className="btn-ghost p-1"
        >
          <MoreHorizontal size={16} />
        </button>
      </div>
      <h3 className="text-sm font-semibold text-white mb-1 group-hover:text-brand-400 transition-colors">
        {agent.name}
      </h3>
      <p className="text-xs text-surface-400 line-clamp-2 mb-3">
        {agent.description}
      </p>
      <div className="flex items-center gap-3 text-xs text-surface-500 mb-3">
        <span className="px-2 py-0.5 rounded bg-surface-800 font-mono">
          {agent.model}
        </span>
        <span>{agent.tools} tools</span>
      </div>
      <div className="flex items-center justify-between pt-3 border-t border-surface-800">
        <div className="flex items-center gap-1.5 text-xs text-surface-500">
          <Clock size={12} />
          {agent.lastRun}
        </div>
        <div className="flex items-center gap-1.5 text-xs text-surface-500">
          <Play size={12} />
          {agent.runs.toLocaleString()} runs
        </div>
      </div>
    </Link>
  );
}

function EmptyState({ hasSearch }: { hasSearch: boolean }) {
  if (hasSearch) {
    return (
      <div className="card p-12 text-center">
        <p className="text-surface-400 mb-2">No agents match your search</p>
        <p className="text-xs text-surface-500">
          Try a different search term or filter
        </p>
      </div>
    );
  }

  return (
    <div className="card p-12 text-center">
      <div className="w-16 h-16 rounded-2xl bg-surface-800 flex items-center justify-center mx-auto mb-4">
        <Sparkles size={28} className="text-surface-500" />
      </div>
      <h3 className="text-lg font-semibold text-white mb-2">No agents yet</h3>
      <p className="text-surface-400 text-sm mb-6 max-w-md mx-auto">
        Create your first AI agent to get started. Agents can be configured with
        custom instructions, tools, guardrails, and memory.
      </p>
      <Link href="/agents/new" className="btn-primary">
        <Plus size={16} />
        Create Your First Agent
      </Link>
    </div>
  );
}
