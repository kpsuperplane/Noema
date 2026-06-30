import assert from "node:assert/strict";
import { describe, test } from "node:test";
import { ApolloClient, ApolloLink, InMemoryCache, Observable } from "@apollo/client";
import { ApolloProvider } from "@apollo/client/react";
import * as React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { AgentsSettingsPaneContent } from "@/components/settings/AgentsSettingsPane";
import { ApprovalsSettingsPaneContent } from "@/components/settings/ApprovalsSettingsPane";
import { AuditSettingsPaneContent } from "@/components/settings/AuditSettingsPane";
import { McpSettingsPaneContent } from "@/components/settings/McpSettingsPane";
import { ProvidersSettingsPaneContent } from "@/components/settings/ProvidersSettingsPane";
import { SettingsSidebar } from "@/components/settings/SettingsSidebar";
import { TrustedIdentitiesSettingsPaneContent } from "@/components/settings/TrustedIdentitiesSettingsPane";
import type { AppRoute } from "@/routes";
import { SettingsPage } from "./SettingsPage";

const providerAccount = {
  providerKind: "codex",
  accountKey: "default",
  displayName: "Codex",
  authMethod: "oauth_device_code",
  status: "AUTHENTICATED",
  isActive: true,
  isDefault: true,
  lastCheckedAt: "2026-06-30T12:00:00Z",
  lastAuthenticatedAt: "2026-06-30T11:59:00Z",
  lastErrorCode: null,
  lastErrorMessage: null
} as const;

describe("SettingsPage", () => {
  test("renders a full-screen settings takeover with Providers selected", () => {
    const markup = renderToStaticMarkup(
      <ApolloProvider client={testApolloClient()}>
        <SettingsPage section="providers" onNavigate={() => {}} onClose={() => {}} />
      </ApolloProvider>
    );

    assert.match(markup, /data-slot="settings-page"/);
    assert.match(markup, /data-slot="settings-sidebar"/);
    assert.match(markup, /Settings/);
    assert.match(markup, /Providers/);
    assert.match(markup, /aria-label="Close settings"/);
  });

  test("renders the Agents section title and copy", () => {
    const markup = renderToStaticMarkup(
      <ApolloProvider client={testApolloClient()}>
        <SettingsPage section="agents" onNavigate={() => {}} onClose={() => {}} />
      </ApolloProvider>
    );

    assert.match(markup, /Agents/);
    assert.match(markup, /read-only for now/);
    assert.match(markup, /Loading agents/);
  });

  test("renders the MCP section title and loading state", () => {
    const markup = renderToStaticMarkup(
      <ApolloProvider client={testApolloClient()}>
        <SettingsPage section="mcps" onNavigate={() => {}} onClose={() => {}} />
      </ApolloProvider>
    );

    assert.match(markup, /MCPs/);
    assert.match(markup, /capability gateway/);
    assert.match(markup, /Loading MCP servers/);
  });

  test("renders placeholder sections", () => {
    assert.match(
      renderToStaticMarkup(
        <ApolloProvider client={testApolloClient()}>
          <SettingsPage
            section="trusted-identities"
            onNavigate={() => {}}
            onClose={() => {}}
          />
        </ApolloProvider>
      ),
      /Trusted identity management is not configured yet/
    );

    assert.match(
      renderToStaticMarkup(
        <ApolloProvider client={testApolloClient()}>
          <SettingsPage section="approvals" onNavigate={() => {}} onClose={() => {}} />
        </ApolloProvider>
      ),
      /No MCP approvals are pending/
    );

    assert.match(
      renderToStaticMarkup(
        <ApolloProvider client={testApolloClient()}>
          <SettingsPage section="audit" onNavigate={() => {}} onClose={() => {}} />
        </ApolloProvider>
      ),
      /MCP audit records will appear here/
    );
  });

  test("routes Settings tab selections through navigation", () => {
    const routes: AppRoute[] = [];
    const providersSidebar = SettingsSidebar({
      activeSection: "providers",
      onSelectSection: (section) => routes.push({ kind: "settings", section })
    });
    findButtonByText(providersSidebar, "Agents").props.onClick();
    assert.deepEqual(routes.pop(), { kind: "settings", section: "agents" });
    findButtonByText(providersSidebar, "MCPs").props.onClick();
    assert.deepEqual(routes.pop(), { kind: "settings", section: "mcps" });
    findButtonByText(providersSidebar, "Trusted identities").props.onClick();
    assert.deepEqual(routes.pop(), {
      kind: "settings",
      section: "trusted-identities"
    });
    findButtonByText(providersSidebar, "Approvals").props.onClick();
    assert.deepEqual(routes.pop(), { kind: "settings", section: "approvals" });
    findButtonByText(providersSidebar, "Audit").props.onClick();
    assert.deepEqual(routes.pop(), { kind: "settings", section: "audit" });

    const agentsSidebar = SettingsSidebar({
      activeSection: "agents",
      onSelectSection: (section) => routes.push({ kind: "settings", section })
    });
    findButtonByText(agentsSidebar, "Providers").props.onClick();
    assert.deepEqual(routes.pop(), { kind: "settings", section: "providers" });
  });
});

describe("McpSettingsPaneContent", () => {
  const servers = [
    {
      __typename: "GraphqlMcpServer" as const,
      mcpServerId: "mcp:filesystem",
      displayName: "Filesystem",
      transportKind: "stdio",
      enabled: true,
      healthStatus: "healthy",
      authStatus: "authenticated",
      toolCount: 3
    },
    {
      __typename: "GraphqlMcpServer" as const,
      mcpServerId: "mcp:archive",
      displayName: "Archive",
      transportKind: "http",
      enabled: false,
      healthStatus: "unknown",
      authStatus: "not_required",
      toolCount: 0
    }
  ];

  test("renders MCP server metadata without unsafe internals", () => {
    const markup = renderToStaticMarkup(
      <McpSettingsPaneContent
        servers={servers}
        loading={false}
        error={null}
        onRetry={() => {}}
      />
    );

    assert.match(markup, /Filesystem/);
    assert.match(markup, /Archive/);
    assert.match(markup, /Enabled/);
    assert.match(markup, /Disabled/);
    assert.match(markup, /stdio/);
    assert.match(markup, /3 tools/);
    assert.match(markup, /Healthy/);
    assert.match(markup, /Authenticated/);
    assert.match(markup, /Not Required/);
    assert.doesNotMatch(markup, /safe_config/);
    assert.doesNotMatch(markup, /raw/);
    assert.doesNotMatch(markup, /schema/);
  });

  test("renders loading, error, and empty states", () => {
    assert.match(
      renderToStaticMarkup(
        <McpSettingsPaneContent
          servers={[]}
          loading
          error={null}
          onRetry={() => {}}
        />
      ),
      /Loading MCP servers/
    );

    assert.match(
      renderToStaticMarkup(
        <McpSettingsPaneContent
          servers={[]}
          loading={false}
          error="Could not load MCP servers"
          onRetry={() => {}}
        />
      ),
      /MCP server metadata could not be loaded/
    );

    assert.match(
      renderToStaticMarkup(
        <McpSettingsPaneContent
          servers={[]}
          loading={false}
          error={null}
          onRetry={() => {}}
        />
      ),
      /No MCP servers are configured/
    );
  });
});

describe("placeholder settings pane content", () => {
  test("renders concise placeholder states", () => {
    assert.match(
      renderToStaticMarkup(<TrustedIdentitiesSettingsPaneContent />),
      /Trusted identity management is not configured yet/
    );
    assert.match(
      renderToStaticMarkup(<ApprovalsSettingsPaneContent />),
      /No MCP approvals are pending/
    );
    assert.match(
      renderToStaticMarkup(<AuditSettingsPaneContent />),
      /MCP audit records will appear here/
    );
  });
});

describe("AgentsSettingsPaneContent", () => {
  const agents = [
    {
      __typename: "GraphqlAgent" as const,
      agentId: "agent:primary",
      displayName: "Noema",
      isPrimary: true
    },
    {
      __typename: "GraphqlAgent" as const,
      agentId: "agent:unnamed",
      displayName: null,
      isPrimary: false
    }
  ];

  test("renders agent metadata without unsafe internals", () => {
    const markup = renderToStaticMarkup(
      <AgentsSettingsPaneContent agents={agents} loading={false} error={null} />
    );

    assert.match(markup, /Noema/);
    assert.match(markup, /Unnamed agent/);
    assert.match(markup, /Primary/);
    assert.match(markup, /Agent id/);
    assert.match(markup, /agent:primary/);
    assert.match(markup, /agent:unnamed/);
    assert.doesNotMatch(markup, /prompt/);
    assert.doesNotMatch(markup, /memory/);
    assert.doesNotMatch(markup, /runtime/);
    assert.doesNotMatch(markup, /credential/);
    assert.doesNotMatch(markup, /conversation/);
  });

  test("renders loading, error, and empty states", () => {
    assert.match(
      renderToStaticMarkup(
        <AgentsSettingsPaneContent agents={[]} loading error={null} />
      ),
      /Loading agents/
    );

    assert.match(
      renderToStaticMarkup(
        <AgentsSettingsPaneContent agents={[]} loading={false} error="Could not load agents" />
      ),
      /Agent metadata could not be loaded/
    );

    assert.match(
      renderToStaticMarkup(
        <AgentsSettingsPaneContent agents={[]} loading={false} error={null} />
      ),
      /No agents were found/
    );
  });
});

describe("ProvidersSettingsPaneContent", () => {
  test("renders connected provider safe metadata", () => {
    const markup = renderToStaticMarkup(
      <ProvidersSettingsPaneContent
        accounts={[providerAccount]}
        loading={false}
        error={null}
        onRetry={() => {}}
      />
    );

    assert.match(markup, /Connected provider/);
    assert.match(markup, /Codex/);
    assert.match(markup, /Authenticated/);
    assert.match(markup, /Provider kind/);
    assert.match(markup, /codex/);
    assert.match(markup, /Account key/);
    assert.match(markup, /default/);
    assert.doesNotMatch(markup, /provider_account:/);
    assert.doesNotMatch(markup, /auth\.json/);
    assert.doesNotMatch(markup, /codex_tokens\.json/);
    assert.doesNotMatch(markup, /api_key/);
  });

  test("renders loading, error, and empty states", () => {
    assert.match(
      renderToStaticMarkup(
        <ProvidersSettingsPaneContent
          accounts={[]}
          loading
          error={null}
          onRetry={() => {}}
        />
      ),
      /Loading provider metadata/
    );

    assert.match(
      renderToStaticMarkup(
        <ProvidersSettingsPaneContent
          accounts={[]}
          loading={false}
          error="Could not load providers"
          onRetry={() => {}}
        />
      ),
      /Could not load providers/
    );

    assert.match(
      renderToStaticMarkup(
        <ProvidersSettingsPaneContent
          accounts={[]}
          loading={false}
          error={null}
          onRetry={() => {}}
        />
      ),
      /No provider accounts are available/
    );
  });
});

function testApolloClient() {
  return new ApolloClient({
    cache: new InMemoryCache(),
    link: new ApolloLink(() => new Observable(() => undefined))
  });
}

function findButtonByText(
  node: React.ReactNode,
  text: string
): React.ReactElement<{ children?: React.ReactNode; onClick: () => void }> {
  if (!React.isValidElement(node)) {
    throw new Error(`Button ${text} not found`);
  }
  const element = node as React.ReactElement<{ children?: React.ReactNode; onClick?: () => void }>;
  if (element.type === "button" && textContent(element.props.children).includes(text)) {
    return element as React.ReactElement<{ children?: React.ReactNode; onClick: () => void }>;
  }
  for (const child of React.Children.toArray(element.props.children)) {
    try {
      return findButtonByText(child, text);
    } catch (error) {
      if (!(error instanceof Error) || !error.message.includes(`Button ${text} not found`)) {
        throw error;
      }
    }
  }
  throw new Error(`Button ${text} not found`);
}

function textContent(node: React.ReactNode): string {
  if (typeof node === "string" || typeof node === "number") {
    return String(node);
  }
  if (Array.isArray(node)) {
    return node.map(textContent).join("");
  }
  if (React.isValidElement(node)) {
    return textContent((node as React.ReactElement<{ children?: React.ReactNode }>).props.children);
  }
  return "";
}
