"use client";

import * as Icons from "lucide-react";

import { NODE_CATALOG, type PaletteEntry } from "../../lib/workflow";

interface NodePaletteProps {
  onAdd: (entry: PaletteEntry) => void;
}

export default function NodePalette({ onAdd }: NodePaletteProps) {
  return (
    <div className="w-48 flex-shrink-0 border-r border-surface-800 bg-surface-950 p-3 space-y-1.5 overflow-y-auto">
      <div className="text-xs font-medium text-surface-500 px-1 mb-2">Node palette</div>
      {NODE_CATALOG.map((entry) => {
        const Icon = (Icons as Record<string, unknown>)[entry.icon] as
          | React.ComponentType<{ size?: number; className?: string }>
          | undefined;
        return (
          <button
            key={entry.kind}
            onClick={() => onAdd(entry)}
            className="w-full flex items-center gap-2.5 rounded-lg border border-surface-800 bg-surface-900 px-2.5 py-2 text-left hover:border-surface-600 transition-colors group"
          >
            <span className={`flex-shrink-0 ${entry.accent}`}>
              {Icon ? <Icon size={16} /> : null}
            </span>
            <span className="min-w-0">
              <span className="block text-sm font-medium text-surface-200 group-hover:text-white">
                {entry.label}
              </span>
              <span className="block text-[11px] text-surface-500 truncate">
                {entry.description}
              </span>
            </span>
          </button>
        );
      })}
    </div>
  );
}
