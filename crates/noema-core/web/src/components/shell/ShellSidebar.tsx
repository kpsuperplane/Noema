import React from "react";
import { Button } from "@astryxdesign/core/Button";
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
  const visibleAttention =
    menuLevel.supportsAttention && attention ? (
      <ShellAttentionItem attention={attention} />
    ) : (
      <div aria-hidden="true" />
    );

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
      <div {...stylex.props(styles.dragRegionSpacer)} data-tauri-drag-region />
      {visibleAttention}

      <nav {...stylex.props(styles.nav)} aria-label={menuLevel.ariaLabel}>
        {menuLevel.items.map((item) => (
          <ShellMenuButton
            key={item.itemId}
            item={item}
            active={item.itemId === menuLevel.activeItemId}
            primaryAgentNamed={primaryAgentNamed}
            primaryAgentLabel={primaryAgentLabel}
            interactive={interactive}
            onSelectItem={onSelectItem}
          />
        ))}
      </nav>

      <div {...stylex.props(styles.bottomSlot)}>
        <ShellMenuButton
          item={menuLevel.bottomItem}
          active={menuLevel.bottomItem.itemId === menuLevel.activeItemId}
          primaryAgentNamed={false}
          primaryAgentLabel={primaryAgentLabel}
          interactive={interactive}
          bottom
          onSelectItem={onSelectItem}
        />
      </div>
    </div>
  );
}

function ShellMenuButton({
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
    <Button
      data-slot={bottom ? "shell-menu-bottom-item" : "shell-menu-item"}
      data-shell-menu-item={item.itemId}
      type="button"
      variant="ghost"
      size="md"
      label={label}
      icon={showPrimaryAgentAvatar ? <PrimaryAgentMenuAvatar /> : <Icon aria-hidden size={16} />}
      {...stylex.props(styles.menuButton, active && styles.menuButtonActive)}
      aria-current={active ? "page" : undefined}
      isDisabled={!interactive}
      onClick={() => onSelectItem(item)}
    >
      <span {...stylex.props(styles.menuButtonLabel)}>{label}</span>
    </Button>
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
    gridTemplateRows: "auto auto minmax(0, 1fr) auto",
    gap: 4,
    paddingBlock: 16,
    paddingInline: 14
  },
  menuFrameNonInteractive: {
    pointerEvents: "none"
  },
  dragRegionSpacer: {
    height: 32
  },
  nav: {
    display: "grid",
    alignContent: "start",
    gap: 4
  },
  bottomSlot: {
    display: "flex",
    alignItems: "flex-end"
  },
  menuButton: {
    width: "100%",
    justifyContent: "flex-start",
    borderRadius: 6,
    paddingInline: 10,
    backgroundColor: "transparent",
    color: "var(--pine-700)",
    fontSize: 14,
    ":hover": {
      backgroundColor: "color-mix(in srgb, var(--pine-700) 10%, transparent)"
    }
  },
  menuButtonActive: {
    backgroundColor: "color-mix(in srgb, var(--pine-700) 10%, transparent)"
  },
  menuButtonLabel: {
    minWidth: 0,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap"
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
