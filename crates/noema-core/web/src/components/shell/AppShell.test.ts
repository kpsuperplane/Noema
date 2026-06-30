import assert from "node:assert/strict";
import { describe, test } from "node:test";
import {
  activeShellDestination,
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
