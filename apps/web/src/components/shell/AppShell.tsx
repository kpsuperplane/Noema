import React from "react";
import { useQuery } from "@apollo/client/react";
import { Button, type ButtonProps } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { Settings } from "lucide-react";
import {
  AnimatePresence,
  useIsPresent,
  useMotionValue,
  useReducedMotion,
  useTransform,
  type MotionStyle,
  type MotionValue
} from "motion/react";
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
  activeL0ItemId,
  breadcrumbForRoute,
  shellMenuLevelForRoute,
  shellPrimaryItems,
  workMenuLevelForProjects,
  type ShellBreadcrumb,
  type ShellMenuItem
} from "./shellNavigation";
import { useShellNavSwipe } from "./useShellNavSwipe";
import { ShellAttentionItem } from "./ShellAttentionItem";
import { ShellContentFrame } from "./ShellContentFrame";
import { MemoryPageTree } from "@/pages/MemoryPageTree";
import {
  IdentityAvatar,
  LOCAL_AGENT_AVATAR_ID,
  type IdentityAvatarActivity
} from "@/components/IdentityAvatar";
import { useAllWorkProjects } from "@/components/work/useAllWorkProjects";
import { WorkSidebar } from "@/components/work/WorkSidebar";

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
export const shellTauriDesktopChromeOffset = "88px";

type ShellRootStyle = React.CSSProperties &
  Record<"--shell-sidebar-width" | "--shell-desktop-chrome-offset", string>;

type ShellSidebarMotionStyle = MotionStyle &
  Record<"--shell-sidebar-reveal", MotionValue<string>>;

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

function PrimaryNavigationLabel({
  active,
  children
}: {
  active: boolean;
  children: React.ReactNode;
}) {
  return (
    <>
      <span aria-hidden="true" {...stylex.props(styles.primaryNavigationDesktopLabel)}>
        <span {...stylex.props(styles.primaryNavigationLabelText)}>{children}</span>
      </span>
      <m.span
        aria-hidden={!active}
        initial={false}
        animate={{
          width: active ? "auto" : 0,
          marginInlineStart: active ? "var(--spacing-2)" : "0px",
          opacity: active ? 1 : 0
        }}
        transition={springs.micro}
        {...stylex.props(styles.primaryNavigationMobileLabel)}
      >
        <span {...stylex.props(styles.primaryNavigationLabelText)}>{children}</span>
      </m.span>
    </>
  );
}

function PrimarySurfaceNavigation({
  route,
  agentName,
  agentAvatarActivity,
  attention,
  onNavigate
}: {
  route: AppRoute;
  agentName: string | null;
  agentAvatarActivity: IdentityAvatarActivity;
  attention: React.ReactNode;
  onNavigate: (route: AppRoute) => void;
}) {
  const settingsActive = route.kind === "settings";
  const activeItemId = route.kind === "settings" ? "settings" : activeL0ItemId(route);
  const namedAgent = agentName?.trim() || null;
  const [agentButtonHovered, setAgentButtonHovered] = React.useState(false);

  return (
    <nav
      aria-label="Primary"
      data-tauri-drag-region
      {...stylex.props(styles.primaryNavigation)}
    >
      {shellPrimaryItems.map((item) => {
        const Icon = item.icon;
        const active = item.itemId === activeItemId;
        const namedChat = item.itemId === "chat" && namedAgent !== null;
        const label = item.itemId === "chat" ? namedAgent ?? item.label : item.label;

        return (
          <Button
            key={item.itemId}
            type="button"
            variant="ghost"
            size="lg"
            label={label}
            icon={namedChat ? (
              <span aria-hidden="true" {...stylex.props(styles.primaryAgentIcon)}>
                <IdentityAvatar
                  actorId={LOCAL_AGENT_AVATAR_ID}
                  actorType="agent"
                  activity={agentAvatarActivity}
                  animated={agentButtonHovered || agentAvatarActivity !== "idle"}
                  focusable={false}
                  size="nav"
                />
              </span>
            ) : <Icon aria-hidden="true" size={20} />}
            aria-current={active ? "page" : undefined}
            xstyle={buttonXStyle(
              styles.primaryNavigationButton,
              namedChat && styles.primaryAgentNavigationButton,
              active && styles.primaryNavigationButtonActive
            )}
            onMouseEnter={namedChat ? () => setAgentButtonHovered(true) : undefined}
            onMouseLeave={namedChat ? () => setAgentButtonHovered(false) : undefined}
            onClick={() => item.route && onNavigate(item.route)}
          >
            <PrimaryNavigationLabel active={active}>{label}</PrimaryNavigationLabel>
          </Button>
        );
      })}
      <Button
        data-slot="shell-settings-button"
        type="button"
        variant="ghost"
        size="lg"
        label="Settings"
        icon={<Settings aria-hidden="true" size={20} />}
        aria-current={settingsActive ? "page" : undefined}
        xstyle={buttonXStyle(
          styles.primaryNavigationButton,
          settingsActive && styles.primaryNavigationButtonActive
        )}
        onClick={() => onNavigate({ kind: "settings", section: "agents" })}
      >
        <PrimaryNavigationLabel active={settingsActive}>Settings</PrimaryNavigationLabel>
      </Button>
      {attention ? (
        <span {...stylex.props(styles.primaryNavigationAttention)}>{attention}</span>
      ) : null}
    </nav>
  );
}

function ShellSidebarRouteContent({ children }: { children: React.ReactNode }) {
  const isPresent = useIsPresent();
  const reduceMotion = useReducedMotion();

  return (
    <m.div
      data-slot="shell-sidebar-route-content"
      aria-hidden={isPresent ? undefined : "true"}
      inert={!isPresent}
      initial={reduceMotion ? false : { "--shell-sidebar-route-reveal": "8px" }}
      animate={{ "--shell-sidebar-route-reveal": shellDesktopSidebarWidth }}
      exit={{ "--shell-sidebar-route-reveal": "8px" }}
      transition={reduceMotion ? { duration: 0 } : springs.surface}
      {...stylex.props(styles.sidebarRouteContent)}
    >
      {children}
    </m.div>
  );
}

export function AppShell({
  route,
  status,
  socketState,
  agentAvatarActivity = "idle",
  providerBlocked = false,
  setupBlocked = false,
  onNavigate,
  children
}: {
  route: AppRoute;
  status: LocalStatusQuery["localStatus"] | null;
  socketState: SocketState;
  agentAvatarActivity?: IdentityAvatarActivity;
  providerBlocked?: boolean;
  setupBlocked?: boolean;
  onNavigate: (route: AppRoute) => void;
  children: React.ReactNode;
}) {
  const [memoryBreadcrumb, setMemoryBreadcrumb] = React.useState<ShellMemoryBreadcrumb | null>(null);
  const shellRootRef = React.useRef<HTMLElement | null>(null);
  const deckX = useMotionValue(0);
  const memoryTreeResult = useQuery<MemoryTreeQuery>(MemoryTreeDocument, {
    fetchPolicy: "cache-only",
    skip: route.kind !== "memory"
  });
  const memoryTree = memoryTreeResult.data?.memoryTree ?? null;
  const memoryOpen = route.kind === "memory";
  const settingsOpen = route.kind === "settings";
  const workOpen = route.kind === "work";
  const workProjectsResult = useAllWorkProjects({ skip: !workOpen });
  const memoryHasSidebar = memoryOpen && memoryTree?.root
    ? memoryTree.pages.some((page) => page.path !== memoryTree.root?.path)
    : false;
  const workMenuLevel = workOpen
    ? workMenuLevelForProjects(workProjectsResult.projects, route.projectId)
    : null;
  const hasShellSidebar = memoryHasSidebar || settingsOpen || workOpen;
  const sidebarLabel = !hasShellSidebar
    ? undefined
    : settingsOpen
      ? "Settings navigation"
      : workOpen
        ? "Task folders"
        : "Memory navigation";
  const menuLevel = settingsOpen
    ? shellMenuLevelForRoute(route)
    : workMenuLevel;
  const breadcrumb = breadcrumbForRoute(route);
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

  React.useEffect(() => {
    const root = shellRootRef.current;
    const viewport = typeof window !== "undefined" ? window.visualViewport : null;
    if (isDesktopRuntime || !root || !viewport) {
      return;
    }

    const viewportProperties = [
      "--shell-chrome-viewport-height",
      "--shell-chrome-viewport-top"
    ] as const;
    let syncFrame: number | null = null;
    const clearVisualViewport = () => {
      for (const property of viewportProperties) root.style.removeProperty(property);
    };
    const setViewportProperty = (property: typeof viewportProperties[number], value: number) => {
      const nextValue = `${Math.round(value)}px`;
      if (root.style.getPropertyValue(property) !== nextValue) root.style.setProperty(property, nextValue);
    };
    const syncVisualViewport = () => {
      if (viewport.scale !== 1) {
        clearVisualViewport();
        return;
      }

      const maximumViewportTop = Math.max(
        0,
        document.documentElement.clientHeight - viewport.height
      );
      const viewportTop = Math.min(maximumViewportTop, Math.max(0, viewport.offsetTop));
      setViewportProperty("--shell-chrome-viewport-height", viewport.height);
      setViewportProperty("--shell-chrome-viewport-top", viewportTop);
    };
    const scheduleVisualViewportSync = () => {
      if (syncFrame !== null) {
        return;
      }
      syncFrame = window.requestAnimationFrame(() => {
        syncFrame = null;
        syncVisualViewport();
      });
    };

    scheduleVisualViewportSync();
    viewport.addEventListener("resize", scheduleVisualViewportSync);
    viewport.addEventListener("scroll", scheduleVisualViewportSync);
    return () => {
      viewport.removeEventListener("resize", scheduleVisualViewportSync);
      viewport.removeEventListener("scroll", scheduleVisualViewportSync);
      if (syncFrame !== null) window.cancelAnimationFrame(syncFrame);
      clearVisualViewport();
    };
  }, [isDesktopRuntime]);

  const {
    state: deckNavigation,
    menuButtonRef,
    openNav,
    closeNav,
    settleSurfaceVisibility
  } = useDeckNavigation();
  const navSwipe = useShellNavSwipe({
    enabled: hasShellSidebar,
    navOpen: deckNavigation.navOpen,
    openNav,
    closeNav,
    deckX,
    onSettled: settleSurfaceVisibility
  });
  const sidebarVisible = deckNavigation.navOpen || navSwipe.active;
  const sidebarReveal = useTransform(deckX, (value) => `${Math.max(0, value)}px`);
  const sidebarStyle = {
    "--shell-sidebar-reveal": sidebarReveal
  } as ShellSidebarMotionStyle;

  const selectShellMenuItem = React.useCallback(
    (item: ShellMenuItem) => {
      if (!item.route) {
        return;
      }

      onNavigate(item.route);
      if (deckNavigation.navOpen) closeNav();
    },
    [closeNav, deckNavigation.navOpen, onNavigate]
  );

  const navigatePrimary = React.useCallback((nextRoute: AppRoute) => {
    onNavigate(nextRoute);
    if (deckNavigation.navOpen) closeNav();
  }, [closeNav, deckNavigation.navOpen, onNavigate]);

  React.useEffect(() => {
    if (!hasShellSidebar && deckNavigation.navOpen) closeNav();
  }, [closeNav, deckNavigation.navOpen, hasShellSidebar]);

  React.useLayoutEffect(() => {
    if (route.kind !== "chat") {
      window.scrollTo({ top: 0, behavior: "auto" });
    }
  }, [route]);

  return (
    <main
      ref={shellRootRef}
      data-slot="shell-root"
      data-nav-open={deckNavigation.navOpen}
      data-nav-swipe-active={navSwipe.dragging ? "true" : undefined}
      data-settings-open={settingsOpen ? "true" : undefined}
      data-tauri-runtime={isDesktopRuntime}
      style={rootStyle}
      {...stylex.props(styles.root, isDesktopRuntime ? styles.desktopRoot : styles.browserRoot)}
      {...navSwipe.pointerHandlers}
    >
      <header
        data-slot="shell-navbar"
        data-tauri-drag-region
        {...stylex.props(styles.shellNavbar)}
      >
        <div
          data-slot="shell-header-offset"
          data-tauri-drag-region
          {...stylex.props(styles.headerOffset, styles.headerOffsetPrimary)}
        >
          <PrimarySurfaceNavigation
            route={route}
            agentName={status?.primaryAgentDisplayName ?? null}
            agentAvatarActivity={agentAvatarActivity}
            attention={attention ? <ShellAttentionItem attention={attention} compact /> : null}
            onNavigate={navigatePrimary}
          />
        </div>
      </header>

      <m.aside
        id="noema-shell-sidebar"
        data-slot="shell-sidebar-ground"
        aria-hidden={!hasShellSidebar}
        aria-label={sidebarLabel}
        inert={!hasShellSidebar}
        style={sidebarStyle}
        {...stylex.props(
          styles.sidebarGround,
          hasShellSidebar && styles.sidebarGroundAvailable,
          sidebarVisible && styles.sidebarGroundVisible,
          deckNavigation.navOpen && styles.sidebarGroundOpen
        )}
      >
        <AnimatePresence initial={false}>
          {hasShellSidebar ? (
            <ShellSidebarRouteContent key="shell-sidebar-route-content">
              {workMenuLevel ? (
                <WorkSidebar
                  menuLevel={workMenuLevel}
                  projects={workProjectsResult.projects}
                  onSelectItem={selectShellMenuItem}
                  onUpdated={async () => {
                    await workProjectsResult.refetch();
                  }}
                />
              ) : menuLevel ? (
                <ShellSidebar menuLevel={menuLevel} onSelectItem={selectShellMenuItem} />
              ) : memoryTree?.root ? (
                <MemoryPageTree
                  activePath={memoryBreadcrumb?.currentPath ?? memoryTree.root.path}
                  pages={memoryTree.pages}
                  root={memoryTree.root}
                  onNavigate={deckNavigation.navOpen ? closeNav : undefined}
                />
              ) : (
                <div role="status" {...stylex.props(styles.sidebarState)}>
                  {memoryTree ? "No memory pages yet." : "Loading memory…"}
                </div>
              )}
            </ShellSidebarRouteContent>
          ) : null}
        </AnimatePresence>
      </m.aside>

      {deckNavigation.navOpen ? (
        <button
          type="button"
          data-slot="shell-nav-backdrop"
          aria-label="Close navigation"
          {...stylex.props(styles.navBackdrop)}
          onClick={closeNav}
        />
      ) : null}

      <ShellContentFrame
        deckX={deckX}
        hasSidebar={hasShellSidebar}
        navOpen={deckNavigation.navOpen}
      />

      <m.section
        data-slot="shell-content-deck"
        data-nav-open={deckNavigation.navOpen}
        data-nav-swipe-active={navSwipe.dragging ? "true" : undefined}
        aria-label={activeLabel}
        style={{ x: deckX }}
        {...stylex.props(
          styles.contentDeck,
          hasShellSidebar ? styles.contentDeckWithSidebar : styles.contentDeckPrimary,
          deckNavigation.navOpen && styles.contentDeckNavOpen
        )}
      >
        <ShellSurfaceProvider
          value={{
            visibility: deckNavigation.surfaceVisibility,
            sidebarAvailable: hasShellSidebar,
            sidebarOpen: deckNavigation.navOpen,
            sidebarTriggerRef: menuButtonRef,
            openSidebar: openNav,
            setMemoryBreadcrumb
          }}
        >
          <div
            data-slot="shell-route-content"
            data-shell-surface-visibility={deckNavigation.surfaceVisibility}
            aria-hidden={deckNavigation.surfaceVisibility !== "visible"}
            inert={deckNavigation.surfaceVisibility !== "visible"}
            {...stylex.props(
              styles.routeContent,
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
    minHeight: "100svh",
    overflow: "visible",
    paddingTop: 52,
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
    position: "fixed",
    top: "calc(52px + var(--shell-chrome-viewport-top, 0px))",
    bottom: "auto",
    left: 0,
    zIndex: 33,
    display: "grid",
    minHeight: 0,
    height: "calc(var(--shell-chrome-viewport-height, 100dvh) - 52px)",
    width: "var(--shell-sidebar-width)",
    pointerEvents: "none",
    "@media (max-width: 760px)": {
      width: "min(286px, 78vw)",
      paddingBottom: "max(1rem, env(safe-area-inset-bottom))",
      clipPath: "inset(0 calc(100% - var(--shell-sidebar-reveal)) 0 0)",
      visibility: "hidden",
    }
  },
  sidebarGroundAvailable: {
    "@media (min-width: 761px)": {
      pointerEvents: "auto"
    }
  },
  sidebarRouteContent: {
    display: "grid",
    minHeight: 0,
    height: "100%",
    width: "100%",
    gridTemplateRows: "minmax(0, 1fr)",
    clipPath: "inset(0 calc(100% - var(--shell-sidebar-route-reveal)) 0 0)",
    "@media (max-width: 760px)": {
      clipPath: "none"
    }
  },
  sidebarGroundVisible: {
    "@media (max-width: 760px)": {
      visibility: "visible"
    }
  },
  sidebarGroundOpen: {
    zIndex: 35,
    "@media (max-width: 760px)": {
      pointerEvents: "auto"
    }
  },
  sidebarState: {
    paddingBlock: "var(--spacing-4)",
    paddingInline: "var(--spacing-5)",
    color: "var(--muted-foreground)",
    fontSize: 13
  },
  navBackdrop: {
    position: "fixed",
    top: "calc(52px + var(--shell-chrome-viewport-top, 0px))",
    right: 0,
    bottom: "auto",
    left: 0,
    zIndex: 20,
    cursor: "default",
    height: "calc(var(--shell-chrome-viewport-height, 100dvh) - 52px)",
    touchAction: "none",
    backgroundColor: "transparent",
    borderWidth: 0,
    padding: 0
  },
  contentDeck: {
    "--shell-deck-header-height": "0px",
    position: "relative",
    zIndex: 30,
    display: "grid",
    minHeight: "calc(100svh - 60px)",
    gridTemplateRows: "minmax(0, 1fr)",
    overflow: "visible",
    touchAction: "pan-y",
    cornerShape: "var(--corner-shape-page)",
    transitionProperty: "width, margin-left, margin-right, scale, border-radius, box-shadow",
    transitionDuration: "var(--motion-spring-surface-duration)",
    transitionTimingFunction: "var(--motion-spring-critical-easing)",
    "@media (prefers-reduced-motion: reduce)": {
      transition: "none"
    },
    "@media (max-width: 760px)": {
      borderRadius: "18px 18px 0 0"
    }
  },
  contentDeckPrimary: {
    width: "calc(100% - 16px)",
    marginRight: 8,
    marginBottom: 8,
    marginLeft: 8,
    borderRadius: 18,
    "@media (max-width: 760px)": {
      width: "100%",
      marginRight: 0,
      marginBottom: 0,
      marginLeft: 0
    }
  },
  contentDeckWithSidebar: {
    width: "calc(100% - var(--shell-sidebar-width) - 8px)",
    marginRight: 8,
    marginBottom: 8,
    marginLeft: "var(--shell-sidebar-width)",
    borderRadius: 18,
    "@media (max-width: 760px)": {
      width: "100%",
      marginRight: 0,
      marginBottom: 0,
      marginLeft: 0
    }
  },
  contentDeckNavOpen: {
    pointerEvents: "none",
    "@media (min-width: 761px)": {
      scale: 0.97
    },
    "@media (max-width: 760px)": {
      borderRadius: 18
    }
  },
  shellNavbar: {
    position: "fixed",
    top: "var(--shell-chrome-viewport-top, 0px)",
    right: 0,
    left: 0,
    zIndex: 40,
    display: "flex",
    boxSizing: "border-box",
    minWidth: 0,
    height: 52,
    alignItems: "center",
    paddingBlock: "var(--spacing-2)",
    paddingInline: "var(--spacing-2)"
  },
  headerOffset: {
    position: "relative",
    zIndex: 1,
    display: "flex",
    flex: 1,
    minWidth: 0,
    alignItems: "center",
    boxSizing: "border-box",
    transitionProperty: "padding-left",
    transitionDuration: "var(--motion-spring-surface-duration)",
    transitionTimingFunction: "var(--motion-spring-critical-easing)",
    "@media (prefers-reduced-motion: reduce)": {
      transition: "none"
    }
  },
  headerOffsetPrimary: {
    "@media (min-width: 761px)": {
      paddingLeft: "var(--shell-desktop-chrome-offset)"
    }
  },
  primaryNavigation: {
    display: "flex",
    flex: 1,
    minWidth: 0,
    alignItems: "center",
    gap: "var(--spacing-1)"
  },
  primaryNavigationAttention: {
    display: "flex",
    minWidth: 0,
    marginLeft: "auto",
    alignItems: "center"
  },
  primaryNavigationButton: {
    borderRadius: 999,
    cornerShape: "var(--corner-shape-full)",
    color: "var(--pine-700)",
    gap: 0,
    paddingInline: "var(--spacing-2)"
  },
  primaryNavigationDesktopLabel: {
    display: "block",
    minWidth: 0,
    marginInlineStart: "var(--spacing-2)",
    overflow: "hidden",
    whiteSpace: "nowrap",
    "@media (max-width: 760px)": {
      display: "none"
    }
  },
  primaryNavigationMobileLabel: {
    display: "block",
    minWidth: 0,
    overflow: "hidden",
    whiteSpace: "nowrap",
    "@media (min-width: 761px)": {
      display: "none"
    }
  },
  primaryNavigationLabelText: {
    display: "block",
    paddingInlineEnd: "var(--spacing-1)"
  },
  primaryAgentNavigationButton: {
    maxWidth: "min(18rem, 42vw)"
  },
  primaryAgentIcon: {
    display: "flex",
    width: 20,
    height: 20,
    flexShrink: 0,
    alignItems: "center",
    justifyContent: "center",
    lineHeight: 0
  },
  primaryNavigationButtonActive: {
    backgroundColor: "var(--pure-white)",
    boxShadow: "var(--shadow-shell-control)",
    color: "var(--pine-700)",
    ":hover": {
      "@media (hover: hover)": {
        backgroundColor: "var(--pure-white)"
      }
    },
    ":active": {
      backgroundColor: "var(--pure-white)"
    }
  },
  routeContent: {
    minHeight: "inherit",
    overflow: "visible"
  },
  routeContentInactive: {
    pointerEvents: "none"
  }
});

function buttonXStyle(...xstyle: unknown[]): ButtonProps["xstyle"] {
  return xstyle as unknown as ButtonProps["xstyle"];
}
