import { AgentsSettingsPane } from "@/components/settings/AgentsSettingsPane";
import { ApprovalsSettingsPane } from "@/components/settings/ApprovalsSettingsPane";
import { AuditSettingsPane } from "@/components/settings/AuditSettingsPane";
import { McpSettingsPane } from "@/components/settings/McpSettingsPane";
import { ProvidersSettingsPane } from "@/components/settings/ProvidersSettingsPane";
import { TrustedIdentitiesSettingsPane } from "@/components/settings/TrustedIdentitiesSettingsPane";
import type { SettingsSection } from "@/routes";

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
  },
  audit: {
    title: "Audit",
    description: "Review mediated MCP activity records."
  }
};

export function SettingsSurface({ section }: SettingsSurfaceProps) {
  const copy = settingsSectionCopy[section];

  return (
    <section
      data-slot="settings-surface"
      className="min-h-0 overflow-auto px-6 py-6 max-[760px]:px-5"
      aria-labelledby="settings-surface-title"
    >
      <div className="grid max-w-3xl gap-5">
        <div className="grid gap-2">
          <h1
            id="settings-surface-title"
            className="m-0 font-heading text-2xl leading-tight tracking-normal text-foreground"
          >
            {copy.title}
          </h1>
          <p className="m-0 max-w-[620px] text-sm text-muted-foreground">
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
    case "audit":
      return <AuditSettingsPane />;
    case "providers":
      return <ProvidersSettingsPane />;
  }
}
