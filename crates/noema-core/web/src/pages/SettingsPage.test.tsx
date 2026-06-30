import assert from "node:assert/strict";
import { describe, test } from "node:test";
import { ApolloClient, ApolloLink, InMemoryCache, Observable } from "@apollo/client";
import { ApolloProvider } from "@apollo/client/react";
import * as React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { AgentsSettingsPaneContent } from "@/components/settings/AgentsSettingsPane";
import { ApprovalsSettingsPaneContent } from "@/components/settings/ApprovalsSettingsPane";
import { AuditSettingsPaneContent } from "@/components/settings/AuditSettingsPane";
import { McpServerSetupFlow } from "@/components/settings/McpServerSetupFlow";
import { McpSettingsPaneContent } from "@/components/settings/McpSettingsPane";
import { ProvidersSettingsPaneContent } from "@/components/settings/ProvidersSettingsPane";
import { TrustedIdentitiesSettingsPaneContent } from "@/components/settings/TrustedIdentitiesSettingsPane";
import { SettingsSurface } from "./SettingsPage";

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

const trustedIdentitySelectors = [
  {
    __typename: "GraphqlTrustedIdentitySelector" as const,
    selectorId: "trusted_identity_selector:email",
    ownerScopeId: "human:local",
    selectorKind: "email",
    normalizedValue: "kevin@example.com",
    effect: "trust",
    issuerActorId: "human:local"
  },
  {
    __typename: "GraphqlTrustedIdentitySelector" as const,
    selectorId: "trusted_identity_selector:phone",
    ownerScopeId: "human:local",
    selectorKind: "phone",
    normalizedValue: "+14155550100",
    effect: "trust",
    issuerActorId: "human:local"
  },
  {
    __typename: "GraphqlTrustedIdentitySelector" as const,
    selectorId: "trusted_identity_selector:domain",
    ownerScopeId: "human:local",
    selectorKind: "domain",
    normalizedValue: "example.org",
    effect: "trust",
    issuerActorId: "human:local"
  }
] as const;

describe("SettingsPage", () => {
  test("renders a shell-contained settings surface with Providers selected", () => {
    const markup = renderToStaticMarkup(
      <ApolloProvider client={testApolloClient()}>
        <SettingsSurface section="providers" />
      </ApolloProvider>
    );

    assert.match(markup, /data-slot="settings-surface"/);
    assert.doesNotMatch(markup, /data-slot="settings-page"/);
    assert.doesNotMatch(markup, /data-slot="settings-sidebar"/);
    assert.doesNotMatch(markup, /aria-label="Close settings"/);
    assert.match(markup, /Providers/);
    assert.match(markup, /Secret credential material stays outside the UI/);
  });

  test("renders the Agents section title and copy", () => {
    const markup = renderToStaticMarkup(
      <ApolloProvider client={testApolloClient()}>
        <SettingsSurface section="agents" />
      </ApolloProvider>
    );

    assert.match(markup, /Agents/);
    assert.match(markup, /read-only for now/);
    assert.match(markup, /Loading agents/);
  });

  test("renders the MCP section title and loading state", () => {
    const markup = renderToStaticMarkup(
      <ApolloProvider client={testApolloClient()}>
        <SettingsSurface section="mcps" />
      </ApolloProvider>
    );

    assert.match(markup, /MCPs/);
    assert.match(markup, /capability gateway/);
    assert.match(markup, /Loading MCP servers/);
  });

  test("renders governance sections", () => {
    assert.match(
      renderToStaticMarkup(
        <ApolloProvider client={testApolloClient()}>
          <SettingsSurface section="trusted-identities" />
        </ApolloProvider>
      ),
      /Loading trusted identities/
    );

    assert.match(
      renderToStaticMarkup(
        <ApolloProvider client={testApolloClient()}>
          <SettingsSurface section="approvals" />
        </ApolloProvider>
      ),
      /Loading MCP approvals/
    );

    assert.match(
      renderToStaticMarkup(
        <ApolloProvider client={testApolloClient()}>
          <SettingsSurface section="audit" />
        </ApolloProvider>
      ),
      /MCP audit records will appear here/
    );
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
      authStatus: "none",
      toolCount: 0
    },
    {
      __typename: "GraphqlMcpServer" as const,
      mcpServerId: "mcp:notion",
      displayName: "Notion",
      transportKind: "http_sse",
      enabled: false,
      healthStatus: "healthy",
      authStatus: "authenticated",
      toolCount: 2
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
    assert.match(markup, /Notion/);
    assert.match(markup, /Enabled/);
    assert.match(markup, /Disabled/);
    assert.match(markup, /Needs tool calibration/);
    assert.match(markup, /stdio/);
    assert.match(markup, /3 tools/);
    assert.match(markup, /Healthy/);
    assert.match(markup, /Authenticated/);
    assert.match(markup, /Not required/);
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

  test("renders MCP list actions", () => {
    const markup = renderToStaticMarkup(
      <McpSettingsPaneContent
        servers={servers}
        loading={false}
        error={null}
        onRetry={() => {}}
      />
    );

    assert.match(markup, /Add MCP server/);
    assert.match(markup, /Configure tools/);
    assert.match(markup, /Delete/);
  });

  test("renders guided MCP setup authentication state", () => {
    const markup = renderToStaticMarkup(
      <McpServerSetupFlow
        setupResult={{
          setupStatus: "needs_auth",
          discoveryStatus: "needs_auth",
          discoveredToolCount: 0,
          setupError: "This MCP server requires authentication before Noema can list tools.",
          server: null
        }}
        setupSubmitting={false}
        setupError={null}
        onCreateServer={() => {}}
      />
    );

    assert.match(markup, /Display name/);
    assert.match(markup, /Authentication required/);
    assert.match(markup, /server has not been saved yet/);
    assert.doesNotMatch(markup, /Verify server/);
  });
});

describe("TrustedIdentitiesSettingsPaneContent", () => {
  test("renders email, phone, and domain selector rows", () => {
    const markup = renderToStaticMarkup(
      <TrustedIdentitiesSettingsPaneContent
        selectors={trustedIdentitySelectors}
        loading={false}
        error={null}
        onRetry={() => {}}
      />
    );

    assert.match(markup, /email/);
    assert.match(markup, /kevin@example\.com/);
    assert.match(markup, /phone/);
    assert.match(markup, /\+14155550100/);
    assert.match(markup, /domain/);
    assert.match(markup, /example\.org/);
    assert.match(markup, /Owner scope/);
    assert.match(markup, /human:local/);
    assert.match(markup, /Issuer/);
    assert.match(markup, /trust/);
  });

  test("renders loading, error, and empty states", () => {
    assert.match(
      renderToStaticMarkup(
        <TrustedIdentitiesSettingsPaneContent
          selectors={[]}
          loading
          error={null}
          onRetry={() => {}}
        />
      ),
      /Loading trusted identities/
    );

    assert.match(
      renderToStaticMarkup(
        <TrustedIdentitiesSettingsPaneContent
          selectors={[]}
          loading={false}
          error="Could not load trusted identities"
          onRetry={() => {}}
        />
      ),
      /Trusted identity selectors could not be loaded/
    );

    assert.match(
      renderToStaticMarkup(
        <TrustedIdentitiesSettingsPaneContent
          selectors={[]}
          loading={false}
          error={null}
          onRetry={() => {}}
        />
      ),
      /No trusted identity selectors are configured/
    );
  });
});

describe("Governance placeholder pane content", () => {
  test("renders concise empty states", () => {
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
