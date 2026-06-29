import assert from "node:assert/strict";
import { describe, test } from "node:test";
import { graphStatusDefaults, graphStatusOptions, normalizeMemoryGraph } from "./memoryGraph";

describe("graphStatusDefaults", () => {
  test("defaults to candidate, active, and confirmed", () => {
    assert.deepEqual(graphStatusDefaults(), ["candidate", "active", "confirmed"]);
  });
});

describe("normalizeMemoryGraph", () => {
  test("keeps readable graph labels and creates edge selection labels", () => {
    const normalized = normalizeMemoryGraph({
      nodes: [
        {
          __typename: "GraphqlMemoryGraphNode",
          nodeId: "entity:human:local",
          entityId: "human:local",
          label: "Local human",
          entityType: "human",
          redacted: false,
          claimCount: 1,
        },
      ],
      edges: [
        {
          __typename: "GraphqlMemoryGraphEdge",
          claimId: "claim:1",
          sourceNodeId: "entity:human:local",
          targetNodeId: "entity:human:local",
          predicateId: "has_note",
          predicateLabel: "has_note",
          fact: "Garage code is 1234.",
          factRedacted: false,
          status: "confirmed",
          sensitivity: "private",
          confidence: 0.9,
          evidenceCount: 2,
          createdAt: "2026-06-29T00:00:00Z",
          updatedAt: "2026-06-29T00:00:00Z",
        },
      ],
      summary: {
        __typename: "GraphqlMemoryGraphSummary",
        returnedClaimCount: 1,
        returnedNodeCount: 1,
        limit: 150,
        truncated: false,
      },
    });

    assert.equal(normalized.nodes[0].label, "Local human");
    assert.equal(normalized.nodes[0].redacted, false);
    assert.equal(normalized.edges[0].label, "has_note");
    assert.equal(normalized.edges[0].claimId, "claim:1");
    assert.equal(normalized.summary.returnedClaimCount, 1);
  });
});

describe("graphStatusOptions", () => {
  test("includes all backed status filters", () => {
    assert.deepEqual(
      graphStatusOptions().map((status) => status.value),
      ["candidate", "active", "confirmed", "disputed", "superseded", "archived", "deleted"],
    );
  });
});
