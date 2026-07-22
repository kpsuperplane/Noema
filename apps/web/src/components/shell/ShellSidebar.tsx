import React from "react";
import * as stylex from "@stylexjs/stylex";
import {
  AnimatePresence,
  useIsPresent,
  useReducedMotion,
  type Variants
} from "motion/react";
import * as m from "motion/react-m";
import { springs } from "@/motion/springs";
import { IdentityAvatar, LOCAL_AGENT_AVATAR_ID } from "../IdentityAvatar";
import type { ShellAttention } from "./AppShell";
import { ShellAttentionItem } from "./ShellAttentionItem";
import type { ShellMenuItem, ShellMenuLevel, ShellMenuLevelId } from "./shellNavigation";

type ShellSidebarTransitionDirection = "forward" | "backward";

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

const shellSidebarMenuVariants: Variants = {
  initial: (direction: ShellSidebarTransitionDirection) => ({
    opacity: 0.4,
    x: direction === "forward" ? "100%" : "-22%"
  }),
  animate: { opacity: 1, x: "0%" },
  exit: (direction: ShellSidebarTransitionDirection) => ({
    opacity: 0.4,
    x: direction === "forward" ? "-22%" : "100%"
  })
};

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
  const transitionDirection = menuLevel.levelId === "settings" ? "forward" : "backward";

  return (
    <div
      data-slot="shell-sidebar-menu-viewport"
      {...stylex.props(styles.viewport)}
    >
      <AnimatePresence initial={false} custom={transitionDirection} mode="sync">
        <ShellSidebarMenuFrame
          key={menuLevel.levelId}
          menuLevel={menuLevel}
          attention={attention}
          primaryAgentNamed={primaryAgentNamed}
          primaryAgentLabel={primaryAgentLabel}
          transitionDirection={transitionDirection}
          onSelectItem={onSelectItem}
        />
      </AnimatePresence>
    </div>
  );
}

function ShellSidebarMenuFrame({
  menuLevel,
  attention,
  primaryAgentNamed,
  primaryAgentLabel,
  transitionDirection,
  onSelectItem
}: {
  menuLevel: ShellMenuLevel;
  attention: ShellAttention | null;
  primaryAgentNamed: boolean;
  primaryAgentLabel: string;
  transitionDirection: ShellSidebarTransitionDirection;
  onSelectItem: (item: ShellMenuItem) => void;
}) {
  const interactive = useIsPresent();
  const reduceMotion = useReducedMotion();

  return (
    <m.div
      data-slot="shell-sidebar-menu-level-frame"
      data-shell-menu-level={menuLevel.levelId}
      data-shell-menu-transition-direction={transitionDirection}
      aria-hidden={interactive ? undefined : "true"}
      inert={!interactive}
      custom={transitionDirection}
      variants={shellSidebarMenuVariants}
      initial={reduceMotion ? false : "initial"}
      animate="animate"
      exit="exit"
      transition={reduceMotion ? { duration: 0 } : springs.standard}
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
    </m.div>
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
        {menuLevel.items.map((entry) =>
          entry.kind === "group" ? (
            <ShellSidebarGroupLabel key={`group-${entry.label}`} label={entry.label} />
          ) : (
            <ShellSidebarNavItem
              key={entry.item.itemId}
              item={entry.item}
              active={entry.item.itemId === menuLevel.activeItemId}
              primaryAgentNamed={primaryAgentNamed}
              primaryAgentLabel={primaryAgentLabel}
              interactive={interactive}
              onSelectItem={onSelectItem}
            />
          )
        )}
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

function ShellSidebarGroupLabel({ label }: { label: string }) {
  return (
    <div
      data-slot="shell-menu-group-label"
      {...stylex.props(styles.groupLabel)}
    >
      {label}
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
      <IdentityAvatar actorId={LOCAL_AGENT_AVATAR_ID} actorType="agent" size="xs" />
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
  groupLabel: {
    paddingBlock: 10,
    paddingInline: 10,
    fontSize: 11,
    fontWeight: 600,
    lineHeight: 1.2,
    letterSpacing: 0,
    color: "var(--muted-foreground)",
    textTransform: "uppercase"
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
    gap: 10,
    width: "100%",
    minWidth: 0,
    height: 34,
    borderWidth: 0,
    borderRadius: 8,
    backgroundColor: "transparent",
    color: "color-mix(in srgb, var(--pine-700) 78%, var(--foreground))",
    paddingBlock: 0,
    paddingInline: 10,
    textAlign: "left",
    fontFamily: "inherit",
    boxSizing: "border-box",
    transitionDuration: "var(--motion-spring-micro-duration)",
    transitionProperty: "background-color, color",
    transitionTimingFunction: "var(--motion-spring-critical-easing)",
    ":hover": {
      "@media (hover: hover)": {
        backgroundColor: "color-mix(in srgb, var(--pine-100) 44%, transparent)"
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
    backgroundColor: "color-mix(in srgb, var(--pine-100) 72%, transparent)",
    color: "var(--pine-700)",
    fontWeight: 500,
    ":hover": {
      "@media (hover: hover)": {
        backgroundColor: "color-mix(in srgb, var(--pine-100) 72%, transparent)"
      }
    },
    ":active": {
      backgroundColor: "color-mix(in srgb, var(--pine-100) 72%, transparent)"
    }
  },
  menuIcon: {
    display: "grid",
    width: 18,
    height: 18,
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
    width: 18,
    height: 18,
    flexShrink: 0,
    placeItems: "center",
    overflow: "hidden"
  }
});
