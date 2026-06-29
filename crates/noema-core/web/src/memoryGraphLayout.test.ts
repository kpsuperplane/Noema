import assert from "node:assert/strict";
import { describe, test } from "node:test";
import { applyMemoryGraphNodeChanges, layoutMemoryGraph } from "./memoryGraphLayout";
import type { NormalizedMemoryGraph } from "./memoryGraph";

const graph: NormalizedMemoryGraph = {
  nodes: [
    memoryNode("entity:b", "B", "concept"),
    memoryNode("entity:a", "A", "human"),
    memoryNode("entity:c", "C", "concept"),
    memoryNode("entity:d", "D", "concept"),
  ],
  edges: [
    memoryEdge("claim:1", "entity:a", "entity:b"),
    memoryEdge("claim:2", "entity:a", "entity:c"),
    memoryEdge("claim:3", "entity:a", "entity:d"),
  ],
  summary: {
    __typename: "GraphqlMemoryGraphSummary",
    returnedClaimCount: 1,
    returnedNodeCount: 2,
    limit: 150,
    truncated: false,
  },
};

describe("layoutMemoryGraph", () => {
  test("returns deterministic draggable nodes with spacious positions and React Flow edges", () => {
    const first = layoutMemoryGraph(graph, { width: 800, height: 500 });
    const second = layoutMemoryGraph(graph, { width: 800, height: 500 });

    assert.deepEqual(first, second);
    assert.equal(first.nodes.length, 4);
    assert.equal(first.edges.length, 3);
    assert(first.nodes.every((node) => node.draggable === true));
    assert(minimumNodeDistance(first.nodes.map((node) => node.position)) >= 220);
    assert.equal(first.edges[0].id, "claim:1");
    assert.equal(first.edges[0].source, "entity:a");
    assert.equal(first.edges[0].target, "entity:b");
  });

  test("applies controlled node drag position changes", () => {
    const layout = layoutMemoryGraph(graph, { width: 800, height: 500 });
    const original = layout.nodes.find((node) => node.id === "entity:a");

    const moved = applyMemoryGraphNodeChanges(layout.nodes, [
      {
        id: "entity:a",
        type: "position",
        position: { x: 123, y: 456 },
        dragging: false,
      },
    ]);
    const movedNode = moved.find((node) => node.id === "entity:a");

    assert.deepEqual(movedNode?.position, { x: 123, y: 456 });
    assert.notDeepEqual(movedNode?.position, original?.position);
  });
});

function memoryNode(nodeId: string, label: string, entityType: string): NormalizedMemoryGraph["nodes"][number] {
  return {
    __typename: "GraphqlMemoryGraphNode",
    nodeId,
    entityId: nodeId.replace("entity:", ""),
    label,
    entityType,
    redacted: false,
    claimCount: 1,
  };
}

function memoryEdge(
  claimId: string,
  sourceNodeId: string,
  targetNodeId: string,
): NormalizedMemoryGraph["edges"][number] {
  return {
    __typename: "GraphqlMemoryGraphEdge",
    claimId,
    sourceNodeId,
    targetNodeId,
    predicateId: "likes",
    predicateLabel: "likes",
    fact: "A likes B.",
    factRedacted: false,
    status: "confirmed",
    sensitivity: "public",
    confidence: 0.9,
    evidenceCount: 1,
    createdAt: "2026-06-29T00:00:00Z",
    updatedAt: "2026-06-29T00:00:00Z",
    label: "likes",
  };
}

function minimumNodeDistance(positions: Array<{ x: number; y: number }>): number {
  let minimum = Number.POSITIVE_INFINITY;

  for (let left = 0; left < positions.length; left += 1) {
    for (let right = left + 1; right < positions.length; right += 1) {
      const dx = positions[left].x - positions[right].x;
      const dy = positions[left].y - positions[right].y;
      minimum = Math.min(minimum, Math.hypot(dx, dy));
    }
  }

  return minimum;
}
