import React from "react";
import * as stylex from "@stylexjs/stylex";
import { IdentityAvatar, LOCAL_AGENT_AVATAR_ID } from "../IdentityAvatar";
import type { ShellAttention } from "./AppShell";
import { ShellAttentionItem } from "./ShellAttentionItem";
import type { ShellMenuItem, ShellMenuLevel, ShellMenuLevelId } from "./shellNavigation";

const shellSidebarMenuTransitionMs = 300;

type ShellSidebarTransitionDirection = "forward" | "backward";
type ShellSidebarMenuFrameState = "current" | "entering" | "exiting";

const shellSidebarMenuLevelOrder: Record<ShellMenuLevelId, number> = {
  l0: 0,
  settings: 1
};

export function shellSidebarTransitionDirection(
  fromLevelId: ShellMenuLevelId,
  toLevelId: ShellMenuLevelId
): ShellSidebarTransitionDirection {
  return shellSidebarMenuLevelOrder[toLevelId] >= shellSidebarMenuLevelOrder[fromLevelId]
    ? "forward"
    : "backward";
}

export function ShellSidebar({
  menuLevel,
  attention,
  primaryAgentNamed,
  primaryAgentLabel,
  onSelectItem
}: {
  menuLevel: ShellMenuLevel;
  attention: ShellAttention | null;
  primaryAgentNamed: boolean;
  primaryAgentLabel: string;
  onSelectItem: (item: ShellMenuItem) => void;
}) {
  const [settledMenuLevel, setSettledMenuLevel] = React.useState(menuLevel);

  const transitioning = settledMenuLevel.levelId !== menuLevel.levelId;
  const transitionDirection = transitioning
    ? shellSidebarTransitionDirection(settledMenuLevel.levelId, menuLevel.levelId)
    : "forward";

  React.useEffect(() => {
    if (!transitioning) {
      return;
    }

    const timeoutId = window.setTimeout(() => {
      setSettledMenuLevel(menuLevel);
    }, shellSidebarMenuTransitionMs);

    return () => window.clearTimeout(timeoutId);
  }, [transitioning, menuLevel]);

  const frames = transitioning
    ? [
        {
          menuLevel: settledMenuLevel,
          frameState: "exiting" as const,
          interactive: false
        },
        {
          menuLevel,
          frameState: "entering" as const,
          interactive: true
        }
      ]
    : [
        {
          menuLevel,
          frameState: "current" as const,
          interactive: true
        }
      ];

  return (
    <div
      data-slot="shell-sidebar-menu-viewport"
      {...stylex.props(styles.viewport)}
    >
      {frames.map((frame) => (
        <ShellSidebarMenuFrame
          key={`${frame.menuLevel.levelId}-${frame.frameState}`}
          menuLevel={frame.menuLevel}
          attention={attention}
          primaryAgentNamed={primaryAgentNamed}
          primaryAgentLabel={primaryAgentLabel}
          frameState={frame.frameState}
          transitionDirection={transitionDirection}
          interactive={frame.interactive}
          onSelectItem={onSelectItem}
        />
      ))}
    </div>
  );
}

function ShellSidebarMenuFrame({
  menuLevel,
  attention,
  primaryAgentNamed,
  primaryAgentLabel,
  frameState,
  transitionDirection,
  interactive,
  onSelectItem
}: {
  menuLevel: ShellMenuLevel;
  attention: ShellAttention | null;
  primaryAgentNamed: boolean;
  primaryAgentLabel: string;
  frameState: ShellSidebarMenuFrameState;
  transitionDirection: ShellSidebarTransitionDirection;
  interactive: boolean;
  onSelectItem: (item: ShellMenuItem) => void;
}) {
  return (
    <div
      data-slot="shell-sidebar-menu-level-frame"
      data-shell-menu-level={menuLevel.levelId}
      data-shell-menu-frame-state={frameState}
      data-shell-menu-transition-direction={transitionDirection}
      aria-hidden={interactive ? undefined : "true"}
      {...stylex.props(
        styles.menuFrame,
        !interactive && styles.menuFrameNonInteractive
      )}
    >
      <ShellSidebarNav
        menuLevel={menuLevel}
        attention={attention}
        primaryAgentNamed={primaryAgentNamed}
        primaryAgentLabel={primaryAgentLabel}
        interactive={interactive}
        onSelectItem={onSelectItem}
      />
    </div>
  );
}

function ShellSidebarNav({
  menuLevel,
  attention,
  primaryAgentNamed,
  primaryAgentLabel,
  interactive,
  onSelectItem
}: {
  menuLevel: ShellMenuLevel;
  attention: ShellAttention | null;
  primaryAgentNamed: boolean;
  primaryAgentLabel: string;
  interactive: boolean;
  onSelectItem: (item: ShellMenuItem) => void;
}) {
  const visibleAttention =
    menuLevel.supportsAttention && attention ? (
      <ShellAttentionItem attention={attention} />
    ) : (
      <div aria-hidden="true" />
    );

  return (
    <nav
      aria-label={menuLevel.ariaLabel}
      data-slot="shell-sidebar-nav"
      {...stylex.props(styles.nav)}
    >
      <ShellSidebarHeader attention={visibleAttention} />
      <div {...stylex.props(styles.sideNavBody)}>
        {menuLevel.items.map((item) => (
          <ShellSidebarNavItem
            key={item.itemId}
            item={item}
            active={item.itemId === menuLevel.activeItemId}
            primaryAgentNamed={primaryAgentNamed}
            primaryAgentLabel={primaryAgentLabel}
            interactive={interactive}
            onSelectItem={onSelectItem}
          />
        ))}
      </div>
      <div data-slot="shell-sidebar-footer" {...stylex.props(styles.footer)}>
        <ShellSidebarNavItem
          item={menuLevel.bottomItem}
          active={menuLevel.bottomItem.itemId === menuLevel.activeItemId}
          primaryAgentNamed={false}
          primaryAgentLabel={primaryAgentLabel}
          interactive={interactive}
          bottom
          onSelectItem={onSelectItem}
        />
      </div>
    </nav>
  );
}

function ShellSidebarHeader({ attention }: { attention: React.ReactNode }) {
  return (
    <div {...stylex.props(styles.header)}>
      <div {...stylex.props(styles.dragRegionSpacer)} data-tauri-drag-region />
      {attention}
    </div>
  );
}

function ShellSidebarNavItem({
  item,
  active,
  primaryAgentNamed,
  primaryAgentLabel,
  interactive,
  bottom = false,
  onSelectItem
}: {
  item: ShellMenuItem;
  active: boolean;
  primaryAgentNamed: boolean;
  primaryAgentLabel: string;
  interactive: boolean;
  bottom?: boolean;
  onSelectItem: (item: ShellMenuItem) => void;
}) {
  const Icon = item.icon;
  const showPrimaryAgentAvatar = item.itemId === "home" && primaryAgentNamed;
  const label = item.itemId === "home" && primaryAgentNamed ? primaryAgentLabel : item.label;

  return (
    <div
      data-slot={bottom ? "shell-menu-bottom-item" : "shell-menu-item"}
      data-shell-menu-item={item.itemId}
      data-current={active ? "true" : undefined}
      {...stylex.props(styles.menuItemFrame)}
    >
      <button
        type="button"
        aria-current={active ? "page" : undefined}
        disabled={!interactive}
        {...stylex.props(styles.menuButton, active && styles.menuButtonActive)}
        onClick={() => onSelectItem(item)}
      >
        <span {...stylex.props(styles.menuIcon)} aria-hidden="true">
          {showPrimaryAgentAvatar ? <PrimaryAgentMenuAvatar /> : <Icon size={16} />}
        </span>
        <span {...stylex.props(styles.menuLabel)}>{label}</span>
      </button>
    </div>
  );
}

function PrimaryAgentMenuAvatar() {
  return (
    <span
      data-slot="shell-primary-agent-avatar"
      aria-hidden="true"
      {...stylex.props(styles.primaryAgentAvatarFrame)}
    >
      <span {...stylex.props(styles.primaryAgentAvatarScale)}>
        <IdentityAvatar actorId={LOCAL_AGENT_AVATAR_ID} actorType="agent" size="sm" />
      </span>
    </span>
  );
}

const styles = stylex.create({
  viewport: {
    position: "relative",
    height: "100%",
    minHeight: 0,
    overflow: "hidden"
  },
  menuFrame: {
    position: "absolute",
    inset: 0,
    display: "grid",
    height: "100%",
    minHeight: 0,
    paddingBlock: 0,
    paddingInline: 0
  },
  menuFrameNonInteractive: {
    pointerEvents: "none"
  },
  nav: {
    display: "grid",
    gridTemplateRows: "auto minmax(0, 1fr) auto",
    gap: 8,
    height: "100%",
    minHeight: 0,
    width: "100%",
    minWidth: 0,
    paddingBlock: 12,
    paddingInline: 10,
    color: "var(--foreground)"
  },
  header: {
    display: "grid",
    gap: 4,
    paddingInline: 2
  },
  dragRegionSpacer: {
    height: 32
  },
  sideNavBody: {
    display: "flex",
    minHeight: 0,
    flexDirection: "column",
    gap: 4,
    overflowY: "auto",
    overscrollBehavior: "contain",
    paddingBlock: 2,
    scrollbarWidth: "none"
  },
  footer: {
    display: "grid",
    gap: 4,
    paddingTop: 6
  },
  menuItemFrame: {
    width: "100%",
    minWidth: 0,
    borderRadius: 8
  },
  menuButton: {
    display: "flex",
    alignItems: "center",
    gap: 8,
    width: "100%",
    minWidth: 0,
    height: 28,
    borderWidth: 0,
    borderRadius: 8,
    backgroundColor: "transparent",
    color: "var(--foreground)",
    paddingBlock: 0,
    paddingInline: 4,
    textAlign: "left",
    fontFamily: "inherit",
    boxSizing: "border-box",
    transitionDuration: "120ms",
    transitionProperty: "background-color, color",
    transitionTimingFunction: "ease",
    ":hover": {
      "@media (hover: hover)": {
        backgroundColor: "var(--surface-hover)"
      }
    },
    ":focus-visible": {
      outlineWidth: 2,
      outlineStyle: "solid",
      outlineColor: "var(--ring)",
      outlineOffset: 2
    },
    ":disabled": {
      opacity: 0.55
    }
  },
  menuButtonActive: {
    backgroundColor: "var(--color-neutral)",
    fontWeight: 500,
    ":hover": {
      "@media (hover: hover)": {
        backgroundColor: "var(--color-neutral)"
      }
    },
    ":active": {
      backgroundColor: "var(--color-neutral)"
    }
  },
  menuIcon: {
    display: "grid",
    width: 16,
    height: 16,
    flexShrink: 0,
    placeItems: "center"
  },
  menuLabel: {
    minWidth: 0,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    fontSize: 14,
    fontWeight: "inherit",
    lineHeight: "20px"
  },
  primaryAgentAvatarFrame: {
    display: "grid",
    width: 16,
    height: 16,
    flexShrink: 0,
    placeItems: "center",
    overflow: "hidden"
  },
  primaryAgentAvatarScale: {
    display: "grid",
    width: 28,
    height: 28,
    placeItems: "center",
    transform: "scale(0.5715)"
  }
});
