"use client";

import { useEffect, useState } from "react";
import { useParams } from "next/navigation";
import { Loader2, AlertCircle } from "lucide-react";

import { api, type Pipeline } from "../../../lib/api";
import WorkflowEditor from "../../../components/workflow/WorkflowEditor";

export default function WorkflowEditorPage() {
  const params = useParams<{ id: string }>();
  const [pipeline, setPipeline] = useState<Pipeline | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api
      .getPipeline(params.id)
      .then(setPipeline)
      .catch((e) => setError(e instanceof Error ? e.message : "Failed to load"));
  }, [params.id]);

  if (error) {
    return (
      <div className="card p-12 text-center">
        <AlertCircle size={32} className="text-red-400 mx-auto mb-3" />
        <p className="text-red-400 font-medium">Failed to load workflow</p>
        <p className="text-surface-500 text-sm mt-1">{error}</p>
      </div>
    );
  }

  if (!pipeline) {
    return (
      <div className="flex items-center justify-center min-h-[400px]">
        <Loader2 size={24} className="text-brand-400 animate-spin" />
      </div>
    );
  }

  return <WorkflowEditor pipeline={pipeline} />;
}
