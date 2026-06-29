import assert from "node:assert/strict";
import { describe, test } from "node:test";
import { layoutMemoryGraph } from "./memoryGraphLayout";
import type { NormalizedMemoryGraph } from "./memoryGraph";

const graph: NormalizedMemoryGraph = {
  nodes: [
    {
      __typename: "GraphqlMemoryGraphNode",
      nodeId: "entity:b",
      entityId: "b",
      label: "B",
      entityType: "concept",
      redacted: false,
      claimCount: 1,
    },
    {
      __typename: "GraphqlMemoryGraphNode",
      nodeId: "entity:a",
      entityId: "a",
      label: "A",
      entityType: "human",
      redacted: false,
      claimCount: 1,
    },
  ],
  edges: [
    {
      __typename: "GraphqlMemoryGraphEdge",
      claimId: "claim:1",
      sourceNodeId: "entity:a",
      targetNodeId: "entity:b",
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
    },
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
  test("returns deterministic node positions and React Flow edges", () => {
    const first = layoutMemoryGraph(graph, { width: 800, height: 500 });
    const second = layoutMemoryGraph(graph, { width: 800, height: 500 });

    assert.deepEqual(first, second);
    assert.equal(first.nodes.length, 2);
    assert.equal(first.edges.length, 1);
    assert.equal(first.edges[0].id, "claim:1");
    assert.equal(first.edges[0].source, "entity:a");
    assert.equal(first.edges[0].target, "entity:b");
  });
});
