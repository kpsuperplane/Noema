# Shell Surface Visibility Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a formal shell-surface visibility API so routed surfaces can run focus and other visible side effects only after shell-owned deck motion has settled.

**Architecture:** `AppShell` owns layered deck visibility and provides a small `ShellSurfaceContext` around route content. The deck navigation reducer tracks surface visibility from the surface's point of view (`visible | hiding | hidden | showing`). Chat consumes this context and focuses the composer only when chat is ready and the surface is visible; `Composer` exposes a normal forwarded textarea ref and does not know about shell state.

**Tech Stack:** React 19, TypeScript, Bun test runner, React server rendering tests, Tailwind class helpers, Vite-generated embedded web assets.

---

## File Structure

- Create `crates/noema-core/web/src/components/shell/ShellSurfaceContext.tsx`
  - Owns `ShellSurfaceVisibility`, `ShellSurfaceState`, `ShellSurfaceProvider`, and `useShellSurface`.
- Create `crates/noema-core/web/src/components/shell/ShellSurfaceContext.test.tsx`
  - Verifies default visibility and provider override behavior.
- Modify `crates/noema-core/web/src/components/shell/deckNavigation.ts`
  - Adds surface visibility to deck navigation state.
  - Adds reducer actions for settling deck transitions.
  - Adds helpers for reduced motion and transition-property filtering.
- Modify `crates/noema-core/web/src/components/shell/AppShell.tsx`
  - Provides shell surface visibility around route content.
  - Settles visibility from deck transition events.
  - Pointer-disables route content whenever visibility is not `visible`.
- Modify `crates/noema-core/web/src/components/shell/AppShell.test.ts`
  - Extends shell tests for visibility state transitions and deck transition filtering.
- Create `crates/noema-core/web/src/components/ChatSurface.tsx`
  - Moves chat route markup out of `App.tsx`.
  - Consumes `useShellSurface()`.
  - Focuses composer when `ready && visibility === "visible"`.
- Create `crates/noema-core/web/src/components/ChatSurface.test.tsx`
  - Tests the pure chat focus gate and rendered chat/composer contract.
- Modify `crates/noema-core/web/src/components/Composer.tsx`
  - Converts `Composer` to `React.forwardRef<HTMLTextAreaElement, ComposerProps>`.
  - Removes all shell/focus-on-ready policy.
- Modify `crates/noema-core/web/src/components/Composer.test.ts`
  - Confirms no native `autoFocus` is rendered.
- Modify `crates/noema-core/web/src/App.tsx`
  - Replaces inline `chatView` markup with `ChatSurface`.
  - Removes aborted route-source focus policy if present.
- Modify `crates/noema-core/web/src/App.test.ts`
  - Removes aborted route-source focus policy tests if present.
- Modify `crates/noema-core/web/src/routes.ts`
  - Removes aborted `routeSource` plumbing if present.
- Rebuild generated assets:
  - `crates/noema-core/src/daemon/web/assets/app.js`
  - `crates/noema-core/src/daemon/web/assets/index.html`
  - `crates/noema-core/src/daemon/web/assets/styles.css`

## Task 0: Restore Interrupted Web Edits

**Files:**
- Restore if dirty: `crates/noema-core/web/src/App.tsx`
- Restore if dirty: `crates/noema-core/web/src/App.test.ts`
- Restore if dirty: `crates/noema-core/web/src/components/Composer.tsx`
- Restore if dirty: `crates/noema-core/web/src/components/Composer.test.ts`
- Restore if dirty: `crates/noema-core/web/src/routes.ts`
- Restore if dirty: `crates/noema-core/src/daemon/web/assets/app.js`
- Restore if dirty: `crates/noema-core/src/daemon/web/assets/index.html`
- Restore if dirty: `crates/noema-core/src/daemon/web/assets/styles.css`

- [ ] **Step 1: Inspect current dirty state**

Run:

```bash
git status --short --branch
```

Expected: note any unrelated dirty files. Do not touch dirty files outside the web files listed in this task.

- [ ] **Step 2: Restore the aborted web attempt**

Run:

```bash
git restore crates/noema-core/web/src/App.tsx crates/noema-core/web/src/App.test.ts crates/noema-core/web/src/components/Composer.tsx crates/noema-core/web/src/components/Composer.test.ts crates/noema-core/web/src/routes.ts crates/noema-core/src/daemon/web/assets/app.js crates/noema-core/src/daemon/web/assets/index.html crates/noema-core/src/daemon/web/assets/styles.css
```

Expected: the aborted `routeSource` and `focusOnReady` changes are gone, and generated assets are no longer deleted.

- [ ] **Step 3: Confirm only unrelated dirty work remains**

Run:

```bash
git status --short --branch
```

Expected: the files restored in Step 2 no longer appear unless they are intentionally modified by another active user change. If they still appear, stop and inspect `git diff -- <path>` before proceeding.

## Task 1: Add Shell Surface Context

**Files:**
- Create: `crates/noema-core/web/src/components/shell/ShellSurfaceContext.tsx`
- Create: `crates/noema-core/web/src/components/shell/ShellSurfaceContext.test.tsx`

- [ ] **Step 1: Write the failing context tests**

Create `crates/noema-core/web/src/components/shell/ShellSurfaceContext.test.tsx`:

```tsx
import assert from "node:assert/strict";
import { describe, test } from "node:test";
import * as React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { ShellSurfaceProvider, useShellSurface } from "./ShellSurfaceContext";

function SurfaceProbe() {
  const { visibility } = useShellSurface();
  return <span data-visibility={visibility}>{visibility}</span>;
}

describe("ShellSurfaceContext", () => {
  test("defaults isolated surfaces to visible", () => {
    const markup = renderToStaticMarkup(<SurfaceProbe />);

    assert.match(markup, /data-visibility="visible"/);
    assert.match(markup, />visible</);
  });

  test("provides shell-owned visibility to route surfaces", () => {
    const markup = renderToStaticMarkup(
      <ShellSurfaceProvider value={{ visibility: "hidden" }}>
        <SurfaceProbe />
      </ShellSurfaceProvider>
    );

    assert.match(markup, /data-visibility="hidden"/);
    assert.match(markup, />hidden</);
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
bun test src/components/shell/ShellSurfaceContext.test.tsx
```

Expected: FAIL because `./ShellSurfaceContext` does not exist.

- [ ] **Step 3: Implement the context**

Create `crates/noema-core/web/src/components/shell/ShellSurfaceContext.tsx`:

```tsx
import React from "react";

export type ShellSurfaceVisibility = "visible" | "hiding" | "hidden" | "showing";

export type ShellSurfaceState = {
  visibility: ShellSurfaceVisibility;
};

export const defaultShellSurfaceState: ShellSurfaceState = {
  visibility: "visible"
};

const ShellSurfaceContext = React.createContext<ShellSurfaceState>(defaultShellSurfaceState);

export function ShellSurfaceProvider({
  value,
  children
}: {
  value: ShellSurfaceState;
  children: React.ReactNode;
}) {
  return <ShellSurfaceContext.Provider value={value}>{children}</ShellSurfaceContext.Provider>;
}

export function useShellSurface() {
  return React.useContext(ShellSurfaceContext);
}
```

- [ ] **Step 4: Run the context test to verify it passes**

Run:

```bash
bun test src/components/shell/ShellSurfaceContext.test.tsx
```

Expected: PASS, 2 tests.

- [ ] **Step 5: Commit Task 1**

Run:

```bash
git add crates/noema-core/web/src/components/shell/ShellSurfaceContext.tsx crates/noema-core/web/src/components/shell/ShellSurfaceContext.test.tsx
git commit -m "feat(web): add shell surface context"
```

Expected: commit includes only the context and its test.

## Task 2: Model Surface Visibility In Deck Navigation

**Files:**
- Modify: `crates/noema-core/web/src/components/shell/deckNavigation.ts`
- Modify: `crates/noema-core/web/src/components/shell/AppShell.test.ts`

- [ ] **Step 1: Write failing reducer tests**

In `crates/noema-core/web/src/components/shell/AppShell.test.ts`, update the shell import block:

```ts
import {
  deckNavigationControlLabels,
  deckNavigationReducer,
  deckTransitionPropertyCanSettleSurfaceVisibility,
  initialDeckNavigationState,
  type DeckNavigationState
} from "./deckNavigation";
```

Update the first reducer test expectations to include `surfaceVisibility`:

```ts
assert.deepEqual(openState, {
  navOpen: true,
  sidebarCollapsed: false,
  surfaceVisibility: "hiding"
});

assert.deepEqual(deckNavigationReducer(openState, { type: "closeNav", animated: true }), {
  navOpen: false,
  sidebarCollapsed: false,
  surfaceVisibility: "showing"
});
```

Add these tests inside `describe("deck navigation behavior", () => { ... })`:

```ts
test("settles surface visibility after deck transitions", () => {
  assert.deepEqual(
    deckNavigationReducer(
      { navOpen: true, sidebarCollapsed: false, surfaceVisibility: "hiding" },
      { type: "settleSurfaceVisibility" }
    ),
    {
      navOpen: true,
      sidebarCollapsed: false,
      surfaceVisibility: "hidden"
    }
  );

  assert.deepEqual(
    deckNavigationReducer(
      { navOpen: false, sidebarCollapsed: false, surfaceVisibility: "showing" },
      { type: "settleSurfaceVisibility" }
    ),
    {
      navOpen: false,
      sidebarCollapsed: false,
      surfaceVisibility: "visible"
    }
  );
});

test("settles synchronously when deck movement is not animated", () => {
  assert.deepEqual(deckNavigationReducer(initialDeckNavigationState, { type: "openNav", animated: false }), {
    navOpen: true,
    sidebarCollapsed: false,
    surfaceVisibility: "hidden"
  });

  assert.deepEqual(
    deckNavigationReducer(
      { navOpen: true, sidebarCollapsed: true, surfaceVisibility: "hidden" },
      { type: "closeNav", animated: false }
    ),
    {
      navOpen: false,
      sidebarCollapsed: true,
      surfaceVisibility: "visible"
    }
  );
});

test("filters transition properties that can settle surface visibility", () => {
  assert.equal(deckTransitionPropertyCanSettleSurfaceVisibility("left"), true);
  assert.equal(deckTransitionPropertyCanSettleSurfaceVisibility("right"), true);
  assert.equal(deckTransitionPropertyCanSettleSurfaceVisibility("transform"), true);
  assert.equal(deckTransitionPropertyCanSettleSurfaceVisibility("box-shadow"), false);
  assert.equal(deckTransitionPropertyCanSettleSurfaceVisibility("opacity"), false);
});
```

Update existing `DeckNavigationState` literals in this test file to include `surfaceVisibility: "visible"` unless the test is specifically about hidden/moving state.

- [ ] **Step 2: Run the shell test to verify it fails**

Run:

```bash
bun test src/components/shell/AppShell.test.ts
```

Expected: FAIL because `surfaceVisibility`, `animated`, `settleSurfaceVisibility`, and `deckTransitionPropertyCanSettleSurfaceVisibility` do not exist.

- [ ] **Step 3: Implement the reducer changes**

Modify `crates/noema-core/web/src/components/shell/deckNavigation.ts`:

```ts
import React from "react";
import type { AppRoute } from "@/routes";
import type { ShellSurfaceVisibility } from "./ShellSurfaceContext";

export type DeckNavigationState = {
  navOpen: boolean;
  sidebarCollapsed: boolean;
  surfaceVisibility: ShellSurfaceVisibility;
};

export type DeckNavigationAction =
  | { type: "openNav"; animated?: boolean }
  | { type: "closeNav"; animated?: boolean }
  | { type: "escape"; animated?: boolean }
  | { type: "navigate"; animated?: boolean }
  | { type: "settleSurfaceVisibility" }
  | { type: "syncWideViewport" }
  | { type: "toggleSidebarCollapsed" };

export const initialDeckNavigationState: DeckNavigationState = {
  navOpen: false,
  sidebarCollapsed: false,
  surfaceVisibility: "visible"
};

function visibilityAfterOpening(animated: boolean | undefined): ShellSurfaceVisibility {
  return animated === false ? "hidden" : "hiding";
}

function visibilityAfterClosing(
  state: DeckNavigationState,
  animated: boolean | undefined
): ShellSurfaceVisibility {
  if (!state.navOpen) {
    return "visible";
  }
  return animated === false ? "visible" : "showing";
}

function settledSurfaceVisibility(visibility: ShellSurfaceVisibility): ShellSurfaceVisibility {
  if (visibility === "hiding") {
    return "hidden";
  }
  if (visibility === "showing") {
    return "visible";
  }
  return visibility;
}

export function deckTransitionPropertyCanSettleSurfaceVisibility(propertyName: string) {
  return propertyName === "left" || propertyName === "right" || propertyName === "transform";
}

export function deckNavigationReducer(
  state: DeckNavigationState,
  action: DeckNavigationAction
): DeckNavigationState {
  switch (action.type) {
    case "openNav":
      return {
        ...state,
        navOpen: true,
        surfaceVisibility: visibilityAfterOpening(action.animated)
      };
    case "closeNav":
    case "escape":
    case "navigate":
      return {
        ...state,
        navOpen: false,
        surfaceVisibility: visibilityAfterClosing(state, action.animated)
      };
    case "settleSurfaceVisibility":
      return {
        ...state,
        surfaceVisibility: settledSurfaceVisibility(state.surfaceVisibility)
      };
    case "syncWideViewport":
      return { ...state, navOpen: false, surfaceVisibility: "visible" };
    case "toggleSidebarCollapsed":
      return {
        navOpen: false,
        sidebarCollapsed: !state.sidebarCollapsed,
        surfaceVisibility: "visible"
      };
  }
}
```

Keep `deckNavigationControlLabels` unchanged below the reducer.

- [ ] **Step 4: Update hook dispatch calls**

In `useDeckNavigation`, add a helper:

```ts
function shouldAnimateDeckNavigation() {
  if (typeof window.matchMedia !== "function") {
    return true;
  }
  return !window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}
```

Update callbacks:

```ts
const openNav = React.useCallback(() => {
  dispatch({ type: "openNav", animated: shouldAnimateDeckNavigation() });
}, []);

const closeNav = React.useCallback(() => {
  dispatch({ type: "closeNav", animated: shouldAnimateDeckNavigation() });
  menuButtonRef.current?.focus();
}, []);

const navigateFromShell = React.useCallback(
  (nextRoute: AppRoute) => {
    onNavigate(nextRoute);
    dispatch({ type: "navigate", animated: shouldAnimateDeckNavigation() });
  },
  [onNavigate]
);

const settleSurfaceVisibility = React.useCallback(() => {
  dispatch({ type: "settleSurfaceVisibility" });
}, []);
```

Return `settleSurfaceVisibility` from `useDeckNavigation`.

- [ ] **Step 5: Run the shell test to verify it passes**

Run:

```bash
bun test src/components/shell/AppShell.test.ts
```

Expected: PASS.

- [ ] **Step 6: Commit Task 2**

Run:

```bash
git add crates/noema-core/web/src/components/shell/deckNavigation.ts crates/noema-core/web/src/components/shell/AppShell.test.ts
git commit -m "feat(web): model shell surface visibility"
```

Expected: commit includes reducer visibility changes and tests.

## Task 3: Provide Visibility From AppShell

**Files:**
- Modify: `crates/noema-core/web/src/components/shell/AppShell.tsx`
- Modify: `crates/noema-core/web/src/components/shell/AppShell.test.ts`

- [ ] **Step 1: Write failing AppShell tests**

In `AppShell.test.ts`, add `shellRouteContentClassName` to the import from `./AppShell`:

```ts
import {
  AppShell,
  activeShellDestination,
  shellContentDeckClassName,
  shellRouteContentClassName,
  shellSidebarGroundClassName,
  shellAttentionForState,
  shellNavItems,
  type ShellAttentionInput
} from "./AppShell";
```

Add these tests inside `describe("AppShell layered deck markup", () => { ... })`:

```ts
test("marks route content with surface visibility", () => {
  const markup = renderShell();

  assert.match(markup, /data-slot="shell-route-content"/);
  assert.match(markup, /data-surface-visibility="visible"/);
});

test("disables route content pointer events while surface is not visible", () => {
  assert.doesNotMatch(shellRouteContentClassName("visible"), /pointer-events-none/);
  assert.match(shellRouteContentClassName("hiding"), /pointer-events-none/);
  assert.match(shellRouteContentClassName("hidden"), /pointer-events-none/);
  assert.match(shellRouteContentClassName("showing"), /pointer-events-none/);
});
```

- [ ] **Step 2: Run the shell test to verify it fails**

Run:

```bash
bun test src/components/shell/AppShell.test.ts
```

Expected: FAIL because `shellRouteContentClassName` and surface visibility markup are not implemented.

- [ ] **Step 3: Implement AppShell provider and transition settling**

Modify imports in `AppShell.tsx`:

```ts
import type { ShellSurfaceVisibility } from "./ShellSurfaceContext";
import { ShellSurfaceProvider } from "./ShellSurfaceContext";
import {
  deckTransitionPropertyCanSettleSurfaceVisibility,
  type DeckNavigationState,
  useDeckNavigation
} from "./deckNavigation";
```

Add helper:

```ts
export function shellRouteContentClassName(visibility: ShellSurfaceVisibility) {
  return cn("min-h-0 overflow-hidden", visibility !== "visible" && "pointer-events-none");
}
```

Destructure `settleSurfaceVisibility` from `useDeckNavigation()`:

```ts
const {
  state: deckNavigation,
  labels,
  menuButtonRef,
  openNav,
  closeNav,
  toggleSidebarCollapsed,
  settleSurfaceVisibility,
  navigateFromShell
} = useDeckNavigation(onNavigate);
```

Add `onTransitionEnd` to the content deck section:

```tsx
onTransitionEnd={(event) => {
  if (
    event.currentTarget === event.target &&
    deckTransitionPropertyCanSettleSurfaceVisibility(event.propertyName)
  ) {
    settleSurfaceVisibility();
  }
}}
```

Wrap route content:

```tsx
<ShellSurfaceProvider value={{ visibility: deckNavigation.surfaceVisibility }}>
  <div
    data-slot="shell-route-content"
    data-surface-visibility={deckNavigation.surfaceVisibility}
    className={shellRouteContentClassName(deckNavigation.surfaceVisibility)}
  >
    {children}
  </div>
</ShellSurfaceProvider>
```

- [ ] **Step 4: Run the shell test to verify it passes**

Run:

```bash
bun test src/components/shell/AppShell.test.ts
```

Expected: PASS.

- [ ] **Step 5: Commit Task 3**

Run:

```bash
git add crates/noema-core/web/src/components/shell/AppShell.tsx crates/noema-core/web/src/components/shell/AppShell.test.ts
git commit -m "feat(web): provide shell surface visibility"
```

Expected: commit includes `AppShell` provider wiring and tests.

## Task 4: Forward Composer Focus Without Shell Policy

**Files:**
- Modify: `crates/noema-core/web/src/components/Composer.tsx`
- Modify: `crates/noema-core/web/src/components/Composer.test.ts`

- [ ] **Step 1: Write failing Composer tests**

Keep or add this test in `Composer.test.ts`:

```ts
test("does not request native focus when mounted already ready", () => {
  const markup = renderComposer({ ready: true });

  assert.doesNotMatch(markup, /<textarea[^>]*autofocus=""/);
});
```

Remove any tests and imports for `shouldFocusComposerOnReadyChange` or `focusOnReady`; those concepts should not exist after this task.

- [ ] **Step 2: Run Composer tests to verify expected failure**

Run:

```bash
bun test src/components/Composer.test.ts
```

Expected: FAIL if `Composer` still renders native `autoFocus` or exposes the aborted `focusOnReady` policy.

- [ ] **Step 3: Implement forwarded ref**

In `Composer.tsx`, add props type:

```ts
export type ComposerProps = {
  value: string;
  ready: boolean;
  pending: boolean;
  placeholder: string;
  onChange: (value: string) => void;
  onSubmit: () => void;
};
```

Replace the component declaration with:

```tsx
export const Composer = React.forwardRef<HTMLTextAreaElement, ComposerProps>(function Composer(
  { value, ready, pending, placeholder, onChange, onSubmit },
  forwardedRef
) {
  const textareaRef = React.useRef<HTMLTextAreaElement | null>(null);

  const setTextareaRef = React.useCallback(
    (node: HTMLTextAreaElement | null) => {
      textareaRef.current = node;
      if (typeof forwardedRef === "function") {
        forwardedRef(node);
      } else if (forwardedRef) {
        forwardedRef.current = node;
      }
    },
    [forwardedRef]
  );
```

Use the callback ref on `Textarea`:

```tsx
<Textarea
  ref={setTextareaRef}
  value={value}
  disabled={isComposerTextareaDisabled({ ready })}
  placeholder={placeholder}
  rows={textareaProps.rows}
  style={textareaStyle}
  className={textareaProps.className}
  onChange={(event) => onChange(event.currentTarget.value)}
  onKeyDown={(event) => {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      submit();
    }
  }}
/>
```

Close the component with:

```tsx
});
```

Delete `focusOnReady`, `previousReadyRef`, and `shouldFocusComposerOnReadyChange` if present.

- [ ] **Step 4: Run Composer tests to verify pass**

Run:

```bash
bun test src/components/Composer.test.ts
```

Expected: PASS.

- [ ] **Step 5: Commit Task 4**

Run:

```bash
git add crates/noema-core/web/src/components/Composer.tsx crates/noema-core/web/src/components/Composer.test.ts
git commit -m "refactor(web): expose composer focus through ref"
```

Expected: commit contains only Composer ref/focus cleanup.

## Task 5: Move Chat Into A Surface Component

**Files:**
- Create: `crates/noema-core/web/src/components/ChatSurface.tsx`
- Create: `crates/noema-core/web/src/components/ChatSurface.test.tsx`
- Modify: `crates/noema-core/web/src/App.tsx`
- Modify: `crates/noema-core/web/src/App.test.ts`
- Modify: `crates/noema-core/web/src/routes.ts`

- [ ] **Step 1: Write failing ChatSurface tests**

Create `crates/noema-core/web/src/components/ChatSurface.test.tsx`:

```tsx
import assert from "node:assert/strict";
import { describe, test } from "node:test";
import * as React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { ShellSurfaceProvider } from "./shell/ShellSurfaceContext";
import { ChatSurface, shouldFocusChatComposer } from "./ChatSurface";

describe("shouldFocusChatComposer", () => {
  test("focuses only when chat is ready and the shell surface is visible", () => {
    assert.equal(shouldFocusChatComposer({ ready: true, visibility: "visible" }), true);
    assert.equal(shouldFocusChatComposer({ ready: true, visibility: "showing" }), false);
    assert.equal(shouldFocusChatComposer({ ready: true, visibility: "hidden" }), false);
    assert.equal(shouldFocusChatComposer({ ready: false, visibility: "visible" }), false);
  });
});

describe("ChatSurface", () => {
  test("renders chat content under shell visibility context without native autofocus", () => {
    const markup = renderToStaticMarkup(
      <ShellSurfaceProvider value={{ visibility: "showing" }}>
        <ChatSurface
          transcript={[]}
          pending={false}
          expandedActivities={new Set()}
          draft=""
          ready={true}
          agentName="Noema"
          onPickStarter={() => undefined}
          onToggleActivity={() => undefined}
          onDraftChange={() => undefined}
          onSubmit={() => undefined}
        />
      </ShellSurfaceProvider>
    );

    assert.match(markup, /aria-label="Noema chat"/);
    assert.doesNotMatch(markup, /<textarea[^>]*autofocus=""/);
  });
});
```

- [ ] **Step 2: Run ChatSurface test to verify it fails**

Run:

```bash
bun test src/components/ChatSurface.test.tsx
```

Expected: FAIL because `ChatSurface` does not exist.

- [ ] **Step 3: Implement ChatSurface**

Create `crates/noema-core/web/src/components/ChatSurface.tsx`:

```tsx
import React from "react";
import type { ShellSurfaceVisibility } from "./shell/ShellSurfaceContext";
import { useShellSurface } from "./shell/ShellSurfaceContext";
import { Composer } from "./Composer";
import { EmptyState } from "./EmptyState";
import { Transcript } from "./Transcript";
import type { TranscriptEntry } from "@/types";

export function shouldFocusChatComposer({
  ready,
  visibility
}: {
  ready: boolean;
  visibility: ShellSurfaceVisibility;
}) {
  return ready && visibility === "visible";
}

export function ChatSurface({
  transcript,
  pending,
  expandedActivities,
  draft,
  ready,
  agentName,
  onPickStarter,
  onToggleActivity,
  onDraftChange,
  onSubmit
}: {
  transcript: TranscriptEntry[];
  pending: boolean;
  expandedActivities: Set<string>;
  draft: string;
  ready: boolean;
  agentName: string | null;
  onPickStarter: (starter: string) => void;
  onToggleActivity: (id: string) => void;
  onDraftChange: (value: string) => void;
  onSubmit: () => void;
}) {
  const { visibility } = useShellSurface();
  const composerRef = React.useRef<HTMLTextAreaElement>(null);

  React.useEffect(() => {
    if (shouldFocusChatComposer({ ready, visibility })) {
      composerRef.current?.focus();
    }
  }, [ready, visibility]);

  const placeholder = !ready
    ? "Starting Noema chat..."
    : agentName?.trim()
      ? `Message ${agentName.trim()}`
      : "Send a message";

  return (
    <section
      className="grid h-full min-h-0 w-full grid-rows-[minmax(0,1fr)_auto] overflow-hidden pb-[22px] [--chat-column-width:min(860px,calc(100%_-_48px))] max-[760px]:pb-[18px] max-[760px]:[--chat-column-width:calc(100%_-_40px)]"
      aria-label="Noema chat"
    >
      {transcript.length === 0 ? (
        <EmptyState onPick={onPickStarter} />
      ) : (
        <Transcript
          entries={transcript}
          pending={pending}
          expandedActivities={expandedActivities}
          onToggleActivity={onToggleActivity}
        />
      )}

      <Composer
        ref={composerRef}
        value={draft}
        ready={ready}
        pending={pending}
        placeholder={placeholder}
        onChange={onDraftChange}
        onSubmit={onSubmit}
      />
    </section>
  );
}
```

- [ ] **Step 4: Move placeholder helper into ChatSurface**

Move `composerPlaceholder` from `App.tsx` into `ChatSurface.tsx`, and update the placeholder tests to import it from `./components/ChatSurface`.

Export this from `ChatSurface.tsx`:

```ts
export function composerPlaceholder({
  ready,
  agentName
}: {
  ready: boolean;
  agentName: string | null | undefined;
}) {
  if (!ready) {
    return "Starting Noema chat...";
  }

  const trimmedName = agentName?.trim();
  return trimmedName ? `Message ${trimmedName}` : "Send a message";
}
```

Replace the inline placeholder in `ChatSurface` with:

```ts
const placeholder = composerPlaceholder({ ready, agentName });
```

- [ ] **Step 5: Wire ChatSurface into App**

In `App.tsx`, remove imports of `Composer`, `EmptyState`, and `Transcript`. Add:

```ts
import { ChatSurface, composerPlaceholder } from "./components/ChatSurface";
```

Replace the current `chatView` JSX with:

```tsx
const chatView = (
  <ChatSurface
    transcript={transcript}
    pending={pending}
    expandedActivities={expandedActivities}
    draft={draft}
    ready={ready}
    agentName={agentName}
    onPickStarter={setDraft}
    onToggleActivity={(id) =>
      setExpandedActivities((current) => {
        const next = new Set(current);
        if (next.has(id)) {
          next.delete(id);
        } else {
          next.add(id);
        }
        return next;
      })
    }
    onDraftChange={setDraft}
    onSubmit={() => void sendMessage(draft)}
  />
);
```

Remove any aborted `shouldFocusComposerOnReady`, `AppRouteSource`, or `routeSource` code from `App.tsx`, `App.test.ts`, and `routes.ts`.

- [ ] **Step 6: Run focused tests**

Run:

```bash
bun test src/components/ChatSurface.test.tsx src/components/Composer.test.ts src/App.test.ts src/routes.test.ts
```

Expected: PASS.

- [ ] **Step 7: Commit Task 5**

Run:

```bash
git add crates/noema-core/web/src/components/ChatSurface.tsx crates/noema-core/web/src/components/ChatSurface.test.tsx crates/noema-core/web/src/App.tsx crates/noema-core/web/src/App.test.ts crates/noema-core/web/src/routes.ts
git commit -m "feat(web): focus chat after shell surface is visible"
```

Expected: commit contains chat surface extraction and focus gating.

## Task 6: Validate And Rebuild Web Assets

**Files:**
- Modify generated: `crates/noema-core/src/daemon/web/assets/app.js`
- Modify generated: `crates/noema-core/src/daemon/web/assets/index.html`
- Modify generated: `crates/noema-core/src/daemon/web/assets/styles.css`

- [ ] **Step 1: Run full web tests**

Run from `crates/noema-core/web`:

```bash
bun test
```

Expected: all tests pass with 0 failures.

- [ ] **Step 2: Run lint and type generation**

Run from `crates/noema-core/web`:

```bash
bun run lint
```

Expected: `gen:types`, `tsc --noEmit`, and `eslint src --max-warnings=0` all pass.

- [ ] **Step 3: Build web assets**

Run from `crates/noema-core/web`:

```bash
bun run build
```

Expected: Vite build succeeds and writes embedded assets under `../src/daemon/web/assets`.

- [ ] **Step 4: Check whitespace**

Run from repo root:

```bash
git diff --check
```

Expected: no output and exit code 0.

- [ ] **Step 5: Inspect final diff**

Run:

```bash
git status --short --branch
git diff --stat
```

Expected: only web shell/chat/source tests and generated web assets are dirty, plus any unrelated pre-existing files that were intentionally preserved.

- [ ] **Step 6: Commit generated assets**

Run:

```bash
git add crates/noema-core/src/daemon/web/assets/app.js crates/noema-core/src/daemon/web/assets/index.html crates/noema-core/src/daemon/web/assets/styles.css
git commit -m "build(web): update embedded shell surface assets"
```

Expected: commit contains only generated web assets.

## Task 7: Final Review

**Files:**
- Review: all files changed by Tasks 1-6

- [ ] **Step 1: Review git history for task commits**

Run:

```bash
git log --oneline -8
```

Expected: recent commits include context, visibility state, AppShell provider, Composer ref, ChatSurface focus, and generated assets.

- [ ] **Step 2: Confirm no route-source focus policy remains**

Run:

```bash
rg "routeSource|shouldFocusComposerOnReady|focusOnReady|visibleEpoch|routePhase|DECK_NAVIGATION_ROUTE_DEFER_MS|deckNavigationRouteTiming" crates/noema-core/web/src
```

Expected: no matches.

- [ ] **Step 3: Confirm shell surface API names**

Run:

```bash
rg "ShellSurface|surfaceVisibility|visibility" crates/noema-core/web/src/components/shell crates/noema-core/web/src/components/ChatSurface.tsx
```

Expected: matches show the formal context API, reducer visibility state, provider wiring, and chat consumption.

- [ ] **Step 4: Final status**

Run:

```bash
git status --short --branch
```

Expected: no unstaged/staged web changes remain. Report any unrelated dirty files separately.

## Self-Review Notes

- Spec coverage:
  - `ShellSurfaceContext` is covered by Task 1.
  - Visibility semantics and reducer flow are covered by Task 2.
  - Provider wrapping and transition settling are covered by Task 3.
  - Composer shell decoupling is covered by Task 4.
  - Chat focus gating and known regression paths are covered by Task 5.
  - Generated assets and validation are covered by Task 6.
- Placeholder scan:
  - No placeholder markers or unspecified implementation steps.
  - Conditional cleanup in Task 0 is bounded to a named list of interrupted web files.
- Type consistency:
  - `ShellSurfaceVisibility`, `ShellSurfaceState`, `ShellSurfaceProvider`, `useShellSurface`, and `surfaceVisibility` names are consistent across tasks.
  - The plan intentionally does not use `routePhase`, `visibleEpoch`, `routeSource`, or route deferral.
