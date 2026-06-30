import assert from "node:assert/strict";
import { describe, test } from "node:test";
import { agentBadgeLabel, agentDisplayName, agentMetadataRows } from "./agentMetadata";

describe("agentMetadata", () => {
  test("formats display names with unnamed fallback", () => {
    assert.equal(agentDisplayName({ displayName: "Noema" }), "Noema");
    assert.equal(agentDisplayName({ displayName: null }), "Unnamed agent");
    assert.equal(agentDisplayName({ displayName: "   " }), "Unnamed agent");
  });

  test("formats primary badge labels", () => {
    assert.equal(agentBadgeLabel({ isPrimary: true }), "Primary");
    assert.equal(agentBadgeLabel({ isPrimary: false }), null);
    assert.equal(agentBadgeLabel({}), null);
  });

  test("returns only safe metadata rows", () => {
    assert.deepEqual(agentMetadataRows({ agentId: "agent:primary" }), [
      { label: "Agent id", value: "agent:primary" }
    ]);
  });
});
