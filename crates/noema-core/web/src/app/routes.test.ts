import { describe, expect, test } from "bun:test";
import { pathForRoute, routeFromPathname } from "./routes";

describe("settings routes", () => {
  test("settings root resolves to agents", () => {
    expect(routeFromPathname("/settings")).toEqual({
      kind: "settings",
      section: "agents"
    });
  });

  test("canonical nested settings paths resolve to grouped sections", () => {
    expect(routeFromPathname("/settings/agents")).toEqual({
      kind: "settings",
      section: "agents"
    });
    expect(routeFromPathname("/settings/tools/web")).toEqual({
      kind: "settings",
      section: "tools-web"
    });
    expect(routeFromPathname("/settings/tools/mcps")).toEqual({
      kind: "settings",
      section: "tools-mcps"
    });
    expect(routeFromPathname("/settings/safety/approvals")).toEqual({
      kind: "settings",
      section: "safety-approvals"
    });
    expect(routeFromPathname("/settings/safety/identities")).toEqual({
      kind: "settings",
      section: "safety-identities"
    });
    expect(routeFromPathname("/settings/system/providers")).toEqual({
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
      expect(routeFromPathname(pathname)).toEqual({ kind: "chat" });
    }
  });

  test("pathForRoute emits only canonical settings paths", () => {
    expect(pathForRoute({ kind: "settings", section: "agents" })).toBe("/settings/agents");
    expect(pathForRoute({ kind: "settings", section: "tools-web" })).toBe("/settings/tools/web");
    expect(pathForRoute({ kind: "settings", section: "tools-mcps" })).toBe("/settings/tools/mcps");
    expect(pathForRoute({ kind: "settings", section: "safety-approvals" })).toBe(
      "/settings/safety/approvals"
    );
    expect(pathForRoute({ kind: "settings", section: "safety-identities" })).toBe(
      "/settings/safety/identities"
    );
    expect(pathForRoute({ kind: "settings", section: "system-providers" })).toBe(
      "/settings/system/providers"
    );
  });
});
