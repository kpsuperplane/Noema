import { Handle, Position, type Node, type NodeProps } from "@xyflow/react";
import type { MemoryGraphNodeData } from "@/memoryGraphLayout";

export function MemoryEntityNode({ data, selected }: NodeProps<Node<MemoryGraphNodeData>>) {
  return (
    <div
      className={[
        "h-[76px] w-[180px] rounded-md border bg-white px-3 py-2 shadow-sm",
        "grid content-center gap-1 overflow-hidden",
        selected ? "border-[var(--pine-500)] ring-2 ring-[var(--pine-500)]/15" : "border-[var(--border-subtle)]"
      ].join(" ")}
    >
      <Handle type="target" position={Position.Left} className="opacity-0" />
      <strong className="block truncate text-sm font-semibold leading-tight">{data.label}</strong>
      <span className="block truncate text-xs text-muted-foreground">
        {data.entityType} / {data.claimCount} claims
      </span>
      <Handle type="source" position={Position.Right} className="opacity-0" />
    </div>
  );
}
