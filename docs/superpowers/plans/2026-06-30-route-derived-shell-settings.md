# Route-Derived Shell Settings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move Settings back into the main app shell by adding a generic route-derived L0 to L1 sidebar navigation model.

**Architecture:** Add a pure shell navigation model that derives menu level, active item, breadcrumbs, and item behavior from `AppRoute`. Refactor `ShellSidebar` to render the active menu level and refactor `SettingsPage` into a shell-contained `SettingsSurface`, while keeping existing Settings pane GraphQL behavior unchanged.

**Tech Stack:** React 19, TypeScript, lucide-react, shadcn/base-ui primitives, Bun, Node test runner, Apollo Client.

---

## File Structure

- Create `crates/noema-core/web/src/components/shell/shellNavigation.ts`
  - Pure route-derived shell navigation model.
  - Owns L0 and Settings L1 menu descriptors, active item derivation, quiet breadcrumbs, and item selection behavior metadata.

- Create `crates/noema-core/web/src/components/shell/shellNavigation.test.ts`
  - Pure unit tests for route-to-menu-level, active item, breadcrumbs, and close-or-stay-open sidebar behavior.

- Modify `crates/noema-core/web/src/routes.ts`
  - Keep existing settings route parsing.
  - Add route-helper state needed for browser-history Settings Go back without adding a semantic navigation stack.

- Modify `crates/noema-core/web/src/routes.test.ts`
  - Cover direct-load Settings fallback and prior-route Settings back behavior.

- Modify `crates/noema-core/web/src/components/settings/SettingsSidebar.tsx`
  - Delete after moving settings navigation into shell, or leave unused only if all imports are removed and a follow-up cleanup task deletes it in the same implementation commit.

- Modify `crates/noema-core/web/src/pages/SettingsPage.tsx`
  - Refactor the current full-screen page into a shell-contained content component.
  - Keep the filename and export a `SettingsSurface` component from it.

- Modify `crates/noema-core/web/src/pages/SettingsPage.test.tsx`
  - Replace full-screen takeover expectations with shell-contained Settings surface expectations.
  - Keep existing pane behavior tests.

- Modify `crates/noema-core/web/src/components/shell/ShellSidebar.tsx`
  - Render a provided `ShellMenuLevel`.
  - Support one top item list plus one stable bottom item.
  - Keep named-agent avatar behavior for Home.

- Modify `crates/noema-core/web/src/components/shell/AppShell.tsx`
  - Use `shellNavigation.ts`.
  - Render quiet breadcrumbs in the existing deck header wrapper that preserves Tauri offset.
  - Decide whether sidebar reveal stays open or closes after each item action.

- Modify `crates/noema-core/web/src/components/shell/AppShell.test.ts`
  - Update old expectations that Settings is an icon-only utility.
  - Add shell-contained Settings, breadcrumb, Tauri offset, and one-active-level tests.

- Modify `crates/noema-core/web/src/App.tsx`
  - Render Settings through `AppShell`.
  - Replace `closeSettings` takeover flow with browser-history Settings back behavior.

- Modify `crates/noema-core/web/src/App.test.ts`
  - Replace `shouldRenderSettingsRoute` tests with onboarded shell-route coverage.

- Modify `docs/frontend/navigation-workflows.md`
  - Replace full-screen takeover wording with route-derived Settings L1 wording.

- Modify `docs/context/current.md`
  - Update the durable shell direction after implementation lands.

## Task 1: Add The Pure Shell Navigation Model

**Files:**
- Create: `crates/noema-core/web/src/components/shell/shellNavigation.ts`
- Create: `crates/noema-core/web/src/components/shell/shellNavigation.test.ts`
- Modify: `crates/noema-core/web/src/components/shell/AppShell.test.ts`

- [ ] **Step 1: Write failing shell navigation model tests**

Create `crates/noema-core/web/src/components/shell/shellNavigation.test.ts`:

```ts
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
```

- [ ] **Step 2: Run the failing shell navigation tests**

Run:

```bash
cd crates/noema-core/web
bun test src/components/shell/shellNavigation.test.ts
```

Expected: FAIL because `shellNavigation.ts` does not exist.

- [ ] **Step 3: Implement the shell navigation model**

Create `crates/noema-core/web/src/components/shell/shellNavigation.ts`:

```ts
import {
  ArrowLeft,
  Bot,
  Brain,
  CheckSquare,
  Fingerprint,
  History,
  House,
  PlugZap,
  ServerCog,
  Settings
} from "lucide-react";
import type { ComponentType } from "react";
import type { AppRoute, SettingsSection } from "@/routes";

export type ShellMenuLevelId = "l0" | "settings";

export type ShellMenuItemId =
  | "home"
  | "memory"
  | "settings"
  | "settings.providers"
  | "settings.agents"
  | "settings.mcps"
  | "settings.trusted-identities"
  | "settings.approvals"
  | "settings.audit"
  | "settings.go-back";

export type ShellMenuSelectionBehavior = "close-reveal" | "keep-reveal-open";

export type ShellMenuItem = {
  itemId: ShellMenuItemId;
  label: string;
  route?: AppRoute;
  action: "navigate" | "goBackFromSettings";
  icon: ComponentType<{ className?: string; "aria-hidden"?: true }>;
};

export type ShellMenuLevel = {
  levelId: ShellMenuLevelId;
  ariaLabel: string;
  title: string;
  activeItemId: ShellMenuItemId;
  items: ShellMenuItem[];
  bottomItem: ShellMenuItem;
  supportsAttention: boolean;
};

export type ShellBreadcrumb =
  | { current: string; parent?: undefined }
  | { parent: string; current: string };

export type ShellSettingsSection = {
  section: SettingsSection;
  itemId: Extract<ShellMenuItemId, `settings.${string}`>;
  label: string;
  icon: ComponentType<{ className?: string; "aria-hidden"?: true }>;
};

export const shellSettingsSections: ShellSettingsSection[] = [
  { section: "providers", itemId: "settings.providers", label: "Providers", icon: ServerCog },
  { section: "agents", itemId: "settings.agents", label: "Agents", icon: Bot },
  { section: "mcps", itemId: "settings.mcps", label: "MCPs", icon: PlugZap },
  {
    section: "trusted-identities",
    itemId: "settings.trusted-identities",
    label: "Trusted identities",
    icon: Fingerprint
  },
  { section: "approvals", itemId: "settings.approvals", label: "Approvals", icon: CheckSquare },
  { section: "audit", itemId: "settings.audit", label: "Audit", icon: History }
];

export function activeL0ItemId(route: AppRoute): Extract<ShellMenuItemId, "home" | "memory"> {
  if (route.kind === "memory_home" || route.kind === "memory_graph") {
    return "memory";
  }
  return "home";
}

export function settingsItemIdForSection(section: SettingsSection): ShellMenuItemId {
  return `settings.${section}` as ShellMenuItemId;
}

export function shellMenuLevelForRoute(route: AppRoute): ShellMenuLevel {
  if (route.kind === "settings") {
    return {
      levelId: "settings",
      ariaLabel: "Settings",
      title: "Settings",
      activeItemId: settingsItemIdForSection(route.section),
      items: shellSettingsSections.map((item) => ({
        itemId: item.itemId,
        label: item.label,
        route: { kind: "settings", section: item.section },
        action: "navigate",
        icon: item.icon
      })),
      bottomItem: {
        itemId: "settings.go-back",
        label: "Go back",
        action: "goBackFromSettings",
        icon: ArrowLeft
      },
      supportsAttention: false
    };
  }

  return {
    levelId: "l0",
    ariaLabel: "Primary",
    title: "Noema",
    activeItemId: activeL0ItemId(route),
    items: [
      { itemId: "home", label: "Home", route: { kind: "chat" }, action: "navigate", icon: House },
      {
        itemId: "memory",
        label: "Memory",
        route: { kind: "memory_home" },
        action: "navigate",
        icon: Brain
      }
    ],
    bottomItem: {
      itemId: "settings",
      label: "Settings",
      route: { kind: "settings", section: "providers" },
      action: "navigate",
      icon: Settings
    },
    supportsAttention: true
  };
}

export function breadcrumbForRoute(route: AppRoute): ShellBreadcrumb {
  if (route.kind === "settings") {
    const current =
      shellSettingsSections.find((section) => section.section === route.section)?.label ??
      "Providers";
    return { parent: "Settings", current };
  }

  if (route.kind === "memory_home" || route.kind === "memory_graph") {
    return { current: "Memory" };
  }

  return { current: "Home" };
}

export function shellMenuSelectionBehavior(
  itemId: ShellMenuItemId
): ShellMenuSelectionBehavior {
  return itemId === "settings" ? "keep-reveal-open" : "close-reveal";
}
```

- [ ] **Step 4: Run shell navigation tests to verify they pass**

Run:

```bash
cd crates/noema-core/web
bun test src/components/shell/shellNavigation.test.ts
```

Expected: PASS.

- [ ] **Step 5: Commit Task 1**

Run:

```bash
git add crates/noema-core/web/src/components/shell/shellNavigation.ts crates/noema-core/web/src/components/shell/shellNavigation.test.ts
git commit -m "feat: add route-derived shell navigation model"
```

Expected: commit succeeds. Preserve all unrelated dirty files.

## Task 2: Make Settings Back Browser-History-Aware

**Files:**
- Modify: `crates/noema-core/web/src/routes.ts`
- Modify: `crates/noema-core/web/src/routes.test.ts`

- [ ] **Step 1: Write failing route-helper tests**

In `crates/noema-core/web/src/routes.test.ts`, update imports:

```ts
import {
  pathForRoute,
  routeFromPathname,
  settingsBackNavigation,
  settingsFallbackRoute,
  shouldRememberAsPreviousAppRoute
} from "./routes";
```

Add this test inside `describe("settings route helpers", () => { ... })`:

```ts
  test("uses browser history for in-session Settings entry and Home for direct loads", () => {
    assert.deepEqual(settingsBackNavigation(true), { kind: "history-back" });
    assert.deepEqual(settingsBackNavigation(false), {
      kind: "navigate",
      route: { kind: "chat" }
    });
  });
```

- [ ] **Step 2: Run route tests to verify they fail**

Run:

```bash
cd crates/noema-core/web
bun test src/routes.test.ts
```

Expected: FAIL because `settingsBackNavigation` is not exported.

- [ ] **Step 3: Implement the route helper**

In `crates/noema-core/web/src/routes.ts`, add this type and function near
`settingsFallbackRoute`:

```ts
export type SettingsBackNavigation =
  | { kind: "history-back" }
  | { kind: "navigate"; route: NonSettingsAppRoute };

export function settingsBackNavigation(canUseBrowserHistory: boolean): SettingsBackNavigation {
  if (canUseBrowserHistory) {
    return { kind: "history-back" };
  }
  return { kind: "navigate", route: { kind: "chat" } };
}
```

Do not remove `settingsFallbackRoute`; existing tests and code can continue to
use it.

- [ ] **Step 4: Run route tests to verify they pass**

Run:

```bash
cd crates/noema-core/web
bun test src/routes.test.ts
```

Expected: PASS.

- [ ] **Step 5: Commit Task 2**

Run:

```bash
git add crates/noema-core/web/src/routes.ts crates/noema-core/web/src/routes.test.ts
git commit -m "feat: add settings back route helper"
```

Expected: commit succeeds.

## Task 3: Refactor Settings Into A Shell-Contained Surface

**Files:**
- Modify: `crates/noema-core/web/src/pages/SettingsPage.tsx`
- Modify: `crates/noema-core/web/src/pages/SettingsPage.test.tsx`
- Modify or delete: `crates/noema-core/web/src/components/settings/SettingsSidebar.tsx`

- [ ] **Step 1: Write failing Settings surface tests**

In `crates/noema-core/web/src/pages/SettingsPage.test.tsx`, replace the
first `SettingsPage` markup test with this shell-contained surface test:

```tsx
  test("renders a shell-contained settings surface with Providers selected", () => {
    const markup = renderToStaticMarkup(
      <ApolloProvider client={testApolloClient()}>
        <SettingsSurface section="providers" />
      </ApolloProvider>
    );

    assert.match(markup, /data-slot="settings-surface"/);
    assert.doesNotMatch(markup, /data-slot="settings-page"/);
    assert.doesNotMatch(markup, /data-slot="settings-sidebar"/);
    assert.doesNotMatch(markup, /aria-label="Close settings"/);
    assert.match(markup, /Providers/);
    assert.match(markup, /Secret credential material stays outside the UI/);
  });
```

Update the import at the top:

```tsx
import { SettingsSurface } from "./SettingsPage";
```

For every existing `SettingsPage` render in this file, replace:

```tsx
<SettingsPage section="agents" onNavigate={() => {}} onClose={() => {}} />
```

with:

```tsx
<SettingsSurface section="agents" />
```

Delete the `routes Settings tab selections through navigation` test from
`SettingsPage.test.tsx`; Task 4 will cover Settings sidebar navigation through
the shell menu model.

- [ ] **Step 2: Run Settings page tests to verify they fail**

Run:

```bash
cd crates/noema-core/web
bun test src/pages/SettingsPage.test.tsx
```

Expected: FAIL because `SettingsSurface` does not exist and the full-screen
settings markup is still present.

- [ ] **Step 3: Implement `SettingsSurface`**

Replace `SettingsPage` props and component in
`crates/noema-core/web/src/pages/SettingsPage.tsx` with:

```tsx
import { AgentsSettingsPane } from "@/components/settings/AgentsSettingsPane";
import { ApprovalsSettingsPane } from "@/components/settings/ApprovalsSettingsPane";
import { AuditSettingsPane } from "@/components/settings/AuditSettingsPane";
import { McpSettingsPane } from "@/components/settings/McpSettingsPane";
import { ProvidersSettingsPane } from "@/components/settings/ProvidersSettingsPane";
import { TrustedIdentitiesSettingsPane } from "@/components/settings/TrustedIdentitiesSettingsPane";
import type { SettingsSection } from "@/routes";

type SettingsSurfaceProps = {
  section: SettingsSection;
};

const settingsSectionCopy: Record<SettingsSection, { title: string; description: string }> = {
  providers: {
    title: "Providers",
    description:
      "Review the provider account Noema uses for chat. Secret credential material stays outside the UI."
  },
  agents: {
    title: "Agents",
    description: "Review the agents currently registered in Noema. This tab is read-only for now."
  },
  mcps: {
    title: "MCPs",
    description: "Review third-party MCP servers mediated by the Noema capability gateway."
  },
  "trusted-identities": {
    title: "Trusted identities",
    description: "Review identity selectors used to resolve tool-result ownership."
  },
  approvals: {
    title: "Approvals",
    description: "Review pending MCP approval checkpoints."
  },
  audit: {
    title: "Audit",
    description: "Review mediated MCP activity records."
  }
};

export function SettingsSurface({ section }: SettingsSurfaceProps) {
  const copy = settingsSectionCopy[section];

  return (
    <section
      data-slot="settings-surface"
      className="min-h-0 overflow-auto px-6 py-6 max-[760px]:px-5"
      aria-labelledby="settings-surface-title"
    >
      <div className="grid max-w-3xl gap-5">
        <div className="grid gap-2">
          <h1
            id="settings-surface-title"
            className="m-0 font-heading text-2xl leading-tight tracking-normal text-foreground"
          >
            {copy.title}
          </h1>
          <p className="m-0 max-w-[620px] text-sm text-muted-foreground">
            {copy.description}
          </p>
        </div>
        <SettingsSectionPane section={section} />
      </div>
    </section>
  );
}

function SettingsSectionPane({ section }: { section: SettingsSection }) {
  switch (section) {
    case "agents":
      return <AgentsSettingsPane />;
    case "mcps":
      return <McpSettingsPane />;
    case "trusted-identities":
      return <TrustedIdentitiesSettingsPane />;
    case "approvals":
      return <ApprovalsSettingsPane />;
    case "audit":
      return <AuditSettingsPane />;
    case "providers":
      return <ProvidersSettingsPane />;
  }
}
```

Remove imports for `X`, `SettingsSidebar`, `Button`, `AppRoute`, and the old
`SettingsPageProps`.

- [ ] **Step 4: Delete the old local Settings sidebar**

Delete `crates/noema-core/web/src/components/settings/SettingsSidebar.tsx`.

Run:

```bash
cd crates/noema-core/web
rg "SettingsSidebar|settings-sidebar" src
```

Expected: no output.

- [ ] **Step 5: Run Settings surface tests to verify they pass**

Run:

```bash
cd crates/noema-core/web
bun test src/pages/SettingsPage.test.tsx
```

Expected: PASS.

- [ ] **Step 6: Commit Task 3**

Run:

```bash
git add crates/noema-core/web/src/pages/SettingsPage.tsx crates/noema-core/web/src/pages/SettingsPage.test.tsx
git add -u crates/noema-core/web/src/components/settings/SettingsSidebar.tsx
git commit -m "feat: make settings a shell-contained surface"
```

Expected: commit succeeds.

## Task 4: Render Route-Derived Menu Levels In The Shell

**Files:**
- Modify: `crates/noema-core/web/src/components/shell/ShellSidebar.tsx`
- Modify: `crates/noema-core/web/src/components/shell/AppShell.tsx`
- Modify: `crates/noema-core/web/src/components/shell/AppShell.test.ts`

- [ ] **Step 1: Write failing AppShell tests for Settings L1**

In `crates/noema-core/web/src/components/shell/AppShell.test.ts`, update imports:

```ts
import {
  AppShell,
  shellBrowserDesktopChromeOffset,
  shellDesktopChromeOffsetForRuntime,
  shellDesktopSidebarWidth,
  shellContentDeckClassName,
  shellRootStyle,
  shellRouteContentClassName,
  shellSidebarCollapseButtonClassName,
  shellSidebarGroundClassName,
  shellAttentionForState,
  shellTauriDesktopChromeOffset,
  type ShellAttentionInput
} from "./AppShell";
```

Delete tests that import or assert `shellNavItems`, `shellNavItemsForStatus`,
and `activeShellDestination`. Their behavior moves to `shellNavigation.test.ts`.

Replace the old settings utility test with:

```ts
  test("renders Settings as a full bottom L0 menu item", () => {
    const markup = renderShell();

    assert.equal(countMatches(markup, /aria-label="Primary"/g), 1);
    assert.equal(countMatches(markup, />Settings</g), 1);
    assert.match(markup, /data-slot="shell-menu-bottom-item"/);
    assert.doesNotMatch(markup, /data-slot="shell-settings-button"/);
    assert.doesNotMatch(markup, /aria-label="Open settings"/);
  });
```

Add these tests inside `describe("AppShell layered deck markup", () => { ... })`:

```ts
  test("renders Settings L1 menu and quiet breadcrumb for settings routes", () => {
    const markup = renderShell({ route: { kind: "settings", section: "mcps" } });

    assert.match(markup, /aria-label="Settings"/);
    assert.match(markup, />Providers</);
    assert.match(markup, />Agents</);
    assert.match(markup, />MCPs</);
    assert.match(markup, />Trusted identities</);
    assert.match(markup, />Approvals</);
    assert.match(markup, />Audit</);
    assert.match(markup, />Go back</);
    assert.match(markup, /data-slot="shell-breadcrumb"/);
    assert.match(markup, /data-slot="shell-breadcrumb-parent"[^>]*>Settings</);
    assert.match(markup, /data-slot="shell-breadcrumb-current"[^>]*>MCPs</);
    assert.match(markup, /data-shell-menu-item="settings\.mcps"[^>]*aria-current="page"/);
  });

  test("keeps the Tauri header offset wrapper around breadcrumbs", () => {
    const markup = renderShell({ route: { kind: "settings", section: "agents" } });
    const headerOffsetClassName = dataSlotClassName(markup, "shell-header-offset");

    assert.match(headerOffsetClassName, /transition-transform/);
    assert.match(
      headerOffsetClassName,
      /translate-x-\[calc\(1\.5rem\+var\(--shell-desktop-chrome-offset\)\)\]/
    );
    assert.match(markup, /data-slot="shell-breadcrumb"/);
  });
```

- [ ] **Step 2: Run AppShell tests to verify they fail**

Run:

```bash
cd crates/noema-core/web
bun test src/components/shell/AppShell.test.ts
```

Expected: FAIL because `ShellSidebar` still hard-codes L0 and Settings is still
icon-only.

- [ ] **Step 3: Refactor `ShellSidebar` to render menu levels**

Replace `crates/noema-core/web/src/components/shell/ShellSidebar.tsx` with:

```tsx
import type { ShellMenuItem, ShellMenuLevel } from "./shellNavigation";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { IdentityAvatar, LOCAL_AGENT_AVATAR_ID } from "../IdentityAvatar";
import type { ShellAttention } from "./AppShell";
import { ShellAttentionItem } from "./ShellAttentionItem";

export function ShellSidebar({
  menuLevel,
  attention,
  primaryAgentNamed,
  primaryAgentLabel,
  onSelectItem
}: {
  menuLevel: ShellMenuLevel;
  attention: ShellAttention | null;
  primaryAgentNamed: boolean;
  primaryAgentLabel: string;
  onSelectItem: (item: ShellMenuItem) => void;
}) {
  const visibleAttention =
    menuLevel.supportsAttention && attention ? (
      <ShellAttentionItem attention={attention} />
    ) : (
      <div aria-hidden="true" />
    );

  return (
    <div
      data-slot="shell-sidebar-menu-level"
      data-shell-menu-level={menuLevel.levelId}
      className="grid h-full min-h-0 grid-rows-[auto_auto_minmax(0,1fr)_auto] gap-1 overflow-hidden"
    >
      <div className="h-8" data-tauri-drag-region />
      {visibleAttention}

      <nav className="grid content-start gap-1" aria-label={menuLevel.ariaLabel}>
        {menuLevel.items.map((item) => (
          <ShellMenuButton
            key={item.itemId}
            item={item}
            active={item.itemId === menuLevel.activeItemId}
            primaryAgentNamed={primaryAgentNamed}
            primaryAgentLabel={primaryAgentLabel}
            onSelectItem={onSelectItem}
          />
        ))}
      </nav>

      <div className="flex items-end">
        <ShellMenuButton
          item={menuLevel.bottomItem}
          active={menuLevel.bottomItem.itemId === menuLevel.activeItemId}
          primaryAgentNamed={false}
          primaryAgentLabel={primaryAgentLabel}
          bottom
          onSelectItem={onSelectItem}
        />
      </div>
    </div>
  );
}

function ShellMenuButton({
  item,
  active,
  primaryAgentNamed,
  primaryAgentLabel,
  bottom = false,
  onSelectItem
}: {
  item: ShellMenuItem;
  active: boolean;
  primaryAgentNamed: boolean;
  primaryAgentLabel: string;
  bottom?: boolean;
  onSelectItem: (item: ShellMenuItem) => void;
}) {
  const Icon = item.icon;
  const showPrimaryAgentAvatar = item.itemId === "home" && primaryAgentNamed;
  const label = item.itemId === "home" && primaryAgentNamed ? primaryAgentLabel : item.label;

  return (
    <Button
      data-slot={bottom ? "shell-menu-bottom-item" : "shell-menu-item"}
      data-shell-menu-item={item.itemId}
      type="button"
      variant="ghost"
      className={cn(
        "w-full justify-start rounded-md px-2.5 text-sm text-[var(--pine-700)] !bg-transparent hover:!bg-[color-mix(in_srgb,var(--pine-700)_10%,transparent)] aria-expanded:!bg-[color-mix(in_srgb,var(--pine-700)_10%,transparent)]",
        active && "!bg-[color-mix(in_srgb,var(--pine-700)_10%,transparent)]"
      )}
      aria-current={active ? "page" : undefined}
      onClick={() => onSelectItem(item)}
    >
      {showPrimaryAgentAvatar ? (
        <span
          data-slot="shell-primary-agent-avatar"
          aria-hidden="true"
          className="grid size-4 shrink-0 place-items-center [&_[data-slot=avatar]]:!size-4"
        >
          <IdentityAvatar
            actorId={LOCAL_AGENT_AVATAR_ID}
            actorType="agent"
            className="!size-4"
            size="sm"
          />
        </span>
      ) : (
        <Icon className="size-4" aria-hidden />
      )}
      <span className="truncate">{label}</span>
    </Button>
  );
}
```

- [ ] **Step 4: Update `AppShell` to pass route-derived menu levels**

In `crates/noema-core/web/src/components/shell/AppShell.tsx`, remove imports for
`Settings`, `House`, `Brain`, `ComponentType`, and the old `ShellDestination`
navigation item code.

Import:

```ts
import {
  breadcrumbForRoute,
  shellMenuLevelForRoute,
  shellMenuSelectionBehavior,
  type ShellBreadcrumb,
  type ShellMenuItem
} from "./shellNavigation";
```

Delete `ShellDestination`, `ShellNavItem`, `shellNavItems`,
`activeShellDestination`, and `shellNavItemsForStatus`.

Add:

```ts
function shellHeaderLabelForBreadcrumb(breadcrumb: ShellBreadcrumb) {
  return breadcrumb.parent ? `${breadcrumb.parent} / ${breadcrumb.current}` : breadcrumb.current;
}

function ShellBreadcrumbLabel({ breadcrumb }: { breadcrumb: ShellBreadcrumb }) {
  if (!breadcrumb.parent) {
    return (
      <strong className="block truncate font-heading text-base tracking-normal">
        {breadcrumb.current}
      </strong>
    );
  }

  return (
    <div data-slot="shell-breadcrumb" className="flex min-w-0 items-center gap-2">
      <span
        data-slot="shell-breadcrumb-parent"
        className="truncate text-sm text-muted-foreground"
      >
        {breadcrumb.parent}
      </span>
      <span className="text-muted-foreground/70" aria-hidden="true">
        /
      </span>
      <strong
        data-slot="shell-breadcrumb-current"
        className="truncate font-heading text-base tracking-normal"
      >
        {breadcrumb.current}
      </strong>
    </div>
  );
}
```

Inside `AppShell`, derive:

```ts
  const menuLevel = shellMenuLevelForRoute(route);
  const breadcrumb = breadcrumbForRoute(route);
  const activeLabel = shellHeaderLabelForBreadcrumb(breadcrumb);
```

Replace the old `ShellSidebar` props with:

```tsx
        <ShellSidebar
          menuLevel={menuLevel}
          attention={attention}
          primaryAgentNamed={Boolean(primaryAgentName)}
          primaryAgentLabel={primaryAgentName || "Home"}
          onSelectItem={selectShellMenuItem}
        />
```

Add the callback before `return`:

```ts
  const selectShellMenuItem = React.useCallback(
    (item: ShellMenuItem) => {
      if (item.action === "goBackFromSettings") {
        goBackFromSettings();
        closeNav();
        return;
      }

      if (!item.route) {
        return;
      }

      onNavigate(item.route);
      if (shellMenuSelectionBehavior(item.itemId) === "close-reveal") {
        closeNav();
      }
    },
    [closeNav, goBackFromSettings, onNavigate]
  );
```

Add `goBackFromSettings` to `AppShell` props:

```ts
  goBackFromSettings: () => void;
```

Replace the deck header label block:

```tsx
            <div className="min-w-0 py-[0.2rem]">
              <ShellBreadcrumbLabel breadcrumb={breadcrumb} />
            </div>
```

Add `data-slot="shell-header-offset"` to the existing wrapper that has
`transition-transform`:

```tsx
          <div
            data-slot="shell-header-offset"
            className={cn(
              "transition-transform duration-300",
              deckNavigation.sidebarCollapsed &&
                "min-[761px]:translate-x-[calc(1.5rem+var(--shell-desktop-chrome-offset))]"
            )}
          >
```

- [ ] **Step 5: Run shell tests to verify they pass**

Run:

```bash
cd crates/noema-core/web
bun test src/components/shell/shellNavigation.test.ts src/components/shell/AppShell.test.ts
```

Expected: PASS.

- [ ] **Step 6: Commit Task 4**

Run:

```bash
git add crates/noema-core/web/src/components/shell/ShellSidebar.tsx crates/noema-core/web/src/components/shell/AppShell.tsx crates/noema-core/web/src/components/shell/AppShell.test.ts
git commit -m "feat: render route-derived shell menu levels"
```

Expected: commit succeeds.

## Task 5: Route Settings Through AppShell

**Files:**
- Modify: `crates/noema-core/web/src/App.tsx`
- Modify: `crates/noema-core/web/src/App.test.ts`

- [ ] **Step 1: Write failing app-level helper tests**

In `crates/noema-core/web/src/App.test.ts`, replace the
`shouldRenderSettingsRoute` import with:

```ts
import {
  canSendMessage,
  shouldRefreshLocalStatusForConversationEvent,
  shouldRouteThroughAppShell
} from "./App";
```

Replace the `describe("shouldRenderSettingsRoute", ...)` block with:

```ts
describe("shouldRouteThroughAppShell", () => {
  test("routes every onboarded product surface through the shell", () => {
    assert.equal(shouldRouteThroughAppShell({ route: { kind: "chat" }, onboarded: true }), true);
    assert.equal(
      shouldRouteThroughAppShell({ route: { kind: "memory_home" }, onboarded: true }),
      true
    );
    assert.equal(
      shouldRouteThroughAppShell({ route: { kind: "memory_graph" }, onboarded: true }),
      true
    );
    assert.equal(
      shouldRouteThroughAppShell({
        route: { kind: "settings", section: "providers" },
        onboarded: true
      }),
      true
    );
    assert.equal(
      shouldRouteThroughAppShell({
        route: { kind: "settings", section: "mcps" },
        onboarded: true
      }),
      true
    );
    assert.equal(shouldRouteThroughAppShell({ route: { kind: "chat" }, onboarded: false }), false);
  });
});
```

- [ ] **Step 2: Run app tests to verify they fail**

Run:

```bash
cd crates/noema-core/web
bun test src/App.test.ts
```

Expected: FAIL because `shouldRouteThroughAppShell` does not exist.

- [ ] **Step 3: Implement shell routing in `App.tsx`**

In `crates/noema-core/web/src/App.tsx`, replace:

```ts
import { SettingsPage } from "./pages/SettingsPage";
import { useBrowserRoute, type AppRoute } from "./routes";
```

with:

```ts
import { SettingsSurface } from "./pages/SettingsPage";
import { useBrowserRoute, type AppRoute } from "./routes";
```

Replace `shouldRenderSettingsRoute` with:

```ts
export function shouldRouteThroughAppShell({
  route,
  onboarded
}: {
  route: AppRoute;
  onboarded: boolean;
}) {
  return onboarded && Boolean(route);
}
```

Use the browser route hook as:

```ts
  const { route, navigate, goBackFromSettings } = useBrowserRoute();
```

Delete the old branch:

```tsx
  if (route.kind === "settings" && shouldRenderSettingsRoute({ route, onboarded })) {
    return <SettingsPage section={route.section} onNavigate={navigate} onClose={closeSettings} />;
  }
```

Update each `AppShell` render to pass `goBackFromSettings`:

```tsx
      <AppShell
        route={route}
        status={status}
        socketState={socketState}
        onNavigate={navigate}
        goBackFromSettings={goBackFromSettings}
      >
```

Add a Settings branch before the final chat return:

```tsx
  if (route.kind === "settings") {
    return (
      <AppShell
        route={route}
        status={status}
        socketState={socketState}
        onNavigate={navigate}
        goBackFromSettings={goBackFromSettings}
      >
        <SettingsSurface section={route.section} />
      </AppShell>
    );
  }
```

- [ ] **Step 4: Update `useBrowserRoute` return shape**

In `crates/noema-core/web/src/routes.ts`, import no new React APIs. Update
`useBrowserRoute` to track whether the user entered Settings from a
non-settings app route in the current session:

```ts
  const canGoBackFromSettingsRef = React.useRef(false);
```

Update the `onPopState` effect so returning to any non-settings route clears
that flag:

```ts
      if (shouldRememberAsPreviousAppRoute(nextRoute)) {
        previousAppRouteRef.current = nextRoute;
        canGoBackFromSettingsRef.current = false;
      }
```

Update `navigate` so an app-route to Settings transition enables browser
history back, Settings-to-Settings leaf navigation preserves it, and any
non-settings route clears it:

```ts
  const navigate = React.useCallback((nextRoute: AppRoute) => {
    setRoute((currentRoute) => {
      if (shouldRememberAsPreviousAppRoute(currentRoute)) {
        previousAppRouteRef.current = currentRoute;
      }

      if (nextRoute.kind === "settings") {
        canGoBackFromSettingsRef.current =
          canGoBackFromSettingsRef.current || shouldRememberAsPreviousAppRoute(currentRoute);
      } else {
        canGoBackFromSettingsRef.current = false;
      }

      const nextPath = pathForRoute(nextRoute);
      window.history.pushState({}, "", nextPath);
      return routeFromPathname(nextPath);
    });
  }, []);
```

Delete the old `closeSettings` callback and add:

```ts
  const goBackFromSettings = React.useCallback(() => {
    const action = settingsBackNavigation(canGoBackFromSettingsRef.current);
    canGoBackFromSettingsRef.current = false;

    if (action.kind === "history-back") {
      window.history.back();
      return;
    }

    const nextPath = pathForRoute(action.route);
    window.history.pushState({}, "", nextPath);
    setRoute(routeFromPathname(nextPath));
  }, []);
```

Update the return object:

```ts
  return {
    route,
    navigate,
    goBackFromSettings
  };
```

Run this check after updating `App.tsx` and `routes.ts`:

```bash
cd crates/noema-core/web
rg "closeSettings|shouldRenderSettingsRoute" src
rg "SettingsPage" src/App.tsx
```

Expected: both commands print no output.

- [ ] **Step 5: Run app tests to verify they pass**

Run:

```bash
cd crates/noema-core/web
bun test src/App.test.ts src/routes.test.ts
```

Expected: PASS.

- [ ] **Step 6: Commit Task 5**

Run:

```bash
git add crates/noema-core/web/src/App.tsx crates/noema-core/web/src/App.test.ts crates/noema-core/web/src/routes.ts
git commit -m "feat: route settings through app shell"
```

Expected: commit succeeds.

## Task 6: Polish Sidebar Level Animation And Accessibility

**Files:**
- Modify: `crates/noema-core/web/src/components/shell/ShellSidebar.tsx`
- Modify: `crates/noema-core/web/src/components/shell/AppShell.tsx`
- Modify: `crates/noema-core/web/src/components/shell/AppShell.test.ts`

- [ ] **Step 1: Add tests for one interactive level and offscreen focus safety**

In `crates/noema-core/web/src/components/shell/AppShell.test.ts`, add:

```ts
  test("renders only one interactive sidebar menu level at a time", () => {
    const primaryMarkup = renderShell({ route: { kind: "chat" } });
    const settingsMarkup = renderShell({ route: { kind: "settings", section: "providers" } });

    assert.equal(countMatches(primaryMarkup, /data-slot="shell-sidebar-menu-level"/g), 1);
    assert.equal(countMatches(settingsMarkup, /data-slot="shell-sidebar-menu-level"/g), 1);
    assert.match(primaryMarkup, /data-shell-menu-level="l0"/);
    assert.match(settingsMarkup, /data-shell-menu-level="settings"/);
    assert.doesNotMatch(settingsMarkup, /data-shell-menu-item="home"/);
  });

  test("labels bottom menu actions accessibly", () => {
    assert.match(renderShell({ route: { kind: "chat" } }), />Settings</);
    assert.match(
      renderShell({ route: { kind: "settings", section: "providers" } }),
      />Go back</
    );
  });
```

- [ ] **Step 2: Run AppShell tests**

Run:

```bash
cd crates/noema-core/web
bun test src/components/shell/AppShell.test.ts
```

Expected: PASS if Task 4 already renders one level; FAIL if old hidden levels
or icon-only bottom actions remain.

- [ ] **Step 3: Add menu-level transition classes without rendering hidden buttons**

In `ShellSidebar`, keep rendering only the current `ShellMenuLevel`. Add
classes and data attributes that allow CSS transitions without mounting two
focusable menus:

```tsx
    <div
      data-slot="shell-sidebar-menu-level"
      data-shell-menu-level={menuLevel.levelId}
      className="grid h-full min-h-0 grid-rows-[auto_auto_minmax(0,1fr)_auto] gap-1 overflow-hidden transition-transform duration-300 ease-out motion-reduce:transition-none"
    >
```

This gives the current level a stable transition surface. Do not render the
previous level with hidden buttons; that would violate the spec's focus-safety
requirement.

- [ ] **Step 4: Run AppShell and shell navigation tests**

Run:

```bash
cd crates/noema-core/web
bun test src/components/shell/shellNavigation.test.ts src/components/shell/AppShell.test.ts
```

Expected: PASS.

- [ ] **Step 5: Commit Task 6**

Run:

```bash
git add crates/noema-core/web/src/components/shell/ShellSidebar.tsx crates/noema-core/web/src/components/shell/AppShell.tsx crates/noema-core/web/src/components/shell/AppShell.test.ts
git commit -m "feat: polish shell menu level accessibility"
```

Expected: commit succeeds.

## Task 7: Update Navigation Documentation And Durable Context

**Files:**
- Modify: `docs/frontend/navigation-workflows.md`
- Modify: `docs/context/current.md`

- [ ] **Step 1: Update frontend navigation docs**

In `docs/frontend/navigation-workflows.md`, replace the current shell behavior
sentence:

```md
Current shell behavior: the sidebar is a persistent navigation ground layer,
and route content sits above it as the active deck. Expanded desktop keeps the
ground-layer sidebar visible; collapsed desktop and mobile move the deck aside
to reveal the same single sidebar instance. Healthy setup, service, chat,
provider, and memory state are silent in the shell. The shell shows a single
compact attention item only when state is degraded or action-worthy; the active
page owns detailed recovery UI. A bottom-left cog opens Settings as a
temporary full-screen takeover rather than a primary navigation destination.
```

with:

```md
Current shell behavior: the sidebar is a persistent navigation ground layer,
and route content sits above it as the active deck. Expanded desktop keeps the
ground-layer sidebar visible; collapsed desktop and mobile move the deck aside
to reveal the same single sidebar instance. Healthy setup, service, chat,
provider, and memory state are silent in the shell. The shell shows a single
compact attention item only when state is degraded or action-worthy; the active
page owns detailed recovery UI. Settings is a bottom-anchored L0 menu item that
opens a route-derived L1 Settings submenu. Settings routes render inside the
main deck and remain deep-linkable.
```

- [ ] **Step 2: Update durable current context**

In `docs/context/current.md`, replace the current Settings settled-decision
bullet:

```md
- The web shell exposes Settings as a bottom-left sidebar cog, not as a primary
  navigation destination. `/settings` renders as a full-screen utility takeover
  after onboarding and defaults to `/settings/providers`. Settings now contains
  read-only Providers and Agents tabs: Providers shows non-secret provider
  account metadata, while Agents is backed by a dedicated GraphQL `agents` read
  model and shows safe registered-agent metadata such as agent ids. Agent
  management actions are not exposed yet.
```

with:

```md
- The web shell exposes Settings as a bottom-anchored L0 sidebar item that
  opens a route-derived L1 Settings submenu inside the same shell. `/settings`
  and `/settings/providers` default to Providers, while section routes such as
  `/settings/mcps` are deep-linkable and restore the Settings submenu after
  refresh. Settings contains Providers, Agents, MCPs, Trusted identities,
  Approvals, and Audit surfaces backed by existing GraphQL read models where
  live data exists. Agent management actions are not exposed yet.
```

- [ ] **Step 3: Run documentation diff check**

Run:

```bash
git diff --check docs/frontend/navigation-workflows.md docs/context/current.md
```

Expected: no output.

- [ ] **Step 4: Commit Task 7**

Run:

```bash
git add docs/frontend/navigation-workflows.md docs/context/current.md
git commit -m "docs: update settings shell navigation context"
```

Expected: commit succeeds.

## Task 8: Final Validation And Cleanup

**Files:**
- Modify only if validation reveals issues: files changed in Tasks 1-7.

- [ ] **Step 1: Run targeted frontend tests**

Run:

```bash
cd crates/noema-core/web
bun test src/routes.test.ts src/App.test.ts src/components/shell/shellNavigation.test.ts src/components/shell/AppShell.test.ts src/pages/SettingsPage.test.tsx
```

Expected: PASS.

- [ ] **Step 2: Run lint**

Run:

```bash
cd crates/noema-core/web
bun run lint
```

Expected: PASS.

- [ ] **Step 3: Run build**

Run:

```bash
cd crates/noema-core/web
bun run build
```

Expected: PASS.

- [ ] **Step 4: Run repository diff checks**

Run:

```bash
git status --short --branch
git diff --check
```

Expected: `git diff --check` prints no output. `git status --short --branch`
may still show unrelated dirty work that existed before this implementation.

- [ ] **Step 5: Inspect remaining changes**

Run:

```bash
git diff --stat
git diff --name-status
```

Expected: only route-derived shell settings implementation files remain
unstaged, plus any unrelated pre-existing dirty files that were preserved.

- [ ] **Step 6: Commit final fixes if any were needed**

If Step 1, 2, 3, or 4 required fixes after the Task 7 commit, stage only those
fixes and commit:

```bash
git add crates/noema-core/web/src/App.tsx crates/noema-core/web/src/App.test.ts crates/noema-core/web/src/routes.ts crates/noema-core/web/src/routes.test.ts crates/noema-core/web/src/components/shell/AppShell.tsx crates/noema-core/web/src/components/shell/AppShell.test.ts crates/noema-core/web/src/components/shell/ShellSidebar.tsx crates/noema-core/web/src/components/shell/shellNavigation.ts crates/noema-core/web/src/components/shell/shellNavigation.test.ts crates/noema-core/web/src/pages/SettingsPage.tsx crates/noema-core/web/src/pages/SettingsPage.test.tsx docs/frontend/navigation-workflows.md docs/context/current.md
git commit -m "chore: validate route-derived settings shell"
```

Expected: commit succeeds only if there were post-validation fixes. Skip this
commit when Tasks 1-7 already passed all validation.
