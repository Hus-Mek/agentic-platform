"use client";

import { useState, useRef, useEffect } from "react";
import { useAgentChat } from "@/hooks/useAgentChat";
import {
  Send,
  StopCircle,
  Bot,
  User,
  AlertCircle,
  Wrench,
} from "lucide-react";

interface AgentChatProps {
  agentId: string | null;
  onRunComplete?: (runId: string) => void;
}

export default function AgentChat({
  agentId,
  onRunComplete,
}: AgentChatProps) {
  const [input, setInput] = useState("");
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  const { messages, sendMessage, isStreaming, error, reset } = useAgentChat(
    agentId ?? undefined
  );

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  useEffect(() => {
    if (!isStreaming && inputRef.current) {
      inputRef.current.focus();
    }
  }, [isStreaming]);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!input.trim() || isStreaming) return;
    sendMessage(input.trim());
    setInput("");
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSubmit(e);
    }
  };

  const getRunId = () => {
    const lastMessage = messages[messages.length - 1];
    if (lastMessage?.type === "run_complete") {
      return lastMessage.runId;
    }
    return undefined;
  };

  const isEmpty = messages.length === 0;

  return (
    <div className="flex flex-col h-[600px] rounded-xl border border-surface-800 bg-surface-900 overflow-hidden">
      {/* Error banner */}
      {error && (
        <div className="flex items-center gap-2 px-4 py-2 bg-red-500/10 border-b border-red-500/20 text-red-400 text-sm">
          <AlertCircle size={14} />
          {error}
          <button
            onClick={reset}
            className="ml-auto text-xs underline hover:text-red-300"
          >
            Reset
          </button>
        </div>
      )}

      {/* Messages */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {isEmpty && !isStreaming && (
          <div className="flex flex-col items-center justify-center h-full text-center">
            <div className="w-12 h-12 rounded-2xl bg-surface-800 flex items-center justify-center mb-3">
              <Bot size={24} className="text-surface-500" />
            </div>
            <p className="text-surface-400 text-sm font-medium">
              Start a conversation with this agent
            </p>
            <p className="text-surface-600 text-xs mt-1">
              Type a message below to begin
            </p>
          </div>
        )}

        {messages.map((msg, i) => {
          if (msg.type === "run_complete") return null;

          return (
            <div
              key={i}
              className={`flex items-start gap-3 ${
                msg.role === "user" ? "flex-row-reverse" : ""
              }`}
            >
              <div
                className={`w-8 h-8 rounded-full flex items-center justify-center flex-shrink-0 ${
                  msg.role === "user"
                    ? "bg-brand-600"
                    : "bg-surface-700"
                }`}
              >
                {msg.role === "user" ? (
                  <User size={14} className="text-white" />
                ) : (
                  <Bot size={14} className="text-surface-300" />
                )}
              </div>

              <div
                className={
                  msg.role === "user"
                    ? "message-bubble-user"
                    : "message-bubble-assistant"
                }
              >
                {/* Tool call badges */}
                {msg.toolCalls && msg.toolCalls.length > 0 && (
                  <div className="flex flex-wrap gap-1.5 mb-2">
                    {msg.toolCalls.map((tc, j) => (
                      <span key={j} className="tool-call-badge">
                        <Wrench size={10} />
                        {tc}
                      </span>
                    ))}
                  </div>
                )}

                <p
                  className={`text-sm leading-relaxed whitespace-pre-wrap ${
                    msg.content === "" && msg.role === "assistant" && isStreaming
                      ? "streaming-cursor"
                      : ""
                  }`}
                >
                  {msg.content}
                </p>

                {/* Streaming cursor */}
                {i === messages.length - 1 &&
                  msg.role === "assistant" &&
                  isStreaming &&
                  msg.content !== "" && (
                    <span className="streaming-cursor inline-block" />
                  )}
              </div>
            </div>
          );
        })}
        <div ref={messagesEndRef} />
      </div>

      {/* Input */}
      <form
        onSubmit={handleSubmit}
        className="border-t border-surface-800 p-4"
      >
        <div className="flex items-end gap-2">
          <textarea
            ref={inputRef}
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Type a message..."
            rows={1}
            className="input flex-1 resize-none min-h-[40px] max-h-[120px] py-2.5"
            disabled={isStreaming}
          />
          {isStreaming ? (
            <button
              type="button"
              onClick={() => {
                /* abort logic in useAgentChat handles this via cleanup */
              }}
              className="btn-secondary h-10 w-10 p-0 flex items-center justify-center"
            >
              <StopCircle size={16} />
            </button>
          ) : (
            <button
              type="submit"
              disabled={!input.trim()}
              className="btn-primary h-10 w-10 p-0 flex items-center justify-center"
            >
              <Send size={16} />
            </button>
          )}
        </div>
      </form>
    </div>
  );
}
