import React from "react";
import { useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { Popover, type PopoverProps } from "@astryxdesign/core/Popover";
import * as stylex from "@stylexjs/stylex";
import { ChevronDown, Menu, PanelLeftClose, PanelLeftOpen } from "lucide-react";
import { useMotionValue } from "motion/react";
import * as m from "motion/react-m";
import {
  MemoryTreeDocument,
  type LocalStatusQuery,
  type MemoryTreeQuery
} from "@/generated/graphql";
import { isTauriRuntime } from "@/graphql/transportMode";
import { springs } from "@/motion/springs";
import type { AppRoute } from "@/app/routes";
import type { SocketState } from "@/shared/types";
import { useDeckNavigation } from "./deckNavigation";
import { ShellSidebar } from "./ShellSidebar";
import {
  ShellSurfaceProvider,
  type ShellMemoryBreadcrumb
} from "./ShellSurfaceContext";
import {
  breadcrumbForRoute,
  shellMenuLevelForRoute,
  shellMenuSelectionBehavior,
  type ShellBreadcrumb,
  type ShellMenuItem
} from "./shellNavigation";
import { useShellNavSwipe } from "./useShellNavSwipe";
import { MemoryUpdateControl } from "@/pages/MemoryUpdateControl";
import { MemoryPageTree } from "@/pages/MemoryPageTree";

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

function MemoryShellBreadcrumb({
  breadcrumb,
  pages,
  root
}: {
  breadcrumb: ShellMemoryBreadcrumb | null;
  pages: MemoryTreeQuery["memoryTree"]["pages"];
  root: NonNullable<MemoryTreeQuery["memoryTree"]["root"]>;
}) {
  const [open, setOpen] = React.useState(false);
  const pathItems = breadcrumb
    ? [...breadcrumb.ancestors, { path: breadcrumb.currentPath, title: breadcrumb.current }]
    : [];
  const activePath = breadcrumb?.currentPath ?? root.path;

  return (
    <Popover
      alignment="start"
      content={(
        <MemoryPageTree
          activePath={activePath}
          pages={pages}
          root={root}
          onNavigate={() => setOpen(false)}
        />
      )}
      isOpen={open}
      label="Memory pages"
      onOpenChange={setOpen}
      placement="below"
      width="min(320px, calc(100vw - var(--spacing-6)))"
      xstyle={popoverXStyle(styles.memoryTreePopoverContent)}
    >
      {(trigger) => (
        <button
          ref={(element) => trigger.ref(element)}
          type="button"
          aria-controls={trigger["aria-controls"]}
          aria-expanded={trigger["aria-expanded"]}
          aria-haspopup={trigger["aria-haspopup"]}
          aria-label={`Browse memory pages; current page ${breadcrumb?.current ?? "Memory"}`}
          data-slot="shell-breadcrumb"
          onClick={trigger.onClick}
          {...stylex.props(styles.memoryBreadcrumbTrigger)}
        >
          <span aria-hidden="true" {...stylex.props(styles.breadcrumb, styles.memoryBreadcrumbPath)}>
            <span {...stylex.props(styles.breadcrumbParent, styles.breadcrumbRoot)}>Memory</span>
            {pathItems.map((item, index) => (
              <React.Fragment key={item.path}>
                <span {...stylex.props(styles.breadcrumbSeparator)}>/</span>
                <span
                  title={item.title}
                  {...stylex.props(
                    index < pathItems.length - 1 ? styles.breadcrumbParent : styles.breadcrumbCurrent
                  )}
                >
                  {item.title}
                </span>
              </React.Fragment>
            ))}
          </span>
          <ChevronDown aria-hidden="true" size={14} strokeWidth={2} {...stylex.props(styles.breadcrumbChevron)} />
        </button>
      )}
    </Popover>
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
  const [memoryBreadcrumb, setMemoryBreadcrumb] = React.useState<ShellMemoryBreadcrumb | null>(null);
  const deckX = useMotionValue(0);
  const memoryTreeResult = useQuery<MemoryTreeQuery>(MemoryTreeDocument, {
    fetchPolicy: "cache-only",
    skip: route.kind !== "memory"
  });
  const memoryTree = memoryTreeResult.data?.memoryTree ?? null;
  const menuLevel = shellMenuLevelForRoute(route);
  const breadcrumb = breadcrumbForRoute(route);
  const primaryAgentName = primaryAgentNameForStatus(status);
  const activeLabel = route.kind === "memory" && memoryBreadcrumb
    ? ["Memory", ...memoryBreadcrumb.ancestors.map((item) => item.title), memoryBreadcrumb.current].join(" / ")
    : shellHeaderLabelForBreadcrumb(breadcrumb);
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
    closeNav,
    deckX,
    onSettled: settleSurfaceVisibility
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

      <m.section
        layout
        data-slot="shell-content-deck"
        data-nav-open={deckNavigation.navOpen}
        data-nav-swipe-active={navSwipe.dragging ? "true" : undefined}
        aria-label={activeLabel}
        style={{ x: deckX }}
        transition={{ layout: springs.surface }}
        {...stylex.props(
          styles.contentDeck,
          deckNavigation.sidebarCollapsed ? styles.contentDeckCollapsed : styles.contentDeckExpanded,
          deckNavigation.navOpen && styles.contentDeckNavOpen
        )}
      >
        <header
          data-slot="shell-deck-header"
          data-tauri-drag-region
          {...stylex.props(styles.deckHeader, route.kind !== "chat" && styles.deckHeaderSolid)}
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
            <div {...stylex.props(styles.breadcrumbWrap, route.kind === "memory" && styles.memoryPopoverScope)}>
              {route.kind === "memory" && memoryTree?.root ? (
                <MemoryShellBreadcrumb
                  breadcrumb={memoryBreadcrumb}
                  pages={memoryTree.pages}
                  root={memoryTree.root}
                />
              ) : (
                <ShellBreadcrumbLabel breadcrumb={breadcrumb} />
              )}
            </div>
          </div>
          {route.kind === "memory" ? (
            <div {...stylex.props(styles.headerAction)}>
              <MemoryUpdateControl />
            </div>
          ) : null}
          {route.kind === "chat" ? (
            <div aria-hidden="true" data-slot="shell-header-scrim" {...stylex.props(styles.headerScrim)} />
          ) : null}
        </header>

        <ShellSurfaceProvider value={{ visibility: deckNavigation.surfaceVisibility, setMemoryBreadcrumb }}>
          <div
            data-slot="shell-route-content"
            data-shell-surface-visibility={deckNavigation.surfaceVisibility}
            {...stylex.props(
              styles.routeContent,
              route.kind !== "chat" && styles.routeContentBelowHeader,
              deckNavigation.surfaceVisibility !== "visible" && styles.routeContentInactive
            )}
          >
            {children}
          </div>
        </ShellSurfaceProvider>
      </m.section>
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
      paddingBottom: "max(1rem, env(safe-area-inset-bottom))",
      visibility: "hidden",
      pointerEvents: "none"
    }
  },
  sidebarGroundOpen: {
    zIndex: 25,
    "@media (max-width: 760px)": {
      visibility: "visible",
      pointerEvents: "auto"
    }
  },
  sidebarCollapseButton: {
    position: "absolute",
    top: 16,
    left: "calc(16px + var(--shell-desktop-chrome-offset))",
    zIndex: 40,
    cursor: 'default',
    backgroundColor: "transparent",
    ":hover": {
      backgroundColor: "rgb(0 0 0 / 0.05)",
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
    touchAction: "pan-y",
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
    transitionProperty: "scale, border-radius, box-shadow",
    transitionDuration: "var(--motion-spring-surface-duration)",
    transitionTimingFunction: "var(--motion-spring-critical-easing)",
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
    pointerEvents: "none",
    "@media (min-width: 761px)": {
      scale: 0.97
    },
    "@media (max-width: 760px)": {
      borderRadius: 12
    }
  },
  deckHeader: {
    position: "relative",
    zIndex: 2,
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: 12,
    borderBottomWidth: 1,
    borderBottomStyle: "solid",
    borderBottomColor: "transparent",
    backgroundColor: "transparent",
    minHeight: "var(--shell-deck-header-height)",
    paddingBlock: 4,
    paddingInline: 16
  },
  deckHeaderSolid: {
    borderBottomColor: "var(--border-subtle)",
    backgroundColor: "var(--background)"
  },
  headerScrim: {
    position: "absolute",
    top: 0,
    right: 0,
    left: 0,
    height: "calc(var(--shell-deck-header-height) + 52px)",
    zIndex: 0,
    pointerEvents: "none",
    backgroundImage:
      "linear-gradient(to bottom, var(--background) 0, rgb(255 255 255 / 0.74) 54px, rgb(255 255 255 / 0) 96px)"
  },
  headerOffset: {
    position: "relative",
    zIndex: 1,
    display: "flex",
    minWidth: 0,
    alignItems: "center",
    transitionProperty: "transform",
    transitionDuration: "var(--motion-spring-surface-duration)",
    transitionTimingFunction: "var(--motion-spring-critical-easing)",
    "@media (prefers-reduced-motion: reduce)": {
      transition: "none"
    }
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
  memoryPopoverScope: {
    "--color-background-popover": "color-mix(in srgb, var(--surface-sunken) 86%, transparent)",
    "--radius-container": "10px",
    "--shadow-low": "var(--shadow-med)"
  },
  memoryTreePopoverContent: {
    overflow: "hidden",
    backgroundColor: "transparent",
    padding: "var(--spacing-2)",
    backdropFilter: "blur(18px) saturate(1.12)",
    WebkitBackdropFilter: "blur(18px) saturate(1.12)"
  },
  headerAction: {
    position: "relative",
    zIndex: 1,
    minWidth: 0
  },
  breadcrumb: {
    display: "flex",
    minWidth: 0,
    alignItems: "center",
    gap: 8,
    overflow: "hidden"
  },
  breadcrumbParent: {
    minWidth: 0,
    flexShrink: 1,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    fontSize: 14,
    color: "var(--muted-foreground)"
  },
  breadcrumbRoot: { flexShrink: 0 },
  memoryBreadcrumbTrigger: {
    display: "flex",
    minWidth: 0,
    maxWidth: "min(70vw, 720px)",
    alignItems: "center",
    gap: "var(--spacing-1)",
    borderWidth: 0,
    borderRadius: 4,
    backgroundColor: "transparent",
    padding: "var(--spacing-1) var(--spacing-1-5)",
    color: "inherit",
    textAlign: "left",
    cursor: "pointer",
    ":hover": { backgroundColor: "var(--surface-hover)" },
    ":focus-visible": {
      outlineWidth: 2,
      outlineStyle: "solid",
      outlineColor: "var(--ring)",
      outlineOffset: 1
    }
  },
  memoryBreadcrumbPath: { flex: 1 },
  breadcrumbChevron: { flexShrink: 0, color: "var(--muted-foreground)" },
  breadcrumbSeparator: {
    color: "color-mix(in srgb, var(--muted-foreground) 70%, transparent)"
  },
  breadcrumbCurrent: {
    display: "block",
    minWidth: 0,
    flexShrink: 1,
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
    height: "calc(100% + var(--shell-deck-header-height))",
    marginTop: "calc(var(--shell-deck-header-height) * -1)",
    overflow: "visible"
  },
  routeContentBelowHeader: {
    "--shell-deck-header-height": "0px",
    height: "100%",
    marginTop: 0
  },
  routeContentInactive: {
    pointerEvents: "none"
  }
});

function popoverXStyle(...xstyle: unknown[]): PopoverProps["xstyle"] {
  return xstyle as unknown as PopoverProps["xstyle"];
}
