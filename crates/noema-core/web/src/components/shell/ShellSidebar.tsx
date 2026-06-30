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
    <div className="grid h-full min-h-0 grid-rows-[auto_auto_minmax(0,1fr)] gap-4">
      <div className="flex min-w-0 items-center gap-2.5 px-2">
        <img src="/assets/noema-mark.svg" width="32" height="32" alt="" />
        <div className="min-w-0">
          <strong className="block truncate font-heading text-base tracking-normal">Noema</strong>
          <span className="block truncate text-xs text-muted-foreground">Local agent OS</span>
        </div>
      </div>

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
                "h-9 justify-start rounded-md px-2.5 text-sm text-muted-foreground",
                active &&
                  "bg-[var(--pine-50)] text-[var(--pine-700)] hover:bg-[var(--pine-50)] hover:text-[var(--pine-700)]"
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
