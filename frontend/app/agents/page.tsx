"use client";

import { useState, useEffect } from "react";
import Link from "next/link";
import {
  Search,
  Plus,
  Bot,
  Clock,
  Sparkles,
  AlertCircle,
  Loader2,
} from "lucide-react";

interface Agent {
  id: string;
  name: string;
  description: string;
  model: string;
  status: string;
  created_at: string;
}

const FILTERS = ["All", "active", "inactive"];

export default function AgentsPage() {
  const [agents, setAgents] = useState<Agent[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [activeFilter, setActiveFilter] = useState("All");

  useEffect(() => {
    const baseUrl =
      process.env.NEXT_PUBLIC_API_URL || "http://localhost:8000";
    fetch(`${baseUrl}/api/agents`)
      .then((res) => {
        if (!res.ok) throw new Error(`Failed to fetch agents: ${res.status}`);
        return res.json();
      })
      .then((data: Agent[]) => {
        setAgents(data);
        setLoading(false);
      })
      .catch((err) => {
        setError(err.message);
        setLoading(false);
      });
  }, []);

  const filtered = agents.filter((agent) => {
    const matchesSearch =
      agent.name.toLowerCase().includes(search.toLowerCase()) ||
      agent.description.toLowerCase().includes(search.toLowerCase());
    const matchesFilter =
      activeFilter === "All" || agent.status === activeFilter;
    return matchesSearch && matchesFilter;
  });

  if (loading) {
    return (
      <div className="flex items-center justify-center min-h-[400px]">
        <Loader2 size={24} className="text-brand-400 animate-spin" />
      </div>
    );
  }

  if (error) {
    return (
      <div className="card p-12 text-center">
        <AlertCircle size={32} className="text-red-400 mx-auto mb-3" />
        <p className="text-red-400 font-medium mb-1">Failed to load agents</p>
        <p className="text-surface-500 text-sm">{error}</p>
      </div>
    );
  }

  return (
    <div className="space-y-6">
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
            {f === "All" ? "All" : f.charAt(0).toUpperCase() + f.slice(1)}
          </button>
        ))}
      </div>

      {filtered.length > 0 ? (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {filtered.map((agent) => (
            <AgentCard key={agent.id} agent={agent} />
          ))}
        </div>
      ) : (
        <EmptyState
          hasSearch={search.length > 0 || activeFilter !== "All"}
          agentsEmpty={agents.length === 0}
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
      <div className="flex items-center pt-3 border-t border-surface-800">
        <div className="flex items-center gap-1.5 text-xs text-surface-500">
          <Clock size={12} />
          {agent.created_at
            ? new Date(agent.created_at).toLocaleDateString()
            : "N/A"}
        </div>
      </div>
    </Link>
  );
}

function EmptyState({
  hasSearch,
  agentsEmpty,
}: {
  hasSearch: boolean;
  agentsEmpty: boolean;
}) {
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

  if (agentsEmpty) {
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

  return null;
}
