import { Bot, CheckSquare, Fingerprint, History, PlugZap, ServerCog } from "lucide-react";
import type { ComponentType } from "react";
import type { SettingsSection } from "@/routes";
import { cn } from "@/lib/utils";

type SettingsSidebarProps = {
  activeSection: SettingsSection;
  onSelectSection: (section: SettingsSection) => void;
};

export function SettingsSidebar({ activeSection, onSelectSection }: SettingsSidebarProps) {
  const sections: SettingsSidebarItem[] = [
    { section: "providers", label: "Providers", icon: ServerCog },
    { section: "agents", label: "Agents", icon: Bot },
    { section: "mcps", label: "MCPs", icon: PlugZap },
    { section: "trusted-identities", label: "Trusted identities", icon: Fingerprint },
    { section: "approvals", label: "Approvals", icon: CheckSquare },
    { section: "audit", label: "Audit", icon: History }
  ];

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
          {sections.map((item) => {
            const Icon = item.icon;
            return (
              <button
                key={item.section}
                type="button"
                aria-current={activeSection === item.section ? "page" : undefined}
                className={sectionButtonClass(activeSection === item.section)}
                onClick={() => onSelectSection(item.section)}
              >
                <Icon className="size-4 shrink-0" aria-hidden="true" />
                <span className="truncate">{item.label}</span>
              </button>
            );
          })}
        </nav>
      </div>
    </aside>
  );
}

type SettingsSidebarItem = {
  section: SettingsSection;
  label: string;
  icon: ComponentType<{ className?: string; "aria-hidden"?: "true" }>;
};

function sectionButtonClass(isActive: boolean) {
  return cn(
    "flex h-9 items-center gap-2 rounded-md px-3 text-left text-sm font-medium tracking-normal",
    isActive
      ? "bg-[color-mix(in_srgb,var(--pine-700)_10%,transparent)] text-[var(--pine-700)]"
      : "text-muted-foreground hover:bg-[color-mix(in_srgb,var(--pine-700)_7%,transparent)] hover:text-foreground"
  );
}
