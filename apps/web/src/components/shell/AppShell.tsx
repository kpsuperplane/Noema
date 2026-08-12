import React from "react";
import { useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { TopNav } from "@astryxdesign/core/TopNav";
import * as stylex from "@stylexjs/stylex";
import { type ShouldBlockFn, useBlocker, useLocation } from "@tanstack/react-router";
import { Settings } from "lucide-react";
import {
  animate,
  AnimatePresence,
  useIsPresent,
  useReducedMotion
} from "motion/react";
import * as m from "motion/react-m";
import {
  MemoryTreeDocument,
  type LocalStatusQuery,
  type MemoryTreeQuery
} from "@/generated/graphql";
import { isTauriRuntime } from "@/graphql/transportMode";
import { iosPageFadeTransition, shouldUseIosPageFade } from "@/motion/pageWave";
import { springs } from "@/motion/springs";
import {
  pageSurfaceKeyForPathname,
  hrefForRoute,
  pathnamesSharePageSurface,
  type AppRoute
} from "@/app/routes";
import type { SocketState } from "@/shared/types";
import { useDeckNavigation } from "./deckNavigation";
import { ShellSidebar } from "./ShellSidebar";
import {
  ShellSurfaceProvider,
  type ShellMemoryBreadcrumb
} from "./ShellSurfaceContext";
import {
  activeL0ItemId,
  adjacentPrimaryRoute,
  breadcrumbForRoute,
  shellMenuLevelForRoute,
  shellPrimaryItems,
  tasksMenuLevelForProjects,
  type ShellBreadcrumb,
  type ShellMenuItem
} from "./shellNavigation";
import { ShellAttentionItem } from "./ShellAttentionItem";
import { MobileTitleNavigation } from "./MobileTitleNavigation";
import { useMobileMenuRevealHeight } from "./useMobileMenuRevealHeight";
import { useMobileMenuPullGesture } from "./useMobileMenuPullGesture";
import { MemoryPageIcon } from "@/pages/MemoryPageIcon";
import { MemoryPageTree } from "@/pages/MemoryPageTree";
import {
  IdentityAvatar,
  LOCAL_AGENT_AVATAR_ID,
  type IdentityAvatarActivity
} from "@/components/IdentityAvatar";
import { useTaskProjects } from "@/components/tasks/useTaskProjects";
import { TasksSidebar } from "@/components/tasks/TasksSidebar";
import type { PwaRuntimeSnapshot } from "@/pwa/runtime";

export type ShellAttention = {
  tone: "progress" | "warning";
  title: string;
  message: string;
};

export type ShellAttentionInput = {
  route: AppRoute;
  status: LocalStatusQuery["localStatus"] | null;
  socketState: SocketState;
  providerBlocked: boolean;
  setupBlocked: boolean;
  recovery: PwaRuntimeSnapshot;
};

export const shellDesktopSidebarWidth = "216px";
export const shellBrowserDesktopChromeOffset = "0px";
export const shellTauriDesktopChromeOffset = "88px";
export const shellContentMaxWidth = "860px";

type ShellRootStyle = React.CSSProperties &
  Record<
    | "--shell-sidebar-width"
    | "--shell-desktop-chrome-offset"
    | "--shell-content-max-width"
    | "--shell-safe-top",
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
    "--shell-content-max-width": shellContentMaxWidth,
    "--shell-safe-top": "env(safe-area-inset-top, 0px)"
  } as ShellRootStyle;
}

export function shellAttentionForState(input: ShellAttentionInput): ShellAttention | null {
  if (input.recovery.updating) {
    return {
      tone: "warning",
      title: "Updating Noema…",
      message: "Your saved work is being secured before Noema refreshes."
    };
  }

  if (input.recovery.state === "checking") {
    return {
      tone: "warning",
      title: "Checking connection…",
      message: "Changes remain locked until Noema verifies this session."
    };
  }

  if (input.recovery.state === "reconciling") {
    return {
      tone: "progress",
      title: "Syncing",
      message: "Noema is updating saved data in the background."
    };
  }

  if (input.recovery.state === "offline") {
    const savedAt = input.recovery.lastSync
      ? new Intl.DateTimeFormat(undefined, {
          dateStyle: "medium",
          timeStyle: "short"
        }).format(input.recovery.lastSync)
      : "an earlier session";
    return {
      tone: "warning",
      title: "Offline",
      message: `Showing saved data from ${savedAt}. Reconnect to make changes.`
    };
  }

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

export function PrimarySurfaceNavigation({
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
                href={item.route ? hrefForRoute(item.route) : undefined}
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
                xstyle={[
                  styles.primaryNavigationButton,
                  namedChat && styles.primaryAgentNavigationButton,
                  active && styles.primaryNavigationButtonActive
                ]}
                onMouseEnter={namedChat ? () => setAgentButtonHovered(true) : undefined}
                onMouseLeave={namedChat ? () => setAgentButtonHovered(false) : undefined}
                onClick={(event) => {
                  if (!item.route || !shouldHandleShellLink(event)) return;
                  event.preventDefault();
                  onNavigate(item.route);
                }}
              >
                <PrimaryNavigationLabel active={active}>{label}</PrimaryNavigationLabel>
              </Button>
            );
          })}
          <Button
            data-slot="shell-settings-button"
            href={hrefForRoute({ kind: "settings", section: "agents" })}
            variant="ghost"
            size="lg"
            label="Settings"
            icon={<Settings aria-hidden="true" size={20} />}
            aria-current={settingsActive ? "page" : undefined}
            xstyle={[
              styles.primaryNavigationButton,
              settingsActive && styles.primaryNavigationButtonActive
            ]}
            onClick={(event) => {
              if (!shouldHandleShellLink(event)) return;
              event.preventDefault();
              onNavigate({ kind: "settings", section: "agents" });
            }}
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

function shouldHandleShellLink(event: React.MouseEvent<HTMLElement>) {
  return event.button === 0
    && !event.metaKey
    && !event.ctrlKey
    && !event.shiftKey
    && !event.altKey;
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
  recovery,
  onNavigate,
  children
}: {
  route: AppRoute;
  status: LocalStatusQuery["localStatus"] | null;
  socketState: SocketState;
  agentAvatarActivity?: IdentityAvatarActivity;
  providerBlocked?: boolean;
  setupBlocked?: boolean;
  recovery: PwaRuntimeSnapshot;
  onNavigate: (route: AppRoute) => Promise<void>;
  children: React.ReactNode;
}) {
  const [memoryBreadcrumb, setMemoryBreadcrumb] = React.useState<ShellMemoryBreadcrumb | null>(null);
  const shellRootRef = React.useRef<HTMLDivElement | null>(null);
  const contentDeckRef = React.useRef<HTMLElement | null>(null);
  const routeContentRef = React.useRef<HTMLElement | null>(null);
  const reduceMotion = useReducedMotion();
  const memoryTreeResult = useQuery<MemoryTreeQuery>(MemoryTreeDocument, {
    fetchPolicy: "cache-only",
    skip: route.kind !== "memory"
  });
  const memoryTree = memoryTreeResult.data?.memoryTree ?? null;
  const memoryOpen = route.kind === "memory";
  const settingsOpen = route.kind === "settings";
  const tasksOpen = route.kind === "tasks";
  const tasksProjectsResult = useTaskProjects({ skip: !tasksOpen });
  const memoryHasSidebar = memoryOpen && memoryTree?.root
    ? memoryTree.pages.some((page) => page.path !== memoryTree.root?.path)
    : false;
  const tasksMenuLevel = tasksOpen
    ? tasksMenuLevelForProjects(tasksProjectsResult.projects, route.projectId)
    : null;
  const hasShellSidebar = memoryHasSidebar || settingsOpen || tasksOpen;
  const sidebarRef = useMobileMenuRevealHeight(hasShellSidebar, shellRootRef);
  const sidebarLabel = !hasShellSidebar
    ? undefined
    : settingsOpen
      ? "Settings navigation"
      : tasksOpen
        ? "Task folders"
        : "Memory navigation";
  const menuLevel = settingsOpen
    ? shellMenuLevelForRoute(route)
    : tasksMenuLevel;
  const activeMenuEntry = menuLevel?.items.find(
    (entry) => entry.kind === "item" && entry.item.itemId === menuLevel.activeItemId
  );
  const activeMemoryPath = memoryBreadcrumb?.currentPath ?? memoryTree?.root?.path;
  const activeMemoryPage = memoryOpen && activeMemoryPath
    ? memoryTree?.pages.find((page) => page.path === activeMemoryPath)
      ?? (memoryTree?.root?.path === activeMemoryPath ? memoryTree.root : null)
    : null;
  const ActiveMenuIcon = activeMenuEntry?.kind === "item" ? activeMenuEntry.item.icon : null;
  const breadcrumb = breadcrumbForRoute(route);
  const activeLabel = route.kind === "memory" && memoryBreadcrumb
    ? ["Memory", ...memoryBreadcrumb.ancestors.map((item) => item.title), memoryBreadcrumb.current].join(" / ")
    : shellHeaderLabelForBreadcrumb(breadcrumb);
  const mobileTitle = memoryOpen
    ? memoryBreadcrumb?.current ?? memoryTree?.root?.title ?? breadcrumb.current
    : activeMenuEntry?.kind === "item"
      ? activeMenuEntry.item.label
      : breadcrumb.current;
  const mobileTitleIcon = activeMemoryPage
    ? <MemoryPageIcon icon={activeMemoryPage.icon} size={18} />
    : ActiveMenuIcon
      ? <ActiveMenuIcon aria-hidden="true" size={18} />
      : null;
  const attention = shellAttentionForState({
    route,
    status,
    socketState,
    providerBlocked,
    setupBlocked,
    recovery
  });
  const isDesktopRuntime = isTauriRuntime();
  const iosPageFade = shouldUseIosPageFade();
  const routePathname = useLocation({ select: (location) => location.pathname });
  const routeSurfaceKey = pageSurfaceKeyForPathname(routePathname);
  const previousRouteSurfaceKeyRef = React.useRef(routeSurfaceKey);
  const [primaryNavigationRoute, setOptimisticPrimaryRoute] = React.useOptimistic(
    route,
    (_currentRoute, nextRoute: AppRoute) => nextRoute
  );
  const fadeBeforeNavigation = React.useCallback<ShouldBlockFn>(async ({ current, next }) => {
    const page = routeContentRef.current;
    if (
      !iosPageFade ||
      reduceMotion ||
      pathnamesSharePageSurface(current.pathname, next.pathname) ||
      !page
    ) {
      return false;
    }

    await animate(page, { opacity: 0 }, iosPageFadeTransition);
    return false;
  }, [iosPageFade, reduceMotion]);
  useBlocker({
    shouldBlockFn: fadeBeforeNavigation,
    enableBeforeUnload: false,
    disabled: !iosPageFade
  });

  React.useLayoutEffect(() => {
    if (!iosPageFade || reduceMotion) return;

    const page = routeContentRef.current;
    if (!page) return;

    const fadeIn = animate(page, { opacity: 1 }, iosPageFadeTransition);
    return () => fadeIn.stop();
  }, [iosPageFade, reduceMotion, routeSurfaceKey]);

  React.useEffect(() => {
    document.title = `${activeLabel} · Noema`;
    if (previousRouteSurfaceKeyRef.current === routeSurfaceKey) return;
    previousRouteSurfaceKeyRef.current = routeSurfaceKey;
    const frame = window.requestAnimationFrame(() => {
      const main = routeContentRef.current;
      const heading = main?.querySelector<HTMLElement>("h1");
      if (heading && !heading.hasAttribute("tabindex")) heading.tabIndex = -1;
      (heading ?? main)?.focus({ preventScroll: true });
    });
    return () => window.cancelAnimationFrame(frame);
  }, [activeLabel, routeSurfaceKey]);

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
    const standalone = window.matchMedia("(display-mode: standalone)");
    let layoutViewportHeight = root.offsetHeight;
    const clearVisualViewport = () => {
      root.style.removeProperty("--shell-visual-viewport-height");
      root.style.removeProperty("--shell-visual-viewport-offset-top");
    };
    const syncVisualViewport = () => {
      if (!mobile.matches || viewport.scale !== 1) {
        clearVisualViewport();
        return;
      }

      const activeElement = document.activeElement;
      const editableFocused = activeElement instanceof HTMLInputElement
        || activeElement instanceof HTMLTextAreaElement
        || activeElement instanceof HTMLSelectElement
        || (activeElement instanceof HTMLElement && activeElement.isContentEditable);
      if (!editableFocused) {
        layoutViewportHeight = Math.max(root.offsetHeight, viewport.height);
      }
      const viewportInset = Math.max(
        0,
        layoutViewportHeight - viewport.height
      );
      const keyboardVisible = editableFocused && viewportInset > 150;
      if (standalone.matches) {
        root.style.setProperty(
          "--shell-visual-viewport-height",
          keyboardVisible ? `${viewport.height}px` : `${layoutViewportHeight}px`
        );
        root.style.setProperty(
          "--shell-visual-viewport-offset-top",
          keyboardVisible ? `${Math.max(0, viewport.offsetTop)}px` : "0px"
        );
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

    let delayedSync: number | null = null;
    const scheduleVisualViewportSync = () => {
      syncVisualViewport();
      if (delayedSync !== null) window.clearTimeout(delayedSync);
      delayedSync = window.setTimeout(() => {
        delayedSync = null;
        syncVisualViewport();
      }, 50);
    };

    scheduleVisualViewportSync();
    viewport.addEventListener("resize", scheduleVisualViewportSync);
    viewport.addEventListener("scroll", scheduleVisualViewportSync);
    mobile.addEventListener("change", scheduleVisualViewportSync);
    document.addEventListener("focusin", scheduleVisualViewportSync);
    document.addEventListener("focusout", scheduleVisualViewportSync);
    return () => {
      if (delayedSync !== null) window.clearTimeout(delayedSync);
      viewport.removeEventListener("resize", scheduleVisualViewportSync);
      viewport.removeEventListener("scroll", scheduleVisualViewportSync);
      mobile.removeEventListener("change", scheduleVisualViewportSync);
      document.removeEventListener("focusin", scheduleVisualViewportSync);
      document.removeEventListener("focusout", scheduleVisualViewportSync);
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

  const selectShellMenuItem = React.useCallback(
    (item: ShellMenuItem) => {
      if (!item.route) {
        return;
      }

      void onNavigate(item.route);
      if (deckNavigation.navOpen) closeNav();
    },
    [closeNav, deckNavigation.navOpen, onNavigate]
  );

  const navigatePrimary = React.useCallback((nextRoute: AppRoute) => {
    React.startTransition(async () => {
      setOptimisticPrimaryRoute(nextRoute);
      await onNavigate(nextRoute);
    });
    if (deckNavigation.navOpen) closeNav();
  }, [closeNav, deckNavigation.navOpen, onNavigate, setOptimisticPrimaryRoute]);

  const navPull = useMobileMenuPullGesture({
    closeNav,
    deckRef: contentDeckRef,
    menuEnabled: hasShellSidebar,
    adjacentRoutes: {
      previous: adjacentPrimaryRoute(route, "previous"),
      next: adjacentPrimaryRoute(route, "next")
    },
    navOpen: deckNavigation.navOpen,
    onNavigateTab: navigatePrimary,
    onNavigationSettled: settleSurfaceVisibility,
    openNav
  });
  const sidebarVisible = deckNavigation.navOpen
    || deckNavigation.surfaceVisibility !== "visible"
    || navPull.visible;
  const toggleNav = deckNavigation.navOpen ? closeNav : openNav;

  React.useEffect(() => {
    if (!hasShellSidebar && deckNavigation.navOpen) closeNav();
  }, [closeNav, deckNavigation.navOpen, hasShellSidebar]);

  return (
    <div
      ref={shellRootRef}
      data-slot="shell-root"
      data-nav-open={deckNavigation.navOpen}
      data-settings-open={settingsOpen ? "true" : undefined}
      data-tauri-runtime={isDesktopRuntime}
      style={rootStyle}
      {...stylex.props(
        styles.root,
        hasShellSidebar && styles.rootWithSidebar,
        isDesktopRuntime ? styles.desktopRoot : styles.browserRoot
      )}
    >
      <a href="#noema-main-content" {...stylex.props(styles.skipLink)}>Skip to content</a>
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
            route={primaryNavigationRoute}
            agentName={status?.primaryAgentDisplayName ?? null}
            agentAvatarActivity={agentAvatarActivity}
            attention={attention ? <ShellAttentionItem attention={attention} compact /> : null}
            onNavigate={navigatePrimary}
          />
        </div>
      </header>

      <aside
        ref={sidebarRef}
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
              {tasksMenuLevel ? (
                <TasksSidebar
                  menuLevel={tasksMenuLevel}
                  projects={tasksProjectsResult.projects}
                  onSelectItem={selectShellMenuItem}
                  onUpdated={async () => {
                    await tasksProjectsResult.refetch();
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
        ref={contentDeckRef}
        data-slot="shell-content-deck"
        data-nav-open={deckNavigation.navOpen}
        data-nav-pull-active={navPull.dragging ? "true" : undefined}
        aria-label={activeLabel}
        {...stylex.props(
          styles.contentDeck,
          hasShellSidebar ? styles.contentDeckWithSidebar : styles.contentDeckPrimary,
          deckNavigation.navOpen && styles.contentDeckNavOpen
        )}
        onTransitionEnd={(event) => {
          if (event.currentTarget === event.target && event.propertyName === "transform") {
            settleSurfaceVisibility();
          }
        }}
      >
        <AnimatePresence initial={false}>
          {hasShellSidebar ? (
            <MobileTitleNavigation
              key="shell-mobile-title"
              icon={mobileTitleIcon}
              label={mobileTitle}
              navOpen={deckNavigation.navOpen}
              triggerRef={menuButtonRef}
              onToggle={toggleNav}
            />
          ) : null}
        </AnimatePresence>

        <ShellSurfaceProvider
          value={{
            visibility: deckNavigation.surfaceVisibility,
            sidebarAvailable: hasShellSidebar,
            setMemoryBreadcrumb
          }}
        >
          <main
            ref={routeContentRef}
            id="noema-main-content"
            tabIndex={-1}
            aria-label={activeLabel}
            aria-hidden={deckNavigation.surfaceVisibility !== "visible" ? true : undefined}
            inert={deckNavigation.surfaceVisibility !== "visible"}
            data-slot="shell-route-content"
            data-shell-surface-visibility={deckNavigation.surfaceVisibility}
            {...stylex.props(
              styles.routeContent,
              deckNavigation.surfaceVisibility !== "visible" && styles.routeContentInactive
            )}
          >
            {children}
          </main>
        </ShellSurfaceProvider>
      </m.section>
    </div>
  );
}

const styles = stylex.create({
  root: {
    "--shell-page-center-offset": "0px",
    position: "relative",
    height: "100dvh",
    overflow: "hidden",
    color: "var(--foreground)"
  },
  skipLink: {
    position: "absolute",
    top: "var(--spacing-2)",
    left: "var(--spacing-2)",
    zIndex: 100,
    transform: "translateY(calc(-100% - var(--spacing-4)))",
    borderRadius: "var(--radius-element)",
    backgroundColor: "var(--background)",
    paddingBlock: "var(--spacing-2)",
    paddingInline: "var(--spacing-3)",
    color: "var(--foreground)",
    textDecoration: "none",
    ":focus-visible": {
      transform: "translateY(0)",
      outlineWidth: 2,
      outlineStyle: "solid",
      outlineColor: "var(--ring)",
      outlineOffset: 2
    }
  },
  rootWithSidebar: {
    "@media (min-width: 761px)": {
      "--shell-page-center-offset":
        "calc((var(--shell-sidebar-width) - var(--spacing-2)) / 2)"
    }
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
    },
    "@media (max-width: 760px) and (display-mode: standalone)": {
      height: "var(--shell-visual-viewport-height, 100vh)",
      top: "var(--shell-visual-viewport-offset-top, 0px)",
      transform: "none",
      transitionProperty: "none"
    }
  },
  sidebarGround: {
    position: "absolute",
    top: "calc(52px + var(--shell-safe-top))",
    bottom: 0,
    left: 0,
    zIndex: 10,
    display: "grid",
    minHeight: 0,
    width: "var(--shell-sidebar-width)",
    pointerEvents: "none",
    "@media (max-width: 760px)": {
      top: "calc(52px + var(--shell-safe-top))",
      width: "100%",
      paddingBottom: "max(var(--spacing-4), env(safe-area-inset-bottom))",
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
    top: "calc(52px + var(--shell-safe-top))",
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
    gridTemplateRows: "var(--shell-deck-header-height) minmax(0, 1fr)",
    overflow: "hidden",
    touchAction: "pan-y",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    backgroundColor: "var(--background)",
    boxShadow: "var(--shadow-shell-frame)",
    cornerShape: "var(--corner-shape-page)",
    transitionProperty: "top, left, transform, border-radius, box-shadow, grid-template-rows",
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
    top: "calc(52px + var(--shell-safe-top))",
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
    top: "calc(52px + var(--shell-safe-top))",
    right: 8,
    bottom: 8,
    left: "calc(var(--shell-sidebar-width))",
    borderRadius: "var(--radius-page)",
    "@media (max-width: 760px)": {
      "--shell-deck-header-height": "calc(var(--spacing-12) + var(--spacing-1))",
      top: "calc(52px + var(--shell-safe-top))",
      right: 0,
      bottom: 0,
      left: 0
    }
  },
  contentDeckNavOpen: {
    pointerEvents: "none",
    "@media (max-width: 760px)": {
      pointerEvents: "auto",
      borderRadius: "var(--radius-page)",
      transform:
        "translateY(min(var(--shell-mobile-nav-reveal-height, 100%), calc(100% - var(--spacing-12))))"
    }
  },
  shellNavbar: {
    position: "absolute",
    top: "var(--shell-safe-top)",
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
    backgroundColor: "color-mix(in srgb, var(--pine-100) 48%, var(--paper-200))",
    backgroundImage: "none",
    color: "var(--pine-700)",
    gap: "var(--spacing-0)",
    paddingInline: "var(--spacing-2)",
    ":hover": {
      "@media (hover: hover)": {
        backgroundColor: "color-mix(in srgb, var(--pine-100) 56%, var(--paper-200))",
        backgroundImage: "none"
      }
    },
    ":active": {
      backgroundColor: "color-mix(in srgb, var(--pine-100) 64%, var(--paper-200))",
      backgroundImage: "none"
    }
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
    gridRow: "1 / -1",
    gridColumn: 1,
    minWidth: 0,
    minHeight: 0,
    height: "100%",
    overflow: "visible"
  },
  routeContentInactive: {
    pointerEvents: "none"
  }
});
