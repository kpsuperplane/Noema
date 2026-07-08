import { createFileRoute } from "@tanstack/react-router";
import { SettingsSurface } from "@/pages/SettingsPage";

export const Route = createFileRoute("/settings/memory")({
  component: MemorySettingsRoute
});

function MemorySettingsRoute() {
  return <SettingsSurface section="memory" />;
}
