import { describe, test } from "node:test";
import assert from "node:assert/strict";
import {
  shellMenuLevelForRoute,
  shellSettingsEntries,
  settingsItemIdForSection
} from "./shellNavigation";

describe("settings shell navigation", () => {
  test("settings entries are grouped with agents top-level and live pages only", () => {
    assert.deepEqual(
      shellSettingsEntries.map((entry) => entry.kind === "group" ? entry.label : entry.item.label),
      [
        "Agents",
        "Memory",
        "Tools",
        "Web",
        "MCPs",
        "Safety",
        "Usage",
        "Approvals",
        "Identities",
        "System",
        "Providers"
      ]
    );
  });

  test("settings bottom item opens agents", () => {
    const level = shellMenuLevelForRoute({ kind: "chat" });

    assert.deepEqual(level.bottomItem.route, {
      kind: "settings",
      section: "agents"
    });
  });

  test("settings level contains group labels and item routes", () => {
    const level = shellMenuLevelForRoute({
      kind: "settings",
      section: "tools-web"
    });

    assert.equal(level.activeItemId, "settings.tools.web");
    assert.deepEqual(
      level.items.map((entry) =>
        entry.kind === "group"
          ? entry
          : {
              kind: entry.kind,
              item: {
                itemId: entry.item.itemId,
                label: entry.item.label,
                route: entry.item.route
              }
            }
      ),
      [
        {
          kind: "item",
          item: {
            itemId: "settings.agents",
            label: "Agents",
            route: { kind: "settings", section: "agents" }
          }
        },
        {
          kind: "item",
          item: {
            itemId: "settings.memory",
            label: "Memory",
            route: { kind: "settings", section: "memory" }
          }
        },
        { kind: "group", label: "Tools" },
        {
          kind: "item",
          item: {
            itemId: "settings.tools.web",
            label: "Web",
            route: { kind: "settings", section: "tools-web" }
          }
        },
        {
          kind: "item",
          item: {
            itemId: "settings.tools.mcps",
            label: "MCPs",
            route: { kind: "settings", section: "tools-mcps" }
          }
        },
        { kind: "group", label: "Safety" },
        {
          kind: "item",
          item: {
            itemId: "settings.safety.usage",
            label: "Usage",
            route: { kind: "settings", section: "safety-usage" }
          }
        },
        {
          kind: "item",
          item: {
            itemId: "settings.safety.approvals",
            label: "Approvals",
            route: { kind: "settings", section: "safety-approvals" }
          }
        },
        {
          kind: "item",
          item: {
            itemId: "settings.safety.identities",
            label: "Identities",
            route: { kind: "settings", section: "safety-identities" }
          }
        },
        { kind: "group", label: "System" },
        {
          kind: "item",
          item: {
            itemId: "settings.system.providers",
            label: "Providers",
            route: { kind: "settings", section: "system-providers" }
          }
        }
      ]
    );
  });

  test("settingsItemIdForSection maps canonical section ids", () => {
    assert.equal(settingsItemIdForSection("agents"), "settings.agents");
    assert.equal(settingsItemIdForSection("tools-web"), "settings.tools.web");
    assert.equal(settingsItemIdForSection("tools-mcps"), "settings.tools.mcps");
    assert.equal(settingsItemIdForSection("safety-approvals"), "settings.safety.approvals");
    assert.equal(settingsItemIdForSection("safety-identities"), "settings.safety.identities");
    assert.equal(settingsItemIdForSection("system-providers"), "settings.system.providers");
  });
});
