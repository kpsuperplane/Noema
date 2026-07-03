import { AgentsSettingsPane } from "@/components/settings/AgentsSettingsPane";
import { ApprovalsSettingsPane } from "@/components/settings/ApprovalsSettingsPane";
import { McpSettingsPane } from "@/components/settings/McpSettingsPane";
import { ProvidersSettingsPane } from "@/components/settings/ProvidersSettingsPane";
import { TrustedIdentitiesSettingsPane } from "@/components/settings/TrustedIdentitiesSettingsPane";
import type { SettingsSection } from "@/app/routes";
import * as stylex from "@stylexjs/stylex";

type SettingsSurfaceProps = {
  section: SettingsSection;
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
  },
  mcps: {
    title: "MCPs",
    description: "Review third-party MCP servers mediated by the Noema capability gateway."
  },
  "trusted-identities": {
    title: "Trusted identities",
    description: "Review identity selectors used to resolve tool-result ownership."
  },
  approvals: {
    title: "Approvals",
    description: "Review pending MCP approval checkpoints."
  }
};

export function SettingsSurface({ section }: SettingsSurfaceProps) {
  const copy = settingsSectionCopy[section];

  return (
    <section
      data-slot="settings-surface"
      {...stylex.props(styles.surface)}
      aria-labelledby="settings-surface-title"
    >
      <div {...stylex.props(styles.content)}>
        <div {...stylex.props(styles.header)}>
          <h1 id="settings-surface-title" {...stylex.props(styles.title)}>
            {copy.title}
          </h1>
          <p {...stylex.props(styles.description)}>
            {copy.description}
          </p>
        </div>
        <SettingsSectionPane section={section} />
      </div>
    </section>
  );
}

function SettingsSectionPane({ section }: { section: SettingsSection }) {
  switch (section) {
    case "agents":
      return <AgentsSettingsPane />;
    case "mcps":
      return <McpSettingsPane />;
    case "trusted-identities":
      return <TrustedIdentitiesSettingsPane />;
    case "approvals":
      return <ApprovalsSettingsPane />;
    case "providers":
      return <ProvidersSettingsPane />;
  }
}

const styles = stylex.create({
  surface: {
    height: "100%",
    minHeight: 0,
    overflowY: "auto",
    overscrollBehavior: "contain",
    padding: "24px 24px",
    "@media (max-width: 760px)": {
      paddingInline: 20
    }
  },
  content: {
    display: "grid",
    maxWidth: 768,
    gap: 20
  },
  header: {
    display: "grid",
    gap: 8
  },
  title: {
    margin: 0,
    fontFamily: "var(--font-heading)",
    fontSize: 24,
    lineHeight: 1.25,
    letterSpacing: 0,
    color: "var(--foreground)"
  },
  description: {
    margin: 0,
    maxWidth: 620,
    fontSize: 14,
    lineHeight: 1.5,
    color: "var(--muted-foreground)"
  }
});
