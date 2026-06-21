"use client";

import { useState, useEffect } from "react";
import { Save, X, ChevronDown, ChevronRight, Wrench, Shield, Database } from "lucide-react";

interface ModelOption {
  value: string;
  label: string;
}

interface AgentFormData {
  name: string;
  description: string;
  instructions: string;
  model: string;
  tools: string[];
  guard_config: {
    enabled: boolean;
    pii_redaction: boolean;
    toxicity_filter: boolean;
    max_tokens_per_response: number;
    allowed_topics: string[];
    blocked_topics: string[];
  };
  memory_config: {
    enabled: boolean;
    type: string;
    max_history: number;
    summarization: boolean;
    vector_search: boolean;
  };
}

interface AgentFormProps {
  initialData?: Partial<AgentFormData>;
  onSubmit: (data: AgentFormData) => void;
  onCancel: () => void;
}

const AVAILABLE_TOOLS = [
  { value: "web_search", label: "Web Search" },
  { value: "kb_search", label: "Knowledge Base Search" },
  { value: "calculator", label: "Calculator" },
  { value: "send_email", label: "Send Email" },
  { value: "calendar", label: "Calendar" },
  { value: "code_interpreter", label: "Code Interpreter" },
  { value: "image_analysis", label: "Image Analysis" },
  { value: "http_request", label: "HTTP Request" },
];

function modelSlugToLabel(id: string): string {
  // "meta-llama/llama-3.3-70b-instruct:free" -> "Llama 3.3 70B Instruct (free)"
  const stripped = id.replace(":free", "").split("/").pop() || id;
  return stripped
    .replace(/[-_]/g, " ")
    .replace(/\b\w/g, (c) => c.toUpperCase())
    .concat(" (free)");
}

export default function AgentForm({
  initialData,
  onSubmit,
  onCancel,
}: AgentFormProps) {
  const [models, setModels] = useState<ModelOption[]>([]);
  const [defaultModel, setDefaultModel] = useState<string>("");

  useEffect(() => {
    const apiUrl = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8000";
    fetch(`${apiUrl}/api/models`)
      .then((r) => (r.ok ? r.json() : []))
      .then((data: { id: string; provider?: string }[]) => {
        const opts: ModelOption[] = data.map((m) => ({
          value: m.id,
          label: `${modelSlugToLabel(m.id)}${m.provider ? ` · ${m.provider}` : ""}`,
        }));
        setModels(opts);
        if (opts.length > 0 && !initialData?.model) {
          setDefaultModel(opts[0].value);
        }
      })
      .catch(() => {
        // Fallback to a sensible default if API is unreachable
        setModels([{ value: "openai/gpt-oss-120b:free", label: "GPT-OSS 120B (free)" }]);
        setDefaultModel("openai/gpt-oss-120b:free");
      });
  }, []);

  const [data, setData] = useState<AgentFormData>({
    name: initialData?.name ?? "",
    description: initialData?.description ?? "",
    instructions: initialData?.instructions ?? "",
    model: initialData?.model ?? "",
    tools: initialData?.tools ?? [],
    guard_config: initialData?.guard_config ?? {
      enabled: false,
      pii_redaction: false,
      toxicity_filter: false,
      max_tokens_per_response: 2000,
      allowed_topics: [],
      blocked_topics: [],
    },
    memory_config: initialData?.memory_config ?? {
      enabled: false,
      type: "conversation",
      max_history: 50,
      summarization: false,
      vector_search: false,
    },
  });

  // Once models load and there's no explicit model from initialData, pick the first free one.
  useEffect(() => {
    if (defaultModel && !initialData?.model && !data.model) {
      setData((d) => ({ ...d, model: defaultModel }));
    }
  }, [defaultModel, initialData?.model, data.model]);

  const [showGuards, setShowGuards] = useState(
    initialData?.guard_config?.enabled ?? false
  );
  const [showMemory, setShowMemory] = useState(
    initialData?.memory_config?.enabled ?? false
  );

  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState(false);

  const [topicInput, setTopicInput] = useState("");
  const [blockedTopicInput, setBlockedTopicInput] = useState("");

  const update = <K extends keyof AgentFormData>(
    key: K,
    value: AgentFormData[K]
  ) => setData((prev) => ({ ...prev, [key]: value }));

  const toggleTool = (tool: string) => {
    setData((prev) => ({
      ...prev,
      tools: prev.tools.includes(tool)
        ? prev.tools.filter((t) => t !== tool)
        : [...prev.tools, tool],
    }));
  };

  const addTopic = () => {
    if (topicInput.trim() && !data.guard_config.allowed_topics.includes(topicInput.trim())) {
      setData((prev) => ({
        ...prev,
        guard_config: {
          ...prev.guard_config,
          allowed_topics: [...prev.guard_config.allowed_topics, topicInput.trim()],
        },
      }));
      setTopicInput("");
    }
  };

  const addBlockedTopic = () => {
    if (blockedTopicInput.trim() && !data.guard_config.blocked_topics.includes(blockedTopicInput.trim())) {
      setData((prev) => ({
        ...prev,
        guard_config: {
          ...prev.guard_config,
          blocked_topics: [...prev.guard_config.blocked_topics, blockedTopicInput.trim()],
        },
      }));
      setBlockedTopicInput("");
    }
  };

  const removeTopic = (topic: string) => {
    setData((prev) => ({
      ...prev,
      guard_config: {
        ...prev.guard_config,
        allowed_topics: prev.guard_config.allowed_topics.filter((t) => t !== topic),
      },
    }));
  };

  const removeBlockedTopic = (topic: string) => {
    setData((prev) => ({
      ...prev,
      guard_config: {
        ...prev.guard_config,
        blocked_topics: prev.guard_config.blocked_topics.filter((t) => t !== topic),
      },
    }));
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setIsSubmitting(true);

    const guard = { ...data.guard_config, enabled: showGuards };
    const mem = { ...data.memory_config, enabled: showMemory };

    const payload = {
      name: data.name,
      description: data.description,
      instructions: data.instructions,
      model: data.model,
      tools: data.tools,
      guard_config: {
        enabled: guard.enabled,
        pii_redaction: guard.pii_redaction,
        toxicity_filter: guard.toxicity_filter,
        max_tokens_per_response: guard.max_tokens_per_response,
        allowed_topics: guard.allowed_topics,
        blocked_topics: guard.blocked_topics,
      },
      memory_config: {
        enabled: mem.enabled,
        memory_type: mem.type,
        max_history: mem.max_history,
        summarization: mem.summarization,
        vector_search: mem.vector_search,
      },
    };

    const baseUrl = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8000";

    try {
      const res = await fetch(`${baseUrl}/api/agents`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
      });
      if (!res.ok) {
        const body = await res.text();
        throw new Error(body || `Request failed with status ${res.status}`);
      }
      setSuccess(true);
      setTimeout(() => onCancel(), 1000);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : "An unknown error occurred");
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-8 max-w-2xl">
      {/* Basic Info */}
      <section className="card p-6 space-y-4">
        <h3 className="text-sm font-semibold text-white">Basic Information</h3>

        <div>
          <label className="label">Name</label>
          <input
            type="text"
            value={data.name}
            onChange={(e) => update("name", e.target.value)}
            placeholder="My Agent"
            className="input"
            required
          />
        </div>

        <div>
          <label className="label">Description</label>
          <textarea
            value={data.description}
            onChange={(e) => update("description", e.target.value)}
            placeholder="What does this agent do?"
            rows={2}
            className="input resize-none"
          />
        </div>

        <div>
          <label className="label">Instructions</label>
          <textarea
            value={data.instructions}
            onChange={(e) => update("instructions", e.target.value)}
            placeholder="You are a helpful assistant..."
            rows={5}
            className="input resize-none font-mono text-xs"
          />
        </div>

        <div>
          <label className="label">Model</label>
          <select
            value={data.model}
            onChange={(e) => update("model", e.target.value)}
            className="input"
          >
            {models.map((m) => (
              <option key={m.value} value={m.value}>
                {m.label}
              </option>
            ))}
          </select>
        </div>
      </section>

      {/* Tools */}
      <section className="card p-6 space-y-4">
        <div className="flex items-center gap-2">
          <Wrench size={16} className="text-amber-400" />
          <h3 className="text-sm font-semibold text-white">Tools</h3>
        </div>
        <div className="grid grid-cols-2 gap-2">
          {AVAILABLE_TOOLS.map((tool) => (
            <label
              key={tool.value}
              className={`flex items-center gap-2 px-3 py-2 rounded-lg border text-sm cursor-pointer transition-colors ${
                data.tools.includes(tool.value)
                  ? "bg-brand-500/10 border-brand-500/30 text-brand-300"
                  : "bg-surface-800 border-surface-700 text-surface-400 hover:border-surface-600"
              }`}
            >
              <input
                type="checkbox"
                checked={data.tools.includes(tool.value)}
                onChange={() => toggleTool(tool.value)}
                className="sr-only"
              />
              <div
                className={`w-4 h-4 rounded border flex items-center justify-center ${
                  data.tools.includes(tool.value)
                    ? "bg-brand-500 border-brand-500"
                    : "border-surface-600"
                }`}
              >
                {data.tools.includes(tool.value) && (
                  <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
                    <path
                      d="M2 5L4 7L8 3"
                      stroke="white"
                      strokeWidth="2"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                    />
                  </svg>
                )}
              </div>
              {tool.label}
            </label>
          ))}
        </div>
      </section>

      {/* Guard Config */}
      <section className="card p-6">
        <button
          type="button"
          onClick={() => setShowGuards(!showGuards)}
          className="flex items-center justify-between w-full"
        >
          <div className="flex items-center gap-2">
            <Shield size={16} className="text-red-400" />
            <h3 className="text-sm font-semibold text-white">
              Guardrails {showGuards ? "(enabled)" : "(disabled)"}
            </h3>
          </div>
          {showGuards ? (
            <ChevronDown size={16} className="text-surface-400" />
          ) : (
            <ChevronRight size={16} className="text-surface-400" />
          )}
        </button>

        {showGuards && (
          <div className="mt-4 space-y-4">
            <div className="flex items-center gap-6">
              <label className="flex items-center gap-2 text-sm text-surface-300">
                <input
                  type="checkbox"
                  checked={data.guard_config.pii_redaction}
                  onChange={(e) =>
                    setData((prev) => ({
                      ...prev,
                      guard_config: {
                        ...prev.guard_config,
                        pii_redaction: e.target.checked,
                      },
                    }))
                  }
                  className="rounded border-surface-600 bg-surface-800"
                />
                PII Redaction
              </label>
              <label className="flex items-center gap-2 text-sm text-surface-300">
                <input
                  type="checkbox"
                  checked={data.guard_config.toxicity_filter}
                  onChange={(e) =>
                    setData((prev) => ({
                      ...prev,
                      guard_config: {
                        ...prev.guard_config,
                        toxicity_filter: e.target.checked,
                      },
                    }))
                  }
                  className="rounded border-surface-600 bg-surface-800"
                />
                Toxicity Filter
              </label>
            </div>

            <div>
              <label className="label">
                Max Tokens Per Response
              </label>
              <input
                type="number"
                value={data.guard_config.max_tokens_per_response}
                onChange={(e) =>
                  setData((prev) => ({
                    ...prev,
                    guard_config: {
                      ...prev.guard_config,
                      max_tokens_per_response: parseInt(e.target.value) || 2000,
                    },
                  }))
                }
                className="input w-32"
                min={1}
                max={16384}
              />
            </div>

            <div>
              <label className="label">Allowed Topics</label>
              <div className="flex flex-wrap gap-1.5 mb-2">
                {data.guard_config.allowed_topics.map((t) => (
                  <span
                    key={t}
                    className="inline-flex items-center gap-1 px-2 py-0.5 rounded bg-green-500/10 text-green-400 text-xs"
                  >
                    {t}
                    <button
                      type="button"
                      onClick={() => removeTopic(t)}
                      className="hover:text-green-300"
                    >
                      <X size={12} />
                    </button>
                  </span>
                ))}
              </div>
              <div className="flex gap-2">
                <input
                  type="text"
                  value={topicInput}
                  onChange={(e) => setTopicInput(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && (e.preventDefault(), addTopic())}
                  placeholder="Add topic..."
                  className="input flex-1"
                />
                <button type="button" onClick={addTopic} className="btn-secondary text-xs">
                  Add
                </button>
              </div>
            </div>

            <div>
              <label className="label">Blocked Topics</label>
              <div className="flex flex-wrap gap-1.5 mb-2">
                {data.guard_config.blocked_topics.map((t) => (
                  <span
                    key={t}
                    className="inline-flex items-center gap-1 px-2 py-0.5 rounded bg-red-500/10 text-red-400 text-xs"
                  >
                    {t}
                    <button
                      type="button"
                      onClick={() => removeBlockedTopic(t)}
                      className="hover:text-red-300"
                    >
                      <X size={12} />
                    </button>
                  </span>
                ))}
              </div>
              <div className="flex gap-2">
                <input
                  type="text"
                  value={blockedTopicInput}
                  onChange={(e) => setBlockedTopicInput(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && (e.preventDefault(), addBlockedTopic())}
                  placeholder="Add blocked topic..."
                  className="input flex-1"
                />
                <button type="button" onClick={addBlockedTopic} className="btn-secondary text-xs">
                  Add
                </button>
              </div>
            </div>
          </div>
        )}
      </section>

      {/* Memory Config */}
      <section className="card p-6">
        <button
          type="button"
          onClick={() => setShowMemory(!showMemory)}
          className="flex items-center justify-between w-full"
        >
          <div className="flex items-center gap-2">
            <Database size={16} className="text-blue-400" />
            <h3 className="text-sm font-semibold text-white">
              Memory {showMemory ? "(enabled)" : "(disabled)"}
            </h3>
          </div>
          {showMemory ? (
            <ChevronDown size={16} className="text-surface-400" />
          ) : (
            <ChevronRight size={16} className="text-surface-400" />
          )}
        </button>

        {showMemory && (
          <div className="mt-4 space-y-4">
            <div>
              <label className="label">Type</label>
              <select
                value={data.memory_config.type}
                onChange={(e) =>
                  setData((prev) => ({
                    ...prev,
                    memory_config: {
                      ...prev.memory_config,
                      type: e.target.value,
                    },
                  }))
                }
                className="input"
              >
                <option value="conversation">Conversation History</option>
                <option value="summary">Summarized Memory</option>
                <option value="vector">Vector Search Memory</option>
                <option value="hybrid">Hybrid</option>
              </select>
            </div>

            <div>
              <label className="label">Max History</label>
              <input
                type="number"
                value={data.memory_config.max_history}
                onChange={(e) =>
                  setData((prev) => ({
                    ...prev,
                    memory_config: {
                      ...prev.memory_config,
                      max_history: parseInt(e.target.value) || 50,
                    },
                  }))
                }
                className="input w-32"
                min={1}
              />
            </div>

            <div className="flex items-center gap-6">
              <label className="flex items-center gap-2 text-sm text-surface-300">
                <input
                  type="checkbox"
                  checked={data.memory_config.summarization}
                  onChange={(e) =>
                    setData((prev) => ({
                      ...prev,
                      memory_config: {
                        ...prev.memory_config,
                        summarization: e.target.checked,
                      },
                    }))
                  }
                  className="rounded border-surface-600 bg-surface-800"
                />
                Auto-Summarization
              </label>
              <label className="flex items-center gap-2 text-sm text-surface-300">
                <input
                  type="checkbox"
                  checked={data.memory_config.vector_search}
                  onChange={(e) =>
                    setData((prev) => ({
                      ...prev,
                      memory_config: {
                        ...prev.memory_config,
                        vector_search: e.target.checked,
                      },
                    }))
                  }
                  className="rounded border-surface-600 bg-surface-800"
                />
                Vector Search
              </label>
            </div>
          </div>
        )}
      </section>

      {error && (
        <div className="p-3 rounded-lg bg-red-500/10 border border-red-500/30 text-red-400 text-sm">
          {error}
        </div>
      )}
      {success && (
        <div className="p-3 rounded-lg bg-green-500/10 border border-green-500/30 text-green-400 text-sm">
          Agent created successfully!
        </div>
      )}

      {/* Actions */}
      <div className="flex items-center justify-end gap-3">
        <button type="button" onClick={onCancel} className="btn-secondary" disabled={isSubmitting}>
          <X size={16} />
          Cancel
        </button>
        <button type="submit" className="btn-primary" disabled={isSubmitting}>
          {isSubmitting ? (
            <span className="animate-spin inline-block w-4 h-4 border-2 border-white/30 border-t-white rounded-full" />
          ) : (
            <Save size={16} />
          )}
          {isSubmitting ? "Saving..." : "Save Agent"}
        </button>
      </div>
    </form>
  );
}
