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
import { ShellPageLayout, ShellPageTrack } from "@/components/shell/ShellPageLayout";
import { ShellSectionHeader } from "@/components/shell/ShellSectionHeader";
import type { SettingsSection } from "@/app/routes";
import { StackItem } from "@astryxdesign/core/Stack";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";

type SettingsSurfaceProps = {
  section: SettingsSection;
  connectionId?: string;
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
  "system-clients": { title: "Clients" }
};

export function SettingsSurface({ section, connectionId }: SettingsSurfaceProps) {
  const copy = settingsSectionCopy[section];
  const isIntegrationManagement = section === "tools-apis" || section === "tools-mcps";

  return (
    <ShellPageLayout width={isIntegrationManagement ? "fluid" : "centered"}>
      <VStack
        as="section"
        data-slot="settings-surface"
        {...stylex.props(styles.surface, isIntegrationManagement && styles.integrationSurface)}
        aria-labelledby="settings-surface-title"
      >
        <ShellSectionHeader
          title={copy.title}
          titleId="settings-surface-title"
        />
        {isIntegrationManagement ? (
          <StackItem size="fill" {...stylex.props(styles.integrationContent)}>
            <SettingsSectionPane section={section} connectionId={connectionId} />
          </StackItem>
        ) : (
          <ShellPageTrack>
            <VStack {...stylex.props(styles.content)}>
              <SettingsSectionPane section={section} connectionId={connectionId} />
            </VStack>
          </ShellPageTrack>
        )}
      </VStack>
    </ShellPageLayout>
  );
}

function SettingsSectionPane({ section, connectionId }: SettingsSurfaceProps) {
  switch (section) {
    case "agents":
      return <AgentsSettingsPane />;
    case "models":
      return <LocalModelsSettingsPane />;
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
      return <ProvidersSettingsPane />;
    case "system-clients":
      return <ClientsSettingsPane />;
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
      paddingTop: "var(--shell-deck-header-height)"
    }
  },
  integrationSurface: {
    overflow: "hidden"
  },
  integrationContent: {
    minHeight: 0,
    overflow: "hidden"
  },
  content: {
    paddingBlock: "var(--spacing-2) var(--spacing-4)",
    "@media (max-width: 760px)": {
      paddingBlock: "var(--spacing-2) var(--spacing-3)"
    }
  }
});
