import {
  Background,
  Controls,
  Handle,
  MiniMap,
  Position,
  ReactFlow,
  type Edge,
  type EdgeProps,
  type EdgeTypes,
  type Node,
  type NodeProps,
  type NodeTypes,
} from "@xyflow/react"
import "@xyflow/react/dist/style.css"
import { useMemo } from "react"

import type { MemoryGraphEdgeData, MemoryGraphNodeData } from "@/memoryGraphLayout"

function MemoryEntityNode({ data, selected }: NodeProps<Node<MemoryGraphNodeData>>) {
  return (
    <div
      className={[
        "h-[76px] w-[180px] rounded-md border bg-white px-3 py-2 shadow-sm",
        "grid content-center gap-1 overflow-hidden",
        selected ? "border-[var(--pine-500)] ring-2 ring-[var(--pine-500)]/15" : "border-[var(--border-subtle)]",
      ].join(" ")}
    >
      <Handle type="target" position={Position.Left} className="opacity-0" />
      <strong className="block truncate text-sm font-semibold leading-tight">{data.label}</strong>
      <span className="block truncate text-xs text-muted-foreground">
        {data.entityType} / {data.claimCount} claims
      </span>
      <Handle type="source" position={Position.Right} className="opacity-0" />
    </div>
  )
}

function MemoryClaimEdge({
  id,
  label,
  selected,
  source,
  sourceX,
  sourceY,
  target,
  targetX,
  targetY,
}: EdgeProps<Edge<MemoryGraphEdgeData>>) {
  const selfEdge = source === target
  const edgePath = selfEdge
    ? `M ${sourceX} ${sourceY} C ${sourceX + 120} ${sourceY - 120}, ${targetX + 120} ${targetY + 120}, ${targetX} ${targetY}`
    : `M ${sourceX} ${sourceY} L ${targetX} ${targetY}`
  const stroke = selected ? "var(--pine-500)" : "rgba(23, 22, 15, 0.32)"

  return (
    <g>
      <path
        className="react-flow__edge-interaction"
        d={edgePath}
        stroke="transparent"
        strokeWidth={18}
        fill="none"
        pointerEvents="stroke"
      />
      <path
        id={id}
        className="react-flow__edge-path"
        d={edgePath}
        stroke={stroke}
        strokeWidth={selected ? 2.5 : 1.5}
        fill="none"
      />
      {label ? (
        <text className="fill-[var(--text-secondary)] text-[11px]">
          <textPath href={`#${id}`} startOffset="50%" textAnchor="middle">
            {label}
          </textPath>
        </text>
      ) : null}
    </g>
  )
}

const nodeTypes = {
  memoryEntity: MemoryEntityNode,
} satisfies NodeTypes

const edgeTypes = {
  memoryClaim: MemoryClaimEdge,
} satisfies EdgeTypes

export function MemoryGraphCanvas({
  nodes,
  edges,
  selectedClaimId,
  onSelectClaim,
}: {
  nodes: Node<MemoryGraphNodeData>[]
  edges: Edge<MemoryGraphEdgeData>[]
  selectedClaimId: string | null
  onSelectClaim: (claimId: string | null) => void
}) {
  const selectedEdges = useMemo(
    () =>
      edges.map((edge) => ({
        ...edge,
        selected: edge.id === selectedClaimId,
      })),
    [edges, selectedClaimId],
  )

  return (
    <div className="h-full min-h-[420px] overflow-hidden bg-[var(--surface-sunken)]">
      <ReactFlow
        nodes={nodes}
        edges={selectedEdges}
        nodeTypes={nodeTypes}
        edgeTypes={edgeTypes}
        fitView
        panOnDrag
        zoomOnScroll
        zoomOnPinch
        onEdgeClick={(_, edge) => onSelectClaim(edge.id)}
        onPaneClick={() => onSelectClaim(null)}
      >
        <Background />
        <Controls />
        <MiniMap pannable zoomable />
      </ReactFlow>
    </div>
  )
}
