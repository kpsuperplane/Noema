import { Bot, ServerCog } from "lucide-react";
import type { SettingsSection } from "@/routes";
import { cn } from "@/lib/utils";

type SettingsSidebarProps = {
  activeSection: SettingsSection;
  onSelectSection: (section: SettingsSection) => void;
};

export function SettingsSidebar({ activeSection, onSelectSection }: SettingsSidebarProps) {
  return (
    <aside
      data-slot="settings-sidebar"
      aria-label="Settings sections"
      className="grid min-h-0 border-r border-[var(--border-subtle)] bg-[var(--pine-50)] px-4 py-5 max-[760px]:border-r-0 max-[760px]:border-b"
    >
      <div className="grid content-start gap-4">
        <div>
          <h1 className="m-0 font-heading text-2xl leading-tight tracking-normal text-foreground">
            Settings
          </h1>
        </div>
        <nav aria-label="Settings" className="grid gap-1">
          <button
            type="button"
            aria-current={activeSection === "providers" ? "page" : undefined}
            className={sectionButtonClass(activeSection === "providers")}
            onClick={() => onSelectSection("providers")}
          >
            <ServerCog className="size-4" aria-hidden="true" />
            Providers
          </button>
          <button
            type="button"
            aria-current={activeSection === "agents" ? "page" : undefined}
            className={sectionButtonClass(activeSection === "agents")}
            onClick={() => onSelectSection("agents")}
          >
            <Bot className="size-4" aria-hidden="true" />
            Agents
          </button>
        </nav>
      </div>
    </aside>
  );
}

function sectionButtonClass(isActive: boolean) {
  return cn(
    "flex h-9 items-center gap-2 rounded-md px-3 text-left text-sm font-medium tracking-normal",
    isActive
      ? "bg-[color-mix(in_srgb,var(--pine-700)_10%,transparent)] text-[var(--pine-700)]"
      : "text-muted-foreground hover:bg-[color-mix(in_srgb,var(--pine-700)_7%,transparent)] hover:text-foreground"
  );
}
