'use client'

import { useState } from 'react'
import Link from 'next/link'
import { Search, ArrowRight, Bot } from 'lucide-react'

interface Run {
  id: string
  agentName: string
  status: 'completed' | 'failed' | 'running' | 'cancelled'
  duration: string
  tokens: number
  cost: string
  timestamp: string
}

const mockRuns: Run[] = [
  { id: 'run_001', agentName: 'Customer Support Bot', status: 'completed', duration: '3.2s', tokens: 260, cost: '$0.0021', timestamp: '2 min ago' },
  { id: 'run_002', agentName: 'Code Reviewer', status: 'completed', duration: '5.8s', tokens: 412, cost: '$0.0038', timestamp: '15 min ago' },
  { id: 'run_003', agentName: 'Data Analyzer', status: 'running', duration: '12.4s', tokens: 180, cost: '$0.0015', timestamp: '1 min ago' },
  { id: 'run_004', agentName: 'Email Composer', status: 'failed', duration: '8.1s', tokens: 150, cost: '$0.0012', timestamp: '32 min ago' },
  { id: 'run_005', agentName: 'Research Assistant', status: 'completed', duration: '6.7s', tokens: 520, cost: '$0.0045', timestamp: '1 hour ago' },
  { id: 'run_006', agentName: 'Customer Support Bot', status: 'cancelled', duration: '1.2s', tokens: 45, cost: '$0.0004', timestamp: '2 hours ago' },
  { id: 'run_007', agentName: 'Social Media Manager', status: 'completed', duration: '4.1s', tokens: 310, cost: '$0.0028', timestamp: '3 hours ago' },
]

const statusStyles: Record<string, string> = {
  completed: 'bg-green-500/10 text-green-400 border border-green-500/20',
  failed: 'bg-red-500/10 text-red-400 border border-red-500/20',
  running: 'bg-blue-500/10 text-blue-400 border border-blue-500/20',
  cancelled: 'bg-surface-700 text-surface-400 border border-surface-600',
}

export default function RunsPage() {
  const [search, setSearch] = useState('')

  const filtered = mockRuns.filter((r) =>
    r.id.toLowerCase().includes(search.toLowerCase()) ||
    r.agentName.toLowerCase().includes(search.toLowerCase())
  )

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
        <div className="text-xs text-surface-500">{mockRuns.length} total runs</div>
      </div>

      {/* Search */}
      <div className="relative">
        <Search size={16} className="absolute left-3 top-1/2 -translate-y-1/2 text-surface-500" />
        <input
          type="text"
          placeholder="Search by run ID or agent name..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="input pl-10"
        />
      </div>

      {/* Table */}
      <div className="card overflow-hidden">
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-surface-800 bg-surface-950/50">
                <th className="px-6 py-3 text-left text-xs font-medium text-surface-500 uppercase tracking-wider">Run ID</th>
                <th className="px-6 py-3 text-left text-xs font-medium text-surface-500 uppercase tracking-wider">Agent</th>
                <th className="px-6 py-3 text-left text-xs font-medium text-surface-500 uppercase tracking-wider">Status</th>
                <th className="px-6 py-3 text-left text-xs font-medium text-surface-500 uppercase tracking-wider">Duration</th>
                <th className="px-6 py-3 text-left text-xs font-medium text-surface-500 uppercase tracking-wider">Tokens</th>
                <th className="px-6 py-3 text-left text-xs font-medium text-surface-500 uppercase tracking-wider">Cost</th>
                <th className="px-6 py-3 text-left text-xs font-medium text-surface-500 uppercase tracking-wider">When</th>
                <th className="px-6 py-3" />
              </tr>
            </thead>
            <tbody className="divide-y divide-surface-800">
              {filtered.map((run) => (
                <tr key={run.id} className="hover:bg-surface-800/40 transition-colors">
                  <td className="px-6 py-4 font-mono text-xs text-surface-300">{run.id}</td>
                  <td className="px-6 py-4">
                    <div className="flex items-center gap-2">
                      <Bot size={14} className="text-surface-500" />
                      <span className="font-medium text-surface-200">{run.agentName}</span>
                    </div>
                  </td>
                  <td className="px-6 py-4">
                    <span className={`inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium ${statusStyles[run.status]}`}>
                      {run.status}
                    </span>
                  </td>
                  <td className="px-6 py-4 text-surface-400">{run.duration}</td>
                  <td className="px-6 py-4 text-surface-400">{run.tokens}</td>
                  <td className="px-6 py-4 text-surface-400 font-mono">{run.cost}</td>
                  <td className="px-6 py-4 text-surface-500 text-xs">{run.timestamp}</td>
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
            {search ? 'No runs match your search.' : 'No runs recorded yet.'}
          </div>
        )}
      </div>
    </div>
  )
}
