import { AgentsSettingsPane } from "@/components/settings/AgentsSettingsPane";
import { AdapterSettingsPane } from "@/components/settings/AdapterSettingsPane";
import { ClientsSettingsPane } from "@/components/settings/ClientsSettingsPane";
import { MemorySettingsPane } from "@/components/settings/MemorySettingsPane";
import { LocalModelsSettingsPane } from "@/components/settings/LocalModelsSettingsPane";
import { McpSettingsPane } from "@/components/settings/McpSettingsPane";
import { ProvidersSettingsPane } from "@/components/settings/ProvidersSettingsPane";
import { PrivacySettingsPane } from "@/components/settings/PrivacySettingsPane";
import { UsageSettingsPane } from "@/components/settings/UsageSettingsPane";
import { WebSettingsPane } from "@/components/settings/WebSettingsPane";
import { NotificationsSettingsPane } from "@/components/settings/NotificationsSettingsPane";
import { DesktopSettingsPane } from "@/components/settings/DesktopSettingsPane";
import {
  ShellPageLayout,
  ShellPageTrack,
} from "@/components/shell/ShellPageLayout";
import { ShellSectionHeader } from "@/components/shell/ShellSectionHeader";
import type { SettingsSection } from "@/app/routes";
import { StackItem } from "@astryxdesign/core/Stack";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";

type SettingsSurfaceProps = {
  section: SettingsSection;
  connectionId?: string;
  providerAccountId?: string;
  clientId?: string;
  installationId?: string;
  modelId?: string;
};

const settingsSectionCopy: Record<SettingsSection, { title: string }> = {
  agents: { title: "Agents" },
  models: { title: "Local Models" },
  memory: { title: "Memory" },
  "tools-web": { title: "Web" },
  "tools-apis": { title: "APIs" },
  "tools-mcps": { title: "MCPs" },
  "safety-privacy": { title: "Privacy" },
  "safety-usage": { title: "Execution" },
  "system-providers": { title: "Providers" },
  "system-notifications": { title: "Notifications" },
  "system-desktop": { title: "Desktop" },
  "system-clients": { title: "Clients" },
};

export function SettingsSurface({
  section,
  connectionId,
  providerAccountId,
  clientId,
  installationId,
  modelId,
}: SettingsSurfaceProps) {
  const copy = settingsSectionCopy[section];
  const isManagement =
    section === "tools-apis" ||
    section === "tools-mcps" ||
    section === "system-providers" ||
    section === "system-clients" ||
    section === "models";

  return (
    <ShellPageLayout width={isManagement ? "fluid" : "centered"}>
      <VStack
        as="section"
        data-slot="settings-surface"
        {...stylex.props(
          styles.surface,
          isManagement && styles.managementSurface,
        )}
        aria-labelledby="settings-surface-title"
      >
        {!isManagement ? (
          <ShellSectionHeader
            title={copy.title}
            titleId="settings-surface-title"
          />
        ) : null}
        {isManagement ? (
          <StackItem size="fill" {...stylex.props(styles.managementContent)}>
            <SettingsSectionPane
              section={section}
              connectionId={connectionId}
              providerAccountId={providerAccountId}
              clientId={clientId}
              installationId={installationId}
              modelId={modelId}
            />
          </StackItem>
        ) : (
          <ShellPageTrack>
            <VStack {...stylex.props(styles.content)}>
              <SettingsSectionPane
                section={section}
                connectionId={connectionId}
              />
            </VStack>
          </ShellPageTrack>
        )}
      </VStack>
    </ShellPageLayout>
  );
}

function SettingsSectionPane({
  section,
  connectionId,
  providerAccountId,
  clientId,
  installationId,
  modelId,
}: SettingsSurfaceProps) {
  switch (section) {
    case "agents":
      return <AgentsSettingsPane />;
    case "models":
      return (
        <LocalModelsSettingsPane
          installationId={installationId}
          modelId={modelId}
        />
      );
    case "memory":
      return <MemorySettingsPane />;
    case "tools-web":
      return <WebSettingsPane />;
    case "tools-apis":
      return <AdapterSettingsPane connectionId={connectionId} />;
    case "tools-mcps":
      return <McpSettingsPane connectionId={connectionId} />;
    case "safety-privacy":
      return <PrivacySettingsPane />;
    case "safety-usage":
      return <UsageSettingsPane />;
    case "system-providers":
      return <ProvidersSettingsPane providerAccountId={providerAccountId} />;
    case "system-notifications":
      return <NotificationsSettingsPane />;
    case "system-desktop":
      return <DesktopSettingsPane />;
    case "system-clients":
      return <ClientsSettingsPane clientId={clientId} />;
  }
}

const styles = stylex.create({
  surface: {
    boxSizing: "border-box",
    height: "100%",
    minHeight: 0,
    overflowY: "auto",
    overscrollBehavior: "contain",
    "@media (max-width: 760px)": {
      paddingTop: "var(--shell-deck-header-height)",
    },
  },
  managementSurface: {
    overflow: "hidden",
  },
  managementContent: {
    minHeight: 0,
    overflow: "hidden",
  },
  content: {
    paddingBlock: "var(--spacing-2) var(--spacing-4)",
    "@media (max-width: 760px)": {
      paddingBlock: "var(--spacing-2) var(--spacing-3)",
    },
  },
});
