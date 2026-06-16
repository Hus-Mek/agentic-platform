"use client";

import { useState, useEffect, useMemo } from "react";
import {
  DollarSign,
  BarChart3,
  Activity,
  TrendingUp,
  AlertCircle,
  Loader2,
  CalendarDays,
} from "lucide-react";
import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  Legend,
} from "recharts";

interface UsageRecord {
  date: string;
  total_tokens: number;
  total_cost: number;
  total_runs: number;
}

const API_URL = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8000";
const WORKSPACE_ID = "00000000-0000-0000-0000-000000000001";

function formatDate(d: Date): string {
  return d.toISOString().slice(0, 10);
}

function today(): string {
  return formatDate(new Date());
}

function thirtyDaysAgo(): string {
  const d = new Date();
  d.setDate(d.getDate() - 30);
  return formatDate(d);
}

function formatCost(cost: number): string {
  return `$${cost.toFixed(4)}`;
}

function formatNumber(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return n.toLocaleString();
}

export default function UsagePage() {
  const [usage, setUsage] = useState<UsageRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [startDate, setStartDate] = useState(thirtyDaysAgo);
  const [endDate, setEndDate] = useState(today);

  useEffect(() => {
    setLoading(true);
    setError(null);
    const baseUrl = API_URL;
    fetch(
      `${baseUrl}/api/workspaces/${WORKSPACE_ID}/usage?start_date=${startDate}&end_date=${endDate}`,
      {
        headers: { "Content-Type": "application/json" },
      }
    )
      .then((res) => {
        if (!res.ok)
          throw new Error(`Failed to fetch usage: ${res.status}`);
        return res.json();
      })
      .then((data: UsageRecord[]) => {
        setUsage(data.reverse());
        setLoading(false);
      })
      .catch((err) => {
        setError(err.message);
        setLoading(false);
      });
  }, [startDate, endDate]);

  const summary = useMemo(() => {
    if (usage.length === 0) {
      return {
        totalCost: 0,
        totalTokens: 0,
        totalRuns: 0,
        avgCostPerRun: 0,
      };
    }
    const totalCost = usage.reduce((s, r) => s + r.total_cost, 0);
    const totalTokens = usage.reduce((s, r) => s + r.total_tokens, 0);
    const totalRuns = usage.reduce((s, r) => s + r.total_runs, 0);
    return {
      totalCost,
      totalTokens,
      totalRuns,
      avgCostPerRun: totalRuns > 0 ? totalCost / totalRuns : 0,
    };
  }, [usage]);

  const chartData = useMemo(
    () =>
      usage.map((r) => ({
        date: r.date,
        Cost: parseFloat(r.total_cost.toFixed(4)),
        Tokens: r.total_tokens,
        Runs: r.total_runs,
      })),
    [usage]
  );

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
        <p className="text-red-400 font-medium mb-1">
          Failed to load usage data
        </p>
        <p className="text-surface-500 text-sm">{error}</p>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between flex-wrap gap-4">
        <div>
          <h1 className="text-2xl font-bold text-white">Usage & Cost</h1>
          <p className="text-surface-400 text-sm mt-1">
            Track LLM token usage and cost across workspaces
          </p>
        </div>
        <div className="flex items-center gap-3">
          <CalendarDays size={16} className="text-surface-500" />
          <input
            type="date"
            value={startDate}
            onChange={(e) => setStartDate(e.target.value)}
            className="input w-auto"
          />
          <span className="text-surface-500 text-sm">to</span>
          <input
            type="date"
            value={endDate}
            onChange={(e) => setEndDate(e.target.value)}
            className="input w-auto"
          />
        </div>
      </div>

      {/* Summary Cards */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        <div className="card p-5">
          <div className="flex items-center gap-3 mb-3">
            <div className="w-10 h-10 rounded-xl bg-brand-500/10 flex items-center justify-center text-brand-400">
              <DollarSign size={20} />
            </div>
          </div>
          <p className="text-2xl font-bold text-white">
            {formatCost(summary.totalCost)}
          </p>
          <p className="text-xs text-surface-500 mt-1">Total Cost (MTD)</p>
        </div>
        <div className="card p-5">
          <div className="flex items-center gap-3 mb-3">
            <div className="w-10 h-10 rounded-xl bg-blue-500/10 flex items-center justify-center text-blue-400">
              <BarChart3 size={20} />
            </div>
          </div>
          <p className="text-2xl font-bold text-white">
            {formatNumber(summary.totalTokens)}
          </p>
          <p className="text-xs text-surface-500 mt-1">Total Tokens (MTD)</p>
        </div>
        <div className="card p-5">
          <div className="flex items-center gap-3 mb-3">
            <div className="w-10 h-10 rounded-xl bg-green-500/10 flex items-center justify-center text-green-400">
              <Activity size={20} />
            </div>
          </div>
          <p className="text-2xl font-bold text-white">
            {formatNumber(summary.totalRuns)}
          </p>
          <p className="text-xs text-surface-500 mt-1">Total Runs (MTD)</p>
        </div>
        <div className="card p-5">
          <div className="flex items-center gap-3 mb-3">
            <div className="w-10 h-10 rounded-xl bg-amber-500/10 flex items-center justify-center text-amber-400">
              <TrendingUp size={20} />
            </div>
          </div>
          <p className="text-2xl font-bold text-white">
            {formatCost(summary.avgCostPerRun)}
          </p>
          <p className="text-xs text-surface-500 mt-1">Avg Cost per Run</p>
        </div>
      </div>

      {/* Chart */}
      {chartData.length > 0 && (
        <div className="card p-6">
          <h2 className="text-sm font-semibold text-white mb-4">
            Daily Cost
          </h2>
          <div className="h-72">
            <ResponsiveContainer width="100%" height="100%">
              <LineChart data={chartData}>
                <CartesianGrid
                  strokeDasharray="3 3"
                  stroke="#2d2d3d"
                />
                <XAxis
                  dataKey="date"
                  tick={{ fill: "#9ca3af", fontSize: 12 }}
                  tickLine={false}
                  axisLine={{ stroke: "#2d2d3d" }}
                />
                <YAxis
                  tick={{ fill: "#9ca3af", fontSize: 12 }}
                  tickLine={false}
                  axisLine={{ stroke: "#2d2d3d" }}
                  tickFormatter={(v) => `$${v.toFixed(2)}`}
                />
                <Tooltip
                  contentStyle={{
                    backgroundColor: "#1c1c2e",
                    border: "1px solid #2d2d3d",
                    borderRadius: "8px",
                    color: "#e4e4e7",
                  }}
                  formatter={(value: any) => [
                    `$${Number(value).toFixed(4)}`,
                    "Cost",
                  ]}
                />
                <Legend
                  wrapperStyle={{ fontSize: "12px", color: "#9ca3af" }}
                />
                <Line
                  type="monotone"
                  dataKey="Cost"
                  stroke="#818cf8"
                  strokeWidth={2}
                  dot={{ fill: "#818cf8", r: 3 }}
                  activeDot={{ r: 5 }}
                />
              </LineChart>
            </ResponsiveContainer>
          </div>
        </div>
      )}

      {/* Daily Breakdown Table */}
      <div className="card overflow-hidden">
        <div className="px-6 py-4 border-b border-surface-800">
          <h2 className="text-sm font-semibold text-white">
            Daily Breakdown
          </h2>
        </div>
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-surface-800 bg-surface-950/50">
                <th className="px-6 py-3 text-left text-xs font-medium text-surface-500 uppercase tracking-wider">
                  Date
                </th>
                <th className="px-6 py-3 text-left text-xs font-medium text-surface-500 uppercase tracking-wider">
                  Tokens
                </th>
                <th className="px-6 py-3 text-left text-xs font-medium text-surface-500 uppercase tracking-wider">
                  Cost
                </th>
                <th className="px-6 py-3 text-left text-xs font-medium text-surface-500 uppercase tracking-wider">
                  Runs
                </th>
              </tr>
            </thead>
            <tbody className="divide-y divide-surface-800">
              {usage.map((row) => (
                <tr
                  key={row.date}
                  className="hover:bg-surface-800/40 transition-colors"
                >
                  <td className="px-6 py-4 text-surface-200 font-medium">
                    {row.date}
                  </td>
                  <td className="px-6 py-4 text-surface-400 font-mono">
                    {formatNumber(row.total_tokens)}
                  </td>
                  <td className="px-6 py-4 text-surface-400 font-mono">
                    {formatCost(row.total_cost)}
                  </td>
                  <td className="px-6 py-4 text-surface-400">
                    {row.total_runs}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        {usage.length === 0 && (
          <div className="px-6 py-12 text-center text-sm text-surface-500">
            No usage data found for the selected date range.
          </div>
        )}
      </div>
    </div>
  );
}
