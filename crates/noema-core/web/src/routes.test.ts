import assert from "node:assert/strict";
import { describe, test } from "node:test";
import {
  pathForRoute,
  routeFromPathname,
  settingsFallbackRoute,
  shouldRememberAsPreviousAppRoute
} from "./routes";

describe("routeFromPathname", () => {
  test("recognizes chat, memory, and settings routes", () => {
    assert.deepEqual(routeFromPathname("/"), { kind: "chat" });
    assert.deepEqual(routeFromPathname("/chat"), { kind: "chat" });
    assert.deepEqual(routeFromPathname("/memory"), { kind: "memory_home" });
    assert.deepEqual(routeFromPathname("/memory/graph"), { kind: "memory_graph" });
    assert.deepEqual(routeFromPathname("/settings"), {
      kind: "settings",
      section: "providers"
    });
  });

  test("falls back to chat for unknown routes", () => {
    assert.deepEqual(routeFromPathname("/memory/nope"), { kind: "chat" });
    assert.deepEqual(routeFromPathname("/settings/providers"), { kind: "chat" });
    assert.deepEqual(routeFromPathname("/not-a-real-route"), { kind: "chat" });
  });
});

describe("pathForRoute", () => {
  test("returns canonical paths", () => {
    assert.equal(pathForRoute({ kind: "chat" }), "/");
    assert.equal(pathForRoute({ kind: "memory_home" }), "/memory");
    assert.equal(pathForRoute({ kind: "memory_graph" }), "/memory/graph");
    assert.equal(pathForRoute({ kind: "settings", section: "providers" }), "/settings");
  });
});

describe("settings route helpers", () => {
  test("remembers only non-settings app routes", () => {
    assert.equal(shouldRememberAsPreviousAppRoute({ kind: "chat" }), true);
    assert.equal(shouldRememberAsPreviousAppRoute({ kind: "memory_home" }), true);
    assert.equal(shouldRememberAsPreviousAppRoute({ kind: "memory_graph" }), true);
    assert.equal(
      shouldRememberAsPreviousAppRoute({ kind: "settings", section: "providers" }),
      false
    );
  });

  test("falls back from settings to home when no previous route exists", () => {
    assert.deepEqual(settingsFallbackRoute(null), { kind: "chat" });
    assert.deepEqual(settingsFallbackRoute({ kind: "memory_home" }), { kind: "memory_home" });
  });
});
