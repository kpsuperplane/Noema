import assert from "node:assert/strict";
import { describe, test } from "node:test";
import { ApolloClient, ApolloLink, InMemoryCache, Observable } from "@apollo/client";
import { ApolloProvider } from "@apollo/client/react";
import * as React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { ProvidersSettingsPaneContent } from "@/components/settings/ProvidersSettingsPane";
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
        <SettingsPage onClose={() => {}} />
      </ApolloProvider>
    );

    assert.match(markup, /data-slot="settings-page"/);
    assert.match(markup, /data-slot="settings-sidebar"/);
    assert.match(markup, /Settings/);
    assert.match(markup, /Providers/);
    assert.match(markup, /aria-label="Close settings"/);
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
