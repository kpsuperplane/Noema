import assert from "node:assert/strict";
import { describe, test } from "node:test";
import {
  pathForRoute,
  routeFromPathname,
  settingsBackNavigation,
  settingsFallbackRoute,
  shouldReplaceHistoryEntryForNavigation,
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
    assert.deepEqual(routeFromPathname("/settings/providers"), {
      kind: "settings",
      section: "providers"
    });
    assert.deepEqual(routeFromPathname("/settings/agents"), {
      kind: "settings",
      section: "agents"
    });
    assert.deepEqual(routeFromPathname("/settings/mcps"), {
      kind: "settings",
      section: "mcps"
    });
    assert.deepEqual(routeFromPathname("/settings/trusted-identities"), {
      kind: "settings",
      section: "trusted-identities"
    });
    assert.deepEqual(routeFromPathname("/settings/approvals"), {
      kind: "settings",
      section: "approvals"
    });
    assert.deepEqual(routeFromPathname("/settings/audit"), {
      kind: "settings",
      section: "audit"
    });
  });

  test("falls back to chat for unknown routes", () => {
    assert.deepEqual(routeFromPathname("/memory/nope"), { kind: "chat" });
    assert.deepEqual(routeFromPathname("/settings/unknown"), { kind: "chat" });
    assert.deepEqual(routeFromPathname("/not-a-real-route"), { kind: "chat" });
  });
});

describe("pathForRoute", () => {
  test("returns canonical paths", () => {
    assert.equal(pathForRoute({ kind: "chat" }), "/");
    assert.equal(pathForRoute({ kind: "memory_home" }), "/memory");
    assert.equal(pathForRoute({ kind: "memory_graph" }), "/memory/graph");
    assert.equal(pathForRoute({ kind: "settings", section: "providers" }), "/settings/providers");
    assert.equal(pathForRoute({ kind: "settings", section: "agents" }), "/settings/agents");
    assert.equal(pathForRoute({ kind: "settings", section: "mcps" }), "/settings/mcps");
    assert.equal(
      pathForRoute({ kind: "settings", section: "trusted-identities" }),
      "/settings/trusted-identities"
    );
    assert.equal(
      pathForRoute({ kind: "settings", section: "approvals" }),
      "/settings/approvals"
    );
    assert.equal(pathForRoute({ kind: "settings", section: "audit" }), "/settings/audit");
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

  test("uses browser history for in-session Settings entry and Home for direct loads", () => {
    assert.deepEqual(settingsBackNavigation(true), { kind: "history-back" });
    assert.deepEqual(settingsBackNavigation(false), {
      kind: "navigate",
      route: { kind: "chat" }
    });
  });

  test("replaces history entries when switching between Settings sections", () => {
    assert.equal(
      shouldReplaceHistoryEntryForNavigation(
        { kind: "settings", section: "providers" },
        { kind: "settings", section: "mcps" }
      ),
      true
    );
    assert.equal(
      shouldReplaceHistoryEntryForNavigation(
        { kind: "chat" },
        { kind: "settings", section: "providers" }
      ),
      false
    );
    assert.equal(
      shouldReplaceHistoryEntryForNavigation({ kind: "settings", section: "mcps" }, { kind: "chat" }),
      false
    );
  });
});
