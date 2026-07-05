import React from "react";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { Menu, PanelLeftClose, PanelLeftOpen } from "lucide-react";
import type { LocalStatusQuery } from "@/generated/graphql";
import { isTauriRuntime } from "@/graphql/transportMode";
import type { AppRoute } from "@/app/routes";
import type { SocketState } from "@/shared/types";
import {
  deckTransitionPropertyCanSettleSurfaceVisibility,
  useDeckNavigation
} from "./deckNavigation";
import { ShellSidebar } from "./ShellSidebar";
import { ShellSurfaceProvider } from "./ShellSurfaceContext";
import {
  breadcrumbForRoute,
  shellMenuLevelForRoute,
  shellMenuSelectionBehavior,
  type ShellBreadcrumb,
  type ShellMenuItem
} from "./shellNavigation";
import { useShellNavSwipe } from "./useShellNavSwipe";

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
type ShellContentDeckStyle = React.CSSProperties;

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

function shellHeaderLabelForBreadcrumb(breadcrumb: ShellBreadcrumb) {
  return breadcrumb.parent ? `${breadcrumb.parent} / ${breadcrumb.current}` : breadcrumb.current;
}

function ShellBreadcrumbLabel({ breadcrumb }: { breadcrumb: ShellBreadcrumb }) {
  if (!breadcrumb.parent) {
    return (
      <strong {...stylex.props(styles.breadcrumbCurrent)}>
        {breadcrumb.current}
      </strong>
    );
  }

  return (
    <div data-slot="shell-breadcrumb" {...stylex.props(styles.breadcrumb)}>
      <span data-slot="shell-breadcrumb-parent" {...stylex.props(styles.breadcrumbParent)}>
        {breadcrumb.parent}
      </span>
      <span {...stylex.props(styles.breadcrumbSeparator)} aria-hidden="true">
        /
      </span>
      <strong data-slot="shell-breadcrumb-current" {...stylex.props(styles.breadcrumbCurrent)}>
        {breadcrumb.current}
      </strong>
    </div>
  );
}

function shellContentDeckStyle({
  dragging,
  offsetPx
}: {
  dragging: boolean;
  offsetPx: number;
}): ShellContentDeckStyle | undefined {
  if (!dragging) {
    return undefined;
  }

  return {
    transform: `translateX(${offsetPx}px)`,
    transition: "none"
  };
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
  const navSwipe = useShellNavSwipe({
    navOpen: deckNavigation.navOpen,
    openNav,
    closeNav
  });

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
      data-nav-swipe-active={navSwipe.dragging ? "true" : undefined}
      data-sidebar-collapsed={deckNavigation.sidebarCollapsed}
      data-tauri-runtime={isDesktopRuntime}
      style={rootStyle}
      {...stylex.props(styles.root, isDesktopRuntime ? styles.desktopRoot : styles.browserRoot)}
      {...navSwipe.pointerHandlers}
    >
      <aside
        id="noema-shell-sidebar"
        data-slot="shell-sidebar-ground"
        aria-label="Noema navigation"
        {...stylex.props(styles.sidebarGround, deckNavigation.navOpen && styles.sidebarGroundOpen)}
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
        size="sm"
        label={labels.collapse}
        icon={
          deckNavigation.sidebarCollapsed ? (
            <PanelLeftOpen aria-hidden="true" size={18} />
          ) : (
            <PanelLeftClose aria-hidden="true" size={18} />
          )
        }
        isIconOnly
        {...stylex.props(styles.sidebarCollapseButton)}
        onClick={toggleSidebarCollapsed}
      />

      {deckNavigation.navOpen ? (
        <button
          type="button"
          data-slot="shell-nav-backdrop"
          aria-label="Close navigation"
          {...stylex.props(styles.navBackdrop)}
          onClick={closeNav}
        />
      ) : null}

      <section
        data-slot="shell-content-deck"
        data-nav-open={deckNavigation.navOpen}
        data-nav-swipe-active={navSwipe.dragging ? "true" : undefined}
        aria-label={activeLabel}
        style={shellContentDeckStyle(navSwipe)}
        {...stylex.props(
          styles.contentDeck,
          deckNavigation.sidebarCollapsed ? styles.contentDeckCollapsed : styles.contentDeckExpanded,
          deckNavigation.navOpen && styles.contentDeckNavOpen
        )}
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
          {...stylex.props(styles.deckHeader)}
        >
          <div
            data-slot="shell-header-offset"
            data-sidebar-collapsed={deckNavigation.sidebarCollapsed}
            {...stylex.props(
              styles.headerOffset,
              deckNavigation.sidebarCollapsed && styles.headerOffsetCollapsed
            )}
          >
            <Button
              ref={menuButtonRef}
              data-slot="shell-menu-button"
              type="button"
              variant="ghost"
              size="sm"
              label={labels.menu}
              icon={<Menu aria-hidden="true" size={18} />}
              isIconOnly
              aria-controls="noema-shell-sidebar"
              aria-expanded={deckNavigation.navOpen}
              {...stylex.props(styles.menuButton)}
              onClick={deckNavigation.navOpen ? closeNav : openNav}
            />
            <div {...stylex.props(styles.breadcrumbWrap)}>
              <ShellBreadcrumbLabel breadcrumb={breadcrumb} />
            </div>
          </div>
          <div aria-hidden="true" data-slot="shell-header-scrim" {...stylex.props(styles.headerScrim)} />
        </header>

        <ShellSurfaceProvider value={{ visibility: deckNavigation.surfaceVisibility }}>
          <div
            data-slot="shell-route-content"
            data-shell-surface-visibility={deckNavigation.surfaceVisibility}
            {...stylex.props(
              styles.routeContent,
              deckNavigation.surfaceVisibility !== "visible" && styles.routeContentInactive
            )}
          >
            {children}
          </div>
        </ShellSurfaceProvider>
      </section>
    </main>
  );
}

const styles = stylex.create({
  root: {
    position: "relative",
    height: "100dvh",
    overflow: "hidden",
    color: "var(--foreground)"
  },
  desktopRoot: {
    backgroundColor: "rgba(233, 242, 236, 0.6)",
    backdropFilter: "brightness(1.05)"
  },
  browserRoot: {
    backgroundColor: "var(--pine-50)"
  },
  sidebarGround: {
    position: "absolute",
    top: 0,
    bottom: 0,
    left: 0,
    zIndex: 10,
    display: "grid",
    minHeight: 0,
    width: "var(--shell-sidebar-width)",
    "@media (max-width: 760px)": {
      width: "min(286px, 78vw)",
      paddingBottom: "max(1rem, env(safe-area-inset-bottom))"
    }
  },
  sidebarGroundOpen: {
    zIndex: 25
  },
  sidebarCollapseButton: {
    position: "absolute",
    top: 12,
    left: "calc(0.75rem + var(--shell-desktop-chrome-offset))",
    zIndex: 40,
    backgroundColor: "transparent",
    ":hover": {
      backgroundColor: "rgb(0 0 0 / 0.05)"
    },
    "@media (max-width: 760px)": {
      display: "none"
    }
  },
  navBackdrop: {
    position: "absolute",
    inset: 0,
    zIndex: 20,
    cursor: "default",
    backgroundColor: "transparent",
    borderWidth: 0,
    padding: 0
  },
  contentDeck: {
    "--shell-deck-header-height": "44px",
    position: "absolute",
    zIndex: 30,
    display: "grid",
    minHeight: 0,
    gridTemplateRows: "auto minmax(0, 1fr)",
    overflow: "hidden",
    touchAction: "pan-y",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    backgroundColor: "var(--background)",
    boxShadow: "0 0 24px color-mix(in srgb, var(--pine-700), transparent 80%)",
    transitionProperty: "inset, left, right, transform, translate, scale, border-radius, box-shadow",
    transitionDuration: "300ms",
    transitionTimingFunction: "cubic-bezier(0.2, 0, 0, 1)",
    "@media (prefers-reduced-motion: reduce)": {
      transition: "none"
    },
    "@media (max-width: 760px)": {
      inset: 0,
      top: 0,
      right: 0,
      bottom: 0,
      left: 0,
      borderWidth: 0,
      borderRadius: 0
    }
  },
  contentDeckCollapsed: {
    inset: 8,
    borderRadius: 12
  },
  contentDeckExpanded: {
    top: 8,
    right: 8,
    bottom: 8,
    left: "calc(var(--shell-sidebar-width))",
    borderRadius: 12
  },
  contentDeckNavOpen: {
    transform: "translateX(min(calc(var(--shell-sidebar-width) + 20px), 68vw))",
    pointerEvents: "none",
    "@media (min-width: 761px)": {
      scale: 0.97
    },
    "@media (max-width: 760px)": {
      top: 0,
      bottom: 0,
      left: "min(252px, 72vw)",
      right: "calc(min(252px, 72vw) * -1)",
      transform: "translateX(0)",
      borderRadius: 12
    }
  },
  deckHeader: {
    position: "relative",
    zIndex: 2,
    display: "flex",
    alignItems: "center",
    gap: 12,
    borderBottomWidth: 1,
    borderBottomStyle: "solid",
    borderBottomColor: "transparent",
    backgroundColor: "transparent",
    minHeight: "var(--shell-deck-header-height)",
    paddingBlock: 4,
    paddingInline: 16
  },
  headerScrim: {
    position: "absolute",
    top: 0,
    right: 0,
    left: 0,
    height: "calc(var(--shell-deck-header-height) + 72px)",
    zIndex: 0,
    pointerEvents: "none",
    background: "linear-gradient(to bottom, var(--background), rgb(255 255 255 / 0))"
  },
  headerOffset: {
    position: "relative",
    zIndex: 1,
    display: "flex",
    alignItems: "center",
    transitionProperty: "transform",
    transitionDuration: "300ms",
    transitionTimingFunction: "cubic-bezier(0.2, 0, 0, 1)"
  },
  headerOffsetCollapsed: {
    "@media (min-width: 761px)": {
      transform: "translateX(calc(1.5rem + var(--shell-desktop-chrome-offset)))"
    }
  },
  menuButton: {
    "@media (min-width: 761px)": {
      display: "none"
    }
  },
  breadcrumbWrap: {
    minWidth: 0,
    paddingBlock: "0.2rem"
  },
  breadcrumb: {
    display: "flex",
    minWidth: 0,
    alignItems: "center",
    gap: 8
  },
  breadcrumbParent: {
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    fontSize: 14,
    color: "var(--muted-foreground)"
  },
  breadcrumbSeparator: {
    color: "color-mix(in srgb, var(--muted-foreground) 70%, transparent)"
  },
  breadcrumbCurrent: {
    display: "block",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    fontFamily: "var(--font-heading)",
    fontSize: 16,
    fontWeight: 700,
    letterSpacing: 0
  },
  routeContent: {
    minHeight: 0,
    overflow: "visible"
  },
  routeContentInactive: {
    pointerEvents: "none"
  }
});
