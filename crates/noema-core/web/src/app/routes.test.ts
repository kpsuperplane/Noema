import { describe, test } from "node:test";
import assert from "node:assert/strict";
import { pathForRoute, routeFromPathname } from "./routes";

describe("settings routes", () => {
  test("settings root resolves to agents", () => {
    assert.deepEqual(routeFromPathname("/settings"), {
      kind: "settings",
      section: "agents"
    });
  });

  test("canonical nested settings paths resolve to grouped sections", () => {
    assert.deepEqual(routeFromPathname("/settings/agents"), {
      kind: "settings",
      section: "agents"
    });
    assert.deepEqual(routeFromPathname("/settings/memory"), {
      kind: "settings",
      section: "memory"
    });
    assert.deepEqual(routeFromPathname("/settings/tools/web"), {
      kind: "settings",
      section: "tools-web"
    });
    assert.deepEqual(routeFromPathname("/settings/tools/mcps"), {
      kind: "settings",
      section: "tools-mcps"
    });
    assert.deepEqual(routeFromPathname("/settings/safety/approvals"), {
      kind: "settings",
      section: "safety-approvals"
    });
    assert.deepEqual(routeFromPathname("/settings/safety/identities"), {
      kind: "settings",
      section: "safety-identities"
    });
    assert.deepEqual(routeFromPathname("/settings/system/providers"), {
      kind: "settings",
      section: "system-providers"
    });
  });

  test("old flat settings paths are not settings routes", () => {
    for (const pathname of [
      "/settings/providers",
      "/settings/mcps",
      "/settings/trusted-identities",
      "/settings/approvals"
    ]) {
      assert.deepEqual(routeFromPathname(pathname), { kind: "chat" });
    }
  });

  test("pathForRoute emits only canonical settings paths", () => {
    assert.equal(pathForRoute({ kind: "settings", section: "agents" }), "/settings/agents");
    assert.equal(pathForRoute({ kind: "settings", section: "memory" }), "/settings/memory");
    assert.equal(pathForRoute({ kind: "settings", section: "tools-web" }), "/settings/tools/web");
    assert.equal(pathForRoute({ kind: "settings", section: "tools-mcps" }), "/settings/tools/mcps");
    assert.equal(
      pathForRoute({ kind: "settings", section: "safety-approvals" }),
      "/settings/safety/approvals"
    );
    assert.equal(
      pathForRoute({ kind: "settings", section: "safety-identities" }),
      "/settings/safety/identities"
    );
    assert.equal(
      pathForRoute({ kind: "settings", section: "system-providers" }),
      "/settings/system/providers"
    );
  });

  test("routes memory to top-level memory", () => {
    assert.deepEqual(routeFromPathname("/memory"), { kind: "memory" });
    assert.equal(pathForRoute({ kind: "memory" }), "/memory");
    assert.equal(pathForRoute({ kind: "settings", section: "memory" }), "/settings/memory");
  });
});
