# Layered Sidebar Deck Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the duplicate desktop/mobile sidebar shell with one navigation ground layer and an elevated content deck that supports desktop collapse and Arc-like reveal behavior.

**Architecture:** Add a focused shell behavior helper that exposes reducer-tested deck navigation state and a React hook for `AppShell`. Refactor `AppShell` to render one persistent `ShellSidebar`, remove the mobile `Sheet`, and drive desktop/mobile layout through deck state attributes and responsive classes.

**Tech Stack:** React 19, TypeScript, Vite, Tailwind class utilities, lucide-react icons, Node `node:test`, React server rendering tests.

---

## File Structure

- Modify: `crates/noema-core/web/src/components/shell/AppShell.tsx`
  - Keep navigation item definitions, active-destination logic, and attention logic.
  - Remove `Sheet` imports and duplicate sidebar rendering.
  - Render one sidebar ground layer and one content deck.
  - Add deck header controls and responsive deck state attributes.
- Create: `crates/noema-core/web/src/components/shell/deckNavigation.ts`
  - Own pure reducer, control-label helpers, and `useDeckNavigation`.
  - Keep shell behavior testable without a browser interaction library.
- Modify: `crates/noema-core/web/src/components/shell/AppShell.test.ts`
  - Add reducer/helper tests for open, close, navigation close, Escape close, and collapse toggle.
  - Add server-render tests proving one nav instance, accessible controls, and deck state attributes.
- Modify: `docs/frontend/navigation-workflows.md`
  - Replace the current desktop-sidebar/mobile-drawer sentence with the layered shell behavior.
- Modify: `docs/context/current.md`
  - Add a concise settled decision/open-loop note after the implementation lands.

## Task 1: Add Deck Navigation Behavior Helpers

**Files:**
- Create: `crates/noema-core/web/src/components/shell/deckNavigation.ts`
- Modify: `crates/noema-core/web/src/components/shell/AppShell.test.ts`

- [ ] **Step 1: Write failing reducer and label tests**

Add these imports to `crates/noema-core/web/src/components/shell/AppShell.test.ts`:

```ts
import {
  deckNavigationReducer,
  deckNavigationControlLabels,
  initialDeckNavigationState,
  type DeckNavigationState
} from "./deckNavigation";
```

Add these tests after the existing `shell navigation helpers` block:

```ts
describe("deck navigation behavior", () => {
  test("opens and closes the navigation reveal state", () => {
    const openState = deckNavigationReducer(initialDeckNavigationState, { type: "openNav" });

    assert.deepEqual(openState, {
      navOpen: true,
      sidebarCollapsed: false
    });

    assert.deepEqual(deckNavigationReducer(openState, { type: "closeNav" }), {
      navOpen: false,
      sidebarCollapsed: false
    });
  });

  test("closes revealed navigation after route navigation", () => {
    const state: DeckNavigationState = {
      navOpen: true,
      sidebarCollapsed: true
    };

    assert.deepEqual(deckNavigationReducer(state, { type: "navigate" }), {
      navOpen: false,
      sidebarCollapsed: true
    });
  });

  test("escape closes revealed navigation without changing collapse state", () => {
    const state: DeckNavigationState = {
      navOpen: true,
      sidebarCollapsed: true
    };

    assert.deepEqual(deckNavigationReducer(state, { type: "escape" }), {
      navOpen: false,
      sidebarCollapsed: true
    });
  });

  test("toggles desktop collapse and closes revealed navigation", () => {
    const collapsed = deckNavigationReducer(initialDeckNavigationState, {
      type: "toggleSidebarCollapsed"
    });

    assert.deepEqual(collapsed, {
      navOpen: false,
      sidebarCollapsed: true
    });

    assert.deepEqual(
      deckNavigationReducer({ navOpen: true, sidebarCollapsed: true }, {
        type: "toggleSidebarCollapsed"
      }),
      {
        navOpen: false,
        sidebarCollapsed: false
      }
    );
  });

  test("returns accessible labels for the next shell action", () => {
    assert.deepEqual(deckNavigationControlLabels(initialDeckNavigationState), {
      menu: "Open navigation",
      collapse: "Collapse sidebar"
    });

    assert.deepEqual(
      deckNavigationControlLabels({ navOpen: true, sidebarCollapsed: true }),
      {
        menu: "Close navigation",
        collapse: "Expand sidebar"
      }
    );
  });
});
```

- [ ] **Step 2: Run the targeted test and verify it fails**

Run:

```bash
cd crates/noema-core/web
bun test src/components/shell/AppShell.test.ts
```

Expected: FAIL because `./deckNavigation` does not exist.

- [ ] **Step 3: Create the minimal helper implementation**

Create `crates/noema-core/web/src/components/shell/deckNavigation.ts`:

```ts
import React from "react";
import type { AppRoute } from "@/routes";

export type DeckNavigationState = {
  navOpen: boolean;
  sidebarCollapsed: boolean;
};

export type DeckNavigationAction =
  | { type: "openNav" }
  | { type: "closeNav" }
  | { type: "escape" }
  | { type: "navigate" }
  | { type: "toggleSidebarCollapsed" };

export const initialDeckNavigationState: DeckNavigationState = {
  navOpen: false,
  sidebarCollapsed: false
};

export function deckNavigationReducer(
  state: DeckNavigationState,
  action: DeckNavigationAction
): DeckNavigationState {
  switch (action.type) {
    case "openNav":
      return { ...state, navOpen: true };
    case "closeNav":
    case "escape":
    case "navigate":
      return { ...state, navOpen: false };
    case "toggleSidebarCollapsed":
      return {
        navOpen: false,
        sidebarCollapsed: !state.sidebarCollapsed
      };
  }
}

export function deckNavigationControlLabels(state: DeckNavigationState) {
  return {
    menu: state.navOpen ? "Close navigation" : "Open navigation",
    collapse: state.sidebarCollapsed ? "Expand sidebar" : "Collapse sidebar"
  };
}

export function useDeckNavigation(onNavigate: (route: AppRoute) => void) {
  const [state, dispatch] = React.useReducer(
    deckNavigationReducer,
    initialDeckNavigationState
  );
  const menuButtonRef = React.useRef<HTMLButtonElement>(null);

  const openNav = React.useCallback(() => {
    dispatch({ type: "openNav" });
  }, []);

  const closeNav = React.useCallback(() => {
    dispatch({ type: "closeNav" });
    menuButtonRef.current?.focus();
  }, []);

  const toggleSidebarCollapsed = React.useCallback(() => {
    dispatch({ type: "toggleSidebarCollapsed" });
  }, []);

  const navigateFromShell = React.useCallback(
    (nextRoute: AppRoute) => {
      onNavigate(nextRoute);
      dispatch({ type: "navigate" });
    },
    [onNavigate]
  );

  React.useEffect(() => {
    if (!state.navOpen) {
      return;
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") {
        return;
      }
      dispatch({ type: "escape" });
      menuButtonRef.current?.focus();
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [state.navOpen]);

  return {
    state,
    labels: deckNavigationControlLabels(state),
    menuButtonRef,
    openNav,
    closeNav,
    toggleSidebarCollapsed,
    navigateFromShell
  };
}
```

- [ ] **Step 4: Run the targeted test and verify it passes**

Run:

```bash
cd crates/noema-core/web
bun test src/components/shell/AppShell.test.ts
```

Expected: PASS.

- [ ] **Step 5: Commit Task 1**

Run:

```bash
git add crates/noema-core/web/src/components/shell/deckNavigation.ts crates/noema-core/web/src/components/shell/AppShell.test.ts
git commit -m "test(web): cover deck navigation state"
```

Expected: commit succeeds.

## Task 2: Refactor `AppShell` To A Single Sidebar Ground Layer

**Files:**
- Modify: `crates/noema-core/web/src/components/shell/AppShell.tsx`
- Modify: `crates/noema-core/web/src/components/shell/AppShell.test.ts`

- [ ] **Step 1: Write failing render tests for the layered shell**

Add these imports to `crates/noema-core/web/src/components/shell/AppShell.test.ts`:

```ts
import * as React from "react";
import { renderToStaticMarkup } from "react-dom/server";
```

Add this test block after the deck navigation behavior tests:

```ts
describe("AppShell layered deck markup", () => {
  test("renders a single navigation ground layer and content deck", () => {
    const markup = renderShell();

    assert.equal(countMatches(markup, /aria-label="Primary"/g), 1);
    assert.match(markup, /data-slot="shell-root"/);
    assert.match(markup, /data-slot="shell-sidebar-ground"/);
    assert.match(markup, /data-slot="shell-content-deck"/);
    assert.match(markup, /data-sidebar-collapsed="false"/);
    assert.match(markup, /data-nav-open="false"/);
  });

  test("renders accessible deck controls and current destination label", () => {
    const markup = renderShell({ route: { kind: "memory_graph" } });

    assert.match(markup, /aria-label="Open navigation"/);
    assert.match(markup, /aria-expanded="false"/);
    assert.match(markup, /aria-controls="noema-shell-sidebar"/);
    assert.match(markup, /aria-label="Collapse sidebar"/);
    assert.match(markup, />Memory</);
    assert.match(markup, />Memory management</);
  });
});

function renderShell({
  route = { kind: "chat" } as const
}: {
  route?: Parameters<typeof AppShell>[0]["route"];
} = {}) {
  return renderToStaticMarkup(
    <AppShell
      route={route}
      status={healthyStatus}
      socketState="ready"
      providerBlocked={false}
      setupBlocked={false}
      onNavigate={() => undefined}
    >
      <section data-testid="route-content">Route content</section>
    </AppShell>
  );
}

function countMatches(value: string, pattern: RegExp) {
  return value.match(pattern)?.length ?? 0;
}
```

- [ ] **Step 2: Run the targeted test and verify it fails**

Run:

```bash
cd crates/noema-core/web
bun test src/components/shell/AppShell.test.ts
```

Expected: FAIL because current `AppShell` renders two `ShellSidebar` instances and does not include the new data slots.

- [ ] **Step 3: Replace `AppShell` imports**

In `crates/noema-core/web/src/components/shell/AppShell.tsx`, replace the current imports:

```ts
import React from "react";
import { Menu } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
  SheetTrigger
} from "@/components/ui/sheet";
```

with:

```ts
import React from "react";
import { Menu, PanelLeftClose, PanelLeftOpen } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
```

Add this import near the local `ShellSidebar` import:

```ts
import { useDeckNavigation } from "./deckNavigation";
```

- [ ] **Step 4: Replace drawer state and navigation callback**

In `AppShell`, replace:

```ts
const [drawerOpen, setDrawerOpen] = React.useState(false);
```

and the existing `navigateFromShell` callback with:

```ts
const {
  state: deckNavigation,
  labels,
  menuButtonRef,
  openNav,
  closeNav,
  toggleSidebarCollapsed,
  navigateFromShell
} = useDeckNavigation(onNavigate);
```

Keep `activeDestination`, `activeLabel`, and `attention` as they are.

- [ ] **Step 5: Replace the returned shell markup**

Replace the entire `return (...)` block in `AppShell` with:

```tsx
return (
  <main
    data-slot="shell-root"
    data-nav-open={deckNavigation.navOpen}
    data-sidebar-collapsed={deckNavigation.sidebarCollapsed}
    className="relative h-dvh min-h-screen overflow-hidden bg-[var(--pine-50)] text-foreground"
  >
    <aside
      id="noema-shell-sidebar"
      data-slot="shell-sidebar-ground"
      aria-label="Noema navigation"
      className={cn(
        "absolute inset-y-0 left-0 z-10 grid min-h-0 w-[236px] px-3.5 py-4",
        "max-[760px]:w-[min(286px,78vw)] max-[760px]:pb-[max(1rem,env(safe-area-inset-bottom))]"
      )}
    >
      <ShellSidebar
        activeDestination={activeDestination}
        attention={attention}
        navItems={shellNavItems}
        onNavigate={navigateFromShell}
      />
    </aside>

    {deckNavigation.navOpen ? (
      <button
        type="button"
        data-slot="shell-nav-backdrop"
        aria-label="Close navigation"
        className="absolute inset-0 z-20 cursor-default bg-transparent"
        onClick={closeNav}
      />
    ) : null}

    <section
      data-slot="shell-content-deck"
      aria-label={activeLabel}
      className={cn(
        "absolute z-30 grid min-h-0 overflow-hidden border border-[var(--border-subtle)] bg-background shadow-[0_24px_70px_rgba(31,38,30,0.18)] transition-[inset,transform,border-radius,box-shadow] duration-300 ease-out",
        "motion-reduce:transition-none",
        deckNavigation.sidebarCollapsed
          ? "inset-2 rounded-xl"
          : "inset-y-2 right-2 left-[244px] rounded-xl",
        deckNavigation.navOpen &&
          "translate-x-[min(236px,68vw)] scale-[0.97] pointer-events-none max-[760px]:translate-x-[min(252px,72vw)]",
        "max-[760px]:inset-0 max-[760px]:rounded-none max-[760px]:border-0 max-[760px]:shadow-none max-[760px]:data-[nav-open=true]:rounded-xl"
      )}
    >
      <header
        data-slot="shell-deck-header"
        className="flex min-h-[56px] items-center gap-3 border-b border-[var(--border-subtle)] bg-white/95 px-4"
      >
        <Button
          ref={menuButtonRef}
          type="button"
          variant="ghost"
          size="icon"
          aria-label={labels.menu}
          aria-controls="noema-shell-sidebar"
          aria-expanded={deckNavigation.navOpen}
          onClick={deckNavigation.navOpen ? closeNav : openNav}
        >
          <Menu aria-hidden="true" />
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          aria-label={labels.collapse}
          className="max-[760px]:hidden"
          onClick={toggleSidebarCollapsed}
        >
          {deckNavigation.sidebarCollapsed ? (
            <PanelLeftOpen aria-hidden="true" />
          ) : (
            <PanelLeftClose aria-hidden="true" />
          )}
        </Button>
        <div className="min-w-0">
          <strong className="block truncate font-heading text-base tracking-normal">
            {activeLabel}
          </strong>
          <span className="block truncate text-xs text-muted-foreground">
            {activeDestination === "home" ? "Primary conversation" : "Memory management"}
          </span>
        </div>
      </header>

      <div
        data-slot="shell-route-content"
        className={cn("min-h-0 overflow-hidden", deckNavigation.navOpen && "pointer-events-none")}
      >
        {children}
      </div>
    </section>
  </main>
);
```

If Tailwind does not generate `max-[760px]:data-[nav-open=true]:rounded-xl`, replace that one class with a plain mobile-safe class combination and keep the `data-nav-open` assertions. Do not add a custom CSS file for this one selector.

- [ ] **Step 6: Fix the nav-open data selector placement**

The class in Step 5 uses `data-[nav-open=true]` on the deck, but the data attribute is on the root. Move `data-nav-open={deckNavigation.navOpen}` onto the deck too:

```tsx
<section
  data-slot="shell-content-deck"
  data-nav-open={deckNavigation.navOpen}
  aria-label={activeLabel}
  className={cn(
    "absolute z-30 grid min-h-0 overflow-hidden border border-[var(--border-subtle)] bg-background shadow-[0_24px_70px_rgba(31,38,30,0.18)] transition-[inset,transform,border-radius,box-shadow] duration-300 ease-out",
    "motion-reduce:transition-none",
    deckNavigation.sidebarCollapsed
      ? "inset-2 rounded-xl"
      : "inset-y-2 right-2 left-[244px] rounded-xl",
    deckNavigation.navOpen &&
      "translate-x-[min(236px,68vw)] scale-[0.97] pointer-events-none max-[760px]:translate-x-[min(252px,72vw)]",
    "max-[760px]:inset-0 max-[760px]:rounded-none max-[760px]:border-0 max-[760px]:shadow-none data-[nav-open=true]:max-[760px]:rounded-xl"
  )}
>
```

- [ ] **Step 7: Run the targeted test and fix formatting issues**

Run:

```bash
cd crates/noema-core/web
bun test src/components/shell/AppShell.test.ts
```

Expected: PASS. If TypeScript reports that `Button` does not accept `ref`, wrap the button in a native `button` only for the menu trigger or check the `Button` primitive signature in `crates/noema-core/web/src/components/ui/button.tsx` and forward the ref through the supported prop shape.

- [ ] **Step 8: Commit Task 2**

Run:

```bash
git add crates/noema-core/web/src/components/shell/AppShell.tsx crates/noema-core/web/src/components/shell/AppShell.test.ts
git commit -m "feat(web): layer sidebar under content deck"
```

Expected: commit succeeds.

## Task 3: Tighten Accessibility, Motion, And Documentation

**Files:**
- Modify: `crates/noema-core/web/src/components/shell/AppShell.tsx`
- Modify: `crates/noema-core/web/src/components/shell/AppShell.test.ts`
- Modify: `docs/frontend/navigation-workflows.md`
- Modify: `docs/context/current.md`

- [ ] **Step 1: Add a failing test for hidden mobile-only duplication and reduced-motion class**

Add this test to the `AppShell layered deck markup` block:

```ts
test("does not render sheet markup and includes reduced-motion deck handling", () => {
  const markup = renderShell();
  const deckClassName = dataSlotClassName(markup, "shell-content-deck");

  assert.doesNotMatch(markup, /data-slot="sheet"/);
  assert.doesNotMatch(markup, /data-slot="sheet-content"/);
  assert.match(deckClassName, /\bmotion-reduce:transition-none\b/);
});
```

Add this helper at the bottom of `AppShell.test.ts` if it does not already exist:

```ts
function dataSlotClassName(markup: string, slot: string) {
  const match = markup.match(new RegExp(`data-slot="${slot}"[^>]*class="([^"]*)"`));
  assert.ok(match, `expected markup to include data-slot="${slot}" class`);
  return match[1];
}
```

- [ ] **Step 2: Run the targeted test**

Run:

```bash
cd crates/noema-core/web
bun test src/components/shell/AppShell.test.ts
```

Expected: PASS if Task 2 already removed `Sheet` and included `motion-reduce:transition-none`; otherwise FAIL and then update `AppShell.tsx` to match the markup from Task 2.

- [ ] **Step 3: Update frontend navigation docs**

In `docs/frontend/navigation-workflows.md`, replace this paragraph:

```md
Current shell behavior: desktop shows a persistent left sidebar, while narrow
screens use a drawer opened from a compact top bar. Healthy setup, service,
chat, provider, and memory state are silent in the shell. The shell shows a
single compact attention item only when state is degraded or action-worthy; the
active page owns detailed recovery UI.
```

with:

```md
Current shell behavior: the sidebar is a persistent navigation ground layer,
and route content sits above it as the active deck. Expanded desktop keeps the
ground-layer sidebar visible; collapsed desktop and mobile move the deck aside
to reveal the same single sidebar instance. Healthy setup, service, chat,
provider, and memory state are silent in the shell. The shell shows a single
compact attention item only when state is degraded or action-worthy; the active
page owns detailed recovery UI.
```

- [ ] **Step 4: Update durable project context**

In `docs/context/current.md`, add this settled decision near the existing web UI/shadcn bullets:

```md
- The web shell uses a layered sidebar deck: one persistent `ShellSidebar`
  ground layer sits under the route content deck. Expanded desktop keeps the
  sidebar visible; collapsed desktop and mobile reveal navigation by moving the
  deck aside rather than rendering a separate drawer/sidebar copy.
```

- [ ] **Step 5: Run documentation and targeted test checks**

Run:

```bash
git diff --check
cd crates/noema-core/web
bun test src/components/shell/AppShell.test.ts
```

Expected: `git diff --check` exits 0 and the targeted test passes.

- [ ] **Step 6: Commit Task 3**

Run:

```bash
git add crates/noema-core/web/src/components/shell/AppShell.tsx crates/noema-core/web/src/components/shell/AppShell.test.ts docs/frontend/navigation-workflows.md docs/context/current.md
git commit -m "docs: document layered web shell"
```

Expected: commit succeeds.

## Task 4: Full Frontend Validation And Visual Inspection

**Files:**
- No planned source edits unless validation exposes a bug.

- [ ] **Step 1: Check worktree and whitespace**

Run:

```bash
git status --short --branch
git diff --check
```

Expected: no unstaged source changes except any deliberate validation fixes; no whitespace errors.

- [ ] **Step 2: Run frontend tests**

Run:

```bash
cd crates/noema-core/web
bun test
```

Expected: PASS.

- [ ] **Step 3: Run lint**

Run:

```bash
cd crates/noema-core/web
bun run lint
```

Expected: PASS. This command regenerates GraphQL types first; if generated files change, inspect the diff and commit only if the changes are relevant.

- [ ] **Step 4: Run production build**

Run:

```bash
cd crates/noema-core/web
bun run build
```

Expected: PASS.

- [ ] **Step 5: Run local app for visual inspection**

Run the web build watcher:

```bash
cd crates/noema-core/web
bun run dev
```

Expected: the script runs `bun run gen:types` and starts `vite build --watch`.
This command does not serve the full app by itself because the web UI expects
the Noema backend GraphQL server. If a local Noema server is already running,
open that app URL and inspect the states below. If no local Noema server is
running and the user has not explicitly asked for browser inspection, record
that visual inspection was not performed and rely on the completed tests,
lint, and build.

Inspect these viewports:

- Desktop expanded: sidebar visible underneath, content deck offset right, no top-level duplicate sidebar.
- Desktop collapsed: content deck covers sidebar content, menu reveal moves deck aside.
- Mobile closed: content deck covers the sidebar ground layer.
- Mobile open: deck translates/scales aside, sidebar is revealed, route content cannot be tapped behind it.
- Memory graph: canvas/detail workspace remains usable inside the deck and is not squeezed by nested card styling.

- [ ] **Step 6: Final status and optional final commit**

Run:

```bash
git status --short --branch
```

If validation or visual inspection required fixes, commit them:

```bash
git add <fixed-files>
git commit -m "fix(web): polish layered shell layout"
```

Expected: implementation branch has only intentional commits and no unrelated dirty work.

## Self-Review Notes

- Spec coverage: Tasks cover the single sidebar instance, elevated content deck, mobile `Sheet` removal, desktop collapse, shared reveal behavior, component-local collapse state, accessibility labels, Escape/backdrop close, reduced motion, route preservation, docs, and validation.
- Red-flag scan: No task uses unfinished-marker or fill-in language. The visual inspection step names exact expected states and the exact web build watcher command.
- Type consistency: `DeckNavigationState`, `deckNavigationReducer`, `deckNavigationControlLabels`, and `useDeckNavigation` names are consistent across tests, implementation, and plan tasks.
