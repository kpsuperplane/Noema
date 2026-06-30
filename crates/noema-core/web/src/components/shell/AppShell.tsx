import React from "react";
import { Menu, PanelLeftClose, PanelLeftOpen } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { LocalStatusQuery } from "@/generated/graphql";
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

export function shellContentDeckClassName(deckNavigation: DeckNavigationState) {
  return cn(
    "absolute z-30 grid min-h-0 grid-rows-[auto_minmax(0,1fr)] overflow-hidden border border-[var(--border-subtle)] bg-background shadow-[0_24px_70px_rgba(31,38,30,0.18)] transition-[inset,left,right,transform,translate,scale,border-radius,box-shadow] duration-300 ease-out",
    "motion-reduce:transition-none",
    deckNavigation.sidebarCollapsed
      ? "inset-2 rounded-xl"
      : "inset-y-2 right-2 left-[244px] rounded-xl",
    "max-[760px]:inset-0 max-[760px]:rounded-none max-[760px]:border-0 data-[nav-open=true]:max-[760px]:rounded-xl",
    deckNavigation.navOpen &&
      "translate-x-[min(236px,68vw)] min-[761px]:scale-[0.97] max-[760px]:left-[min(252px,72vw)] max-[760px]:right-[calc(min(252px,72vw)*-1)] max-[760px]:translate-x-0 pointer-events-none"
  );
}

export function shellSidebarGroundClassName(deckNavigation: DeckNavigationState) {
  return cn(
    "absolute inset-y-0 left-0 z-10 grid min-h-0 w-[236px] px-3.5 py-4",
    "max-[760px]:w-[min(286px,78vw)] max-[760px]:pb-[max(1rem,env(safe-area-inset-bottom))]",
    deckNavigation.navOpen && "z-[25]"
  );
}

export function shellRouteContentClassName(visibility: ShellSurfaceVisibility) {
  return cn("min-h-0 overflow-hidden", visibility !== "visible" && "pointer-events-none");
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
  const activeDestination = activeShellDestination(route);
  const activeLabel =
    shellNavItems.find((item) => item.destination === activeDestination)?.label ?? "Home";
  const attention = shellAttentionForState({
    route,
    status,
    socketState,
    providerBlocked,
    setupBlocked
  });

  const {
    state: deckNavigation,
    labels,
    menuButtonRef,
    openNav,
    closeNav,
    toggleSidebarCollapsed,
    navigateFromShell,
    settleSurfaceVisibility
  } = useDeckNavigation(onNavigate);

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
        className={shellSidebarGroundClassName(deckNavigation)}
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
          className="flex min-h-[56px] items-center gap-3 border-b border-[var(--border-subtle)] bg-white/95 px-4"
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
          <Button
            data-slot="shell-deck-collapse-button"
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
