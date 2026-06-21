"use client";

import { useState, useEffect } from "react";
import Link from "next/link";
import {
  Bot,
  Activity,
  TrendingUp,
  Clock,
  ArrowRight,
  Plus,
  Zap,
  Sparkles,
  ChevronRight,
  Loader2,
} from "lucide-react";

interface Stat {
  label: string;
  value: string;
  icon: React.ReactNode;
  change: string;
  positive: boolean;
}

interface RecentRun {
  id: string;
  agentName: string;
  status: "completed" | "failed" | "running";
  duration: string;
  timestamp: string;
}

const API_URL = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8000";
const WORKSPACE_ID = process.env.NEXT_PUBLIC_WORKSPACE_ID || "00000000-0000-0000-0000-000000000001";

// Demo token for local dev (set DEMO_TOKEN env var, or login first)
const DEMO_TOKEN = process.env.NEXT_PUBLIC_DEMO_TOKEN || "";

async function fetchJson(url: string, token?: string): Promise<any> {
  const headers: Record<string, string> = { "Content-Type": "application/json" };
  const authToken = token || localStorage.getItem("token") || DEMO_TOKEN;
  if (authToken) headers["Authorization"] = `Bearer ${authToken}`;
  const res = await fetch(url, { headers });
  if (!res.ok) throw new Error(`HTTP ${res.status}: ${res.statusText}`);
  return res.json();
}

function relativeTime(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime();
  const seconds = Math.floor(diff / 1000);
  if (seconds < 60) return "Just now";
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes} min ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} hr ago`;
  if (hours < 48) return "1 day ago";
  return `${Math.floor(hours / 24)} days ago`;
}

function formatDuration(ms: number | null): string {
  if (ms === null) return "--";
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

export default function DashboardPage() {
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [stats, setStats] = useState<Stat[]>([]);
  const [recentRuns, setRecentRuns] = useState<RecentRun[]>([]);

  useEffect(() => {
    async function loadDashboard() {
      try {
        setLoading(true);
        setError(null);

        const [agents, runs] = await Promise.all([
          fetchJson(`${API_URL}/api/agents?workspace_id=${WORKSPACE_ID}`),
          fetchJson(`${API_URL}/api/runs?workspace_id=${WORKSPACE_ID}`),
        ]);

        const agentMap: Record<string, string> = {};
        for (const a of agents) {
          agentMap[a.id] = a.name;
        }

        const totalAgents = agents.length;
        const totalRuns = runs.length;
        const completed = runs.filter((r: any) => r.status === "completed").length;
        const failed = runs.filter((r: any) => r.status === "failed" || r.status === "cancelled").length;
        const successRate = totalRuns > 0 ? (completed / totalRuns) * 100 : 0;
        const durations = runs
          .map((r: any) => r.duration_ms)
          .filter((d: number | null): d is number => d !== null);
        const avgDuration =
          durations.length > 0
            ? durations.reduce((a: number, b: number) => a + b, 0) / durations.length
            : 0;

        setStats([
          {
            label: "Active Agents",
            value: String(totalAgents),
            icon: <Bot size={20} />,
            change: `${totalAgents} deployed`,
            positive: totalAgents > 0,
          },
          {
            label: "Total Runs",
            value: String(totalRuns),
            icon: <Activity size={20} />,
            change: `${completed} completed`,
            positive: totalRuns > 0,
          },
          {
            label: "Success Rate",
            value: totalRuns > 0 ? `${successRate.toFixed(1)}%` : "--",
            icon: <TrendingUp size={20} />,
            change: `${completed} ok, ${failed} fail`,
            positive: successRate >= 50,
          },
          {
            label: "Avg Response Time",
            value: durations.length > 0 ? formatDuration(Math.round(avgDuration)) : "--",
            icon: <Clock size={20} />,
            change: `${durations.length} measured`,
            positive: true,
          },
        ]);

        const recent = runs
          .slice()
          .sort(
            (a: any, b: any) =>
              new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
          )
          .slice(0, 5)
          .map((r: any) => ({
            id: r.id,
            agentName: agentMap[r.agent_id] || r.agent_id.slice(0, 8),
            status: (
              r.status === "completed"
                ? "completed"
                : r.status === "failed" || r.status === "cancelled"
                  ? "failed"
                  : "running"
            ) as RecentRun["status"],
            duration: formatDuration(r.duration_ms),
            timestamp: relativeTime(r.created_at),
          }));

        setRecentRuns(recent);
      } catch (err: any) {
        setError(err.message || "Failed to load dashboard data");
      } finally {
        setLoading(false);
      }
    }
    loadDashboard();
  }, []);

  if (loading) {
    return (
      <div className="flex items-center justify-center min-h-[60vh]">
        <div className="flex flex-col items-center gap-3 text-surface-400">
          <Loader2 size={32} className="animate-spin text-brand-400" />
          <p className="text-sm">Loading dashboard...</p>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex items-center justify-center min-h-[60vh]">
        <div className="card p-8 max-w-md text-center">
          <div className="text-red-400 text-lg font-semibold mb-2">
            Failed to load
          </div>
          <p className="text-surface-400 text-sm mb-4">{error}</p>
          <button
            onClick={() => window.location.reload()}
            className="btn-primary"
          >
            Retry
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-8">
      {/* Hero */}
      <div className="relative overflow-hidden rounded-2xl bg-gradient-to-br from-brand-900/40 via-surface-900 to-surface-900 border border-brand-800/30 p-8 md:p-12">
        <div className="absolute top-0 right-0 w-96 h-96 bg-brand-600/10 rounded-full blur-3xl -translate-y-1/2 translate-x-1/2" />
        <div className="relative">
          <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-brand-500/10 text-brand-300 text-xs font-medium border border-brand-500/20 mb-4">
            <Sparkles size={12} />
            Welcome to AgenticOS
          </div>
          <h1 className="text-3xl md:text-4xl font-bold text-white mb-3">
            Build, deploy, and monitor
            <span className="text-brand-400"> AI agents</span>
          </h1>
          <p className="text-surface-400 max-w-xl mb-6">
            Create intelligent agents with powerful tools, guardrails, and
            memory. Monitor every run with full observability.
          </p>
          <div className="flex items-center gap-3">
            <Link href="/agents" className="btn-primary">
              <Plus size={16} />
              New Agent
            </Link>
            <Link href="/agents" className="btn-secondary">
              <Zap size={16} />
              View Agents
            </Link>
          </div>
        </div>
      </div>

      {/* Stats */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        {stats.map((stat) => (
          <div key={stat.label} className="card p-5">
            <div className="flex items-center justify-between mb-3">
              <span className="text-surface-400 text-sm font-medium">
                {stat.label}
              </span>
              <span className="text-brand-400/80">{stat.icon}</span>
            </div>
            <div className="text-2xl font-bold text-white mb-1">
              {stat.value}
            </div>
            <div
              className={`text-xs font-medium ${
                stat.positive ? "text-green-400" : "text-red-400"
              }`}
            >
              {stat.change}
            </div>
          </div>
        ))}
      </div>

      {/* Quick Actions */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <QuickAction
          href="/agents"
          icon={<Bot size={20} />}
          title="Create Agent"
          description="Define a new AI agent with tools and memory"
          color="brand"
        />
        <QuickAction
          href="/agents"
          icon={<Zap size={20} />}
          title="Quick Run"
          description="Test an agent with a prompt right away"
          color="amber"
        />
        <QuickAction
          href="/runs"
          icon={<Activity size={20} />}
          title="View Runs"
          description="Inspect recent agent executions and traces"
          color="emerald"
        />
      </div>

      {/* Recent Runs */}
      <div>
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-semibold text-white">Recent Runs</h2>
          <Link
            href="/agents"
            className="text-sm text-brand-400 hover:text-brand-300 flex items-center gap-1 transition-colors"
          >
            View all <ChevronRight size={14} />
          </Link>
        </div>
        <div className="card divide-y divide-surface-800 overflow-hidden">
          {recentRuns.map((run) => (
            <Link
              key={run.id}
              href={`/runs/${run.id}`}
              className="flex items-center justify-between px-5 py-4 hover:bg-surface-800/50 transition-colors"
            >
              <div className="flex items-center gap-3">
                <StatusBadge status={run.status} />
                <div>
                  <div className="text-sm font-medium text-surface-200">
                    {run.agentName}
                  </div>
                  <div className="text-xs text-surface-500 font-mono">
                    {run.id}
                  </div>
                </div>
              </div>
              <div className="flex items-center gap-4 text-xs text-surface-400">
                <span>{run.duration}</span>
                <span>{run.timestamp}</span>
                <ArrowRight size={14} className="text-surface-500" />
              </div>
            </Link>
          ))}
        </div>
      </div>
    </div>
  );
}

function StatusBadge({ status }: { status: RecentRun["status"] }) {
  const config = {
    completed: {
      bg: "bg-green-500/10",
      text: "text-green-400",
      dot: "bg-green-400",
    },
    failed: { bg: "bg-red-500/10", text: "text-red-400", dot: "bg-red-400" },
    running: {
      bg: "bg-blue-500/10",
      text: "text-blue-400",
      dot: "bg-blue-400 animate-pulse-slow",
    },
  };
  const c = config[status];
  return (
    <span
      className={`inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-medium ${c.bg} ${c.text}`}
    >
      <span className={`w-1.5 h-1.5 rounded-full ${c.dot}`} />
      {status}
    </span>
  );
}

function QuickAction({
  href,
  icon,
  title,
  description,
  color,
}: {
  href: string;
  icon: React.ReactNode;
  title: string;
  description: string;
  color: "brand" | "amber" | "emerald";
}) {
  const colorMap = {
    brand: "from-brand-600/20 to-brand-600/5 border-brand-500/20",
    amber: "from-amber-600/20 to-amber-600/5 border-amber-500/20",
    emerald: "from-emerald-600/20 to-emerald-600/5 border-emerald-500/20",
  };
  const iconColorMap = {
    brand: "text-brand-400",
    amber: "text-amber-400",
    emerald: "text-emerald-400",
  };
  return (
    <Link
      href={href}
      className={`card p-5 bg-gradient-to-br ${colorMap[color]} hover:brightness-110 transition-all group`}
    >
      <div className={`mb-3 ${iconColorMap[color]}`}>{icon}</div>
      <h3 className="text-sm font-semibold text-white mb-1">{title}</h3>
      <p className="text-xs text-surface-400">{description}</p>
    </Link>
  );
}
