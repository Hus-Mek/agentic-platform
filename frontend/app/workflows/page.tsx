"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { Plus, Workflow, Loader2, AlertCircle, Sparkles } from "lucide-react";

import { api, type Pipeline } from "../../lib/api";

export default function WorkflowsPage() {
  const router = useRouter();
  const [pipelines, setPipelines] = useState<Pipeline[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);

  useEffect(() => {
    api
      .listPipelines()
      .then(setPipelines)
      .catch((e) => setError(e instanceof Error ? e.message : "Failed to load"))
      .finally(() => setLoading(false));
  }, []);

  const createWorkflow = async () => {
    setCreating(true);
    try {
      const created = await api.createPipeline({
        name: "Untitled workflow",
        graph: { nodes: [], edges: [] },
      });
      router.push(`/workflows/${created.id}`);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to create");
      setCreating(false);
    }
  };

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
        <p className="text-red-400 font-medium mb-1">Failed to load workflows</p>
        <p className="text-surface-500 text-sm">{error}</p>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-white">Workflows</h1>
          <p className="text-surface-400 text-sm mt-1">
            Visually design, run, and version multi-step pipelines
          </p>
        </div>
        <button onClick={createWorkflow} disabled={creating} className="btn-primary">
          {creating ? <Loader2 size={16} className="animate-spin" /> : <Plus size={16} />}
          New Workflow
        </button>
      </div>

      {pipelines.length > 0 ? (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {pipelines.map((p) => (
            <Link key={p.id} href={`/workflows/${p.id}`} className="card-hover p-5 group block">
              <div className="w-10 h-10 rounded-xl bg-brand-500/10 flex items-center justify-center text-brand-400 mb-3 group-hover:bg-brand-500/20 transition-colors">
                <Workflow size={20} />
              </div>
              <h3 className="text-sm font-semibold text-white mb-1 group-hover:text-brand-400 transition-colors">
                {p.name}
              </h3>
              <p className="text-xs text-surface-400 line-clamp-2 mb-3">
                {p.description || "No description"}
              </p>
              <div className="flex items-center gap-2 text-xs text-surface-500">
                <span className="px-2 py-0.5 rounded bg-surface-800 font-mono">v{p.version}</span>
                <span>{p.graph?.nodes?.length || 0} nodes</span>
              </div>
            </Link>
          ))}
        </div>
      ) : (
        <div className="card p-12 text-center">
          <Sparkles size={28} className="text-surface-500 mx-auto mb-3" />
          <h3 className="text-lg font-semibold text-white mb-2">No workflows yet</h3>
          <p className="text-surface-400 text-sm mb-6 max-w-md mx-auto">
            Build your first multi-step pipeline on the visual canvas.
          </p>
          <button onClick={createWorkflow} disabled={creating} className="btn-primary mx-auto">
            <Plus size={16} />
            Create Your First Workflow
          </button>
        </div>
      )}
    </div>
  );
}
