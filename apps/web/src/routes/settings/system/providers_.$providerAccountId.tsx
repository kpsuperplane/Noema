import { createFileRoute } from "@tanstack/react-router";
import { SettingsSurface } from "@/pages/SettingsPage";

export const Route = createFileRoute("/settings/system/providers_/$providerAccountId")({
  component: ProviderAccountSettingsRoute
});

function ProviderAccountSettingsRoute() {
  const { providerAccountId } = Route.useParams();
  return <SettingsSurface section="system-providers" providerAccountId={providerAccountId} />;
}
