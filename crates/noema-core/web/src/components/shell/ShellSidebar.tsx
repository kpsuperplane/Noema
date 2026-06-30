import { Brain, House, Settings } from "lucide-react";
import type { ComponentType } from "react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { AppRoute } from "@/routes";
import { IdentityAvatar, LOCAL_AGENT_AVATAR_ID } from "../IdentityAvatar";
import type { ShellAttention, ShellDestination, ShellNavItem } from "./AppShell";
import { ShellAttentionItem } from "./ShellAttentionItem";

const navIcon: Record<
  ShellDestination,
  ComponentType<{ className?: string; "aria-hidden"?: true }>
> = {
  home: House,
  memory: Brain
};

export function ShellSidebar({
  activeDestination,
  attention,
  navItems,
  primaryAgentNamed,
  onNavigate
}: {
  activeDestination: ShellDestination;
  attention: ShellAttention | null;
  navItems: ShellNavItem[];
  primaryAgentNamed: boolean;
  onNavigate: (route: AppRoute) => void;
}) {
  return (
    <div className="grid h-full min-h-0 grid-rows-[auto_auto_minmax(0,1fr)_auto] gap-1">
      <div className="h-8" data-tauri-drag-region />
      {attention ? <ShellAttentionItem attention={attention} /> : <div aria-hidden="true" />}

      <nav className="grid content-start gap-1" aria-label="Primary">
        {navItems.map((item) => {
          const Icon = navIcon[item.destination];
          const showPrimaryAgentAvatar = item.destination === "home" && primaryAgentNamed;
          const active = item.destination === activeDestination;
          return (
            <Button
              key={item.destination}
              type="button"
              variant="ghost"
              className={cn(
                "justify-start rounded-md px-2.5 text-sm text-[var(--pine-700)] !bg-transparent hover:!bg-[color-mix(in_srgb,var(--pine-700)_10%,transparent)] aria-expanded:!bg-[color-mix(in_srgb,var(--pine-700)_10%,transparent)]",
                active &&
                  "!bg-[color-mix(in_srgb,var(--pine-700)_10%,transparent)]"
              )}
              aria-current={active ? "page" : undefined}
              onClick={() => onNavigate(item.route)}
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
              {item.label}
            </Button>
          );
        })}
      </nav>

      <div className="flex items-end">
        <Button
          data-slot="shell-settings-button"
          type="button"
          variant="ghost"
          size="icon"
          aria-label="Open settings"
          className="rounded-md text-[var(--pine-700)] !bg-transparent hover:!bg-[color-mix(in_srgb,var(--pine-700)_10%,transparent)]"
          onClick={() => onNavigate({ kind: "settings", section: "providers" })}
        >
          <Settings className="size-4" aria-hidden="true" />
        </Button>
      </div>
    </div>
  );
}
