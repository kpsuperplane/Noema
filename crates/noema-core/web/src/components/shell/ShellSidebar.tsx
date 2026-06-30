import { Brain, House } from "lucide-react";
import type { ComponentType } from "react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { AppRoute } from "@/routes";
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
  onNavigate
}: {
  activeDestination: ShellDestination;
  attention: ShellAttention | null;
  navItems: ShellNavItem[];
  onNavigate: (route: AppRoute) => void;
}) {
  return (
    <div className="grid h-full min-h-0 grid-rows-[auto_auto_minmax(0,1fr)] gap-1">
      <div className="h-8" data-tauri-drag-region />
      {attention ? <ShellAttentionItem attention={attention} /> : <div aria-hidden="true" />}

      <nav className="grid content-start gap-1" aria-label="Primary">
        {navItems.map((item) => {
          const Icon = navIcon[item.destination];
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
              <Icon className="size-4" aria-hidden />
              {item.label}
            </Button>
          );
        })}
      </nav>
    </div>
  );
}
