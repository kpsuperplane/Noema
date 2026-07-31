import React from "react";
import { useQuery } from "@apollo/client/react";
import { Button, type ButtonProps } from "@astryxdesign/core/Button";
import { TopNav } from "@astryxdesign/core/TopNav";
import * as stylex from "@stylexjs/stylex";
import { Settings } from "lucide-react";
import {
  AnimatePresence,
  useIsPresent,
  useMotionValue,
  useReducedMotion
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
export const shellContentMaxWidth = "860px";

type ShellRootStyle = React.CSSProperties &
  Record<
    "--shell-sidebar-width" | "--shell-desktop-chrome-offset" | "--shell-content-max-width",
    string
  >;

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
    "--shell-desktop-chrome-offset": desktopChromeOffset,
    "--shell-content-max-width": shellContentMaxWidth
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
    <TopNav
      label="Primary"
      data-tauri-drag-region
      centerContent={
        <>
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
        </>
      }
      endContent={attention ? (
        <span {...stylex.props(styles.primaryNavigationAttention)}>{attention}</span>
      ) : null}
    />
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
      initial={reduceMotion ? false : { opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
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

    const mobile = window.matchMedia("(max-width: 760px)");
    const syncVisualViewport = () => {
      if (!mobile.matches || viewport.scale !== 1) {
        root.style.removeProperty("--shell-visual-viewport-height");
        root.style.removeProperty("--shell-visual-viewport-offset-top");
        return;
      }

      const maximumViewportTop = Math.max(
        0,
        document.documentElement.clientHeight - viewport.height
      );
      const viewportTop = Math.min(maximumViewportTop, Math.max(0, viewport.offsetTop));
      root.style.setProperty("--shell-visual-viewport-height", `${viewport.height}px`);
      root.style.setProperty("--shell-visual-viewport-offset-top", `${viewportTop}px`);
    };

    syncVisualViewport();
    viewport.addEventListener("resize", syncVisualViewport);
    viewport.addEventListener("scroll", syncVisualViewport);
    mobile.addEventListener("change", syncVisualViewport);
    return () => {
      viewport.removeEventListener("resize", syncVisualViewport);
      viewport.removeEventListener("scroll", syncVisualViewport);
      mobile.removeEventListener("change", syncVisualViewport);
      root.style.removeProperty("--shell-visual-viewport-height");
      root.style.removeProperty("--shell-visual-viewport-offset-top");
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

      <aside
        id="noema-shell-sidebar"
        data-slot="shell-sidebar-ground"
        aria-hidden={!hasShellSidebar}
        aria-label={sidebarLabel}
        inert={!hasShellSidebar}
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
      </aside>

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
    height: "100dvh",
    overflow: "hidden",
    color: "var(--foreground)"
  },
  desktopRoot: {
    backgroundColor: "rgba(233, 242, 236, 0.6)",
    backdropFilter: "brightness(1.05)"
  },
  browserRoot: {
    backgroundColor: "var(--pine-50)",
    "@media (max-width: 760px) and (prefers-reduced-motion: no-preference)": {
      transitionProperty: "height, transform",
      transitionDuration: "var(--motion-spring-standard-duration)",
      transitionTimingFunction: "var(--motion-spring-critical-easing)"
    },
    "@media (max-width: 760px)": {
      position: "fixed",
      top: 0,
      right: 0,
      bottom: "auto",
      left: 0,
      height: "var(--shell-visual-viewport-height, 100dvh)",
      transform: "translateY(var(--shell-visual-viewport-offset-top, 0px))"
    }
  },
  sidebarGround: {
    position: "absolute",
    top: 52,
    bottom: 0,
    left: 0,
    zIndex: 10,
    display: "grid",
    minHeight: 0,
    width: "var(--shell-sidebar-width)",
    pointerEvents: "none",
    "@media (max-width: 760px)": {
      width: "min(286px, 78vw)",
      paddingBottom: "max(1rem, env(safe-area-inset-bottom))",
      visibility: "hidden"
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
    gridTemplateRows: "minmax(0, 1fr)"
  },
  sidebarGroundVisible: {
    "@media (max-width: 760px)": {
      visibility: "visible"
    }
  },
  sidebarGroundOpen: {
    zIndex: 25,
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
    position: "absolute",
    top: 52,
    right: 0,
    bottom: 0,
    left: 0,
    zIndex: 20,
    cursor: "default",
    touchAction: "pan-y",
    backgroundColor: "transparent",
    borderWidth: 0,
    padding: "var(--spacing-0)"
  },
  contentDeck: {
    "--shell-deck-header-height": "0px",
    position: "absolute",
    zIndex: 30,
    display: "grid",
    minHeight: 0,
    gridTemplateRows: "minmax(0, 1fr)",
    overflow: "hidden",
    touchAction: "pan-y",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    backgroundColor: "var(--background)",
    boxShadow: "var(--shadow-shell-frame)",
    cornerShape: "var(--corner-shape-page)",
    transitionProperty: "scale, border-radius, box-shadow",
    transitionDuration: "var(--motion-spring-surface-duration)",
    transitionTimingFunction: "var(--motion-spring-critical-easing)",
    "@media (prefers-reduced-motion: reduce)": {
      transition: "none"
    },
    "@media (max-width: 760px)": {
      borderWidth: 0,
      borderRadius: "var(--radius-page) var(--radius-page) 0 0"
    }
  },
  contentDeckPrimary: {
    top: 52,
    right: 8,
    bottom: 8,
    left: 8,
    borderRadius: "var(--radius-page)",
    "@media (max-width: 760px)": {
      right: 0,
      bottom: 0,
      left: 0
    }
  },
  contentDeckWithSidebar: {
    top: 52,
    right: 8,
    bottom: 8,
    left: "calc(var(--shell-sidebar-width))",
    borderRadius: "var(--radius-page)",
    "@media (max-width: 760px)": {
      right: 0,
      bottom: 0,
      left: 0
    }
  },
  contentDeckNavOpen: {
    pointerEvents: "none",
    "@media (min-width: 761px)": {
      scale: 0.97
    },
    "@media (max-width: 760px)": {
      borderRadius: "var(--radius-page)"
    }
  },
  shellNavbar: {
    position: "absolute",
    top: 0,
    right: 0,
    left: 0,
    zIndex: 40,
    display: "flex",
    boxSizing: "border-box",
    minWidth: 0,
    height: 52,
    alignItems: "center"
  },
  headerOffset: {
    position: "relative",
    zIndex: 1,
    display: "flex",
    flex: 1,
    minWidth: 0,
    alignItems: "center",
    boxSizing: "border-box",
    transitionProperty: "padding-left, padding-right",
    transitionDuration: "var(--motion-spring-surface-duration)",
    transitionTimingFunction: "var(--motion-spring-critical-easing)",
    "@media (prefers-reduced-motion: reduce)": {
      transition: "none"
    }
  },
  headerOffsetPrimary: {
    "@media (min-width: 761px)": {
      paddingInline: "var(--shell-desktop-chrome-offset)"
    }
  },
  primaryNavigationAttention: {
    display: "flex",
    minWidth: 0,
    alignItems: "center"
  },
  primaryNavigationButton: {
    borderRadius: 999,
    cornerShape: "var(--corner-shape-full)",
    color: "var(--pine-700)",
    gap: "var(--spacing-0)",
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
    minHeight: 0,
    height: "100%",
    overflow: "visible"
  },
  routeContentInactive: {
    pointerEvents: "none"
  }
});

function buttonXStyle(...xstyle: unknown[]): ButtonProps["xstyle"] {
  return xstyle as unknown as ButtonProps["xstyle"];
}
