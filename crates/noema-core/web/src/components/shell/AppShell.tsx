import React from "react";
import { Menu } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
  SheetTrigger
} from "@/components/ui/sheet";
import type { LocalStatusQuery } from "@/generated/graphql";
import type { AppRoute } from "@/routes";
import type { SocketState } from "@/types";
import { ShellSidebar } from "./ShellSidebar";

export type ShellDestination = "home" | "memory";

export type ShellNavItem = {
  destination: ShellDestination;
  label: string;
  route: AppRoute;
};

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

export const shellNavItems: ShellNavItem[] = [
  { destination: "home", label: "Home", route: { kind: "chat" } },
  { destination: "memory", label: "Memory", route: { kind: "memory_home" } }
];

export function activeShellDestination(route: AppRoute): ShellDestination {
  if (route.kind === "memory_home" || route.kind === "memory_graph") {
    return "memory";
  }
  return "home";
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

  if (
    (input.route.kind === "memory_home" || input.route.kind === "memory_graph") &&
    input.status?.memoryStorage === "UNAVAILABLE"
  ) {
    return {
      tone: "warning",
      title: "Memory unavailable",
      message: "Memory storage is not ready for this page."
    };
  }

  return null;
}

export function AppShell({
  route,
  status,
  socketState,
  providerBlocked = false,
  setupBlocked = false,
  onNavigate,
  children
}: {
  route: AppRoute;
  status: LocalStatusQuery["localStatus"] | null;
  socketState: SocketState;
  providerBlocked?: boolean;
  setupBlocked?: boolean;
  onNavigate: (route: AppRoute) => void;
  children: React.ReactNode;
}) {
  const [drawerOpen, setDrawerOpen] = React.useState(false);
  const activeDestination = activeShellDestination(route);
  const activeLabel =
    shellNavItems.find((item) => item.destination === activeDestination)?.label ?? "Home";
  const attention = shellAttentionForState({
    route,
    status,
    socketState,
    providerBlocked,
    setupBlocked
  });

  const navigateFromShell = React.useCallback(
    (nextRoute: AppRoute) => {
      onNavigate(nextRoute);
      setDrawerOpen(false);
    },
    [onNavigate]
  );

  return (
    <main className="grid h-dvh min-h-screen grid-cols-[236px_minmax(0,1fr)] overflow-hidden bg-background max-[760px]:grid-cols-1 max-[760px]:grid-rows-[auto_minmax(0,1fr)]">
      <aside className="grid min-h-0 border-r border-[var(--border-subtle)] px-3.5 py-4 max-[760px]:hidden">
        <ShellSidebar
          activeDestination={activeDestination}
          attention={attention}
          navItems={shellNavItems}
          onNavigate={navigateFromShell}
        />
      </aside>

      <div className="hidden min-h-[56px] items-center gap-3 border-b border-[var(--border-subtle)] bg-white/95 px-4 max-[760px]:flex">
        <Sheet open={drawerOpen} onOpenChange={setDrawerOpen}>
          <SheetTrigger
            render={
              <Button type="button" variant="ghost" size="icon" aria-label="Open navigation" />
            }
          >
            <Menu aria-hidden="true" />
          </SheetTrigger>
          <SheetContent
            side="left"
            className="w-[min(320px,86vw)] p-0"
            showCloseButton={false}
          >
            <SheetHeader className="sr-only">
              <SheetTitle>Noema navigation</SheetTitle>
              <SheetDescription>Choose Home or Memory.</SheetDescription>
            </SheetHeader>
            <div className="grid h-full p-3.5">
              <ShellSidebar
                activeDestination={activeDestination}
                attention={attention}
                navItems={shellNavItems}
                onNavigate={navigateFromShell}
              />
            </div>
          </SheetContent>
        </Sheet>
        <div className="min-w-0">
          <strong className="block truncate font-heading text-base tracking-normal">{activeLabel}</strong>
          <span className="block truncate text-xs text-muted-foreground">
            {activeDestination === "home" ? "Primary conversation" : "Memory management"}
          </span>
        </div>
      </div>

      <div className="min-h-0 overflow-hidden">{children}</div>
    </main>
  );
}
