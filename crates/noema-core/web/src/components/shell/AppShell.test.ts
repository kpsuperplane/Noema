import assert from "node:assert/strict";
import { describe, test } from "node:test";
import * as React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { avatarSeedForActorId, LOCAL_AGENT_AVATAR_ID } from "../IdentityAvatar";
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
import {
  deckNavigationControlLabels,
  deckNavigationReducer,
  deckTransitionPropertyCanSettleSurfaceVisibility,
  initialDeckNavigationState,
  type DeckNavigationState
} from "./deckNavigation";
import { shellSidebarTransitionDirection } from "./ShellSidebar";

const healthyStatus = {
  localService: "RUNNING",
  assistantConnection: "CODEX",
  memoryStorage: "READY",
  primaryAgentDisplayName: null
} as const;

describe("deck navigation behavior", () => {
  test("opens and closes the navigation reveal state", () => {
    const openState = deckNavigationReducer(initialDeckNavigationState, { type: "openNav" });

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
  });

  test("closes revealed navigation after route navigation", () => {
    const state: DeckNavigationState = {
      navOpen: true,
      sidebarCollapsed: true,
      surfaceVisibility: "visible"
    };

    assert.deepEqual(deckNavigationReducer(state, { type: "navigate" }), {
      navOpen: false,
      sidebarCollapsed: true,
      surfaceVisibility: "showing"
    });
  });

  test("escape closes revealed navigation without changing collapse state", () => {
    const state: DeckNavigationState = {
      navOpen: true,
      sidebarCollapsed: true,
      surfaceVisibility: "visible"
    };

    assert.deepEqual(deckNavigationReducer(state, { type: "escape" }), {
      navOpen: false,
      sidebarCollapsed: true,
      surfaceVisibility: "showing"
    });
  });

  test("wide viewport sync closes mobile reveal without changing collapse state", () => {
    const state: DeckNavigationState = {
      navOpen: true,
      sidebarCollapsed: true,
      surfaceVisibility: "visible"
    };

    assert.deepEqual(deckNavigationReducer(state, { type: "syncWideViewport" }), {
      navOpen: false,
      sidebarCollapsed: true,
      surfaceVisibility: "visible"
    });
  });

  test("toggles desktop collapse and closes revealed navigation", () => {
    const collapsed = deckNavigationReducer(initialDeckNavigationState, {
      type: "toggleSidebarCollapsed"
    });

    assert.deepEqual(collapsed, {
      navOpen: false,
      sidebarCollapsed: true,
      surfaceVisibility: "visible"
    });

    assert.deepEqual(
      deckNavigationReducer(
        { navOpen: true, sidebarCollapsed: true, surfaceVisibility: "visible" },
        {
          type: "toggleSidebarCollapsed"
        }
      ),
      {
        navOpen: false,
        sidebarCollapsed: false,
        surfaceVisibility: "visible"
      }
    );
  });

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
    assert.deepEqual(
      deckNavigationReducer(initialDeckNavigationState, { type: "openNav", animated: false }),
      {
        navOpen: true,
        sidebarCollapsed: false,
        surfaceVisibility: "hidden"
      }
    );

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
    assert.equal(deckTransitionPropertyCanSettleSurfaceVisibility("translate"), true);
    assert.equal(deckTransitionPropertyCanSettleSurfaceVisibility("scale"), true);
    assert.equal(deckTransitionPropertyCanSettleSurfaceVisibility("box-shadow"), false);
    assert.equal(deckTransitionPropertyCanSettleSurfaceVisibility("opacity"), false);
  });

  test("returns accessible labels for the next shell action", () => {
    assert.deepEqual(deckNavigationControlLabels(initialDeckNavigationState), {
      menu: "Open navigation",
      collapse: "Collapse sidebar"
    });

    assert.deepEqual(
      deckNavigationControlLabels({
        navOpen: true,
        sidebarCollapsed: true,
        surfaceVisibility: "visible"
      }),
      {
        menu: "Close navigation",
        collapse: "Expand sidebar"
      }
    );
  });
});

describe("AppShell layered deck markup", () => {
  test("renders a single navigation ground layer and content deck", () => {
    const markup = renderShell();

    assert.equal(countMatches(markup, /aria-label="Primary"/g), 1);
    assert.match(markup, /data-slot="shell-root"/);
    assert.match(markup, /data-slot="shell-sidebar-ground"/);
    assert.match(markup, /data-slot="shell-content-deck"/);
    assert.match(markup, /data-slot="shell-route-content"/);
    assert.match(markup, /data-surface-visibility="visible"/);
    assert.match(markup, /data-sidebar-collapsed="false"/);
    assert.match(markup, /data-nav-open="false"/);
    assert.equal(shellRootStyle()["--shell-sidebar-width"], shellDesktopSidebarWidth);
    assert.equal(
      shellRootStyle()["--shell-desktop-chrome-offset"],
      shellBrowserDesktopChromeOffset
    );
    assert.match(
      markup,
      /style="--shell-sidebar-width:216px;--shell-desktop-chrome-offset:0px"/
    );
  });

  test("sets a desktop chrome offset only for Tauri runtime", () => {
    assert.equal(shellDesktopChromeOffsetForRuntime(false), shellBrowserDesktopChromeOffset);
    assert.equal(shellDesktopChromeOffsetForRuntime(true), shellTauriDesktopChromeOffset);
    assert.equal(
      shellRootStyle({ desktopChromeOffset: shellTauriDesktopChromeOffset })[
        "--shell-desktop-chrome-offset"
      ],
      "72px"
    );
  });

  test("uses a frosted outer shell substrate only in Tauri", () => {
    const browserRootClassName = dataSlotClassName(renderShell(), "shell-root");
    assert.match(browserRootClassName, /bg-\[var\(--pine-50\)\]/);
    assert.doesNotMatch(browserRootClassName, /\bbg-transparent\b/);

    withTauriRuntime(() => {
      const tauriMarkup = renderShell();
      const tauriRootClassName = dataSlotClassName(tauriMarkup, "shell-root");

      assert.match(tauriMarkup, /data-tauri-runtime="true"/);
      assert.match(tauriRootClassName, /bg-\[rgba\(233,242,236,0\.6\)\]/);
      assert.doesNotMatch(tauriRootClassName, /backdrop-blur/);
    });
  });

  test("renders accessible deck controls and current destination label", () => {
    const markup = renderShell({ route: { kind: "memory_graph" } });

    assert.match(markup, /aria-label="Open navigation"/);
    assert.match(markup, /aria-expanded="false"/);
    assert.match(markup, /aria-controls="noema-shell-sidebar"/);
    assert.match(markup, /aria-label="Collapse sidebar"/);
    assert.match(markup, /data-slot="shell-sidebar-collapse-button"/);
    assert.doesNotMatch(markup, /data-slot="shell-deck-collapse-button"/);
    assert.match(markup, />Memory</);
  });

  test("renders Settings as a full bottom L0 menu item", () => {
    const markup = renderShell();

    assert.equal(countMatches(markup, /aria-label="Primary"/g), 1);
    assert.equal(countMatches(markup, />Settings</g), 1);
    assert.match(markup, /data-slot="shell-menu-bottom-item"/);
    assert.doesNotMatch(markup, /data-slot="shell-settings-button"/);
    assert.doesNotMatch(markup, /aria-label="Open settings"/);
  });

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

  test("renders the named primary agent in the sidebar and chat header", () => {
    const markup = renderShell({
      status: { ...healthyStatus, primaryAgentDisplayName: "Aster" }
    });

    assert.equal(countMatches(markup, />Aster</g), 1);
    assert.match(markup, new RegExp(`data-avatar-seed="${avatarSeedForActorId(LOCAL_AGENT_AVATAR_ID)}"`));
    assert.match(markup, /data-avatar-variant="beam"/);
    assert.match(markup, /data-slot="shell-primary-agent-avatar"[^>]*class="[^"]*\bsize-4\b/);
    assert.doesNotMatch(markup, /lucide-house/);
    assert.match(markup, />Home</);
  });

  test("keeps the desktop collapse toggle out of the header", () => {
    const markup = renderShell();
    const menuClassName = dataSlotClassName(markup, "shell-menu-button");
    const collapseClassName = dataSlotClassName(markup, "shell-sidebar-collapse-button");

    assert.match(menuClassName, /min-\[761px\]:!hidden/);
    assert.match(collapseClassName, /max-\[760px\]:hidden/);
    assert.match(collapseClassName, /left-\[calc\(0\.75rem\+var\(--shell-desktop-chrome-offset\)\)\]/);
    assert.match(collapseClassName, /\bz-40\b/);
    assert.doesNotMatch(markup, /data-slot="shell-deck-collapse-button"/);
  });

  test("keeps sidebar navigation hover states translucent", () => {
    const markup = renderShell();

    assert.match(markup, /hover:!bg-\[color-mix\(in_srgb,var\(--pine-700\)_10%,transparent\)\]/);
    assert.match(markup, /aria-expanded:!bg-\[color-mix\(in_srgb,var\(--pine-700\)_10%,transparent\)\]/);
  });

  test("keeps the shell header row compact above short pages", () => {
    const className = shellContentDeckClassName(initialDeckNavigationState);

    assert.match(className, /grid-rows-\[auto_minmax\(0,1fr\)\]/);
  });

  test("leaves visible route content interactive", () => {
    const className = shellRouteContentClassName("visible");

    assert.doesNotMatch(className, /pointer-events-none/);
  });

  test("disables route content pointer events while surface visibility is transitional or hidden", () => {
    assert.match(shellRouteContentClassName("hiding"), /pointer-events-none/);
    assert.match(shellRouteContentClassName("hidden"), /pointer-events-none/);
    assert.match(shellRouteContentClassName("showing"), /pointer-events-none/);
  });

  test("reveals mobile navigation without shrinking the deck vertically", () => {
    const className = shellContentDeckClassName({
      navOpen: true,
      sidebarCollapsed: true,
      surfaceVisibility: "visible"
    });

    assert.match(
      className,
      /transition-\[inset,left,right,transform,translate,scale,border-radius,box-shadow\]/
    );
    assert.match(className, /min-\[761px\]:scale-\[0\.97\]/);
    assert.doesNotMatch(className, /(^|\s)scale-\[0\.97\](\s|$)/);
    assert.doesNotMatch(className, /max-\[760px\]:translate-x-\[min\(252px,72vw\)\]/);
    assert.match(className, /max-\[760px\]:left-\[min\(252px,72vw\)\]/);
    assert.match(className, /max-\[760px\]:right-\[calc\(min\(252px,72vw\)\*-1\)\]/);
    assert.match(className, /max-\[760px\]:translate-x-0/);
  });

  test("derives desktop deck geometry from one sidebar width variable", () => {
    const closedSidebarClassName = shellSidebarGroundClassName(initialDeckNavigationState);
    const expandedDeckClassName = shellContentDeckClassName(initialDeckNavigationState);
    const revealedDeckClassName = shellContentDeckClassName({
      navOpen: true,
      sidebarCollapsed: true,
      surfaceVisibility: "visible"
    });

    assert.match(closedSidebarClassName, /w-\[var\(--shell-sidebar-width\)\]/);
    assert.match(expandedDeckClassName, /left-\[calc\(var\(--shell-sidebar-width\)\)\]/);
    assert.match(
      revealedDeckClassName,
      /translate-x-\[min\(calc\(var\(--shell-sidebar-width\)\+20px\),68vw\)\]/
    );
    assert.doesNotMatch(closedSidebarClassName, /w-\[216px\]/);
    assert.doesNotMatch(expandedDeckClassName, /left-\[244px\]/);
    assert.doesNotMatch(revealedDeckClassName, /translate-x-\[min\(236px,68vw\)\]/);
  });

  test("keeps the content deck shadow on mobile", () => {
    const className = shellContentDeckClassName(initialDeckNavigationState);

    assert.match(
      className,
      /shadow-\[0_0_24px_color-mix\(in_srgb,var\(--pine-700\),transparent_80%\)\]/
    );
    assert.doesNotMatch(className, /max-\[760px\]:shadow-none/);
  });

  test("keeps revealed sidebar clickable above the dismissal backdrop", () => {
    const closedClassName = shellSidebarGroundClassName(initialDeckNavigationState);
    const openClassName = shellSidebarGroundClassName({
      navOpen: true,
      sidebarCollapsed: true,
      surfaceVisibility: "visible"
    });

    assert.match(closedClassName, /\bz-10\b/);
    assert.doesNotMatch(closedClassName, /z-\[25\]/);
    assert.match(openClassName, /z-\[25\]/);
    assert.doesNotMatch(openClassName, /\bz-30\b/);
  });

  test("anchors the desktop collapse affordance to the sidebar left edge", () => {
    const className = shellSidebarCollapseButtonClassName();

    assert.match(className, /\babsolute\b/);
    assert.match(className, /\btop-3\b/);
    assert.match(className, /left-\[calc\(0\.75rem\+var\(--shell-desktop-chrome-offset\)\)\]/);
    assert.match(className, /\bz-40\b/);
    assert.match(className, /max-\[760px\]:hidden/);
    assert.doesNotMatch(className, /translate-x/);
    assert.doesNotMatch(className, /right-/);
  });

  test("does not render sheet markup and includes reduced-motion deck handling", () => {
    const markup = renderShell();
    const deckClassName = dataSlotClassName(markup, "shell-content-deck");

    assert.doesNotMatch(markup, /data-slot="sheet"/);
    assert.doesNotMatch(markup, /data-slot="sheet-content"/);
    assert.match(deckClassName, /\bmotion-reduce:transition-none\b/);
  });

  test("renders only one interactive sidebar menu level at a time", () => {
    const primaryMarkup = renderShell({ route: { kind: "chat" } });
    const settingsMarkup = renderShell({ route: { kind: "settings", section: "providers" } });

    assert.equal(countMatches(primaryMarkup, /data-slot="shell-sidebar-menu-level-frame"/g), 1);
    assert.equal(countMatches(settingsMarkup, /data-slot="shell-sidebar-menu-level-frame"/g), 1);
    assert.match(primaryMarkup, /data-shell-menu-level="l0"/);
    assert.match(settingsMarkup, /data-shell-menu-level="settings"/);
    assert.match(primaryMarkup, /data-shell-menu-frame-state="current"/);
    assert.match(settingsMarkup, /data-shell-menu-frame-state="current"/);
    assert.doesNotMatch(settingsMarkup, /data-shell-menu-item="home"/);
  });

  test("renders sidebar menu levels inside an animated viewport", () => {
    const markup = renderShell({ route: { kind: "chat" } });

    assert.match(markup, /data-slot="shell-sidebar-menu-viewport"/);
    assert.match(markup, /data-slot="shell-sidebar-menu-level-frame"/);
    assert.match(markup, /data-shell-menu-frame-state="current"/);
  });

  test("keeps sidebar padding inside the animated menu frame", () => {
    const markup = renderShell({ route: { kind: "chat" } });
    const groundClassName = dataSlotClassName(markup, "shell-sidebar-ground");
    const viewportClassName = dataSlotClassName(markup, "shell-sidebar-menu-viewport");
    const frameClassName = dataSlotClassName(markup, "shell-sidebar-menu-level-frame");

    assert.match(viewportClassName, /\boverflow-hidden\b/);
    assert.doesNotMatch(groundClassName, /(?:^|\s)p[xy]-/);
    assert.doesNotMatch(viewportClassName, /(?:^|\s)p[xy]-/);
    assert.match(frameClassName, /(?:^|\s)px-3\.5(?:\s|$)/);
    assert.match(frameClassName, /(?:^|\s)py-4(?:\s|$)/);
  });

  test("defines sidebar menu transition directions between route levels", () => {
    assert.equal(shellSidebarTransitionDirection("l0", "settings"), "forward");
    assert.equal(shellSidebarTransitionDirection("settings", "l0"), "backward");
    assert.equal(shellSidebarTransitionDirection("settings", "settings"), "forward");
  });

  test("labels bottom menu actions accessibly", () => {
    assert.match(renderShell({ route: { kind: "chat" } }), />Settings</);
    assert.match(
      renderShell({ route: { kind: "settings", section: "providers" } }),
      />Go back</
    );
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
  route = { kind: "chat" } as const,
  status = healthyStatus
}: {
  route?: Parameters<typeof AppShell>[0]["route"];
  status?: Parameters<typeof AppShell>[0]["status"];
} = {}) {
  const routeContent = React.createElement(
    "section",
    { "data-testid": "route-content" },
    "Route content"
  );

  return renderToStaticMarkup(
    React.createElement(AppShell, {
      route,
      status,
      socketState: "ready",
      providerBlocked: false,
      setupBlocked: false,
      onNavigate: () => undefined,
      goBackFromSettings: () => undefined,
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

function withTauriRuntime(callback: () => void) {
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, "window");
  Object.defineProperty(globalThis, "window", {
    configurable: true,
    value: {
      __TAURI_INTERNALS__: {
        invoke: () => undefined
      }
    }
  });

  try {
    callback();
  } finally {
    if (descriptor) {
      Object.defineProperty(globalThis, "window", descriptor);
    } else {
      Reflect.deleteProperty(globalThis, "window");
    }
  }
}
