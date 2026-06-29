import type { ReactNode } from "react";

export function SetupFrame({ children }: { children: ReactNode }) {
  return (
    <main className="grid h-dvh min-h-screen grid-rows-[64px_minmax(0,1fr)] overflow-hidden bg-background">
      <div className="flex min-w-0 items-center gap-2.5 border-b border-[var(--border-subtle)] bg-white/95 px-5">
        <img src="/assets/noema-mark.svg" width="32" height="32" alt="" />
        <div className="min-w-0">
          <strong className="block truncate font-heading text-base tracking-normal">Noema</strong>
          <span className="block truncate text-xs text-muted-foreground">Local setup</span>
        </div>
      </div>
      <div className="min-h-0 overflow-auto">{children}</div>
    </main>
  );
}
