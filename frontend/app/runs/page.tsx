"use client";

import { useState, useEffect } from "react";
import Link from "next/link";
import {
  Search,
  ArrowRight,
  Bot,
  AlertCircle,
  Loader2,
  Filter,
} from "lucide-react";

interface Run {
  id: string;
  agent_name: string;
  agent_id: string;
  status: string;
  duration: number;
  total_tokens: number;
  created_at: string;
}

const STATUS_OPTIONS = ["all", "running", "completed", "failed", "cancelled"];

const STATUS_STYLES: Record<string, string> = {
  completed: "bg-green-500/10 text-green-400 border border-green-500/20",
  failed: "bg-red-500/10 text-red-400 border border-red-500/20",
  running: "bg-blue-500/10 text-blue-400 border border-blue-500/20",
  cancelled: "bg-surface-700 text-surface-400 border border-surface-600",
};

export default function RunsPage() {
  const [runs, setRuns] = useState<Run[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [statusFilter, setStatusFilter] = useState("all");

  useEffect(() => {
    const baseUrl =
      process.env.NEXT_PUBLIC_API_URL || "http://localhost:8000";
    fetch(`${baseUrl}/api/runs`)
      .then((res) => {
        if (!res.ok) throw new Error(`Failed to fetch runs: ${res.status}`);
        return res.json();
      })
      .then((data: Run[]) => {
        setRuns(data);
        setLoading(false);
      })
      .catch((err) => {
        setError(err.message);
        setLoading(false);
      });
  }, []);

  const filtered = runs.filter((run) => {
    const matchesSearch =
      run.id.toLowerCase().includes(search.toLowerCase()) ||
      (run.agent_name || "")
        .toLowerCase()
        .includes(search.toLowerCase());
    const matchesStatus =
      statusFilter === "all" || run.status === statusFilter;
    return matchesSearch && matchesStatus;
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
        <p className="text-red-400 font-medium mb-1">Failed to load runs</p>
        <p className="text-surface-500 text-sm">{error}</p>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-white">Runs</h1>
          <p className="text-surface-400 text-sm mt-1">
            Monitor and review agent execution runs
          </p>
        </div>
        <div className="text-xs text-surface-500">{runs.length} total runs</div>
      </div>

      {/* Search + Filter */}
      <div className="flex flex-col sm:flex-row gap-3">
        <div className="relative flex-1">
          <Search
            size={16}
            className="absolute left-3 top-1/2 -translate-y-1/2 text-surface-500"
          />
          <input
            type="text"
            placeholder="Search by run ID or agent name..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="input pl-10"
          />
        </div>
        <div className="relative">
          <Filter
            size={16}
            className="absolute left-3 top-1/2 -translate-y-1/2 text-surface-500 pointer-events-none"
          />
          <select
            value={statusFilter}
            onChange={(e) => setStatusFilter(e.target.value)}
            className="input pl-10 pr-8 appearance-none cursor-pointer min-w-[140px]"
          >
            {STATUS_OPTIONS.map((s) => (
              <option key={s} value={s}>
                {s === "all" ? "All Statuses" : s.charAt(0).toUpperCase() + s.slice(1)}
              </option>
            ))}
          </select>
        </div>
      </div>

      {/* Table */}
      <div className="card overflow-hidden">
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-surface-800 bg-surface-950/50">
                <th className="px-6 py-3 text-left text-xs font-medium text-surface-500 uppercase tracking-wider">
                  Run ID
                </th>
                <th className="px-6 py-3 text-left text-xs font-medium text-surface-500 uppercase tracking-wider">
                  Agent
                </th>
                <th className="px-6 py-3 text-left text-xs font-medium text-surface-500 uppercase tracking-wider">
                  Status
                </th>
                <th className="px-6 py-3 text-left text-xs font-medium text-surface-500 uppercase tracking-wider">
                  Duration
                </th>
                <th className="px-6 py-3 text-left text-xs font-medium text-surface-500 uppercase tracking-wider">
                  Tokens
                </th>
                <th className="px-6 py-3 text-left text-xs font-medium text-surface-500 uppercase tracking-wider">
                  Created At
                </th>
                <th className="px-6 py-3" />
              </tr>
            </thead>
            <tbody className="divide-y divide-surface-800">
              {filtered.map((run) => (
                <tr
                  key={run.id}
                  className="hover:bg-surface-800/40 transition-colors"
                >
                  <td className="px-6 py-4 font-mono text-xs text-surface-300">
                    {run.id}
                  </td>
                  <td className="px-6 py-4">
                    <div className="flex items-center gap-2">
                      <Bot size={14} className="text-surface-500" />
                      <Link
                        href={`/agents/${run.agent_id}`}
                        className="font-medium text-surface-200 hover:text-brand-400 transition-colors"
                      >
                        {run.agent_name || run.agent_id}
                      </Link>
                    </div>
                  </td>
                  <td className="px-6 py-4">
                    <span
                      className={`inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium ${
                        STATUS_STYLES[run.status] || ""
                      }`}
                    >
                      {run.status}
                    </span>
                  </td>
                  <td className="px-6 py-4 text-surface-400">
                    {run.duration != null
                      ? `${(run.duration / 1000).toFixed(1)}s`
                      : "—"}
                  </td>
                  <td className="px-6 py-4 text-surface-400">
                    {run.total_tokens ?? "—"}
                  </td>
                  <td className="px-6 py-4 text-surface-500 text-xs">
                    {run.created_at
                      ? new Date(run.created_at).toLocaleString()
                      : "—"}
                  </td>
                  <td className="px-6 py-4 text-right">
                    <Link
                      href={`/runs/${run.id}`}
                      className="inline-flex items-center gap-1 text-brand-400 hover:text-brand-300 text-xs font-medium transition-colors"
                    >
                      View <ArrowRight size={12} />
                    </Link>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        {filtered.length === 0 && (
          <div className="px-6 py-12 text-center text-sm text-surface-500">
            {search || statusFilter !== "all"
              ? "No runs match your filters."
              : "No runs recorded yet."}
          </div>
        )}
      </div>
    </div>
  );
}
