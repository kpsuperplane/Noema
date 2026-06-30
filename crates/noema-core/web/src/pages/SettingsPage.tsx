import { X } from "lucide-react";
import { AgentsSettingsPane } from "@/components/settings/AgentsSettingsPane";
import { ProvidersSettingsPane } from "@/components/settings/ProvidersSettingsPane";
import { SettingsSidebar } from "@/components/settings/SettingsSidebar";
import { Button } from "@/components/ui/button";
import type { AppRoute, SettingsSection } from "@/routes";

type SettingsPageProps = {
  section: SettingsSection;
  onNavigate: (route: AppRoute) => void;
  onClose: () => void;
};

const settingsSectionCopy: Record<SettingsSection, { title: string; description: string }> = {
  providers: {
    title: "Providers",
    description:
      "Review the provider account Noema uses for chat. Secret credential material stays outside the UI."
  },
  agents: {
    title: "Agents",
    description: "Review the agents currently registered in Noema. This tab is read-only for now."
  }
};

export function SettingsPage({ section, onNavigate, onClose }: SettingsPageProps) {
  const copy = settingsSectionCopy[section];
  const handleSelectSection = (nextSection: SettingsSection) => {
    onNavigate({ kind: "settings", section: nextSection });
  };

  return (
    <main
      data-slot="settings-page"
      className="grid h-dvh min-h-screen grid-cols-[240px_minmax(0,1fr)] overflow-hidden bg-background text-foreground max-[760px]:grid-cols-1 max-[760px]:grid-rows-[auto_minmax(0,1fr)]"
    >
      <SettingsSidebar activeSection={section} onSelectSection={handleSelectSection} />
      <section className="grid min-h-0 grid-rows-[auto_minmax(0,1fr)] overflow-hidden">
        <header className="flex items-center justify-between gap-3 border-b border-[var(--border-subtle)] bg-white/95 px-5 py-3">
          <div className="min-w-0">
            <strong className="block truncate font-heading text-base tracking-normal">
              {copy.title}
            </strong>
          </div>
          <Button
            type="button"
            variant="ghost"
            size="icon"
            aria-label="Close settings"
            onClick={onClose}
          >
            <X className="size-4" aria-hidden="true" />
          </Button>
        </header>
        <div className="min-h-0 overflow-auto px-6 py-6 max-[760px]:px-5">
          <div className="grid max-w-3xl gap-5">
            <div className="grid gap-2">
              <p className="m-0 max-w-[620px] text-sm text-muted-foreground">
                {copy.description}
              </p>
            </div>
            {section === "agents" ? <AgentsSettingsPane /> : <ProvidersSettingsPane />}
          </div>
        </div>
      </section>
    </main>
  );
}
