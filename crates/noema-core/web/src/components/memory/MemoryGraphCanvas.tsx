import type { Edge, Node } from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { useMemo } from "react";

import { MemoryGraphFlow } from "./MemoryGraphFlow";
import type { MemoryGraphEdgeData, MemoryGraphNodeData } from "@/memoryGraphLayout";

export function MemoryGraphCanvas({
  nodes,
  edges,
  selectedClaimId,
  onSelectClaim,
}: {
  nodes: Node<MemoryGraphNodeData>[];
  edges: Edge<MemoryGraphEdgeData>[];
  selectedClaimId: string | null;
  onSelectClaim: (claimId: string | null) => void;
}) {
  const layoutKey = useMemo(
    () =>
      nodes
        .map((node) => `${node.id}:${Math.round(node.position.x)}:${Math.round(node.position.y)}`)
        .join("|"),
    [nodes]
  );

  return (
    <MemoryGraphFlow
      key={layoutKey}
      initialNodes={nodes}
      edges={edges}
      selectedClaimId={selectedClaimId}
      onSelectClaim={onSelectClaim}
    />
  );
}
