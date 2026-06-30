import { ServerCog } from "lucide-react";

export function SettingsSidebar() {
  return (
    <aside
      data-slot="settings-sidebar"
      aria-label="Settings sections"
      className="grid min-h-0 border-r border-[var(--border-subtle)] bg-[var(--pine-50)] px-4 py-5 max-[760px]:border-r-0 max-[760px]:border-b"
    >
      <div className="grid content-start gap-4">
        <div>
          <p className="m-0 font-mono text-[11px] tracking-[0.12em] text-[var(--text-accent)] uppercase">
            Settings
          </p>
          <h1 className="m-0 font-heading text-2xl leading-tight tracking-normal text-foreground">
            Settings
          </h1>
        </div>
        <nav aria-label="Settings" className="grid gap-1">
          <button
            type="button"
            aria-current="page"
            className="flex items-center gap-2 rounded-md bg-[color-mix(in_srgb,var(--pine-700)_10%,transparent)] px-2.5 py-2 text-left text-sm font-medium text-[var(--pine-700)]"
          >
            <ServerCog className="size-4" aria-hidden="true" />
            Providers
          </button>
        </nav>
      </div>
    </aside>
  );
}
