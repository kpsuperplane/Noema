import React from "react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
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
      className="relative h-full min-h-0 overflow-hidden"
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
      className={cn(
        "absolute inset-0 grid h-full min-h-0 grid-rows-[auto_auto_minmax(0,1fr)_auto] gap-1 px-3.5 py-4",
        !interactive && "pointer-events-none"
      )}
    >
      <div className="h-8" data-tauri-drag-region />
      {visibleAttention}

      <nav className="grid content-start gap-1" aria-label={menuLevel.ariaLabel}>
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

      <div className="flex items-end">
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
      className={cn(
        "w-full justify-start rounded-md px-2.5 text-sm text-[var(--pine-700)] !bg-transparent hover:!bg-[color-mix(in_srgb,var(--pine-700)_10%,transparent)] aria-expanded:!bg-[color-mix(in_srgb,var(--pine-700)_10%,transparent)]",
        active && "!bg-[color-mix(in_srgb,var(--pine-700)_10%,transparent)]"
      )}
      aria-current={active ? "page" : undefined}
      disabled={!interactive}
      onClick={() => onSelectItem(item)}
    >
      {showPrimaryAgentAvatar ? (
        <span
          data-slot="shell-primary-agent-avatar"
          aria-hidden="true"
          className="grid size-4 shrink-0 place-items-center [&_[data-slot=avatar]]:!size-4"
        >
          <IdentityAvatar
            actorId={LOCAL_AGENT_AVATAR_ID}
            actorType="agent"
            className="!size-4"
            size="sm"
          />
        </span>
      ) : (
        <Icon className="size-4" aria-hidden />
      )}
      <span className="truncate">{label}</span>
    </Button>
  );
}
