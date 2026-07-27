import { AgentsSettingsPane } from "@/components/settings/AgentsSettingsPane";
import { AdapterSettingsPane } from "@/components/settings/AdapterSettingsPane";
import { CapabilityConnectionDetail } from "@/components/settings/CapabilityConnectionDetail";
import { MemorySettingsPane } from "@/components/settings/MemorySettingsPane";
import { LocalModelsSettingsPane } from "@/components/settings/LocalModelsSettingsPane";
import { McpSettingsPane } from "@/components/settings/McpSettingsPane";
import { ProvidersSettingsPane } from "@/components/settings/ProvidersSettingsPane";
import { PrivacySettingsPane } from "@/components/settings/PrivacySettingsPane";
import { UsageSettingsPane } from "@/components/settings/UsageSettingsPane";
import { WebSettingsPane } from "@/components/settings/WebSettingsPane";
import { ShellPageLayout, ShellPageSubtitle, ShellPageTrack } from "@/components/shell/ShellPageLayout";
import { ShellSectionHeader } from "@/components/shell/ShellSectionHeader";
import type { SettingsSection } from "@/app/routes";
import * as stylex from "@stylexjs/stylex";

type SettingsSurfaceProps = {
  section: SettingsSection;
  connectionId?: string;
};

const settingsSectionCopy: Record<SettingsSection, { title: string; description: string }> = {
  agents: {
    title: "Agents",
    description: "Review the agents currently registered in Noema and choose their runtime models."
  },
  models: {
    title: "Local Models",
    description:
      "Install and manage private local models, their llama.cpp runtime, and Noema's system model default."
  },
  memory: {
    title: "Memory",
    description: "Choose the model used for background updates to native Markdown memory."
  },
  "tools-web": {
    title: "Web",
    description: "Review first-party web search and fetch behavior."
  },
  "tools-apis": {
    title: "APIs",
    description: "Manage reviewed APIs, connected accounts, sharing, approvals, and tool behavior."
  },
  "tools-mcps": {
    title: "MCPs",
    description: "Review third-party MCP servers mediated by the Noema capability gateway."
  },
  "safety-privacy": {
    title: "Privacy",
    description: "Choose how Noema reviews actions that can write data or send it elsewhere."
  },
  "safety-usage": {
    title: "Usage",
    description: "Review runtime usage limits and model-assisted progress checks."
  },
  "system-providers": {
    title: "Providers",
    description:
      "Review the provider account Noema uses for chat. Secret credential material stays outside the UI."
  }
};

export function SettingsSurface({ section, connectionId }: SettingsSurfaceProps) {
  const copy = settingsSectionCopy[section];

  return (
    <ShellPageLayout width="standard">
      <section
        data-slot="settings-surface"
        {...stylex.props(styles.surface)}
        aria-labelledby="settings-surface-title"
      >
        <ShellSectionHeader
          navigationLabel="Settings"
          title={copy.title}
          titleId="settings-surface-title"
        />
        <ShellPageTrack>
          <div {...stylex.props(styles.content)}>
            <ShellPageSubtitle>{copy.description}</ShellPageSubtitle>
            <SettingsSectionPane section={section} connectionId={connectionId} />
          </div>
        </ShellPageTrack>
      </section>
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
      return connectionId
        ? <CapabilityConnectionDetail kind="API" connectionId={connectionId} />
        : <AdapterSettingsPane />;
    case "tools-mcps":
      return connectionId
        ? <CapabilityConnectionDetail kind="MCP" connectionId={connectionId} />
        : <McpSettingsPane />;
    case "safety-privacy":
      return <PrivacySettingsPane />;
    case "safety-usage":
      return <UsageSettingsPane />;
    case "system-providers":
      return <ProvidersSettingsPane />;
  }
}

const styles = stylex.create({
  surface: {
    height: "100%",
    minHeight: 0,
    overflowY: "auto",
    overscrollBehavior: "contain"
  },
  content: {
    display: "grid",
    paddingBlock: "var(--spacing-2) var(--spacing-4)",
    "@media (max-width: 760px)": {
      paddingBlock: "var(--spacing-2) var(--spacing-3)"
    }
  }
});
