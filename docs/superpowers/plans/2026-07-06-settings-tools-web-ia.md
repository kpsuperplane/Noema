# Settings Tools Web IA Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move Noema Settings to grouped canonical IA and relocate Web fetch summarizer settings under `Settings > Tools > Web`.

**Architecture:** Keep Settings as direct pages, not group landing pages. Update route parsing/emission to canonical nested paths, make the shell Settings submenu data-driven with group labels, extract the model preference editor into neutral settings components, and add a Web settings page that owns static web capability status plus the fetch summarizer editor.

**Tech Stack:** React 19, TypeScript, StyleX, Astryx components, Apollo Client GraphQL codegen, Bun tests/lint.

---

## File Map

- Modify `crates/noema-core/web/src/app/routes.ts`
  - Own canonical Settings section ids and nested paths.
  - Make `/settings` resolve to Agents.
  - Stop recognizing old flat Settings paths.
- Add `crates/noema-core/web/src/app/routes.test.ts`
  - Unit tests for canonical paths, emitted paths, and old flat path fallthrough.
- Modify `crates/noema-core/web/src/components/shell/shellNavigation.ts`
  - Replace flat settings section list with grouped nav entries.
  - Make Settings bottom item route to Agents.
  - Keep breadcrumb and active item lookup aligned with new ids.
- Modify `crates/noema-core/web/src/components/shell/ShellSidebar.tsx`
  - Render non-clickable group labels inside menu item lists.
- Add `crates/noema-core/web/src/components/shell/shellNavigation.test.ts`
  - Unit tests for grouped settings nav shape and Settings bottom route.
- Add `crates/noema-core/web/src/components/settings/modelPreferenceTypes.ts`
  - Shared model preference/provider/profile types.
- Add `crates/noema-core/web/src/components/settings/modelPreferenceMetadata.ts`
  - Shared provider/model label and selected preference warning helpers.
- Add `crates/noema-core/web/src/components/settings/ModelPreferenceEditor.tsx`
  - Shared provider/model selector form extracted from Agents.
- Modify `crates/noema-core/web/src/components/settings/AgentsSettingsPane.tsx`
  - Remove Web fetch settings query/mutation.
- Modify `crates/noema-core/web/src/components/settings/AgentsSettingsPaneContent.tsx`
  - Remove Web fetch summarizer card and local model editor implementation.
  - Use shared model preference editor/types.
- Add `crates/noema-core/web/src/components/settings/WebSettingsPane.tsx`
  - Query/save Web fetch settings.
- Add `crates/noema-core/web/src/components/settings/WebSettingsPaneContent.tsx`
  - Render Search, Fetch, and nested Fetch summarizer UI.
- Modify `crates/noema-core/web/src/pages/SettingsPage.tsx`
  - Add Web pane and update section copy for canonical grouped sections.
- Modify `docs/context/current.md`
  - Update durable settings IA note after implementation lands.

---

## Task 1: Canonical Settings Routes

**Files:**
- Modify: `crates/noema-core/web/src/app/routes.ts`
- Create: `crates/noema-core/web/src/app/routes.test.ts`

- [ ] **Step 1: Write route tests**

Create `crates/noema-core/web/src/app/routes.test.ts`:

```ts
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
```

- [ ] **Step 2: Run route tests to verify they fail**

Run:

```bash
cd crates/noema-core/web && bun test src/app/routes.test.ts
```

Expected: FAIL. Current code still maps `/settings` to `providers`, does not recognize nested paths, and still recognizes old flat paths.

- [ ] **Step 3: Implement canonical route ids and paths**

Modify the top of `crates/noema-core/web/src/app/routes.ts`:

```ts
export type SettingsSection =
  | "agents"
  | "tools-web"
  | "tools-mcps"
  | "safety-approvals"
  | "safety-identities"
  | "system-providers";
```

Replace the Settings branch in `routeFromPathname`:

```ts
  if (pathname === "/settings" || pathname === "/settings/agents") {
    return { kind: "settings", section: "agents" };
  }
  if (pathname === "/settings/tools/web") {
    return { kind: "settings", section: "tools-web" };
  }
  if (pathname === "/settings/tools/mcps") {
    return { kind: "settings", section: "tools-mcps" };
  }
  if (pathname === "/settings/safety/approvals") {
    return { kind: "settings", section: "safety-approvals" };
  }
  if (pathname === "/settings/safety/identities") {
    return { kind: "settings", section: "safety-identities" };
  }
  if (pathname === "/settings/system/providers") {
    return { kind: "settings", section: "system-providers" };
  }
```

Replace the Settings branch in `pathForRoute`:

```ts
  if (route.kind === "settings") {
    switch (route.section) {
      case "agents":
        return "/settings/agents";
      case "tools-web":
        return "/settings/tools/web";
      case "tools-mcps":
        return "/settings/tools/mcps";
      case "safety-approvals":
        return "/settings/safety/approvals";
      case "safety-identities":
        return "/settings/safety/identities";
      case "system-providers":
        return "/settings/system/providers";
    }
  }
```

- [ ] **Step 4: Run route tests to verify they pass**

Run:

```bash
cd crates/noema-core/web && bun test src/app/routes.test.ts
```

Expected: PASS.

- [ ] **Step 5: Commit routes**

Run:

```bash
git add crates/noema-core/web/src/app/routes.ts crates/noema-core/web/src/app/routes.test.ts
git commit -m "feat(settings): use canonical grouped routes"
```

Expected: commit succeeds. Leave unrelated `crates/noema-core/web/src/components/shell/AppShell.tsx` unstaged if it is still dirty.

---

## Task 2: Grouped Settings Sidebar Navigation

**Files:**
- Modify: `crates/noema-core/web/src/components/shell/shellNavigation.ts`
- Modify: `crates/noema-core/web/src/components/shell/ShellSidebar.tsx`
- Create: `crates/noema-core/web/src/components/shell/shellNavigation.test.ts`

- [ ] **Step 1: Write shell navigation tests**

Create `crates/noema-core/web/src/components/shell/shellNavigation.test.ts`:

```ts
import { describe, expect, test } from "bun:test";
import {
  shellMenuLevelForRoute,
  shellSettingsEntries,
  settingsItemIdForSection
} from "./shellNavigation";

describe("settings shell navigation", () => {
  test("settings entries are grouped with agents top-level and live pages only", () => {
    expect(shellSettingsEntries.map((entry) => entry.kind === "group" ? entry.label : entry.item.label)).toEqual([
      "Agents",
      "Tools",
      "Web",
      "MCPs",
      "Safety",
      "Approvals",
      "Identities",
      "System",
      "Providers"
    ]);
  });

  test("settings bottom item opens agents", () => {
    const level = shellMenuLevelForRoute({ kind: "chat" });

    expect(level.bottomItem.route).toEqual({
      kind: "settings",
      section: "agents"
    });
  });

  test("settings level contains group labels and item routes", () => {
    const level = shellMenuLevelForRoute({
      kind: "settings",
      section: "tools-web"
    });

    expect(level.activeItemId).toBe("settings.tools.web");
    expect(level.items).toEqual([
      {
        kind: "item",
        item: expect.objectContaining({
          itemId: "settings.agents",
          label: "Agents",
          route: { kind: "settings", section: "agents" }
        })
      },
      { kind: "group", label: "Tools" },
      {
        kind: "item",
        item: expect.objectContaining({
          itemId: "settings.tools.web",
          label: "Web",
          route: { kind: "settings", section: "tools-web" }
        })
      },
      {
        kind: "item",
        item: expect.objectContaining({
          itemId: "settings.tools.mcps",
          label: "MCPs",
          route: { kind: "settings", section: "tools-mcps" }
        })
      },
      { kind: "group", label: "Safety" },
      {
        kind: "item",
        item: expect.objectContaining({
          itemId: "settings.safety.approvals",
          label: "Approvals",
          route: { kind: "settings", section: "safety-approvals" }
        })
      },
      {
        kind: "item",
        item: expect.objectContaining({
          itemId: "settings.safety.identities",
          label: "Identities",
          route: { kind: "settings", section: "safety-identities" }
        })
      },
      { kind: "group", label: "System" },
      {
        kind: "item",
        item: expect.objectContaining({
          itemId: "settings.system.providers",
          label: "Providers",
          route: { kind: "settings", section: "system-providers" }
        })
      }
    ]);
  });

  test("settingsItemIdForSection maps canonical section ids", () => {
    expect(settingsItemIdForSection("agents")).toBe("settings.agents");
    expect(settingsItemIdForSection("tools-web")).toBe("settings.tools.web");
    expect(settingsItemIdForSection("tools-mcps")).toBe("settings.tools.mcps");
    expect(settingsItemIdForSection("safety-approvals")).toBe("settings.safety.approvals");
    expect(settingsItemIdForSection("safety-identities")).toBe("settings.safety.identities");
    expect(settingsItemIdForSection("system-providers")).toBe("settings.system.providers");
  });
});
```

- [ ] **Step 2: Run shell navigation tests to verify they fail**

Run:

```bash
cd crates/noema-core/web && bun test src/components/shell/shellNavigation.test.ts
```

Expected: FAIL because `shellSettingsEntries` and grouped item shape do not exist yet.

- [ ] **Step 3: Update shell navigation model**

Modify `crates/noema-core/web/src/components/shell/shellNavigation.ts`.

Update icon imports:

```ts
import {
  ArrowLeft,
  Bot,
  Brain,
  CheckSquare,
  Fingerprint,
  Globe,
  House,
  PlugZap,
  ServerCog,
  Settings
} from "lucide-react";
```

Replace `ShellMenuItemId` settings ids:

```ts
export type ShellMenuItemId =
  | "home"
  | "memory"
  | "settings"
  | "settings.agents"
  | "settings.tools.web"
  | "settings.tools.mcps"
  | "settings.safety.approvals"
  | "settings.safety.identities"
  | "settings.system.providers"
  | "settings.go-back";
```

Add grouped entry types:

```ts
export type ShellMenuGroupLabel = {
  kind: "group";
  label: string;
};

export type ShellMenuItemEntry = {
  kind: "item";
  item: ShellMenuItem;
};

export type ShellMenuEntry = ShellMenuGroupLabel | ShellMenuItemEntry;
```

Change `ShellMenuLevel.items`:

```ts
  items: ShellMenuEntry[];
```

Replace `ShellSettingsSection` and `shellSettingsSections`:

```ts
export type ShellSettingsSection = {
  section: SettingsSection;
  itemId: Extract<ShellMenuItemId, `settings.${string}`>;
  label: string;
  icon: LucideIcon;
};

export type ShellSettingsEntry =
  | { kind: "section"; item: ShellSettingsSection }
  | { kind: "group"; label: string };

export const shellSettingsEntries: ShellSettingsEntry[] = [
  { kind: "section", item: { section: "agents", itemId: "settings.agents", label: "Agents", icon: Bot } },
  { kind: "group", label: "Tools" },
  { kind: "section", item: { section: "tools-web", itemId: "settings.tools.web", label: "Web", icon: Globe } },
  { kind: "section", item: { section: "tools-mcps", itemId: "settings.tools.mcps", label: "MCPs", icon: PlugZap } },
  { kind: "group", label: "Safety" },
  { kind: "section", item: { section: "safety-approvals", itemId: "settings.safety.approvals", label: "Approvals", icon: CheckSquare } },
  { kind: "section", item: { section: "safety-identities", itemId: "settings.safety.identities", label: "Identities", icon: Fingerprint } },
  { kind: "group", label: "System" },
  { kind: "section", item: { section: "system-providers", itemId: "settings.system.providers", label: "Providers", icon: ServerCog } }
];

export const shellSettingsSections = shellSettingsEntries.flatMap((entry) =>
  entry.kind === "section" ? [entry.item] : []
);
```

Replace `settingsItemIdForSection`:

```ts
export function settingsItemIdForSection(section: SettingsSection): ShellMenuItemId {
  return (
    shellSettingsSections.find((candidate) => candidate.section === section)?.itemId ??
    "settings.agents"
  );
}
```

Replace the Settings `items` construction in `shellMenuLevelForRoute`:

```ts
      items: shellSettingsEntries.map((entry) => {
        if (entry.kind === "group") {
          return { kind: "group", label: entry.label };
        }
        return {
          kind: "item",
          item: {
            itemId: entry.item.itemId,
            label: entry.item.label,
            route: { kind: "settings", section: entry.item.section },
            action: "navigate",
            icon: entry.item.icon
          }
        };
      }),
```

Change the L0 Settings bottom route:

```ts
      route: { kind: "settings", section: "agents" },
```

- [ ] **Step 4: Render group labels in ShellSidebar**

Modify `crates/noema-core/web/src/components/shell/ShellSidebar.tsx`.

Change the imports:

```ts
import type { ShellAttention } from "./AppShell";
import { ShellAttentionItem } from "./ShellAttentionItem";
import type { ShellMenuEntry, ShellMenuItem, ShellMenuLevel, ShellMenuLevelId } from "./shellNavigation";
```

Replace the `menuLevel.items.map` block:

```tsx
        {menuLevel.items.map((entry) =>
          entry.kind === "group" ? (
            <ShellSidebarGroupLabel key={`group-${entry.label}`} label={entry.label} />
          ) : (
            <ShellSidebarNavItem
              key={entry.item.itemId}
              item={entry.item}
              active={entry.item.itemId === menuLevel.activeItemId}
              primaryAgentNamed={primaryAgentNamed}
              primaryAgentLabel={primaryAgentLabel}
              interactive={interactive}
              onSelectItem={onSelectItem}
            />
          )
        )}
```

Add this component before `ShellSidebarNavItem`:

```tsx
function ShellSidebarGroupLabel({ label }: { label: string }) {
  return (
    <div
      data-slot="shell-menu-group-label"
      {...stylex.props(styles.groupLabel)}
    >
      {label}
    </div>
  );
}
```

Add a StyleX style:

```ts
  groupLabel: {
    paddingBlock: 10,
    paddingInline: 10,
    fontSize: 11,
    fontWeight: 600,
    lineHeight: 1.2,
    letterSpacing: 0,
    color: "var(--muted-foreground)",
    textTransform: "uppercase"
  },
```

- [ ] **Step 5: Run shell navigation tests**

Run:

```bash
cd crates/noema-core/web && bun test src/components/shell/shellNavigation.test.ts
```

Expected: PASS.

- [ ] **Step 6: Run TypeScript check**

Run:

```bash
cd crates/noema-core/web && bun run lint
```

Expected: PASS. If TypeScript reports formatting or union-type errors, fix only the changed nav/sidebar files.

- [ ] **Step 7: Commit grouped sidebar**

Run:

```bash
git add crates/noema-core/web/src/components/shell/shellNavigation.ts crates/noema-core/web/src/components/shell/ShellSidebar.tsx crates/noema-core/web/src/components/shell/shellNavigation.test.ts
git commit -m "feat(settings): group settings navigation"
```

Expected: commit succeeds.

---

## Task 3: Extract Shared Model Preference Editor

**Files:**
- Create: `crates/noema-core/web/src/components/settings/modelPreferenceTypes.ts`
- Create: `crates/noema-core/web/src/components/settings/modelPreferenceMetadata.ts`
- Create: `crates/noema-core/web/src/components/settings/ModelPreferenceEditor.tsx`
- Modify: `crates/noema-core/web/src/components/settings/AgentsSettingsPaneContent.tsx`

- [ ] **Step 1: Create shared model preference types**

Create `crates/noema-core/web/src/components/settings/modelPreferenceTypes.ts`:

```ts
export type ModelProfileOption = {
  id: string;
  label: string;
  disabledReason?: string | null;
};

export type ModelProviderOption = {
  providerKind: string;
  providerAccountId: string;
  providerDisplayName: string;
  status: string;
  disabledReason?: string | null;
  profiles: readonly ModelProfileOption[];
};

export type ModelPreference = {
  providerKind: string;
  providerAccountId: string;
  modelProfile: string;
};

export type ModelPreferenceSaveInput = {
  providerAccountId: string;
  modelProfile: string;
};
```

- [ ] **Step 2: Create shared model preference metadata helpers**

Create `crates/noema-core/web/src/components/settings/modelPreferenceMetadata.ts`:

```ts
import type { ModelPreference, ModelProviderOption } from "./modelPreferenceTypes";

export function providerPreferenceLabel(
  preference: ModelPreference,
  options: readonly ModelProviderOption[]
) {
  const provider = options.find(
    (option) => option.providerAccountId === preference.providerAccountId
  );
  return provider?.providerDisplayName ?? preference.providerKind;
}

export function modelPreferenceLabel(
  preference: ModelPreference,
  options: readonly ModelProviderOption[]
) {
  const provider = options.find(
    (option) => option.providerAccountId === preference.providerAccountId
  );
  const profile = provider?.profiles.find((candidate) => candidate.id === preference.modelProfile);
  return profile?.label ?? preference.modelProfile;
}

export function modelProfileLabel(
  modelProfile: string,
  options: readonly ModelProviderOption[]
) {
  for (const option of options) {
    const profile = option.profiles.find((candidate) => candidate.id === modelProfile);
    if (profile) {
      return profile.label;
    }
  }
  return modelProfile;
}

export function selectedPreferenceWarning(
  preference: ModelPreference | null,
  options: readonly ModelProviderOption[]
) {
  if (!preference) {
    return null;
  }
  const provider = options.find(
    (option) => option.providerAccountId === preference.providerAccountId
  );
  const profile = provider?.profiles.find((candidate) => candidate.id === preference.modelProfile);
  if (!provider) {
    return "Selected provider is not available.";
  }
  if (!profile) {
    return "Selected model is not available.";
  }
  return profile.disabledReason ?? provider.disabledReason ?? null;
}
```

- [ ] **Step 3: Extract the editor component**

Create `crates/noema-core/web/src/components/settings/ModelPreferenceEditor.tsx` by moving the existing `ModelPreferenceEditor`, `ModelPreferenceEditorFields`, `modelSelectionSeed`, `resolveInitialModelSelection`, and `defaultProfileForProvider` code out of `AgentsSettingsPaneContent.tsx`.

Use these imports:

```tsx
import { useMemo, useState } from "react";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import type {
  ModelPreference,
  ModelPreferenceSaveInput,
  ModelProviderOption
} from "./modelPreferenceTypes";
```

Export the component:

```tsx
export function ModelPreferenceEditor({
  options,
  preference,
  defaultModelProfile,
  saving,
  onCancel,
  onSave
}: {
  options: readonly ModelProviderOption[];
  preference?: ModelPreference | null;
  defaultModelProfile?: string;
  saving: boolean;
  onCancel: () => void;
  onSave: (input: ModelPreferenceSaveInput) => Promise<unknown>;
}) {
  const providerOptions = useMemo(() => options, [options]);
  const selectionSeed = useMemo(
    () => modelSelectionSeed(providerOptions, preference ?? null, defaultModelProfile),
    [providerOptions, preference, defaultModelProfile]
  );
  const initialSelection = useMemo(
    () => resolveInitialModelSelection(providerOptions, preference ?? null, defaultModelProfile),
    [providerOptions, preference, defaultModelProfile]
  );

  return (
    <ModelPreferenceEditorFields
      key={selectionSeed}
      providerOptions={providerOptions}
      initialSelection={initialSelection}
      defaultModelProfile={defaultModelProfile}
      saving={saving}
      onCancel={onCancel}
      onSave={onSave}
    />
  );
}
```

Copy the existing editor styles from `AgentsSettingsPaneContent.tsx` into this new file:

```ts
const styles = stylex.create({
  editor: {
    display: "grid",
    gap: 12,
    borderTopWidth: 1,
    borderTopStyle: "solid",
    borderTopColor: "var(--border-subtle)",
    paddingTop: 12
  },
  editorGrid: {
    display: "grid",
    gridTemplateColumns: "repeat(2, minmax(0, 1fr))",
    gap: 12,
    "@media (max-width: 640px)": {
      gridTemplateColumns: "1fr"
    }
  },
  field: {
    display: "grid",
    gap: 6
  },
  fieldLabel: {
    fontSize: 12,
    fontWeight: 600,
    lineHeight: 1.3,
    color: "var(--muted-foreground)"
  },
  select: {
    minHeight: 36,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border)",
    borderRadius: 6,
    backgroundColor: "white",
    paddingInline: 10,
    fontSize: 14,
    color: "var(--foreground)"
  },
  warningList: {
    marginBlock: 0,
    paddingInlineStart: 18,
    fontSize: 13,
    lineHeight: 1.5,
    color: "var(--muted-foreground)"
  },
  actions: {
    display: "flex",
    justifyContent: "flex-end",
    gap: 8
  }
});
```

- [ ] **Step 4: Update Agents content imports and props**

Modify `crates/noema-core/web/src/components/settings/AgentsSettingsPaneContent.tsx`.

Change imports:

```tsx
import { useState } from "react";
import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { Settings2 } from "lucide-react";
import { ModelPreferenceEditor } from "./ModelPreferenceEditor";
import type {
  ModelPreference,
  ModelPreferenceSaveInput,
  ModelProviderOption
} from "./modelPreferenceTypes";
import {
  agentBadgeLabel,
  agentDisplayName,
  agentMetadataRows,
  selectedModelWarning
} from "./agentMetadata";
```

Delete local `AgentModelProfileOption`, `AgentModelProviderOption`, `ModelPreference`, and `ModelPreferenceSaveInput` types. Change agent options to use `ModelProviderOption`:

```ts
  modelOptions?: readonly ModelProviderOption[];
```

Delete `WebFetchSummarizerSettings`, `WebFetchSummarizerCard`, `webFetchSummarizerRows`, `providerPreferenceLabel`, `modelPreferenceLabel`, `modelProfileLabel`, `selectedPreferenceWarning`, and the local model editor functions.

Change component props to:

```ts
export function AgentsSettingsPaneContent({
  agents,
  loading,
  error,
  saving,
  saveError,
  onSaveModelPreference
}: {
  agents: readonly AgentSettingsAgent[];
  loading: boolean;
  error: string | null;
  saving: boolean;
  saveError: string | null;
  onSaveModelPreference: (input: SaveAgentModelPreferenceInput) => Promise<unknown>;
}) {
```

Delete `editingWebFetch` state and the `<WebFetchSummarizerCard ... />` render block.

Remove editor-only styles from `AgentsSettingsPaneContent.tsx`: `editor`, `editorGrid`, `field`, `fieldLabel`, `select`, `warningList`, and `actions`.

- [ ] **Step 5: Run TypeScript check**

Run:

```bash
cd crates/noema-core/web && bun run lint
```

Expected: PASS. If TypeScript fails because a moved helper is missing, move the helper exactly once into the shared file rather than re-adding duplicates.

- [ ] **Step 6: Commit editor extraction**

Run:

```bash
git add crates/noema-core/web/src/components/settings/modelPreferenceTypes.ts crates/noema-core/web/src/components/settings/modelPreferenceMetadata.ts crates/noema-core/web/src/components/settings/ModelPreferenceEditor.tsx crates/noema-core/web/src/components/settings/AgentsSettingsPaneContent.tsx
git commit -m "refactor(settings): share model preference editor"
```

Expected: commit succeeds.

---

## Task 4: Move Web Fetch Summarizer Out Of Agents

**Files:**
- Modify: `crates/noema-core/web/src/components/settings/AgentsSettingsPane.tsx`
- Create: `crates/noema-core/web/src/components/settings/WebSettingsPane.tsx`
- Create: `crates/noema-core/web/src/components/settings/WebSettingsPaneContent.tsx`
- Modify: `crates/noema-core/web/src/pages/SettingsPage.tsx`

- [ ] **Step 1: Remove Web fetch query/mutation from Agents pane**

Modify `crates/noema-core/web/src/components/settings/AgentsSettingsPane.tsx`.

Use these imports:

```tsx
import { useMutation, useQuery } from "@apollo/client/react";
import {
  AgentsDocument,
  SaveAgentModelPreferenceDocument,
  type AgentsQuery,
  type SaveAgentModelPreferenceMutation,
  type SaveAgentModelPreferenceMutationVariables
} from "@/generated/graphql";
import { AgentsSettingsPaneContent } from "./AgentsSettingsPaneContent";
```

The component should query only agents:

```tsx
export function AgentsSettingsPane() {
  const agentsResult = useQuery<AgentsQuery>(AgentsDocument, { fetchPolicy: "cache-and-network" });
  const [savePreference, saveResult] = useMutation<
    SaveAgentModelPreferenceMutation,
    SaveAgentModelPreferenceMutationVariables
  >(SaveAgentModelPreferenceDocument, {
    refetchQueries: [{ query: AgentsDocument }],
    awaitRefetchQueries: true
  });

  return (
    <AgentsSettingsPaneContent
      agents={agentsResult.data?.agents ?? []}
      loading={agentsResult.loading && !agentsResult.data}
      error={agentsResult.error?.message ?? null}
      saving={saveResult.loading}
      saveError={saveResult.error?.message ?? null}
      onSaveModelPreference={(input) => savePreference({ variables: { input } })}
    />
  );
}
```

- [ ] **Step 2: Create Web settings pane data owner**

Create `crates/noema-core/web/src/components/settings/WebSettingsPane.tsx`:

```tsx
import { useMutation, useQuery } from "@apollo/client/react";
import {
  SaveWebFetchSummarizerPreferenceDocument,
  WebFetchSettingsDocument,
  type SaveWebFetchSummarizerPreferenceMutation,
  type SaveWebFetchSummarizerPreferenceMutationVariables,
  type WebFetchSettingsQuery
} from "@/generated/graphql";
import { WebSettingsPaneContent } from "./WebSettingsPaneContent";

export { WebSettingsPaneContent } from "./WebSettingsPaneContent";

export function WebSettingsPane() {
  const webFetchResult = useQuery<WebFetchSettingsQuery>(WebFetchSettingsDocument, {
    fetchPolicy: "cache-and-network"
  });
  const [saveWebFetchSummarizerPreference, saveWebFetchResult] = useMutation<
    SaveWebFetchSummarizerPreferenceMutation,
    SaveWebFetchSummarizerPreferenceMutationVariables
  >(SaveWebFetchSummarizerPreferenceDocument, {
    refetchQueries: [{ query: WebFetchSettingsDocument }],
    awaitRefetchQueries: true
  });

  return (
    <WebSettingsPaneContent
      webFetchSummarizer={webFetchResult.data?.webFetchSettings.summarizer ?? null}
      loading={webFetchResult.loading && !webFetchResult.data}
      error={webFetchResult.error?.message ?? null}
      saving={saveWebFetchResult.loading}
      saveError={saveWebFetchResult.error?.message ?? null}
      onSaveWebFetchSummarizerPreference={(input) =>
        saveWebFetchSummarizerPreference({ variables: { input } })
      }
    />
  );
}
```

- [ ] **Step 3: Create Web settings content**

Create `crates/noema-core/web/src/components/settings/WebSettingsPaneContent.tsx`:

```tsx
import { useState } from "react";
import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { Settings2 } from "lucide-react";
import { ModelPreferenceEditor } from "./ModelPreferenceEditor";
import {
  modelPreferenceLabel,
  modelProfileLabel,
  providerPreferenceLabel,
  selectedPreferenceWarning
} from "./modelPreferenceMetadata";
import type {
  ModelPreference,
  ModelPreferenceSaveInput,
  ModelProviderOption
} from "./modelPreferenceTypes";

export type WebFetchSummarizerSettings = {
  defaultModelProfile: string;
  modelPreference?: ModelPreference | null;
  modelOptions: readonly ModelProviderOption[];
};

export function WebSettingsPaneContent({
  webFetchSummarizer,
  loading,
  error,
  saving,
  saveError,
  onSaveWebFetchSummarizerPreference
}: {
  webFetchSummarizer: WebFetchSummarizerSettings | null;
  loading: boolean;
  error: string | null;
  saving: boolean;
  saveError: string | null;
  onSaveWebFetchSummarizerPreference: (input: ModelPreferenceSaveInput) => Promise<unknown>;
}) {
  const [editingFetchSummarizer, setEditingFetchSummarizer] = useState(false);

  return (
    <div {...stylex.props(styles.stack)}>
      <section {...stylex.props(styles.card)} aria-labelledby="web-search-settings-title">
        <div {...stylex.props(styles.cardHeader)}>
          <div {...stylex.props(styles.titleRow)}>
            <h2 id="web-search-settings-title" {...stylex.props(styles.cardTitle)}>Search</h2>
            <Badge variant="neutral" label="Enabled" />
          </div>
        </div>
        <dl {...stylex.props(styles.definitionList)}>
          <MetadataRow label="Tool" value="web.search" />
          <MetadataRow label="Provider" value="DuckDuckGo public adapter" />
          <MetadataRow label="Contract" value="Search results only; pages are not fetched or read." />
        </dl>
      </section>

      <section {...stylex.props(styles.card)} aria-labelledby="web-fetch-settings-title">
        <div {...stylex.props(styles.cardHeader)}>
          <div {...stylex.props(styles.titleRow)}>
            <h2 id="web-fetch-settings-title" {...stylex.props(styles.cardTitle)}>Fetch</h2>
            <Badge variant="neutral" label="Enabled" />
          </div>
        </div>
        <dl {...stylex.props(styles.definitionList)}>
          <MetadataRow label="Tool" value="web.fetch" />
          <MetadataRow label="Provider" value="Direct HTTP" />
          <MetadataRow label="Extraction" value="readabilityrs markdown" />
          <MetadataRow
            label="Safety"
            value="Public HTTP(S), checked redirects, private/local targets blocked, response size caps."
          />
        </dl>
        <FetchSummarizerCard
          settings={webFetchSummarizer}
          loading={loading}
          error={error}
          saveError={saveError}
          editing={editingFetchSummarizer}
          saving={saving}
          onToggleEditing={() => setEditingFetchSummarizer((current) => !current)}
          onCancel={() => setEditingFetchSummarizer(false)}
          onSave={async (input) => {
            await onSaveWebFetchSummarizerPreference(input);
            setEditingFetchSummarizer(false);
          }}
        />
      </section>
    </div>
  );
}

function FetchSummarizerCard({
  settings,
  loading,
  error,
  saveError,
  editing,
  saving,
  onToggleEditing,
  onCancel,
  onSave
}: {
  settings: WebFetchSummarizerSettings | null;
  loading: boolean;
  error: string | null;
  saveError: string | null;
  editing: boolean;
  saving: boolean;
  onToggleEditing: () => void;
  onCancel: () => void;
  onSave: (input: ModelPreferenceSaveInput) => Promise<unknown>;
}) {
  const preference = settings?.modelPreference ?? null;
  const rows = webFetchSummarizerRows(settings);
  const warning = settings ? selectedPreferenceWarning(preference, settings.modelOptions) : null;
  const unavailable = Boolean(error) || !settings;

  return (
    <div {...stylex.props(styles.subcard)}>
      <div {...stylex.props(styles.cardHeader)}>
        <div {...stylex.props(styles.titleRow)}>
          <h3 {...stylex.props(styles.subcardTitle)}>Fetch summarizer</h3>
          {!preference && settings ? <Badge variant="neutral" label="Default" /> : null}
        </div>
        <Button
          type="button"
          variant="secondary"
          size="sm"
          label="Model"
          icon={<Settings2 {...stylex.props(styles.icon)} aria-hidden="true" />}
          aria-label={`${editing ? "Close" : "Edit"} model settings for fetch summarizer`}
          aria-expanded={editing}
          onClick={onToggleEditing}
          isDisabled={unavailable}
        />
      </div>
      {loading ? (
        <p {...stylex.props(styles.mutedText)}>Loading fetch summarizer settings...</p>
      ) : error ? (
        <p {...stylex.props(styles.mutedText)}>Fetch summarizer settings could not be loaded.</p>
      ) : (
        <>
          {saveError ? (
            <p {...stylex.props(styles.saveError)}>Noema could not save the fetch summarizer model.</p>
          ) : null}
          <dl {...stylex.props(styles.definitionList)}>
            {rows.map((row) => (
              <MetadataRow key={row.label} label={row.label} value={row.value} />
            ))}
          </dl>
          {warning ? <p {...stylex.props(styles.warningText)}>{warning}</p> : null}
          {editing && settings ? (
            <ModelPreferenceEditor
              options={settings.modelOptions}
              preference={preference}
              defaultModelProfile={settings.defaultModelProfile}
              saving={saving}
              onCancel={onCancel}
              onSave={onSave}
            />
          ) : null}
        </>
      )}
    </div>
  );
}

function MetadataRow({ label, value }: { label: string; value: string }) {
  return (
    <div {...stylex.props(styles.definitionRow)}>
      <dt {...stylex.props(styles.definitionTerm)}>{label}</dt>
      <dd {...stylex.props(styles.definitionValue)}>{value}</dd>
    </div>
  );
}

function webFetchSummarizerRows(settings: WebFetchSummarizerSettings | null) {
  if (!settings) {
    return [{ label: "Model", value: "Unavailable" }];
  }
  const preference = settings.modelPreference ?? null;
  if (!preference) {
    return [
      { label: "Provider", value: "Default provider" },
      {
        label: "Model",
        value: `${modelProfileLabel(settings.defaultModelProfile, settings.modelOptions)} (default)`
      }
    ];
  }
  return [
    { label: "Provider", value: providerPreferenceLabel(preference, settings.modelOptions) },
    { label: "Model", value: modelPreferenceLabel(preference, settings.modelOptions) }
  ];
}

const styles = stylex.create({
  stack: {
    display: "grid",
    gap: 12
  },
  card: {
    display: "grid",
    gap: 12,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    borderRadius: 6,
    backgroundColor: "white",
    padding: 16
  },
  subcard: {
    display: "grid",
    gap: 12,
    borderTopWidth: 1,
    borderTopStyle: "solid",
    borderTopColor: "var(--border-subtle)",
    paddingTop: 12
  },
  cardHeader: {
    display: "flex",
    flexWrap: "wrap",
    alignItems: "center",
    justifyContent: "space-between",
    gap: 12
  },
  titleRow: {
    display: "flex",
    flexWrap: "wrap",
    alignItems: "center",
    gap: 12
  },
  cardTitle: {
    margin: 0,
    fontFamily: "var(--font-heading)",
    fontSize: 20,
    lineHeight: 1.25,
    letterSpacing: 0,
    color: "var(--foreground)"
  },
  subcardTitle: {
    margin: 0,
    fontFamily: "var(--font-heading)",
    fontSize: 16,
    lineHeight: 1.3,
    letterSpacing: 0,
    color: "var(--foreground)"
  },
  definitionList: {
    display: "grid",
    gap: 8,
    margin: 0
  },
  definitionRow: {
    display: "grid",
    gridTemplateColumns: "minmax(110px, 0.35fr) minmax(0, 1fr)",
    gap: 12,
    "@media (max-width: 560px)": {
      gridTemplateColumns: "1fr",
      gap: 2
    }
  },
  definitionTerm: {
    color: "var(--muted-foreground)",
    fontSize: 13,
    lineHeight: 1.45
  },
  definitionValue: {
    margin: 0,
    color: "var(--foreground)",
    fontSize: 13,
    lineHeight: 1.45
  },
  mutedText: {
    margin: 0,
    fontSize: 14,
    lineHeight: 1.5,
    color: "var(--muted-foreground)"
  },
  saveError: {
    margin: 0,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "color-mix(in srgb, var(--destructive) 30%, transparent)",
    borderRadius: 6,
    backgroundColor: "color-mix(in srgb, var(--destructive) 5%, transparent)",
    padding: 12,
    fontSize: 14,
    lineHeight: 1.5,
    color: "var(--destructive)"
  },
  warningText: {
    margin: 0,
    fontSize: 13,
    lineHeight: 1.45,
    color: "var(--muted-foreground)"
  },
  icon: {
    width: 14,
    height: 14
  }
});
```

- [ ] **Step 4: Wire Web pane into SettingsPage**

Modify `crates/noema-core/web/src/pages/SettingsPage.tsx`.

Add the import:

```ts
import { WebSettingsPane } from "@/components/settings/WebSettingsPane";
```

Replace `settingsSectionCopy`:

```ts
const settingsSectionCopy: Record<SettingsSection, { title: string; description: string }> = {
  agents: {
    title: "Agents",
    description: "Review the agents currently registered in Noema and choose their runtime models."
  },
  "tools-web": {
    title: "Web",
    description: "Review first-party web search and fetch behavior."
  },
  "tools-mcps": {
    title: "MCPs",
    description: "Review third-party MCP servers mediated by the Noema capability gateway."
  },
  "safety-approvals": {
    title: "Approvals",
    description: "Review pending MCP approval checkpoints."
  },
  "safety-identities": {
    title: "Identities",
    description: "Review identity selectors used to resolve tool-result ownership."
  },
  "system-providers": {
    title: "Providers",
    description:
      "Review the provider account Noema uses for chat. Secret credential material stays outside the UI."
  }
};
```

Replace `SettingsSectionPane`:

```tsx
function SettingsSectionPane({ section }: { section: SettingsSection }) {
  switch (section) {
    case "agents":
      return <AgentsSettingsPane />;
    case "tools-web":
      return <WebSettingsPane />;
    case "tools-mcps":
      return <McpSettingsPane />;
    case "safety-approvals":
      return <ApprovalsSettingsPane />;
    case "safety-identities":
      return <TrustedIdentitiesSettingsPane />;
    case "system-providers":
      return <ProvidersSettingsPane />;
  }
}
```

- [ ] **Step 5: Run targeted and full web validation**

Run:

```bash
cd crates/noema-core/web && bun test src/app/routes.test.ts src/components/shell/shellNavigation.test.ts
cd crates/noema-core/web && bun run lint
```

Expected: tests PASS, lint PASS.

- [ ] **Step 6: Commit Web page move**

Run:

```bash
git add crates/noema-core/web/src/components/settings/AgentsSettingsPane.tsx crates/noema-core/web/src/components/settings/WebSettingsPane.tsx crates/noema-core/web/src/components/settings/WebSettingsPaneContent.tsx crates/noema-core/web/src/pages/SettingsPage.tsx
git commit -m "feat(settings): add web tools settings page"
```

Expected: commit succeeds.

---

## Task 5: Context Docs And Final Validation

**Files:**
- Modify: `docs/context/current.md`

- [ ] **Step 1: Update durable project context**

Modify the existing Settings IA bullet in `docs/context/current.md` so it reflects the new grouped IA. Replace the paragraph that currently says Settings contains Providers, Agents, MCPs, Trusted identities, and Approvals with:

```md
- The web shell exposes Settings as a bottom-anchored L0 sidebar item that
  opens a route-derived L1 Settings submenu inside the same shell. `/settings`
  and `/settings/agents` default to Agents. Settings now uses grouped live
  sections: Agents; Tools with Web and MCPs; Safety with Approvals and
  Identities; and System with Providers. The Web page owns first-party
  `web.search` and `web.fetch` status plus the fetch summarizer model
  preference. Settings routes are canonical nested paths such as
  `/settings/tools/web`, `/settings/safety/approvals`, and
  `/settings/system/providers`; old flat settings paths are not supported.
```

Keep nearby details about route-derived Settings, shell deck behavior, and frontend route ownership intact.

- [ ] **Step 2: Run ship checklist validation**

Run:

```bash
git status --short --branch
git diff --check
cd crates/noema-core/web && bun test src/app/routes.test.ts src/components/shell/shellNavigation.test.ts
cd crates/noema-core/web && bun run lint
```

Expected:

- `git diff --check` exits 0.
- Bun route/navigation tests pass.
- `bun run lint` exits 0.
- `git status --short --branch` shows only intentional files plus any pre-existing unrelated `crates/noema-core/web/src/components/shell/AppShell.tsx` change.

- [ ] **Step 3: Inspect and commit docs**

Run:

```bash
git add docs/context/current.md
git diff --cached --stat
git diff --cached --name-status
git commit -m "docs: update settings ia context"
```

Expected: docs-only commit succeeds.

- [ ] **Step 4: Final status**

Run:

```bash
git status --short --branch
```

Expected: branch is ahead by the task commits. Any unrelated pre-existing `crates/noema-core/web/src/components/shell/AppShell.tsx` change remains unstaged and is reported to the user.

---

## Self-Review

- Spec coverage: Tasks cover canonical nested routes, no old aliases, grouped live settings nav with Agents top-level and no future categories, `Settings > Tools > Web`, Search/Fetch sections, Fetch-owned summarizer editor, Agents cleanup, shared model preference UI, local errors, and validation.
- Placeholder scan: This plan intentionally contains no TBD/TODO/fill-in-later steps. Every code-changing step names exact files and concrete code or replacement structure.
- Type consistency: The plan consistently uses `SettingsSection` ids `agents`, `tools-web`, `tools-mcps`, `safety-approvals`, `safety-identities`, and `system-providers`; shell item ids map to those ids; Web summarizer props use the shared model preference types.

