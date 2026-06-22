"use client";

import { useCallback, useRef, useState } from "react";

import { API_URL, getAuthToken } from "../lib/api";

export type NodeRunStatus = "running" | "completed" | "failed";

export interface LiveNodeState {
  nodeId: string;
  nodeName?: string;
  status: NodeRunStatus;
  tokens?: number;
  cost?: number;
  error?: string;
}

// Server-sent PipelineStreamEvent (serde tag = "type", snake_case).
interface StreamEvent {
  type: "node_started" | "node_completed" | "node_failed" | "done" | "error";
  node_id?: string;
  node_name?: string;
  output?: unknown;
  tokens?: number;
  cost?: number;
  error?: string;
  message?: string;
}

interface PipelineRunState {
  isRunning: boolean;
  nodes: Record<string, LiveNodeState>;
  order: string[];
  error: string | null;
  done: boolean;
}

const INITIAL: PipelineRunState = {
  isRunning: false,
  nodes: {},
  order: [],
  error: null,
  done: false,
};

/**
 * Streams a pipeline run over SSE, surfacing per-node live status. Mirrors the
 * fetch-reader + `data:` parsing approach in hooks/useAgentChat.ts.
 */
export function usePipelineRun(pipelineId: string) {
  const [state, setState] = useState<PipelineRunState>(INITIAL);
  const abortRef = useRef<AbortController | null>(null);

  const run = useCallback(
    async (data: Record<string, unknown>) => {
      if (state.isRunning) return;
      setState({ ...INITIAL, isRunning: true });

      const controller = new AbortController();
      abortRef.current = controller;

      try {
        const headers: Record<string, string> = {
          "Content-Type": "application/json",
          Accept: "text/event-stream",
        };
        const token = getAuthToken();
        if (token) headers["Authorization"] = `Bearer ${token}`;

        const res = await fetch(`${API_URL}/api/pipelines/${pipelineId}/run-stream`, {
          method: "POST",
          headers,
          body: JSON.stringify({ data }),
          signal: controller.signal,
        });

        if (!res.ok || !res.body) {
          const body = await res.text().catch(() => "");
          setState((s) => ({ ...s, isRunning: false, error: body || `HTTP ${res.status}` }));
          return;
        }

        const reader = res.body.getReader();
        const decoder = new TextDecoder();
        let buffer = "";

        while (true) {
          const { done, value } = await reader.read();
          if (done) break;
          buffer += decoder.decode(value, { stream: true });
          const lines = buffer.split("\n");
          buffer = lines.pop() || "";

          for (const line of lines) {
            const trimmed = line.trim();
            if (!trimmed.startsWith("data: ")) continue;
            const json = trimmed.slice(6);
            if (!json) continue;
            try {
              applyEvent(JSON.parse(json) as StreamEvent, setState);
            } catch {
              // ignore malformed frames
            }
          }
        }
        setState((s) => ({ ...s, isRunning: false, done: true }));
      } catch (err: unknown) {
        if (err instanceof DOMException && err.name === "AbortError") {
          setState((s) => ({ ...s, isRunning: false }));
          return;
        }
        setState((s) => ({
          ...s,
          isRunning: false,
          error: err instanceof Error ? err.message : "Run failed",
        }));
      }
    },
    [pipelineId, state.isRunning],
  );

  const reset = useCallback(() => {
    abortRef.current?.abort();
    setState(INITIAL);
  }, []);

  return { ...state, run, reset };
}

function applyEvent(
  evt: StreamEvent,
  setState: React.Dispatch<React.SetStateAction<PipelineRunState>>,
) {
  setState((s) => {
    const nodes = { ...s.nodes };
    const order = [...s.order];
    switch (evt.type) {
      case "node_started":
        if (evt.node_id) {
          if (!order.includes(evt.node_id)) order.push(evt.node_id);
          nodes[evt.node_id] = {
            nodeId: evt.node_id,
            nodeName: evt.node_name,
            status: "running",
          };
        }
        return { ...s, nodes, order };
      case "node_completed":
        if (evt.node_id && nodes[evt.node_id]) {
          nodes[evt.node_id] = {
            ...nodes[evt.node_id],
            status: "completed",
            tokens: evt.tokens,
            cost: evt.cost,
          };
        }
        return { ...s, nodes };
      case "node_failed":
        if (evt.node_id) {
          nodes[evt.node_id] = {
            ...(nodes[evt.node_id] || { nodeId: evt.node_id }),
            status: "failed",
            error: evt.error,
          };
        }
        return { ...s, nodes, error: evt.error || s.error };
      case "error":
        return { ...s, error: evt.message || "Run failed" };
      case "done":
        return { ...s, done: true };
      default:
        return s;
    }
  });
}
