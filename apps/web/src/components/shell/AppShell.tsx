import React from "react";
import { useQuery } from "@apollo/client/react";
import { Button, type ButtonProps } from "@astryxdesign/core/Button";
import { Popover, type PopoverProps } from "@astryxdesign/core/Popover";
import * as stylex from "@stylexjs/stylex";
import { ChevronDown, Settings } from "lucide-react";
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

function MemoryNavigationButton({
  breadcrumb,
  icon,
  pages,
  root
}: {
  breadcrumb: ShellMemoryBreadcrumb | null;
  icon: React.ReactNode;
  pages: MemoryTreeQuery["memoryTree"]["pages"];
  root: NonNullable<MemoryTreeQuery["memoryTree"]["root"]>;
}) {
  const [open, setOpen] = React.useState(false);
  const activePath = breadcrumb?.currentPath ?? root.path;

  return (
    <span {...stylex.props(styles.memoryPopoverScope)}>
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
          <Button
            ref={(element) => trigger.ref(element)}
            type="button"
            variant="ghost"
            size="lg"
            label={`Memory pages; current page ${breadcrumb?.current ?? "Memory"}`}
            icon={icon}
            endContent={<ChevronDown aria-hidden="true" size={14} strokeWidth={2} />}
            aria-controls={trigger["aria-controls"]}
            aria-current="page"
            aria-expanded={trigger["aria-expanded"]}
            aria-haspopup={trigger["aria-haspopup"]}
            xstyle={buttonXStyle(styles.primaryNavigationButton, styles.primaryNavigationButtonActive)}
            onClick={trigger.onClick}
          >
            <PrimaryNavigationLabel active>Memory</PrimaryNavigationLabel>
          </Button>
        )}
      </Popover>
    </span>
  );
}

function PrimaryNavigationLabel({
  active,
  children
}: {
  active: boolean;
  children: React.ReactNode;
}) {
  return (
    <m.span
      aria-hidden={!active}
      initial={false}
      animate={{
        width: active ? "auto" : 0,
        marginInlineStart: active ? "var(--spacing-2)" : "0px",
        opacity: active ? 1 : 0
      }}
      transition={springs.micro}
      {...stylex.props(styles.primaryNavigationLabel)}
    >
      {children}
    </m.span>
  );
}

function PrimarySurfaceNavigation({
  route,
  agentName,
  agentAvatarActivity,
  attention,
  memoryBreadcrumb,
  memoryTree,
  settingsButtonRef,
  onNavigate,
  onSelectSettings
}: {
  route: AppRoute;
  agentName: string | null;
  agentAvatarActivity: IdentityAvatarActivity;
  attention: React.ReactNode;
  memoryBreadcrumb: ShellMemoryBreadcrumb | null;
  memoryTree: MemoryTreeQuery["memoryTree"] | null;
  settingsButtonRef: React.Ref<HTMLButtonElement>;
  onNavigate: (route: AppRoute) => void;
  onSelectSettings: () => void;
}) {
  const settingsActive = route.kind === "settings";
  const activeItemId = route.kind === "settings" ? "settings" : activeL0ItemId(route);
  const namedAgent = agentName?.trim() || null;
  const [agentButtonHovered, setAgentButtonHovered] = React.useState(false);

  return (
    <nav aria-label="Primary" {...stylex.props(styles.primaryNavigation)}>
      {shellPrimaryItems.map((item) => {
        const Icon = item.icon;
        const active = item.itemId === activeItemId;
        const namedChat = item.itemId === "chat" && namedAgent !== null;
        const label = item.itemId === "chat" ? namedAgent ?? item.label : item.label;

        if (active && item.itemId === "memory" && memoryTree?.root) {
          return (
            <MemoryNavigationButton
              key={item.itemId}
              breadcrumb={memoryBreadcrumb}
              icon={<Icon aria-hidden="true" size={20} />}
              pages={memoryTree.pages}
              root={memoryTree.root}
            />
          );
        }

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
      <span {...stylex.props(styles.primaryNavigationEnd)}>
        {attention}
        <Button
          ref={settingsButtonRef}
          data-slot="shell-settings-button"
          type="button"
          variant="ghost"
          size="lg"
          label="Settings"
          icon={<Settings aria-hidden="true" size={20} />}
          aria-controls={settingsActive ? "noema-shell-sidebar" : undefined}
          aria-current={settingsActive ? "page" : undefined}
          xstyle={buttonXStyle(
            styles.primaryNavigationButton,
            settingsActive && styles.primaryNavigationButtonActive
          )}
          onClick={onSelectSettings}
        >
          <PrimaryNavigationLabel active={settingsActive}>Settings</PrimaryNavigationLabel>
        </Button>
      </span>
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
  const settingsOpen = route.kind === "settings";
  const menuLevel = route.kind === "settings" ? shellMenuLevelForRoute(route) : null;
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

      root.style.setProperty("--shell-visual-viewport-height", `${viewport.height}px`);
      root.style.setProperty("--shell-visual-viewport-offset-top", `${viewport.offsetTop}px`);
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
    enabled: settingsOpen,
    navOpen: deckNavigation.navOpen,
    openNav,
    closeNav,
    deckX,
    onSettled: settleSurfaceVisibility
  });

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

  const selectSettings = React.useCallback(() => {
    const narrowViewport =
      typeof window !== "undefined" && window.matchMedia("(max-width: 760px)").matches;
    if (settingsOpen) {
      if (narrowViewport) {
        if (deckNavigation.navOpen) closeNav();
        else openNav();
      }
      return;
    }

    onNavigate({ kind: "settings", section: "agents" });
    if (narrowViewport) openNav();
  }, [closeNav, deckNavigation.navOpen, onNavigate, openNav, settingsOpen]);

  React.useEffect(() => {
    if (!settingsOpen && deckNavigation.navOpen) closeNav();
  }, [closeNav, deckNavigation.navOpen, settingsOpen]);

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
      {menuLevel ? (
        <aside
          id="noema-shell-sidebar"
          data-slot="shell-sidebar-ground"
          aria-label="Settings navigation"
          {...stylex.props(styles.sidebarGround, deckNavigation.navOpen && styles.sidebarGroundOpen)}
        >
          <ShellSidebar menuLevel={menuLevel} onSelectItem={selectShellMenuItem} />
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
          settingsOpen ? styles.contentDeckSettings : styles.contentDeckPrimary,
          deckNavigation.navOpen && styles.contentDeckNavOpen
        )}
      >
        <header
          data-slot="shell-deck-header"
          data-tauri-drag-region
          {...stylex.props(
            styles.deckHeader,
            styles.deckHeaderPrimary,
            route.kind !== "chat" && styles.deckHeaderSolid
          )}
        >
          <div
            data-slot="shell-header-offset"
            {...stylex.props(styles.headerOffset, styles.headerOffsetPrimary)}
          >
            <PrimarySurfaceNavigation
              route={route}
              agentName={status?.primaryAgentDisplayName ?? null}
              agentAvatarActivity={agentAvatarActivity}
              attention={attention ? <ShellAttentionItem attention={attention} compact /> : null}
              memoryBreadcrumb={memoryBreadcrumb}
              memoryTree={memoryTree}
              settingsButtonRef={menuButtonRef}
              onNavigate={onNavigate}
              onSelectSettings={selectSettings}
            />
          </div>
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
    backgroundColor: "var(--pine-50)",
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
  contentDeckPrimary: {
    inset: 8,
    borderRadius: 18
  },
  contentDeckSettings: {
    top: 8,
    right: 8,
    bottom: 8,
    left: "calc(var(--shell-sidebar-width))",
    borderRadius: 18
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
  deckHeaderPrimary: {
    minHeight: 52,
    paddingBlock: "var(--spacing-2)",
    paddingInline: "var(--spacing-2)"
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
  primaryNavigationEnd: {
    display: "flex",
    minWidth: 0,
    marginLeft: "auto",
    alignItems: "center",
    gap: "var(--spacing-2)"
  },
  primaryNavigationButton: {
    borderRadius: 999,
    color: "var(--muted-foreground)",
    gap: 0,
    paddingInline: "var(--spacing-2)"
  },
  primaryNavigationLabel: {
    display: "block",
    minWidth: 0,
    overflow: "hidden",
    whiteSpace: "nowrap"
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
    backgroundColor: "color-mix(in srgb, var(--pine-100) 44%, transparent)",
    color: "var(--pine-700)"
  },
  memoryPopoverScope: {
    display: "contents",
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

function buttonXStyle(...xstyle: unknown[]): ButtonProps["xstyle"] {
  return xstyle as unknown as ButtonProps["xstyle"];
}
