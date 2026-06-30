import { X } from "lucide-react";
import { ProvidersSettingsPane } from "@/components/settings/ProvidersSettingsPane";
import { SettingsSidebar } from "@/components/settings/SettingsSidebar";
import { Button } from "@/components/ui/button";

export function SettingsPage({ onClose }: { onClose: () => void }) {
  return (
    <main
      data-slot="settings-page"
      className="grid h-dvh min-h-screen grid-cols-[240px_minmax(0,1fr)] overflow-hidden bg-background text-foreground max-[760px]:grid-cols-1 max-[760px]:grid-rows-[auto_minmax(0,1fr)]"
    >
      <SettingsSidebar />
      <section className="grid min-h-0 grid-rows-[auto_minmax(0,1fr)] overflow-hidden">
        <header className="flex items-center justify-between gap-3 border-b border-[var(--border-subtle)] bg-white/95 px-5 py-3">
          <div className="min-w-0">
            <strong className="block truncate font-heading text-base tracking-normal">
              Providers
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
                Review the provider account Noema uses for chat. Secret credential material stays
                outside the UI.
              </p>
            </div>
            <ProvidersSettingsPane />
          </div>
        </div>
      </section>
    </main>
  );
}
