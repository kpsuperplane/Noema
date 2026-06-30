import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { IdentityAvatar, LOCAL_AGENT_AVATAR_ID } from "../IdentityAvatar";
import type { ShellAttention } from "./AppShell";
import { ShellAttentionItem } from "./ShellAttentionItem";
import type { ShellMenuItem, ShellMenuLevel } from "./shellNavigation";

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
  const visibleAttention =
    menuLevel.supportsAttention && attention ? (
      <ShellAttentionItem attention={attention} />
    ) : (
      <div aria-hidden="true" />
    );

  return (
    <div
      data-slot="shell-sidebar-menu-level"
      data-shell-menu-level={menuLevel.levelId}
      className="grid h-full min-h-0 grid-rows-[auto_auto_minmax(0,1fr)_auto] gap-1 overflow-hidden transition-transform duration-300 ease-out motion-reduce:transition-none"
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
  bottom = false,
  onSelectItem
}: {
  item: ShellMenuItem;
  active: boolean;
  primaryAgentNamed: boolean;
  primaryAgentLabel: string;
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
