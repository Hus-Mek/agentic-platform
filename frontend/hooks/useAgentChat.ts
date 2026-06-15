"use client";

import { useReducer, useCallback, useRef, useEffect } from "react";

// Types
interface ToolCall {
  name: string;
  arguments: string;
}

interface Message {
  id?: string;
  role: "user" | "assistant";
  content: string;
  toolCalls?: string[];
  type?: string;
}

interface StreamToken {
  type: "token" | "tool_call" | "tool_result" | "run_start" | "run_complete" | "error";
  content?: string;
  toolName?: string;
  toolCallId?: string;
  runId?: string;
  error?: string;
}

interface ChatState {
  messages: (Message & { runId?: string })[];
  isStreaming: boolean;
  error: string | null;
  runId: string | null;
  currentToolCalls: string[];
}

type ChatAction =
  | { type: "ADD_USER_MESSAGE"; content: string }
  | { type: "STREAM_TOKEN"; token: string }
  | { type: "ADD_TOOL_CALL"; name: string }
  | { type: "SET_RUN_ID"; runId: string }
  | { type: "STREAM_END" }
  | { type: "SET_ERROR"; error: string }
  | { type: "RESET" };

function chatReducer(state: ChatState, action: ChatAction): ChatState {
  switch (action.type) {
    case "ADD_USER_MESSAGE":
      return {
        ...state,
        messages: [
          ...state.messages,
          { role: "user", content: action.content },
        ],
        error: null,
      };

    case "STREAM_TOKEN": {
      const msgs = [...state.messages];
      const last = msgs[msgs.length - 1];
      if (last && last.role === "assistant") {
        msgs[msgs.length - 1] = {
          ...last,
          content: last.content + action.token,
          toolCalls:
            state.currentToolCalls.length > 0
              ? [...state.currentToolCalls]
              : last.toolCalls,
        };
      } else {
        msgs.push({
          role: "assistant",
          content: action.token,
          toolCalls:
            state.currentToolCalls.length > 0
              ? [...state.currentToolCalls]
              : undefined,
        });
      }
      return { ...state, messages: msgs };
    }

    case "ADD_TOOL_CALL": {
      const toolCalls = [...state.currentToolCalls, action.name];
      const msgs = [...state.messages];
      const last = msgs[msgs.length - 1];
      if (last && last.role === "assistant" && last.content === "") {
        msgs[msgs.length - 1] = {
          ...last,
          toolCalls: toolCalls.length > 0 ? toolCalls : last.toolCalls,
        };
      }
      return { ...state, currentToolCalls: toolCalls, messages: msgs };
    }

    case "SET_RUN_ID":
      return { ...state, runId: action.runId };

    case "STREAM_END":
      return { ...state, isStreaming: false, currentToolCalls: [] };

    case "SET_ERROR":
      return {
        ...state,
        error: action.error,
        isStreaming: false,
        currentToolCalls: [],
      };

    case "RESET":
      return {
        messages: [],
        isStreaming: false,
        error: null,
        runId: null,
        currentToolCalls: [],
      };

    default:
      return state;
  }
}

interface UseAgentChatReturn {
  messages: ChatState["messages"];
  sendMessage: (content: string) => void;
  isStreaming: boolean;
  error: string | null;
  reset: () => void;
  runId: string | null;
}

export function useAgentChat(agentId?: string): UseAgentChatReturn {
  const [state, dispatch] = useReducer(chatReducer, {
    messages: [],
    isStreaming: false,
    error: null,
    runId: null,
    currentToolCalls: [],
  });

  const abortRef = useRef<AbortController | null>(null);
  const bufferRef = useRef<string>("");
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const flushBuffer = useCallback(() => {
    if (bufferRef.current) {
      dispatch({ type: "STREAM_TOKEN", token: bufferRef.current });
      bufferRef.current = "";
    }
  }, []);

  useEffect(() => {
    return () => {
      if (abortRef.current) {
        abortRef.current.abort();
      }
      if (timerRef.current) {
        clearTimeout(timerRef.current);
      }
    };
  }, []);

  const sendMessage = useCallback(
    async (content: string) => {
      if (!content.trim() || state.isStreaming) return;

      dispatch({ type: "ADD_USER_MESSAGE", content });

      const controller = new AbortController();
      abortRef.current = controller;

      dispatch({ type: "SET_RUN_ID", runId: "" });

      try {
        const baseUrl = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8000";
        const endpoint = agentId
          ? `${baseUrl}/api/agents/${agentId}/chat`
          : `${baseUrl}/api/chat`;

        const response = await fetch(endpoint, {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            Accept: "text/event-stream",
          },
          body: JSON.stringify({
            message: content,
            stream: true,
          }),
          signal: controller.signal,
        });

        if (!response.ok) {
          const errBody = await response.text().catch(() => "");
          dispatch({
            type: "SET_ERROR",
            error: `HTTP ${response.status}: ${errBody || response.statusText}`,
          });
          return;
        }

        const reader = response.body?.getReader();
        if (!reader) {
          dispatch({ type: "SET_ERROR", error: "Response body is not readable" });
          return;
        }

        const decoder = new TextDecoder();
        let buffer = "";

        dispatch({ type: "SET_RUN_ID", runId: "" });

        while (true) {
          const { done, value } = await reader.read();
          if (done) break;

          buffer += decoder.decode(value, { stream: true });
          const lines = buffer.split("\n");
          buffer = lines.pop() || "";

          for (const line of lines) {
            const trimmed = line.trim();
            if (!trimmed || !trimmed.startsWith("data: ")) continue;

            const jsonStr = trimmed.slice(6);
            if (!jsonStr) continue;

            try {
              const data: StreamToken = JSON.parse(jsonStr);

              switch (data.type) {
                case "run_start":
                  if (data.runId) {
                    dispatch({ type: "SET_RUN_ID", runId: data.runId });
                    if (data.content) {
                      bufferRef.current += data.content;
                      if (timerRef.current) clearTimeout(timerRef.current);
                      timerRef.current = setTimeout(flushBuffer, 50);
                    }
                  }
                  break;

                case "token":
                  bufferRef.current += data.content || "";
                  if (timerRef.current) clearTimeout(timerRef.current);
                  timerRef.current = setTimeout(flushBuffer, 50);
                  break;

                case "tool_call":
                  if (data.toolName) {
                    dispatch({ type: "ADD_TOOL_CALL", name: data.toolName });
                  }
                  if (data.content) {
                    bufferRef.current += data.content;
                    if (timerRef.current) clearTimeout(timerRef.current);
                    timerRef.current = setTimeout(flushBuffer, 50);
                  }
                  break;

                case "tool_result":
                  if (data.content) {
                    bufferRef.current += data.content;
                    if (timerRef.current) clearTimeout(timerRef.current);
                    timerRef.current = setTimeout(flushBuffer, 50);
                  }
                  break;

                case "run_complete":
                  flushBuffer();
                  if (data.runId) {
                    dispatch({ type: "SET_RUN_ID", runId: data.runId });
                  }
                  break;

                case "error":
                  dispatch({
                    type: "SET_ERROR",
                    error: data.error || "Unknown error from server",
                  });
                  break;
              }
            } catch {
              // If JSON parse fails, treat raw line as token content
              bufferRef.current += jsonStr + "\n";
              if (timerRef.current) clearTimeout(timerRef.current);
              timerRef.current = setTimeout(flushBuffer, 50);
            }
          }
        }

        // Flush any remaining buffer
        flushBuffer();
        dispatch({ type: "STREAM_END" });
      } catch (err: unknown) {
        if (err instanceof DOMException && err.name === "AbortError") {
          flushBuffer();
          dispatch({ type: "STREAM_END" });
          return;
        }
        dispatch({
          type: "SET_ERROR",
          error:
            err instanceof Error ? err.message : "An unexpected error occurred",
        });
      }
    },
    [agentId, state.isStreaming, flushBuffer]
  );

  const reset = useCallback(() => {
    if (abortRef.current) {
      abortRef.current.abort();
    }
    if (timerRef.current) {
      clearTimeout(timerRef.current);
    }
    dispatch({ type: "RESET" });
  }, []);

  return {
    messages: state.messages,
    sendMessage,
    isStreaming: state.isStreaming,
    error: state.error,
    reset,
    runId: state.runId,
  };
}
