import React from "react";
import { useQuery } from "@apollo/client/react";
import { Button, type ButtonProps } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { Settings } from "lucide-react";
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
        root.style.removeProperty("--shell-visual-viewport-bottom-inset");
        return;
      }

      root.style.setProperty("--shell-visual-viewport-height", `${viewport.height}px`);
      root.style.setProperty("--shell-visual-viewport-offset-top", `${viewport.offsetTop}px`);
      root.style.setProperty(
        "--shell-visual-viewport-bottom-inset",
        `${Math.max(0, window.innerHeight - viewport.height - viewport.offsetTop)}px`
      );
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
      root.style.removeProperty("--shell-visual-viewport-bottom-inset");
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

      {hasShellSidebar ? (
        <aside
          id="noema-shell-sidebar"
          data-slot="shell-sidebar-ground"
          aria-label={settingsOpen ? "Settings navigation" : workOpen ? "Task folders" : "Memory navigation"}
          {...stylex.props(
            styles.sidebarGround,
            sidebarVisible && styles.sidebarGroundVisible,
            deckNavigation.navOpen && styles.sidebarGroundOpen
          )}
        >
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
        </aside>
      ) : null}

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
    minHeight: "100dvh",
    overflow: "visible",
    paddingTop: 52,
    color: "var(--foreground)"
  },
  desktopRoot: {
    backgroundColor: "rgba(233, 242, 236, 0.6)",
    backdropFilter: "brightness(1.05)"
  },
  browserRoot: {
    backgroundColor: "var(--pine-50)",
    "@media (max-width: 760px)": {
      minHeight: "var(--shell-visual-viewport-height, 100dvh)"
    }
  },
  sidebarGround: {
    position: "fixed",
    top: "calc(52px + var(--shell-visual-viewport-offset-top, 0px))",
    bottom: "auto",
    left: 0,
    zIndex: 10,
    display: "grid",
    minHeight: 0,
    height: "calc(var(--shell-visual-viewport-height, 100dvh) - 52px)",
    transitionProperty: "top, height",
    transitionDuration: "var(--motion-spring-standard-duration)",
    transitionTimingFunction: "var(--motion-spring-critical-easing)",
    "@media (prefers-reduced-motion: reduce)": {
      transition: "none"
    },
    width: "var(--shell-sidebar-width)",
    "@media (max-width: 760px)": {
      width: "min(286px, 78vw)",
      paddingBottom: "max(1rem, env(safe-area-inset-bottom))",
      visibility: "hidden",
      pointerEvents: "none"
    }
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
    position: "fixed",
    top: "calc(52px + var(--shell-visual-viewport-offset-top, 0px))",
    right: 0,
    bottom: 0,
    left: 0,
    zIndex: 20,
    cursor: "default",
    touchAction: "pan-y",
    backgroundColor: "transparent",
    borderWidth: 0,
    padding: 0
  },
  contentDeck: {
    "--shell-deck-header-height": "0px",
    position: "relative",
    zIndex: 30,
    display: "grid",
    minHeight: "calc(var(--shell-visual-viewport-height, 100dvh) - 60px)",
    gridTemplateRows: "minmax(0, 1fr)",
    overflow: "visible",
    touchAction: "pan-y",
    cornerShape: "var(--corner-shape-page)",
    transitionProperty: "top, height, width, margin-left, margin-right, scale, border-radius, box-shadow",
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
    top: "var(--shell-visual-viewport-offset-top, 0px)",
    right: 0,
    left: 0,
    zIndex: 40,
    display: "flex",
    boxSizing: "border-box",
    minWidth: 0,
    height: 52,
    alignItems: "center",
    paddingBlock: "var(--spacing-2)",
    paddingInline: "var(--spacing-2)",
    transitionProperty: "top",
    transitionDuration: "var(--motion-spring-standard-duration)",
    transitionTimingFunction: "var(--motion-spring-critical-easing)",
    "@media (prefers-reduced-motion: reduce)": {
      transition: "none"
    }
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
    boxShadow: "var(--shadow-low)",
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
