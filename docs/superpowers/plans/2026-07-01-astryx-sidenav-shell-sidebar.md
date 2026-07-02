# Astryx SideNav Shell Sidebar Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the shell sidebar's Astryx `SideNav` adoption inside `ShellSidebar` while preserving Noema's layered deck shell.

**Architecture:** Keep `AppShell`, `deckNavigation`, `ShellSurfaceContext`, and `shellNavigation` as the owners of shell frame, reveal/collapse state, and route-derived menu data. Refactor `ShellSidebar.tsx` into clearer adapter boundaries that map a Noema `ShellMenuLevel` into Astryx `SideNav` zones and `SideNavItem`s, using Astryx `xstyle` instead of treating `SideNav` like a plain DOM node.

**Tech Stack:** React 19, TypeScript, StyleX, Astryx `@astryxdesign/core/SideNav`, Bun validation.

---

## Current Dirty Worktree

At plan-writing time, these files were already dirty and must not be staged or
reverted unless the user explicitly asks:

- `.gitignore`
- `crates/noema-core/src/daemon/web/assets/app.js`
- `crates/noema-core/src/daemon/web/assets/styles.css`

Implementation should stage only files changed for this sidebar slice.

## File Structure

Modify:

- `crates/noema-core/web/src/components/shell/ShellSidebar.tsx`
  - Keep the route-derived menu transition behavior.
  - Add a `ShellSidebarNav` adapter around Astryx `SideNav`.
  - Rename the local item renderer to `ShellSidebarNavItem`.
  - Pass sidebar StyleX overrides through `SideNav`'s `xstyle` prop.
  - Keep Noema data attributes, attention rendering, bottom item behavior, and
    primary-agent avatar behavior.

Do not modify:

- `crates/noema-core/web/src/components/shell/AppShell.tsx`
- `crates/noema-core/web/src/components/shell/deckNavigation.ts`
- `crates/noema-core/web/src/components/shell/ShellSurfaceContext.tsx`
- `crates/noema-core/web/src/components/shell/shellNavigation.ts`
- Route, GraphQL, chat, memory, and settings pane files.

## Task 1: Refactor `ShellSidebar` Into A SideNav Adapter

**Files:**
- Modify: `crates/noema-core/web/src/components/shell/ShellSidebar.tsx`

- [ ] **Step 1: Update the SideNav import and add an xstyle helper type**

In `crates/noema-core/web/src/components/shell/ShellSidebar.tsx`, change the
SideNav import from:

```tsx
import { SideNav, SideNavItem } from "@astryxdesign/core/SideNav";
```

to:

```tsx
import { SideNav, SideNavItem, type SideNavProps } from "@astryxdesign/core/SideNav";
```

After the `ShellSidebarMenuFrameState` type, add:

```tsx
type SideNavXStyle = SideNavProps["xstyle"];
```

- [ ] **Step 2: Replace the inline `SideNav` block in `ShellSidebarMenuFrame`**

Inside `ShellSidebarMenuFrame`, remove the `visibleAttention` constant and the
inline `<SideNav ...>` block. The component should return the transition frame
wrapper with one child:

```tsx
return (
  <div
    data-slot="shell-sidebar-menu-level-frame"
    data-shell-menu-level={menuLevel.levelId}
    data-shell-menu-frame-state={frameState}
    data-shell-menu-transition-direction={transitionDirection}
    aria-hidden={interactive ? undefined : "true"}
    {...stylex.props(styles.menuFrame, !interactive && styles.menuFrameNonInteractive)}
  >
    <ShellSidebarNav
      menuLevel={menuLevel}
      attention={attention}
      primaryAgentNamed={primaryAgentNamed}
      primaryAgentLabel={primaryAgentLabel}
      interactive={interactive}
      onSelectItem={onSelectItem}
    />
  </div>
);
```

- [ ] **Step 3: Add `ShellSidebarNav` below `ShellSidebarMenuFrame`**

Add this function below `ShellSidebarMenuFrame` and above
`ShellSidebarHeader`:

```tsx
function ShellSidebarNav({
  menuLevel,
  attention,
  primaryAgentNamed,
  primaryAgentLabel,
  interactive,
  onSelectItem
}: {
  menuLevel: ShellMenuLevel;
  attention: ShellAttention | null;
  primaryAgentNamed: boolean;
  primaryAgentLabel: string;
  interactive: boolean;
  onSelectItem: (item: ShellMenuItem) => void;
}) {
  const visibleAttention =
    menuLevel.supportsAttention && attention ? (
      <ShellAttentionItem attention={attention} />
    ) : (
      <div aria-hidden="true" />
    );

  return (
    <SideNav
      aria-label={menuLevel.ariaLabel}
      header={<ShellSidebarHeader attention={visibleAttention} />}
      footer={
        <ShellSidebarNavItem
          item={menuLevel.bottomItem}
          active={menuLevel.bottomItem.itemId === menuLevel.activeItemId}
          primaryAgentNamed={false}
          primaryAgentLabel={primaryAgentLabel}
          interactive={interactive}
          bottom
          onSelectItem={onSelectItem}
        />
      }
      xstyle={sideNavXStyle(styles.sideNav)}
    >
      {menuLevel.items.map((item) => (
        <ShellSidebarNavItem
          key={item.itemId}
          item={item}
          active={item.itemId === menuLevel.activeItemId}
          primaryAgentNamed={primaryAgentNamed}
          primaryAgentLabel={primaryAgentLabel}
          interactive={interactive}
          onSelectItem={onSelectItem}
        />
      ))}
    </SideNav>
  );
}
```

Do not pass `collapsible` or `resizable` to `SideNav`.

- [ ] **Step 4: Rename the local item renderer**

Rename the local `ShellMenuItem` component to `ShellSidebarNavItem`:

```tsx
function ShellSidebarNavItem({
  item,
  active,
  primaryAgentNamed,
  primaryAgentLabel,
  interactive,
  bottom = false,
  onSelectItem
}: {
  item: ShellMenuItem;
  active: boolean;
  primaryAgentNamed: boolean;
  primaryAgentLabel: string;
  interactive: boolean;
  bottom?: boolean;
  onSelectItem: (item: ShellMenuItem) => void;
}) {
  const Icon = item.icon;
  const showPrimaryAgentAvatar = item.itemId === "home" && primaryAgentNamed;
  const label = item.itemId === "home" && primaryAgentNamed ? primaryAgentLabel : item.label;

  return (
    <div
      data-slot={bottom ? "shell-menu-bottom-item" : "shell-menu-item"}
      data-shell-menu-item={item.itemId}
      data-current={active ? "true" : undefined}
      {...stylex.props(styles.menuItemFrame)}
    >
      <SideNavItem
        size="sm"
        label={label}
        icon={showPrimaryAgentAvatar ? <PrimaryAgentMenuAvatar /> : <Icon aria-hidden size={16} />}
        isSelected={active}
        isDisabled={!interactive}
        onClick={() => onSelectItem(item)}
      />
    </div>
  );
}
```

Ensure all call sites use `ShellSidebarNavItem`.

- [ ] **Step 5: Add the SideNav xstyle helper**

Add this function near the bottom of the file before `const styles`:

```tsx
function sideNavXStyle(xstyle: unknown): SideNavXStyle {
  return xstyle as unknown as SideNavXStyle;
}
```

- [ ] **Step 6: Run source checks for SideNav ownership**

Run:

```bash
rg -n "<SideNav|<SideNavItem|xstyle=\\{sideNavXStyle\\}" crates/noema-core/web/src/components/shell/ShellSidebar.tsx
rg -n "collapsible=|resizable=|@astryxdesign/core/AppShell|LayoutPanel" crates/noema-core/web/src/components/shell/ShellSidebar.tsx
```

Expected:

- First command shows `SideNav`, `SideNavItem`, and `xstyle={sideNavXStyle(styles.sideNav)}`.
- Second command prints no output and exits `1`.

- [ ] **Step 7: Run lint**

Run:

```bash
cd crates/noema-core/web
bun run lint
```

Expected: pass.

- [ ] **Step 8: Commit Task 1**

```bash
git add crates/noema-core/web/src/components/shell/ShellSidebar.tsx
git commit -m "refactor: clarify shell sidebar sidenav adapter"
```

## Task 2: Final Validation

**Files:**
- Inspect only unless validation requires fixes.

- [ ] **Step 1: Confirm the shell owner files stayed unchanged**

Run:

```bash
git diff --name-only HEAD~1..HEAD
```

Expected output:

```text
crates/noema-core/web/src/components/shell/ShellSidebar.tsx
```

Then run:

```bash
git diff --name-only HEAD~1..HEAD -- crates/noema-core/web/src/components/shell/AppShell.tsx \
  crates/noema-core/web/src/components/shell/deckNavigation.ts \
  crates/noema-core/web/src/components/shell/ShellSurfaceContext.tsx \
  crates/noema-core/web/src/components/shell/shellNavigation.ts
```

Expected: no output.

- [ ] **Step 2: Confirm no forbidden shell replacement imports were introduced**

Run:

```bash
rg -n "@astryxdesign/core/AppShell|LayoutPanel|MobileNav|SideNavCollapseButton|useSideNavCollapse" \
  crates/noema-core/web/src/components/shell
```

Expected: no output and exit code `1`.

- [ ] **Step 3: Run frontend build**

Run:

```bash
cd crates/noema-core/web
bun run build
```

Expected: pass. Vite may print the existing large chunk warning. The build may
rewrite `crates/noema-core/src/daemon/web/assets/app.js` and
`crates/noema-core/src/daemon/web/assets/styles.css`; do not stage those files.

- [ ] **Step 4: Run repository diff checks**

Run:

```bash
git status --short --branch
git diff --check
```

Expected:

- `git diff --check` prints no output and exits `0`.
- `git status --short --branch` may still show only the pre-existing dirty
  files listed at the top of this plan. Do not stage those files.

- [ ] **Step 5: Commit validation fixes only if needed**

If Task 2 required source fixes after Task 1's commit, stage only
`ShellSidebar.tsx`:

```bash
git add crates/noema-core/web/src/components/shell/ShellSidebar.tsx
git diff --cached --stat
git diff --cached --name-status
git commit -m "chore: validate shell sidenav adapter"
```

If no validation fixes were needed after Task 1, do not create an empty commit.

## Self-Review Notes

- Spec coverage: Task 1 implements the clearer `ShellMenuLevel` to `SideNav`
  adapter, keeps `SideNav` and `SideNavItem` as the real sidebar primitives,
  preserves Noema attention and primary-agent avatar behavior, and avoids
  Astryx collapse/resize. Task 2 validates shell ownership and build hygiene.
- Scope: only `ShellSidebar.tsx` should change. `AppShell`, `deckNavigation`,
  `ShellSurfaceContext`, and `shellNavigation` remain untouched.
- Deferred work: Astryx `AppShell`, `LayoutPanel`, and `SideNav` controlled
  collapse remain future architecture choices.
