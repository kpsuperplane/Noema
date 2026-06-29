# Sidebar Layout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the current top app header with an onboarded web shell that uses a desktop sidebar, a mobile drawer, `Home` for the primary conversation, and `Memory` for memory management.

**Architecture:** Keep the change frontend-only. Simplify route fallback so unknown paths resolve to the chat/home route, add a shared `AppShell` with pure navigation and attention helpers, render existing chat and memory pages inside that shell, and delete the old `AppHeader`/status-pill path. Setup and onboarding keep a small brand frame outside the onboarded shell.

**Tech Stack:** React 19, TypeScript, Tailwind CSS, shadcn/base-rhea UI primitives, Base UI sheet, lucide-react icons, Bun scripts, `node:test`.

---

## File Structure

- Modify `crates/noema-core/web/src/routes.ts`: remove the not-found route variant and make unknown paths fall back to chat/home.
- Modify `crates/noema-core/web/src/routes.test.ts`: update route tests for unknown-path fallback.
- Create `crates/noema-core/web/src/components/shell/AppShell.tsx`: shared onboarded shell, sidebar, mobile drawer, active navigation helpers, and exception-only attention helper.
- Create `crates/noema-core/web/src/components/shell/AppShell.test.ts`: pure helper tests for destination selection, navigation labels, and attention behavior.
- Create `crates/noema-core/web/src/components/shell/SetupFrame.tsx`: small brand frame for loading/setup/onboarding states after `AppHeader` is retired.
- Modify `crates/noema-core/web/src/App.tsx`: render setup states in `SetupFrame`, render onboarded routes in `AppShell`, remove the not-found branch, and keep existing chat/memory GraphQL flows.
- Modify `crates/noema-core/web/src/components/Onboarding.tsx`: let onboarding fill the new setup frame rather than compensating for the old app header height.
- Delete `crates/noema-core/web/src/components/shell/AppHeader.tsx`: retired top chrome.
- Delete `crates/noema-core/web/src/components/StatusCluster.tsx`: retired healthy status pill UI.
- Modify `docs/frontend/current-contract.md`: record that `Home` is the visible primary conversation label and unknown paths fall back to `Home`.
- Modify `docs/frontend/navigation-workflows.md`: record the current sidebar/drawer shell and exception-only attention model.

## Task 1: Route Fallback

**Files:**
- Modify: `crates/noema-core/web/src/routes.ts`
- Modify: `crates/noema-core/web/src/routes.test.ts`

- [ ] **Step 1: Write the failing route test**

Update `crates/noema-core/web/src/routes.test.ts` so unknown paths are expected to map to chat/home:

```ts
import assert from "node:assert/strict";
import { describe, test } from "node:test";
import { pathForRoute, routeFromPathname } from "./routes";

describe("routeFromPathname", () => {
  test("recognizes chat and memory routes", () => {
    assert.deepEqual(routeFromPathname("/"), { kind: "chat" });
    assert.deepEqual(routeFromPathname("/chat"), { kind: "chat" });
    assert.deepEqual(routeFromPathname("/memory"), { kind: "memory_home" });
    assert.deepEqual(routeFromPathname("/memory/graph"), { kind: "memory_graph" });
  });

  test("falls back to chat for unknown routes", () => {
    assert.deepEqual(routeFromPathname("/memory/nope"), { kind: "chat" });
    assert.deepEqual(routeFromPathname("/not-a-real-route"), { kind: "chat" });
  });
});

describe("pathForRoute", () => {
  test("returns canonical paths", () => {
    assert.equal(pathForRoute({ kind: "chat" }), "/");
    assert.equal(pathForRoute({ kind: "memory_home" }), "/memory");
    assert.equal(pathForRoute({ kind: "memory_graph" }), "/memory/graph");
  });
});
```

- [ ] **Step 2: Run the route test to verify it fails**

Run:

```bash
cd crates/noema-core/web
bun test src/routes.test.ts
```

Expected: FAIL because `routeFromPathname("/memory/nope")` still returns `{ kind: "not_found", path: "/memory/nope" }`.

- [ ] **Step 3: Implement route fallback**

Replace `crates/noema-core/web/src/routes.ts` with:

```ts
import React from "react";

export type AppRoute =
  | { kind: "chat" }
  | { kind: "memory_home" }
  | { kind: "memory_graph" };

export function routeFromPathname(pathname: string): AppRoute {
  if (pathname === "/memory") {
    return { kind: "memory_home" };
  }
  if (pathname === "/memory/graph") {
    return { kind: "memory_graph" };
  }
  return { kind: "chat" };
}

export function pathForRoute(route: AppRoute): string {
  if (route.kind === "memory_home") {
    return "/memory";
  }
  if (route.kind === "memory_graph") {
    return "/memory/graph";
  }
  return "/";
}

export function useBrowserRoute() {
  const [route, setRoute] = React.useState(() => routeFromPathname(window.location.pathname));

  React.useEffect(() => {
    const onPopState = () => setRoute(routeFromPathname(window.location.pathname));
    window.addEventListener("popstate", onPopState);
    return () => window.removeEventListener("popstate", onPopState);
  }, []);

  const navigate = React.useCallback((nextRoute: AppRoute) => {
    const nextPath = pathForRoute(nextRoute);
    window.history.pushState({}, "", nextPath);
    setRoute(routeFromPathname(nextPath));
  }, []);

  return { route, navigate };
}
```

- [ ] **Step 4: Run the route test to verify it passes**

Run:

```bash
cd crates/noema-core/web
bun test src/routes.test.ts
```

Expected: PASS.

- [ ] **Step 5: Commit route fallback**

Run:

```bash
git add crates/noema-core/web/src/routes.ts crates/noema-core/web/src/routes.test.ts
git commit -m "feat: route unknown web paths home"
```

## Task 2: Shell Helper Tests

**Files:**
- Create: `crates/noema-core/web/src/components/shell/AppShell.test.ts`
- Create: `crates/noema-core/web/src/components/shell/AppShell.tsx`

- [ ] **Step 1: Write failing helper tests**

Create `crates/noema-core/web/src/components/shell/AppShell.test.ts`:

```ts
import assert from "node:assert/strict";
import { describe, test } from "node:test";
import {
  activeShellDestination,
  shellAttentionForState,
  shellNavItems,
  type ShellAttentionInput
} from "./AppShell";

const healthyStatus = {
  localService: "RUNNING",
  assistantConnection: "CODEX",
  memoryStorage: "READY"
} as const;

describe("shell navigation helpers", () => {
  test("labels the primary conversation as Home", () => {
    assert.deepEqual(
      shellNavItems.map((item) => item.label),
      ["Home", "Memory"]
    );
  });

  test("marks chat routes as Home and memory routes as Memory", () => {
    assert.equal(activeShellDestination({ kind: "chat" }), "home");
    assert.equal(activeShellDestination({ kind: "memory_home" }), "memory");
    assert.equal(activeShellDestination({ kind: "memory_graph" }), "memory");
  });
});

describe("shell attention helper", () => {
  test("returns no attention item for healthy state", () => {
    const input: ShellAttentionInput = {
      route: { kind: "chat" },
      status: healthyStatus,
      socketState: "ready",
      providerBlocked: false,
      setupBlocked: false
    };

    assert.equal(shellAttentionForState(input), null);
  });

  test("prioritizes setup and provider blocked states", () => {
    assert.deepEqual(
      shellAttentionForState({
        route: { kind: "chat" },
        status: healthyStatus,
        socketState: "ready",
        providerBlocked: true,
        setupBlocked: false
      }),
      {
        tone: "warning",
        title: "Connection needed",
        message: "Codex sign-in needs attention."
      }
    );

    assert.deepEqual(
      shellAttentionForState({
        route: { kind: "chat" },
        status: healthyStatus,
        socketState: "ready",
        providerBlocked: false,
        setupBlocked: true
      }),
      {
        tone: "warning",
        title: "Setup needed",
        message: "Noema needs setup before this surface is ready."
      }
    );
  });

  test("flags chat and memory degradation only when it matters", () => {
    assert.deepEqual(
      shellAttentionForState({
        route: { kind: "chat" },
        status: healthyStatus,
        socketState: "closed",
        providerBlocked: false,
        setupBlocked: false
      }),
      {
        tone: "warning",
        title: "Chat disconnected",
        message: "Reconnect before sending another message."
      }
    );

    assert.deepEqual(
      shellAttentionForState({
        route: { kind: "memory_home" },
        status: {
          localService: "RUNNING",
          assistantConnection: "CODEX",
          memoryStorage: "UNAVAILABLE"
        },
        socketState: "ready",
        providerBlocked: false,
        setupBlocked: false
      }),
      {
        tone: "warning",
        title: "Memory unavailable",
        message: "Memory storage is not ready for this page."
      }
    );
  });
});
```

- [ ] **Step 2: Add a temporary export stub so the failure is about behavior**

Create `crates/noema-core/web/src/components/shell/AppShell.tsx`:

```tsx
import type { LocalStatusQuery } from "@/generated/graphql";
import type { AppRoute } from "@/routes";
import type { SocketState } from "@/types";

export type ShellDestination = "home" | "memory";

export type ShellNavItem = {
  destination: ShellDestination;
  label: string;
  route: AppRoute;
};

export type ShellAttention = {
  tone: "warning";
  title: string;
  message: string;
};

export type ShellAttentionInput = {
  route: AppRoute;
  status: LocalStatusQuery["localStatus"] | null;
  socketState: SocketState;
  providerBlocked: boolean;
  setupBlocked: boolean;
};

export const shellNavItems: ShellNavItem[] = [];

export function activeShellDestination(_route: AppRoute): ShellDestination {
  return "home";
}

export function shellAttentionForState(_input: ShellAttentionInput): ShellAttention | null {
  return null;
}
```

- [ ] **Step 3: Run the helper test to verify it fails**

Run:

```bash
cd crates/noema-core/web
bun test src/components/shell/AppShell.test.ts
```

Expected: FAIL because `shellNavItems` is empty and memory routes are not active as `Memory`.

- [ ] **Step 4: Implement the helper behavior**

Replace `crates/noema-core/web/src/components/shell/AppShell.tsx` with:

```tsx
import type { LocalStatusQuery } from "@/generated/graphql";
import type { AppRoute } from "@/routes";
import type { SocketState } from "@/types";

export type ShellDestination = "home" | "memory";

export type ShellNavItem = {
  destination: ShellDestination;
  label: string;
  route: AppRoute;
};

export type ShellAttention = {
  tone: "warning";
  title: string;
  message: string;
};

export type ShellAttentionInput = {
  route: AppRoute;
  status: LocalStatusQuery["localStatus"] | null;
  socketState: SocketState;
  providerBlocked: boolean;
  setupBlocked: boolean;
};

export const shellNavItems: ShellNavItem[] = [
  { destination: "home", label: "Home", route: { kind: "chat" } },
  { destination: "memory", label: "Memory", route: { kind: "memory_home" } }
];

export function activeShellDestination(route: AppRoute): ShellDestination {
  if (route.kind === "memory_home" || route.kind === "memory_graph") {
    return "memory";
  }
  return "home";
}

export function shellAttentionForState(input: ShellAttentionInput): ShellAttention | null {
  if (input.setupBlocked) {
    return {
      tone: "warning",
      title: "Setup needed",
      message: "Noema needs setup before this surface is ready."
    };
  }

  if (input.providerBlocked) {
    return {
      tone: "warning",
      title: "Connection needed",
      message: "Codex sign-in needs attention."
    };
  }

  if (input.route.kind === "chat" && input.socketState === "closed") {
    return {
      tone: "warning",
      title: "Chat disconnected",
      message: "Reconnect before sending another message."
    };
  }

  if (
    (input.route.kind === "memory_home" || input.route.kind === "memory_graph") &&
    input.status?.memoryStorage === "UNAVAILABLE"
  ) {
    return {
      tone: "warning",
      title: "Memory unavailable",
      message: "Memory storage is not ready for this page."
    };
  }

  return null;
}
```

- [ ] **Step 5: Run the helper test to verify it passes**

Run:

```bash
cd crates/noema-core/web
bun test src/components/shell/AppShell.test.ts
```

Expected: PASS.

- [ ] **Step 6: Commit shell helpers**

Run:

```bash
git add crates/noema-core/web/src/components/shell/AppShell.tsx crates/noema-core/web/src/components/shell/AppShell.test.ts
git commit -m "feat: add web shell navigation helpers"
```

## Task 3: App Shell UI

**Files:**
- Modify: `crates/noema-core/web/src/components/shell/AppShell.tsx`

- [ ] **Step 1: Extend `AppShell.tsx` with the shell component**

Replace `crates/noema-core/web/src/components/shell/AppShell.tsx` with:

```tsx
import React from "react";
import { AlertTriangle, Brain, House, Menu } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
  SheetTrigger
} from "@/components/ui/sheet";
import type { LocalStatusQuery } from "@/generated/graphql";
import { cn } from "@/lib/utils";
import type { AppRoute } from "@/routes";
import type { SocketState } from "@/types";

export type ShellDestination = "home" | "memory";

export type ShellNavItem = {
  destination: ShellDestination;
  label: string;
  route: AppRoute;
};

export type ShellAttention = {
  tone: "warning";
  title: string;
  message: string;
};

export type ShellAttentionInput = {
  route: AppRoute;
  status: LocalStatusQuery["localStatus"] | null;
  socketState: SocketState;
  providerBlocked: boolean;
  setupBlocked: boolean;
};

export const shellNavItems: ShellNavItem[] = [
  { destination: "home", label: "Home", route: { kind: "chat" } },
  { destination: "memory", label: "Memory", route: { kind: "memory_home" } }
];

const navIcon: Record<ShellDestination, React.ComponentType<{ className?: string; "aria-hidden"?: boolean }>> = {
  home: House,
  memory: Brain
};

export function activeShellDestination(route: AppRoute): ShellDestination {
  if (route.kind === "memory_home" || route.kind === "memory_graph") {
    return "memory";
  }
  return "home";
}

export function shellAttentionForState(input: ShellAttentionInput): ShellAttention | null {
  if (input.setupBlocked) {
    return {
      tone: "warning",
      title: "Setup needed",
      message: "Noema needs setup before this surface is ready."
    };
  }

  if (input.providerBlocked) {
    return {
      tone: "warning",
      title: "Connection needed",
      message: "Codex sign-in needs attention."
    };
  }

  if (input.route.kind === "chat" && input.socketState === "closed") {
    return {
      tone: "warning",
      title: "Chat disconnected",
      message: "Reconnect before sending another message."
    };
  }

  if (
    (input.route.kind === "memory_home" || input.route.kind === "memory_graph") &&
    input.status?.memoryStorage === "UNAVAILABLE"
  ) {
    return {
      tone: "warning",
      title: "Memory unavailable",
      message: "Memory storage is not ready for this page."
    };
  }

  return null;
}

export function AppShell({
  route,
  status,
  socketState,
  providerBlocked = false,
  setupBlocked = false,
  onNavigate,
  children
}: {
  route: AppRoute;
  status: LocalStatusQuery["localStatus"] | null;
  socketState: SocketState;
  providerBlocked?: boolean;
  setupBlocked?: boolean;
  onNavigate: (route: AppRoute) => void;
  children: React.ReactNode;
}) {
  const [drawerOpen, setDrawerOpen] = React.useState(false);
  const activeDestination = activeShellDestination(route);
  const activeLabel = shellNavItems.find((item) => item.destination === activeDestination)?.label ?? "Home";
  const attention = shellAttentionForState({
    route,
    status,
    socketState,
    providerBlocked,
    setupBlocked
  });

  const navigateFromShell = React.useCallback(
    (nextRoute: AppRoute) => {
      onNavigate(nextRoute);
      setDrawerOpen(false);
    },
    [onNavigate]
  );

  return (
    <main className="grid h-dvh min-h-screen grid-cols-[236px_minmax(0,1fr)] overflow-hidden bg-background max-[760px]:grid-cols-1 max-[760px]:grid-rows-[auto_minmax(0,1fr)]">
      <aside className="grid min-h-0 border-r border-[var(--border-subtle)] bg-[var(--surface-sunken)] px-3.5 py-4 max-[760px]:hidden">
        <ShellSidebar activeDestination={activeDestination} attention={attention} onNavigate={navigateFromShell} />
      </aside>

      <div className="hidden min-h-[56px] items-center gap-3 border-b border-[var(--border-subtle)] bg-white/95 px-4 max-[760px]:flex">
        <Sheet open={drawerOpen} onOpenChange={setDrawerOpen}>
          <SheetTrigger
            render={
              <Button type="button" variant="ghost" size="icon" aria-label="Open navigation" />
            }
          >
            <Menu aria-hidden="true" />
          </SheetTrigger>
          <SheetContent side="left" className="w-[min(320px,86vw)] bg-[var(--surface-sunken)] p-0" showCloseButton={false}>
            <SheetHeader className="sr-only">
              <SheetTitle>Noema navigation</SheetTitle>
              <SheetDescription>Choose Home or Memory.</SheetDescription>
            </SheetHeader>
            <div className="grid h-full p-3.5">
              <ShellSidebar activeDestination={activeDestination} attention={attention} onNavigate={navigateFromShell} />
            </div>
          </SheetContent>
        </Sheet>
        <div className="min-w-0">
          <strong className="block truncate font-heading text-base tracking-normal">{activeLabel}</strong>
          <span className="block truncate text-xs text-muted-foreground">
            {activeDestination === "home" ? "Primary conversation" : "Memory management"}
          </span>
        </div>
      </div>

      <div className="min-h-0 overflow-hidden">{children}</div>
    </main>
  );
}

function ShellSidebar({
  activeDestination,
  attention,
  onNavigate
}: {
  activeDestination: ShellDestination;
  attention: ShellAttention | null;
  onNavigate: (route: AppRoute) => void;
}) {
  return (
    <div className="grid h-full min-h-0 grid-rows-[auto_auto_minmax(0,1fr)] gap-4">
      <div className="flex min-w-0 items-center gap-2.5 px-2">
        <img src="/assets/noema-mark.svg" width="32" height="32" alt="" />
        <div className="min-w-0">
          <strong className="block truncate font-heading text-base tracking-normal">Noema</strong>
          <span className="block truncate text-xs text-muted-foreground">Local agent OS</span>
        </div>
      </div>

      {attention ? <ShellAttentionItem attention={attention} /> : <div aria-hidden="true" />}

      <nav className="grid content-start gap-1" aria-label="Primary">
        {shellNavItems.map((item) => {
          const Icon = navIcon[item.destination];
          const active = item.destination === activeDestination;
          return (
            <Button
              key={item.destination}
              type="button"
              variant="ghost"
              className={cn(
                "h-9 justify-start rounded-md px-2.5 text-sm text-muted-foreground",
                active && "bg-[var(--pine-50)] text-[var(--pine-700)] hover:bg-[var(--pine-50)] hover:text-[var(--pine-700)]"
              )}
              aria-current={active ? "page" : undefined}
              onClick={() => onNavigate(item.route)}
            >
              <Icon className="size-4" aria-hidden="true" />
              {item.label}
            </Button>
          );
        })}
      </nav>
    </div>
  );
}

function ShellAttentionItem({ attention }: { attention: ShellAttention }) {
  return (
    <div
      className="grid gap-1 rounded-md border border-[color-mix(in_srgb,var(--clay-600)_32%,transparent)] bg-[var(--clay-50)] px-3 py-2.5 text-[var(--red-700)]"
      role="status"
    >
      <span className="flex items-center gap-2 text-xs font-semibold">
        <AlertTriangle className="size-3.5" aria-hidden="true" />
        {attention.title}
      </span>
      <span className="text-xs leading-snug">{attention.message}</span>
    </div>
  );
}
```

- [ ] **Step 2: Run the AppShell helper tests**

Run:

```bash
cd crates/noema-core/web
bun test src/components/shell/AppShell.test.ts
```

Expected: PASS.

- [ ] **Step 3: Run TypeScript through lint to catch component integration errors**

Run:

```bash
cd crates/noema-core/web
bun run lint
```

Expected: PASS. If TypeScript reports a prop mismatch in `AppShell.tsx`, fix the component before moving on.

- [ ] **Step 4: Commit shell UI**

Run:

```bash
git add crates/noema-core/web/src/components/shell/AppShell.tsx crates/noema-core/web/src/components/shell/AppShell.test.ts
git commit -m "feat: add sidebar app shell"
```

## Task 4: Setup Frame And App Integration

**Files:**
- Create: `crates/noema-core/web/src/components/shell/SetupFrame.tsx`
- Modify: `crates/noema-core/web/src/components/Onboarding.tsx`
- Modify: `crates/noema-core/web/src/App.tsx`

- [ ] **Step 1: Create setup frame**

Create `crates/noema-core/web/src/components/shell/SetupFrame.tsx`:

```tsx
import type { ReactNode } from "react";

export function SetupFrame({ children }: { children: ReactNode }) {
  return (
    <main className="grid h-dvh min-h-screen grid-rows-[64px_minmax(0,1fr)] overflow-hidden bg-background">
      <div className="flex min-w-0 items-center gap-2.5 border-b border-[var(--border-subtle)] bg-white/95 px-5">
        <img src="/assets/noema-mark.svg" width="32" height="32" alt="" />
        <div className="min-w-0">
          <strong className="block truncate font-heading text-base tracking-normal">Noema</strong>
          <span className="block truncate text-xs text-muted-foreground">Local setup</span>
        </div>
      </div>
      <div className="min-h-0 overflow-auto">{children}</div>
    </main>
  );
}
```

- [ ] **Step 2: Let onboarding fill the setup frame**

In `crates/noema-core/web/src/components/Onboarding.tsx`, replace the outer `<section>` className with:

```tsx
className="mx-auto grid min-h-full w-[min(760px,100%)] content-center px-6 py-[34px] max-[760px]:content-start max-[760px]:px-5 max-[760px]:py-7"
```

The resulting opening section should be:

```tsx
    <section
      className="mx-auto grid min-h-full w-[min(760px,100%)] content-center px-6 py-[34px] max-[760px]:content-start max-[760px]:px-5 max-[760px]:py-7"
      aria-label="Noema onboarding"
    >
```

- [ ] **Step 3: Import the new shell components in `App.tsx`**

At the top of `crates/noema-core/web/src/App.tsx`:

Remove:

```ts
import { AppHeader } from "@/components/shell/AppHeader";
import { Button } from "@/components/ui/button";
```

Add:

```ts
import { AppShell } from "@/components/shell/AppShell";
import { SetupFrame } from "@/components/shell/SetupFrame";
```

- [ ] **Step 4: Replace the loading setup branch**

In `crates/noema-core/web/src/App.tsx`, replace the `if (!onboarding)` return block with:

```tsx
  if (!onboarding) {
    return (
      <SetupFrame>
        <section
          className="mx-auto grid min-h-full w-[min(760px,100%)] content-center px-6 max-[760px]:content-start max-[760px]:px-5"
          aria-label="Noema onboarding"
        >
          <div className="grid min-w-0 gap-3.5 py-[18px]">
            <p className="m-0 font-mono text-[11px] tracking-[0.12em] text-[var(--text-accent)] uppercase">First run</p>
            <h1 className="m-0 font-heading text-[34px] leading-[1.1] tracking-normal text-foreground [overflow-wrap:anywhere] max-[760px]:text-3xl">
              Checking setup
            </h1>
            <p className="m-0 max-w-[560px] text-muted-foreground [overflow-wrap:anywhere]">
              Noema is checking whether chat can start.
            </p>
            {displayedOnboardingError ? <ErrorMarker message={displayedOnboardingError} /> : null}
          </div>
        </section>
      </SetupFrame>
    );
  }
```

- [ ] **Step 5: Replace the not-onboarded branch**

In `crates/noema-core/web/src/App.tsx`, replace the `if (!onboarding.isUserOnboarded)` return block with:

```tsx
  if (!onboarding.isUserOnboarded) {
    return (
      <SetupFrame>
        <Onboarding
          onboarding={onboarding}
          attempt={authAttempt}
          error={displayedOnboardingError}
          onConnect={() => void connectProvider()}
          onRetry={() => {
            setAuthAttempt(null);
            setOnboardingError(null);
          }}
        />
      </SetupFrame>
    );
  }
```

- [ ] **Step 6: Create a local chat view constant**

In `crates/noema-core/web/src/App.tsx`, after `const ready = socketState === "ready" && conversationId !== null;` and the waiting constants, add:

```tsx
  const chatView = (
    <section
      className="grid h-full min-h-0 w-full grid-rows-[minmax(0,1fr)_auto] overflow-hidden pb-[22px] [--chat-column-width:min(860px,calc(100%_-_48px))] max-[760px]:pb-[18px] max-[760px]:[--chat-column-width:calc(100%_-_40px)]"
      aria-label="Noema chat"
    >
      {transcript.length === 0 ? (
        <EmptyState onPick={(starter) => setDraft(starter)} />
      ) : (
        <Transcript
          entries={transcript}
          pending={pending}
          expandedActivities={expandedActivities}
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
        />
      )}

      <Composer
        value={draft}
        ready={ready}
        pending={pending}
        placeholder={ready ? "Message Noema" : "Starting Noema chat..."}
        onChange={setDraft}
        onSubmit={() => void sendMessage(draft)}
      />
    </section>
  );
```

- [ ] **Step 7: Render memory routes inside `AppShell`**

In `crates/noema-core/web/src/App.tsx`, replace the `memory_home` and `memory_graph` route branches with:

```tsx
  if (route.kind === "memory_home") {
    return (
      <AppShell route={route} status={status} socketState={socketState} onNavigate={navigate}>
        <MemoryHomePage onOpenGraph={() => navigate({ kind: "memory_graph" })} />
      </AppShell>
    );
  }

  if (route.kind === "memory_graph") {
    return (
      <AppShell route={route} status={status} socketState={socketState} onNavigate={navigate}>
        <MemoryGraphPage />
      </AppShell>
    );
  }
```

- [ ] **Step 8: Remove the not-found branch and render chat inside `AppShell`**

In `crates/noema-core/web/src/App.tsx`, delete the entire `if (route.kind === "not_found")` branch.

Replace the final return with:

```tsx
  return (
    <AppShell route={route} status={status} socketState={socketState} onNavigate={navigate}>
      {chatView}
    </AppShell>
  );
}
```

- [ ] **Step 9: Run focused tests**

Run:

```bash
cd crates/noema-core/web
bun test src/routes.test.ts src/components/shell/AppShell.test.ts src/components/Onboarding.test.ts src/App.test.ts
```

Expected: PASS.

- [ ] **Step 10: Run lint**

Run:

```bash
cd crates/noema-core/web
bun run lint
```

Expected: PASS. If lint reports an unused import from the old header or button path, remove that import and rerun the command.

- [ ] **Step 11: Commit app integration**

Run:

```bash
git add crates/noema-core/web/src/App.tsx crates/noema-core/web/src/components/Onboarding.tsx crates/noema-core/web/src/components/shell/SetupFrame.tsx
git commit -m "feat: render web routes in sidebar shell"
```

## Task 5: Retire Header And Status Pill Components

**Files:**
- Delete: `crates/noema-core/web/src/components/shell/AppHeader.tsx`
- Delete: `crates/noema-core/web/src/components/StatusCluster.tsx`

- [ ] **Step 1: Confirm old components are unused**

Run:

```bash
rg -n "AppHeader|StatusCluster" crates/noema-core/web/src -g '!components/shell/AppHeader.tsx' -g '!components/StatusCluster.tsx'
```

Expected: no matches.

- [ ] **Step 2: Delete retired components**

Delete:

```text
crates/noema-core/web/src/components/shell/AppHeader.tsx
crates/noema-core/web/src/components/StatusCluster.tsx
```

- [ ] **Step 3: Run lint**

Run:

```bash
cd crates/noema-core/web
bun run lint
```

Expected: PASS.

- [ ] **Step 4: Commit component removal**

Run:

```bash
git add crates/noema-core/web/src/components/shell/AppHeader.tsx crates/noema-core/web/src/components/StatusCluster.tsx
git commit -m "refactor: retire web app header"
```

## Task 6: Frontend IA Documentation

**Files:**
- Modify: `docs/frontend/current-contract.md`
- Modify: `docs/frontend/navigation-workflows.md`

- [ ] **Step 1: Update the route matrix label**

In `docs/frontend/current-contract.md`, change the route matrix row for `/` from label `Chat` to `Home`, and make the capability text name the primary conversation:

```markdown
| `/` | Home | setup health, local service state, primary conversation | route to the durable primary conversation when ready; show guided readiness state if blocked | Current |
```

- [ ] **Step 2: Add the current shell note**

In `docs/frontend/current-contract.md`, add this paragraph after the route matrix:

```markdown
Current shell note: after onboarding, the web UI uses a sidebar-first shell.
`Home` is the visible label for the durable primary conversation, not a
dashboard. `Memory` is the only other primary sidebar destination in the
current slice. Unknown browser paths fall back to `Home`.
```

- [ ] **Step 3: Update the navigation model**

In `docs/frontend/navigation-workflows.md`, replace the paragraph that begins `User-facing labels should favor clear product nouns` with:

```markdown
User-facing labels should favor clear product nouns over architecture nouns. In
the current slice, the primary visible destinations are `Home` and `Memory`.
`Home` is the durable primary conversation, not an operational dashboard.
`Memory` opens the memory management surface. Terms like `Governance`, `Audit`,
`RunEnvelope`, and `ContextPacket` belong in advanced inspection panels until
the user has a reason to inspect them.
```

- [ ] **Step 4: Add the shell behavior note**

In `docs/frontend/navigation-workflows.md`, add this paragraph after the `### Surface Order` table:

```markdown
Current shell behavior: desktop shows a persistent left sidebar, while narrow
screens use a drawer opened from a compact top bar. Healthy setup, service,
chat, provider, and memory state are silent in the shell. The shell shows a
single compact attention item only when state is degraded or action-worthy; the
active page owns detailed recovery UI.
```

- [ ] **Step 5: Run docs diff check**

Run:

```bash
git diff --check docs/frontend/current-contract.md docs/frontend/navigation-workflows.md
```

Expected: PASS with no whitespace errors.

- [ ] **Step 6: Commit docs**

Run:

```bash
git add docs/frontend/current-contract.md docs/frontend/navigation-workflows.md
git commit -m "docs: record sidebar web shell"
```

## Task 7: Full Frontend Validation

**Files:**
- Validate: `crates/noema-core/web`

- [ ] **Step 1: Run all web tests**

Run:

```bash
cd crates/noema-core/web
bun test
```

Expected: PASS.

- [ ] **Step 2: Run web lint**

Run:

```bash
cd crates/noema-core/web
bun run lint
```

Expected: PASS.

- [ ] **Step 3: Run web build**

Run:

```bash
cd crates/noema-core/web
bun run build
```

Expected: PASS.

- [ ] **Step 4: Run final git checks**

Run:

```bash
git status --short --branch
git diff --check
git diff --cached --stat
git diff --cached --name-status
```

Expected:

- `git diff --check` prints no errors.
- `git diff --cached --stat` and `git diff --cached --name-status` are empty unless a final release commit is being prepared.
- `git status --short --branch` may still show unrelated pre-existing files, but it should not show partially completed sidebar-shell changes.

- [ ] **Step 5: Report remaining worktree state**

In the final implementation summary, report any untracked or unstaged files that were not part of the sidebar shell work. At plan-writing time, the known unrelated entries were:

```text
crates/noema-core/src/daemon.rs
crates/noema-core/src/daemon/runtime.rs
crates/noema-core/src/daemon/prompts.rs
.superpowers/
```

Do not stage or modify those files as part of this plan.
