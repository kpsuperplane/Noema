import assert from "node:assert/strict";
import { describe, test } from "node:test";
import * as React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import {
  AppShell,
  activeShellDestination,
  shellContentDeckClassName,
  shellSidebarGroundClassName,
  shellAttentionForState,
  shellNavItems,
  type ShellAttentionInput
} from "./AppShell";
import {
  deckNavigationControlLabels,
  deckNavigationReducer,
  initialDeckNavigationState,
  type DeckNavigationState
} from "./deckNavigation";

const healthyStatus = {
  localService: "RUNNING",
  assistantConnection: "CODEX",
  memoryStorage: "READY",
  primaryAgentDisplayName: null
} as const;

describe("shell navigation helpers", () => {
  test("defines the primary shell navigation contract", () => {
    assert.deepEqual(shellNavItems, [
      { destination: "home", label: "Home", route: { kind: "chat" } },
      { destination: "memory", label: "Memory", route: { kind: "memory_home" } }
    ]);
  });

  test("marks chat routes as Home and memory routes as Memory", () => {
    assert.equal(activeShellDestination({ kind: "chat" }), "home");
    assert.equal(activeShellDestination({ kind: "memory_home" }), "memory");
    assert.equal(activeShellDestination({ kind: "memory_graph" }), "memory");
  });
});

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
      deckNavigationReducer(
        { navOpen: true, sidebarCollapsed: true },
        {
          type: "toggleSidebarCollapsed"
        }
      ),
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

    assert.deepEqual(deckNavigationControlLabels({ navOpen: true, sidebarCollapsed: true }), {
      menu: "Close navigation",
      collapse: "Expand sidebar"
    });
  });
});

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
    assert.match(markup, /data-slot="shell-deck-collapse-button"/);
    assert.doesNotMatch(markup, /data-slot="shell-sidebar-collapse-button"/);
    assert.match(markup, />Memory</);
    assert.match(markup, />Memory management</);
  });

  test("uses the collapse toggle as the only wide header affordance", () => {
    const markup = renderShell();
    const menuClassName = dataSlotClassName(markup, "shell-menu-button");
    const collapseClassName = dataSlotClassName(markup, "shell-deck-collapse-button");

    assert.match(menuClassName, /min-\[761px\]:!hidden/);
    assert.match(collapseClassName, /max-\[760px\]:hidden/);
  });

  test("keeps the shell header row compact above short pages", () => {
    const className = shellContentDeckClassName(initialDeckNavigationState);

    assert.match(className, /grid-rows-\[auto_minmax\(0,1fr\)\]/);
  });

  test("reveals mobile navigation without shrinking the deck vertically", () => {
    const className = shellContentDeckClassName({
      navOpen: true,
      sidebarCollapsed: true
    });

    assert.match(
      className,
      /transition-\[inset,left,right,transform,border-radius,box-shadow\]/
    );
    assert.match(className, /min-\[761px\]:scale-\[0\.97\]/);
    assert.doesNotMatch(className, /(^|\s)scale-\[0\.97\](\s|$)/);
    assert.doesNotMatch(className, /max-\[760px\]:translate-x-\[min\(252px,72vw\)\]/);
    assert.match(className, /max-\[760px\]:left-\[min\(252px,72vw\)\]/);
    assert.match(className, /max-\[760px\]:right-\[calc\(min\(252px,72vw\)\*-1\)\]/);
    assert.match(className, /max-\[760px\]:translate-x-0/);
  });

  test("keeps the content deck shadow on mobile", () => {
    const className = shellContentDeckClassName(initialDeckNavigationState);

    assert.match(className, /shadow-\[0_24px_70px_rgba\(31,38,30,0\.18\)\]/);
    assert.doesNotMatch(className, /max-\[760px\]:shadow-none/);
  });

  test("keeps revealed sidebar clickable above the dismissal backdrop", () => {
    const closedClassName = shellSidebarGroundClassName(initialDeckNavigationState);
    const openClassName = shellSidebarGroundClassName({
      navOpen: true,
      sidebarCollapsed: true
    });

    assert.match(closedClassName, /\bz-10\b/);
    assert.doesNotMatch(closedClassName, /z-\[25\]/);
    assert.match(openClassName, /z-\[25\]/);
    assert.doesNotMatch(openClassName, /\bz-30\b/);
  });

  test("does not render sheet markup and includes reduced-motion deck handling", () => {
    const markup = renderShell();
    const deckClassName = dataSlotClassName(markup, "shell-content-deck");

    assert.doesNotMatch(markup, /data-slot="sheet"/);
    assert.doesNotMatch(markup, /data-slot="sheet-content"/);
    assert.match(deckClassName, /\bmotion-reduce:transition-none\b/);
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

  test("prioritizes overlapping attention states", () => {
    assert.deepEqual(
      shellAttentionForState({
        route: { kind: "chat" },
        status: healthyStatus,
        socketState: "ready",
        providerBlocked: true,
        setupBlocked: true
      }),
      {
        tone: "warning",
        title: "Setup needed",
        message: "Noema needs setup before this surface is ready."
      }
    );

    assert.deepEqual(
      shellAttentionForState({
        route: { kind: "chat" },
        status: healthyStatus,
        socketState: "closed",
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
        route: { kind: "memory_graph" },
        status: {
          localService: "RUNNING",
          assistantConnection: "CODEX",
          memoryStorage: "UNAVAILABLE",
          primaryAgentDisplayName: null
        },
        socketState: "closed",
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
          memoryStorage: "UNAVAILABLE",
          primaryAgentDisplayName: null
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

  test("ignores degradation outside its route scope", () => {
    assert.equal(
      shellAttentionForState({
        route: { kind: "chat" },
        status: {
          localService: "RUNNING",
          assistantConnection: "CODEX",
          memoryStorage: "UNAVAILABLE",
          primaryAgentDisplayName: null
        },
        socketState: "ready",
        providerBlocked: false,
        setupBlocked: false
      }),
      null
    );

    assert.equal(
      shellAttentionForState({
        route: { kind: "memory_home" },
        status: healthyStatus,
        socketState: "closed",
        providerBlocked: false,
        setupBlocked: false
      }),
      null
    );
  });
});

function renderShell({
  route = { kind: "chat" } as const
}: {
  route?: Parameters<typeof AppShell>[0]["route"];
} = {}) {
  const routeContent = React.createElement(
    "section",
    { "data-testid": "route-content" },
    "Route content"
  );

  return renderToStaticMarkup(
    React.createElement(AppShell, {
      route,
      status: healthyStatus,
      socketState: "ready",
      providerBlocked: false,
      setupBlocked: false,
      onNavigate: () => undefined,
      children: routeContent
    })
  );
}

function countMatches(value: string, pattern: RegExp) {
  return value.match(pattern)?.length ?? 0;
}

function dataSlotClassName(markup: string, slot: string) {
  const match = markup.match(new RegExp(`data-slot="${slot}"[^>]*class="([^"]*)"`));
  assert.ok(match, `expected markup to include data-slot="${slot}" class`);
  return match[1];
}
