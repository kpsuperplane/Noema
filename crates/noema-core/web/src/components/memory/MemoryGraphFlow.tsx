import {
  Background,
  Controls,
  MiniMap,
  ReactFlow,
  type Edge,
  type Node,
  type NodeChange,
  type EdgeTypes,
  type NodeTypes
} from "@xyflow/react";
import { useCallback, useMemo, useState } from "react";

import { applyMemoryGraphNodeChanges, type MemoryGraphEdgeData, type MemoryGraphNodeData } from "@/memoryGraphLayout";
import { MemoryClaimEdge } from "./MemoryClaimEdge";
import { MemoryEntityNode } from "./MemoryEntityNode";

const nodeTypes = {
  memoryEntity: MemoryEntityNode
} satisfies NodeTypes;

const edgeTypes = {
  memoryClaim: MemoryClaimEdge
} satisfies EdgeTypes;

export function MemoryGraphFlow({
  initialNodes,
  edges,
  selectedClaimId,
  onSelectClaim
}: {
  initialNodes: Node<MemoryGraphNodeData>[];
  edges: Edge<MemoryGraphEdgeData>[];
  selectedClaimId: string | null;
  onSelectClaim: (claimId: string | null) => void;
}) {
  const [flowNodes, setFlowNodes] = useState(initialNodes);

  const handleNodesChange = useCallback((changes: NodeChange<Node<MemoryGraphNodeData>>[]) => {
    setFlowNodes((currentNodes) => applyMemoryGraphNodeChanges(currentNodes, changes));
  }, []);

  const selectedEdges = useMemo(
    () =>
      edges.map((edge) => ({
        ...edge,
        selected: edge.id === selectedClaimId
      })),
    [edges, selectedClaimId]
  );

  return (
    <div className="h-full min-h-[420px] overflow-hidden bg-[var(--surface-sunken)]">
      <ReactFlow
        nodes={flowNodes}
        edges={selectedEdges}
        nodeTypes={nodeTypes}
        edgeTypes={edgeTypes}
        fitView
        panOnDrag
        nodesDraggable
        zoomOnScroll
        zoomOnPinch
        onNodesChange={handleNodesChange}
        onEdgeClick={(_, edge) => onSelectClaim(edge.id)}
        onPaneClick={() => onSelectClaim(null)}
      >
        <Background />
        <Controls />
        <MiniMap pannable zoomable />
      </ReactFlow>
    </div>
  );
}
