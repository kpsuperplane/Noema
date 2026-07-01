import React from "react";
import { Menu, PanelLeftClose, PanelLeftOpen } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { LocalStatusQuery } from "@/generated/graphql";
import { isTauriRuntime } from "@/graphql/transportMode";
import { cn } from "@/lib/utils";
import type { AppRoute } from "@/routes";
import type { SocketState } from "@/types";
import {
  deckTransitionPropertyCanSettleSurfaceVisibility,
  type DeckNavigationState,
  useDeckNavigation
} from "./deckNavigation";
import { ShellSidebar } from "./ShellSidebar";
import { ShellSurfaceProvider, type ShellSurfaceVisibility } from "./ShellSurfaceContext";
import {
  breadcrumbForRoute,
  shellMenuLevelForRoute,
  shellMenuSelectionBehavior,
  type ShellBreadcrumb,
  type ShellMenuItem
} from "./shellNavigation";

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

export const shellDesktopSidebarWidth = "216px";
export const shellBrowserDesktopChromeOffset = "0px";
export const shellTauriDesktopChromeOffset = "72px";

type ShellRootStyle = React.CSSProperties &
  Record<"--shell-sidebar-width" | "--shell-desktop-chrome-offset", string>;

export function shellDesktopChromeOffsetForRuntime(isDesktop = isTauriRuntime()) {
  return isDesktop ? shellTauriDesktopChromeOffset : shellBrowserDesktopChromeOffset;
}

export function shellRootStyle({
  desktopChromeOffset = shellBrowserDesktopChromeOffset
}: {
  desktopChromeOffset?: string;
} = {}): ShellRootStyle {
  return {
    "--shell-sidebar-width": shellDesktopSidebarWidth,
    "--shell-desktop-chrome-offset": desktopChromeOffset
  } as ShellRootStyle;
}

export function shellRootClassName(isDesktopRuntime = isTauriRuntime()) {
  return cn(
    "relative h-dvh min-h-screen overflow-hidden text-foreground",
    isDesktopRuntime
      ? "bg-[rgba(233,242,236,0.6)] backdrop-brightness-105"
      : "bg-[var(--pine-50)]"
  );
}

function primaryAgentNameForStatus(status: LocalStatusQuery["localStatus"] | null) {
  return status?.primaryAgentDisplayName?.trim() ?? "";
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

export function shellContentDeckClassName(deckNavigation: DeckNavigationState) {
  return cn(
    "absolute z-30 grid min-h-0 grid-rows-[auto_minmax(0,1fr)] overflow-hidden border border-[var(--border-subtle)] bg-background shadow-[0_0_24px_color-mix(in_srgb,var(--pine-700),transparent_80%)] transition-[inset,left,right,transform,translate,scale,border-radius,box-shadow] duration-300 ease-out",
    "motion-reduce:transition-none",
    deckNavigation.sidebarCollapsed
      ? "inset-2 rounded-xl"
      : "inset-y-2 right-2 left-[calc(var(--shell-sidebar-width))] rounded-xl",
    "max-[760px]:inset-0 max-[760px]:rounded-none max-[760px]:border-0 data-[nav-open=true]:max-[760px]:rounded-xl",
    deckNavigation.navOpen &&
      "translate-x-[min(calc(var(--shell-sidebar-width)+20px),68vw)] min-[761px]:scale-[0.97] max-[760px]:left-[min(252px,72vw)] max-[760px]:right-[calc(min(252px,72vw)*-1)] max-[760px]:translate-x-0 pointer-events-none"
  );
}

export function shellDeckHeaderClassName() {
  return cn(
    "flex py-1 items-center gap-3 border-b border-[var(--border-subtle)] bg-white/95 px-4"
  );
}

export function shellSidebarCollapseButtonClassName() {
  return cn(
    "absolute top-3 left-[calc(0.75rem+var(--shell-desktop-chrome-offset))] z-40 max-[760px]:hidden hover:bg-black/5"
  );
}

export function shellSidebarGroundClassName(deckNavigation: DeckNavigationState) {
  return cn(
    "absolute inset-y-0 left-0 z-10 grid min-h-0 w-[var(--shell-sidebar-width)]",
    "max-[760px]:w-[min(286px,78vw)] max-[760px]:pb-[max(1rem,env(safe-area-inset-bottom))]",
    deckNavigation.navOpen && "z-[25]"
  );
}

export function shellRouteContentClassName(visibility: ShellSurfaceVisibility) {
  return cn("min-h-0 overflow-hidden", visibility !== "visible" && "pointer-events-none");
}

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

export function AppShell({
  route,
  status,
  socketState,
  providerBlocked = false,
  setupBlocked = false,
  onNavigate,
  goBackFromSettings,
  children
}: {
  route: AppRoute;
  status: LocalStatusQuery["localStatus"] | null;
  socketState: SocketState;
  providerBlocked?: boolean;
  setupBlocked?: boolean;
  onNavigate: (route: AppRoute) => void;
  goBackFromSettings: () => void;
  children: React.ReactNode;
}) {
  const menuLevel = shellMenuLevelForRoute(route);
  const breadcrumb = breadcrumbForRoute(route);
  const primaryAgentName = primaryAgentNameForStatus(status);
  const activeLabel = shellHeaderLabelForBreadcrumb(breadcrumb);
  const attention = shellAttentionForState({
    route,
    status,
    socketState,
    providerBlocked,
    setupBlocked
  });
  const isDesktopRuntime = isTauriRuntime();
  const rootStyle = shellRootStyle({
    desktopChromeOffset: shellDesktopChromeOffsetForRuntime(isDesktopRuntime)
  });

  React.useEffect(() => {
    if (typeof document === "undefined") {
      return;
    }

    if (isDesktopRuntime) {
      document.documentElement.dataset.tauriRuntime = "true";
      return () => {
        delete document.documentElement.dataset.tauriRuntime;
      };
    }

    delete document.documentElement.dataset.tauriRuntime;
  }, [isDesktopRuntime]);

  const {
    state: deckNavigation,
    labels,
    menuButtonRef,
    openNav,
    closeNav,
    toggleSidebarCollapsed,
    settleSurfaceVisibility
  } = useDeckNavigation(onNavigate);

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

  return (
    <main
      data-slot="shell-root"
      data-nav-open={deckNavigation.navOpen}
      data-sidebar-collapsed={deckNavigation.sidebarCollapsed}
      data-tauri-runtime={isDesktopRuntime}
      style={rootStyle}
      className={shellRootClassName(isDesktopRuntime)}
    >
      <aside
        id="noema-shell-sidebar"
        data-slot="shell-sidebar-ground"
        aria-label="Noema navigation"
        className={shellSidebarGroundClassName(deckNavigation)}
      >
        <ShellSidebar
          menuLevel={menuLevel}
          attention={attention}
          primaryAgentNamed={Boolean(primaryAgentName)}
          primaryAgentLabel={primaryAgentName || "Home"}
          onSelectItem={selectShellMenuItem}
        />
      </aside>

      <Button
        data-slot="shell-sidebar-collapse-button"
        type="button"
        variant="ghost"
        size="icon"
        aria-label={labels.collapse}
        className={shellSidebarCollapseButtonClassName()}
        onClick={toggleSidebarCollapsed}
      >
        {deckNavigation.sidebarCollapsed ? (
          <PanelLeftOpen aria-hidden="true" />
        ) : (
          <PanelLeftClose aria-hidden="true" />
        )}
      </Button>

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
        data-nav-open={deckNavigation.navOpen}
        aria-label={activeLabel}
        className={shellContentDeckClassName(deckNavigation)}
        onTransitionEnd={(event) => {
          if (
            event.currentTarget === event.target &&
            deckTransitionPropertyCanSettleSurfaceVisibility(event.propertyName)
          ) {
            settleSurfaceVisibility();
          }
        }}
      >
        <header
          data-slot="shell-deck-header"
          data-tauri-drag-region
          className={shellDeckHeaderClassName()}
        >
          <div
            data-slot="shell-header-offset"
            data-sidebar-collapsed={deckNavigation.sidebarCollapsed}
            className={cn(
              "transition-transform duration-300",
              "data-[sidebar-collapsed=true]:min-[761px]:translate-x-[calc(1.5rem+var(--shell-desktop-chrome-offset))]"
            )}
          >
            <Button
              ref={menuButtonRef}
              data-slot="shell-menu-button"
              type="button"
              variant="ghost"
              size="icon"
              aria-label={labels.menu}
              aria-controls="noema-shell-sidebar"
              aria-expanded={deckNavigation.navOpen}
              className="min-[761px]:!hidden"
              onClick={deckNavigation.navOpen ? closeNav : openNav}
            >
              <Menu aria-hidden="true" />
            </Button>
            <div className="min-w-0 py-[0.2rem]">
              <ShellBreadcrumbLabel breadcrumb={breadcrumb} />
            </div>
          </div>
        </header>

        <ShellSurfaceProvider value={{ visibility: deckNavigation.surfaceVisibility }}>
          <div
            data-slot="shell-route-content"
            data-surface-visibility={deckNavigation.surfaceVisibility}
            className={shellRouteContentClassName(deckNavigation.surfaceVisibility)}
          >
            {children}
          </div>
        </ShellSurfaceProvider>
      </section>
    </main>
  );
}
