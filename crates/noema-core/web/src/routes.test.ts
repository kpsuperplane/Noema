import assert from "node:assert/strict";
import { describe, test } from "node:test";
import { pathForRoute, routeFromPathname } from "./routes";

describe("routeFromPathname", () => {
  test("recognizes chat and memory routes", () => {
    assert.deepEqual(routeFromPathname("/"), { kind: "chat" });
    assert.deepEqual(routeFromPathname("/chat"), { kind: "chat" });
    assert.deepEqual(routeFromPathname("/memory"), { kind: "memory_home" });
    assert.deepEqual(routeFromPathname("/memory/graph"), { kind: "memory_graph" });
  });

  test("falls back to chat for unknown routes", () => {
    assert.deepEqual(routeFromPathname("/memory/nope"), { kind: "chat" });
    assert.deepEqual(routeFromPathname("/not-a-real-route"), { kind: "chat" });
  });
});

describe("pathForRoute", () => {
  test("returns canonical paths", () => {
    assert.equal(pathForRoute({ kind: "chat" }), "/");
    assert.equal(pathForRoute({ kind: "memory_home" }), "/memory");
    assert.equal(pathForRoute({ kind: "memory_graph" }), "/memory/graph");
  });
});
