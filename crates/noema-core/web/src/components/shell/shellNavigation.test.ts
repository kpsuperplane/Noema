import assert from "node:assert/strict";
import { describe, test } from "node:test";
import {
  breadcrumbForRoute,
  shellMenuLevelForRoute,
  shellMenuSelectionBehavior,
  shellSettingsSections
} from "./shellNavigation";

describe("shell route-derived navigation", () => {
  test("derives L0 for Home and Memory routes", () => {
    assert.equal(shellMenuLevelForRoute({ kind: "chat" }).levelId, "l0");
    assert.equal(shellMenuLevelForRoute({ kind: "memory_home" }).levelId, "l0");
    assert.equal(shellMenuLevelForRoute({ kind: "memory_graph" }).levelId, "l0");
    assert.deepEqual(
      shellMenuLevelForRoute({ kind: "chat" }).items.map((item) => item.itemId),
      ["home", "memory"]
    );
    assert.equal(shellMenuLevelForRoute({ kind: "chat" }).bottomItem?.itemId, "settings");
  });

  test("derives Settings L1 for every settings route", () => {
    assert.equal(
      shellMenuLevelForRoute({ kind: "settings", section: "providers" }).levelId,
      "settings"
    );
    assert.deepEqual(
      shellMenuLevelForRoute({ kind: "settings", section: "mcps" }).items.map(
        (item) => item.itemId
      ),
      [
        "settings.providers",
        "settings.agents",
        "settings.mcps",
        "settings.trusted-identities",
        "settings.approvals",
        "settings.audit"
      ]
    );
    assert.equal(
      shellMenuLevelForRoute({ kind: "settings", section: "mcps" }).bottomItem?.itemId,
      "settings.go-back"
    );
  });

  test("marks the active L0 and Settings leaf items", () => {
    assert.equal(shellMenuLevelForRoute({ kind: "chat" }).activeItemId, "home");
    assert.equal(shellMenuLevelForRoute({ kind: "memory_graph" }).activeItemId, "memory");
    assert.equal(
      shellMenuLevelForRoute({ kind: "settings", section: "providers" }).activeItemId,
      "settings.providers"
    );
    assert.equal(
      shellMenuLevelForRoute({ kind: "settings", section: "trusted-identities" }).activeItemId,
      "settings.trusted-identities"
    );
  });

  test("defines Settings sections in display order", () => {
    assert.deepEqual(
      shellSettingsSections.map((section) => [section.section, section.label]),
      [
        ["providers", "Providers"],
        ["agents", "Agents"],
        ["mcps", "MCPs"],
        ["trusted-identities", "Trusted identities"],
        ["approvals", "Approvals"],
        ["audit", "Audit"]
      ]
    );
  });

  test("builds quiet breadcrumbs for settings routes", () => {
    assert.deepEqual(breadcrumbForRoute({ kind: "settings", section: "mcps" }), {
      parent: "Settings",
      current: "MCPs"
    });
    assert.deepEqual(breadcrumbForRoute({ kind: "chat" }), {
      current: "Home"
    });
    assert.deepEqual(breadcrumbForRoute({ kind: "memory_graph" }), {
      current: "Memory"
    });
  });

  test("keeps sidebar reveal open for Settings parent and closes for leaves", () => {
    assert.equal(shellMenuSelectionBehavior("settings"), "keep-reveal-open");
    assert.equal(shellMenuSelectionBehavior("settings.providers"), "close-reveal");
    assert.equal(shellMenuSelectionBehavior("settings.mcps"), "close-reveal");
    assert.equal(shellMenuSelectionBehavior("home"), "close-reveal");
    assert.equal(shellMenuSelectionBehavior("memory"), "close-reveal");
    assert.equal(shellMenuSelectionBehavior("settings.go-back"), "close-reveal");
  });
});
