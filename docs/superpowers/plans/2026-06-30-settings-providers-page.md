# Settings Providers Page Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a bottom-left sidebar settings cog that opens a full-screen Settings takeover with a Providers tab showing the connected provider and safe non-secret metadata.

**Architecture:** Add a dedicated GraphQL `providerAccounts` read model over existing non-secret `ProviderAccountRecord` data, then expose it to the web frontend through generated GraphQL types. Add `/settings` as a route rendered outside `AppShell`, while `AppShell` keeps Home and Memory as primary navigation and exposes Settings only as a bottom utility button.

**Tech Stack:** Rust, async-graphql, SurrealDB store APIs, React, Apollo Client, TypeScript, Bun, lucide-react, existing shadcn/base UI primitives.

---

## File Structure

- Create `crates/noema-core/src/graphql/provider_accounts.rs`
  - Owns the safe provider-account GraphQL object and resolver.
  - Converts `ProviderAccountRecord` into UI-safe GraphQL fields.
- Modify `crates/noema-core/src/graphql.rs`
  - Registers the new GraphQL module.
- Modify `crates/noema-core/src/graphql/schema.rs`
  - Exposes `providerAccounts` on `QueryRoot`.
  - Adds resolver tests for safe metadata and SDL exposure.
- Modify `crates/noema-core/web/src/graphql/operations.ts`
  - Adds the frontend `ProviderAccounts` query.
- Regenerate `crates/noema-core/web/src/generated/schema.graphql` and `crates/noema-core/web/src/generated/graphql.ts`
  - Produced by `bun run gen:types`; do not hand-edit generated files.
- Modify `crates/noema-core/web/src/routes.ts`
  - Adds the `/settings` route and settings close-route state.
- Modify `crates/noema-core/web/src/routes.test.ts`
  - Covers `/settings`, canonical path, and previous non-settings route behavior.
- Modify `crates/noema-core/web/src/components/shell/ShellSidebar.tsx`
  - Adds the bottom settings cog utility button.
- Modify `crates/noema-core/web/src/components/shell/AppShell.tsx`
  - Passes shell navigation into the sidebar settings button without making Settings a primary destination.
- Modify `crates/noema-core/web/src/components/shell/AppShell.test.ts`
  - Covers the primary nav contract and bottom utility cog rendering.
- Create `crates/noema-core/web/src/components/settings/providerMetadata.ts`
  - Formats provider status, auth method, field labels, and display values.
- Create `crates/noema-core/web/src/components/settings/providerMetadata.test.ts`
  - Verifies metadata excludes provider ids and credential-like fields.
- Create `crates/noema-core/web/src/components/settings/SettingsSidebar.tsx`
  - Renders the local Settings sidebar with Providers selected.
- Create `crates/noema-core/web/src/components/settings/ProvidersSettingsPane.tsx`
  - Renders loading, error, empty, and provider metadata states.
- Create `crates/noema-core/web/src/pages/SettingsPage.tsx`
  - Owns the full-screen settings takeover layout and close/back button.
- Create `crates/noema-core/web/src/pages/SettingsPage.test.tsx`
  - Verifies takeover markup, Providers tab, safe metadata rendering, and close control.
- Modify `crates/noema-core/web/src/App.tsx`
  - Renders `SettingsPage` outside `AppShell` only after onboarding succeeds.
- Modify `crates/noema-core/web/src/App.test.ts`
  - Adds pure helper tests for settings route access gating.
- Modify `docs/frontend/navigation-workflows.md`
  - Documents Settings as a utility takeover opened from the sidebar cog.
- Modify `docs/frontend/current-contract.md`
  - Updates `/settings` backing to provider-account metadata and onboarding-gated access.
- Modify `docs/context/current.md`
  - Adds a concise settled decision/open loop note after implementation.

## Task 1: Backend Provider Accounts GraphQL Read Model

**Files:**
- Create: `crates/noema-core/src/graphql/provider_accounts.rs`
- Modify: `crates/noema-core/src/graphql.rs`
- Modify: `crates/noema-core/src/graphql/schema.rs`

- [ ] **Step 1: Write failing GraphQL resolver tests**

Append these tests inside the existing `#[cfg(test)] mod tests` in `crates/noema-core/src/graphql/schema.rs`:

```rust
    #[tokio::test]
    async fn provider_accounts_query_returns_safe_metadata() {
        use crate::{ProviderAccountStatus, store::tests::test_store};

        let store = test_store().await;
        let account = store
            .ensure_default_provider_account()
            .await
            .expect("provider account");
        store
            .update_provider_account_status(
                &account.provider_account_id,
                ProviderAccountStatus::Authenticated,
                Some("codex_ok"),
                Some("Codex credentials are usable"),
            )
            .await
            .expect("status update");

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  providerAccounts {
                    providerKind
                    accountKey
                    displayName
                    authMethod
                    status
                    isActive
                    isDefault
                    lastCheckedAt
                    lastAuthenticatedAt
                    lastErrorCode
                    lastErrorMessage
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let account = &data["providerAccounts"][0];
        assert_eq!(account["providerKind"], "codex");
        assert_eq!(account["accountKey"], "default");
        assert_eq!(account["displayName"], "Codex");
        assert_eq!(account["authMethod"], "oauth_device_code");
        assert_eq!(account["status"], "AUTHENTICATED");
        assert_eq!(account["isActive"], true);
        assert_eq!(account["isDefault"], true);
        assert_eq!(account["lastErrorCode"], "codex_ok");
        assert_eq!(account["lastErrorMessage"], "Codex credentials are usable");

        let json_text = serde_json::to_string(&data).expect("provider json");
        assert!(!json_text.contains("provider_account:codex:default"));
        assert!(!json_text.contains("auth.json"));
        assert!(!json_text.contains("codex_tokens.json"));
        assert!(!json_text.contains("api_key"));
        assert!(!json_text.contains("token"));
    }
```

Also add this assertion to `schema_sdl_exposes_initial_noema_fields`:

```rust
        assert!(sdl.contains("providerAccounts"));
        assert!(sdl.contains("type GraphqlProviderAccount"));
```

- [ ] **Step 2: Run the focused backend tests to verify failure**

Run:

```bash
cargo test -p noema-core graphql::schema::tests --lib
```

Expected: FAIL because `providerAccounts` and `GraphqlProviderAccount` do not exist yet.

- [ ] **Step 3: Add the GraphQL provider account module**

Create `crates/noema-core/src/graphql/provider_accounts.rs`:

```rust
use async_graphql::{Result, SimpleObject};

use crate::{ProviderAccountRecord, ProviderAuthMethod};

use super::{errors::graphql_error, onboarding::GraphqlProviderAccountStatus, schema::GraphqlState};

/// Provider account metadata safe to show in Settings.
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlProviderAccount {
    /// Provider family, such as `codex`.
    pub provider_kind: String,
    /// Provider-local account key.
    pub account_key: String,
    /// Human-readable account name.
    pub display_name: String,
    /// Authentication method used for this account.
    pub auth_method: String,
    /// Last known account readiness status.
    pub status: GraphqlProviderAccountStatus,
    /// Whether the account may be selected.
    pub is_active: bool,
    /// Whether the account is the default account for its provider.
    pub is_default: bool,
    /// Last time Noema checked the account status.
    pub last_checked_at: Option<String>,
    /// Last time Noema observed successful authentication.
    pub last_authenticated_at: Option<String>,
    /// Last non-secret provider error code.
    pub last_error_code: Option<String>,
    /// Last non-secret provider error message.
    pub last_error_message: Option<String>,
}

impl From<ProviderAccountRecord> for GraphqlProviderAccount {
    fn from(account: ProviderAccountRecord) -> Self {
        Self {
            provider_kind: account.provider_kind,
            account_key: account.account_key,
            display_name: account.display_name,
            auth_method: auth_method_label(account.auth_method).to_string(),
            status: account.status.into(),
            is_active: account.is_active,
            is_default: account.is_default,
            last_checked_at: account.last_checked_at,
            last_authenticated_at: account.last_authenticated_at,
            last_error_code: account.last_error_code,
            last_error_message: account.last_error_message,
        }
    }
}

const fn auth_method_label(method: ProviderAuthMethod) -> &'static str {
    method.as_str()
}

pub(super) async fn provider_accounts(
    state: &GraphqlState,
) -> Result<Vec<GraphqlProviderAccount>> {
    let store = state.store()?;
    let account = store
        .active_provider_account("codex")
        .await
        .map_err(graphql_error)?;
    Ok(account.into_iter().map(Into::into).collect())
}
```

- [ ] **Step 4: Register the module and query**

In `crates/noema-core/src/graphql.rs`, add:

```rust
mod provider_accounts;
```

next to the other module declarations.

In `crates/noema-core/src/graphql/schema.rs`, update the imports:

```rust
    provider_accounts::{self, GraphqlProviderAccount},
```

Then add this query to `impl QueryRoot`:

```rust
    /// List provider account metadata safe to show in Settings.
    async fn provider_accounts(&self, ctx: &Context<'_>) -> Result<Vec<GraphqlProviderAccount>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        provider_accounts::provider_accounts(state).await
    }
```

- [ ] **Step 5: Run backend tests**

Run:

```bash
cargo test -p noema-core graphql::schema::tests --lib
```

Expected: PASS.

- [ ] **Step 6: Commit backend read model**

Run:

```bash
git add crates/noema-core/src/graphql.rs crates/noema-core/src/graphql/provider_accounts.rs crates/noema-core/src/graphql/schema.rs
git commit -m "feat: expose provider account settings metadata"
```

## Task 2: Frontend GraphQL Operation And Generated Types

**Files:**
- Modify: `crates/noema-core/web/src/graphql/operations.ts`
- Generate: `crates/noema-core/web/src/generated/schema.graphql`
- Generate: `crates/noema-core/web/src/generated/graphql.ts`

- [ ] **Step 1: Add the frontend query before generation**

Add this operation to `crates/noema-core/web/src/graphql/operations.ts` after `OnboardingStatusDocument`:

```ts
export const ProviderAccountsDocument = gql`
  query ProviderAccounts {
    providerAccounts {
      providerKind
      accountKey
      displayName
      authMethod
      status
      isActive
      isDefault
      lastCheckedAt
      lastAuthenticatedAt
      lastErrorCode
      lastErrorMessage
    }
  }
`;
```

- [ ] **Step 2: Run type generation**

Run:

```bash
cd crates/noema-core/web
bun run gen:types
```

Expected: PASS. `src/generated/schema.graphql` contains `providerAccounts`; `src/generated/graphql.ts` exports `ProviderAccountsDocument` and `ProviderAccountsQuery`.

- [ ] **Step 3: Inspect generated diff for secret fields**

Run:

```bash
git diff -- crates/noema-core/web/src/generated/schema.graphql crates/noema-core/web/src/generated/graphql.ts
```

Expected: generated provider account types include the fields from `ProviderAccountsDocument` and do not include `providerAccountId`, credential paths, token fields, API key fields, or metadata blobs.

- [ ] **Step 4: Commit GraphQL operation and generated types**

Run:

```bash
git add crates/noema-core/web/src/graphql/operations.ts crates/noema-core/web/src/generated/schema.graphql crates/noema-core/web/src/generated/graphql.ts
git commit -m "feat: add provider accounts web query"
```

## Task 3: Settings Route And Previous Route State

**Files:**
- Modify: `crates/noema-core/web/src/routes.ts`
- Modify: `crates/noema-core/web/src/routes.test.ts`

- [ ] **Step 1: Write failing route tests**

Update `crates/noema-core/web/src/routes.test.ts`:

```ts
import assert from "node:assert/strict";
import { describe, test } from "node:test";
import {
  pathForRoute,
  routeFromPathname,
  settingsFallbackRoute,
  shouldRememberAsPreviousAppRoute
} from "./routes";

describe("routeFromPathname", () => {
  test("recognizes chat, memory, and settings routes", () => {
    assert.deepEqual(routeFromPathname("/"), { kind: "chat" });
    assert.deepEqual(routeFromPathname("/chat"), { kind: "chat" });
    assert.deepEqual(routeFromPathname("/memory"), { kind: "memory_home" });
    assert.deepEqual(routeFromPathname("/memory/graph"), { kind: "memory_graph" });
    assert.deepEqual(routeFromPathname("/settings"), {
      kind: "settings",
      section: "providers"
    });
  });

  test("falls back to chat for unknown routes", () => {
    assert.deepEqual(routeFromPathname("/memory/nope"), { kind: "chat" });
    assert.deepEqual(routeFromPathname("/settings/providers"), { kind: "chat" });
    assert.deepEqual(routeFromPathname("/not-a-real-route"), { kind: "chat" });
  });
});

describe("pathForRoute", () => {
  test("returns canonical paths", () => {
    assert.equal(pathForRoute({ kind: "chat" }), "/");
    assert.equal(pathForRoute({ kind: "memory_home" }), "/memory");
    assert.equal(pathForRoute({ kind: "memory_graph" }), "/memory/graph");
    assert.equal(pathForRoute({ kind: "settings", section: "providers" }), "/settings");
  });
});

describe("settings route helpers", () => {
  test("remembers only non-settings app routes", () => {
    assert.equal(shouldRememberAsPreviousAppRoute({ kind: "chat" }), true);
    assert.equal(shouldRememberAsPreviousAppRoute({ kind: "memory_home" }), true);
    assert.equal(shouldRememberAsPreviousAppRoute({ kind: "memory_graph" }), true);
    assert.equal(
      shouldRememberAsPreviousAppRoute({ kind: "settings", section: "providers" }),
      false
    );
  });

  test("falls back from settings to home when no previous route exists", () => {
    assert.deepEqual(settingsFallbackRoute(null), { kind: "chat" });
    assert.deepEqual(settingsFallbackRoute({ kind: "memory_home" }), { kind: "memory_home" });
  });
});
```

- [ ] **Step 2: Run route tests to verify failure**

Run:

```bash
cd crates/noema-core/web
bun test src/routes.test.ts
```

Expected: FAIL because settings route helpers do not exist yet.

- [ ] **Step 3: Implement route changes**

Replace `crates/noema-core/web/src/routes.ts` with this structure while preserving imports:

```ts
import React from "react";

export type AppRoute =
  | { kind: "chat" }
  | { kind: "memory_home" }
  | { kind: "memory_graph" }
  | { kind: "settings"; section: "providers" };

export type NonSettingsAppRoute = Exclude<AppRoute, { kind: "settings" }>;

export function routeFromPathname(pathname: string): AppRoute {
  if (pathname === "/memory") {
    return { kind: "memory_home" };
  }
  if (pathname === "/memory/graph") {
    return { kind: "memory_graph" };
  }
  if (pathname === "/settings") {
    return { kind: "settings", section: "providers" };
  }
  return { kind: "chat" };
}

export function pathForRoute(route: AppRoute): string {
  if (route.kind === "memory_home") {
    return "/memory";
  }
  if (route.kind === "memory_graph") {
    return "/memory/graph";
  }
  if (route.kind === "settings") {
    return "/settings";
  }
  return "/";
}

export function shouldRememberAsPreviousAppRoute(
  route: AppRoute
): route is NonSettingsAppRoute {
  return route.kind !== "settings";
}

export function settingsFallbackRoute(route: NonSettingsAppRoute | null): NonSettingsAppRoute {
  return route ?? { kind: "chat" };
}

export function useBrowserRoute() {
  const [route, setRoute] = React.useState(() => routeFromPathname(window.location.pathname));
  const previousAppRouteRef = React.useRef<NonSettingsAppRoute>(
    shouldRememberAsPreviousAppRoute(route) ? route : { kind: "chat" }
  );

  React.useEffect(() => {
    const onPopState = () => {
      const nextRoute = routeFromPathname(window.location.pathname);
      if (shouldRememberAsPreviousAppRoute(nextRoute)) {
        previousAppRouteRef.current = nextRoute;
      }
      setRoute(nextRoute);
    };
    window.addEventListener("popstate", onPopState);
    return () => window.removeEventListener("popstate", onPopState);
  }, []);

  const navigate = React.useCallback((nextRoute: AppRoute) => {
    setRoute((currentRoute) => {
      if (shouldRememberAsPreviousAppRoute(currentRoute)) {
        previousAppRouteRef.current = currentRoute;
      }
      const nextPath = pathForRoute(nextRoute);
      window.history.pushState({}, "", nextPath);
      return routeFromPathname(nextPath);
    });
  }, []);

  const closeSettings = React.useCallback(() => {
    const nextRoute = settingsFallbackRoute(previousAppRouteRef.current);
    const nextPath = pathForRoute(nextRoute);
    window.history.pushState({}, "", nextPath);
    setRoute(routeFromPathname(nextPath));
  }, []);

  return {
    route,
    navigate,
    closeSettings,
    previousAppRoute: previousAppRouteRef.current
  };
}
```

- [ ] **Step 4: Run route tests**

Run:

```bash
cd crates/noema-core/web
bun test src/routes.test.ts
```

Expected: PASS.

- [ ] **Step 5: Commit route support**

Run:

```bash
git add crates/noema-core/web/src/routes.ts crates/noema-core/web/src/routes.test.ts
git commit -m "feat: add settings route"
```

## Task 4: Sidebar Settings Cog

**Files:**
- Modify: `crates/noema-core/web/src/components/shell/ShellSidebar.tsx`
- Modify: `crates/noema-core/web/src/components/shell/AppShell.tsx`
- Modify: `crates/noema-core/web/src/components/shell/AppShell.test.ts`

- [ ] **Step 1: Write failing shell tests**

In `crates/noema-core/web/src/components/shell/AppShell.test.ts`, update the primary nav contract test to keep Home and Memory unchanged:

```ts
  test("defines the primary shell navigation contract without settings", () => {
    assert.deepEqual(shellNavItems, [
      { destination: "home", label: "Home", route: { kind: "chat" } },
      { destination: "memory", label: "Memory", route: { kind: "memory_home" } }
    ]);
  });
```

Add this test in `describe("AppShell layered deck markup", ...)`:

```ts
  test("renders settings as one bottom utility button", () => {
    const markup = renderShell();

    assert.equal(countMatches(markup, /aria-label="Primary"/g), 1);
    assert.equal(countMatches(markup, /Open settings/g), 1);
    assert.match(markup, /data-slot="shell-settings-button"/);
    assert.doesNotMatch(markup, />Settings<\/button>/);
  });
```

- [ ] **Step 2: Run shell tests to verify failure**

Run:

```bash
cd crates/noema-core/web
bun test src/components/shell/AppShell.test.ts
```

Expected: FAIL because the settings button is not rendered.

- [ ] **Step 3: Add the settings cog to `ShellSidebar`**

In `crates/noema-core/web/src/components/shell/ShellSidebar.tsx`, change the imports:

```ts
import { Brain, House, Settings } from "lucide-react";
```

Add the settings route to the component body without adding a primary nav item:

```tsx
export function ShellSidebar({
  activeDestination,
  attention,
  navItems,
  primaryAgentNamed,
  onNavigate
}: {
  activeDestination: ShellDestination;
  attention: ShellAttention | null;
  navItems: ShellNavItem[];
  primaryAgentNamed: boolean;
  onNavigate: (route: AppRoute) => void;
}) {
  return (
    <div className="grid h-full min-h-0 grid-rows-[auto_auto_minmax(0,1fr)_auto] gap-1">
      <div className="h-8" data-tauri-drag-region />
      {attention ? <ShellAttentionItem attention={attention} /> : <div aria-hidden="true" />}

      <nav className="grid content-start gap-1" aria-label="Primary">
        {navItems.map((item) => {
          const Icon = navIcon[item.destination];
          const showPrimaryAgentAvatar = item.destination === "home" && primaryAgentNamed;
          const active = item.destination === activeDestination;
          return (
            <Button
              key={item.destination}
              type="button"
              variant="ghost"
              className={cn(
                "justify-start rounded-md px-2.5 text-sm text-[var(--pine-700)] !bg-transparent hover:!bg-[color-mix(in_srgb,var(--pine-700)_10%,transparent)] aria-expanded:!bg-[color-mix(in_srgb,var(--pine-700)_10%,transparent)]",
                active && "!bg-[color-mix(in_srgb,var(--pine-700)_10%,transparent)]"
              )}
              aria-current={active ? "page" : undefined}
              onClick={() => onNavigate(item.route)}
            >
              {showPrimaryAgentAvatar ? (
                <span
                  data-slot="shell-primary-agent-avatar"
                  aria-hidden="true"
                  className="grid size-4 shrink-0 place-items-center [&_[data-slot=avatar]]:!size-4"
                >
                  <IdentityAvatar
                    actorId={LOCAL_AGENT_AVATAR_ID}
                    actorType="agent"
                    className="!size-4"
                    size="sm"
                  />
                </span>
              ) : (
                <Icon className="size-4" aria-hidden />
              )}
              {item.label}
            </Button>
          );
        })}
      </nav>

      <div className="flex items-end">
        <Button
          data-slot="shell-settings-button"
          type="button"
          variant="ghost"
          size="icon"
          aria-label="Open settings"
          className="rounded-md text-[var(--pine-700)] !bg-transparent hover:!bg-[color-mix(in_srgb,var(--pine-700)_10%,transparent)]"
          onClick={() => onNavigate({ kind: "settings", section: "providers" })}
        >
          <Settings className="size-4" aria-hidden="true" />
        </Button>
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Verify `AppShell` passes the existing navigation callback**

No new prop is needed in `crates/noema-core/web/src/components/shell/AppShell.tsx`; confirm the existing `onNavigate={navigateFromShell}` remains:

```tsx
        <ShellSidebar
          activeDestination={activeDestination}
          attention={attention}
          navItems={navItems}
          primaryAgentNamed={Boolean(primaryAgentName)}
          onNavigate={navigateFromShell}
        />
```

- [ ] **Step 5: Run shell tests**

Run:

```bash
cd crates/noema-core/web
bun test src/components/shell/AppShell.test.ts
```

Expected: PASS.

- [ ] **Step 6: Commit sidebar cog**

Run:

```bash
git add crates/noema-core/web/src/components/shell/ShellSidebar.tsx crates/noema-core/web/src/components/shell/AppShell.test.ts
git commit -m "feat: add settings cog to sidebar"
```

## Task 5: Settings Provider Metadata Model

**Files:**
- Create: `crates/noema-core/web/src/components/settings/providerMetadata.ts`
- Create: `crates/noema-core/web/src/components/settings/providerMetadata.test.ts`

- [ ] **Step 1: Write failing metadata tests**

Create `crates/noema-core/web/src/components/settings/providerMetadata.test.ts`:

```ts
import assert from "node:assert/strict";
import { describe, test } from "node:test";
import {
  providerAuthMethodLabel,
  providerStatusLabel,
  providerTechnicalRows,
  type ProviderSettingsAccount
} from "./providerMetadata";

const connectedAccount: ProviderSettingsAccount = {
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
};

describe("provider settings metadata", () => {
  test("formats status and auth method labels", () => {
    assert.equal(providerStatusLabel("AUTHENTICATED"), "Authenticated");
    assert.equal(providerStatusLabel("UNAUTHENTICATED"), "Unauthenticated");
    assert.equal(providerAuthMethodLabel("oauth_device_code"), "OAuth device code");
  });

  test("returns safe technical rows without provider account ids or credentials", () => {
    const rows = providerTechnicalRows(connectedAccount);

    assert.deepEqual(rows, [
      { label: "Provider kind", value: "codex" },
      { label: "Account key", value: "default" },
      { label: "Auth method", value: "OAuth device code" },
      { label: "Status", value: "Authenticated" },
      { label: "Active", value: "Yes" },
      { label: "Default", value: "Yes" },
      { label: "Last checked", value: "2026-06-30T12:00:00Z" },
      { label: "Last authenticated", value: "2026-06-30T11:59:00Z" }
    ]);

    const text = JSON.stringify(rows);
    assert.equal(text.includes("provider_account:"), false);
    assert.equal(text.includes("auth.json"), false);
    assert.equal(text.includes("codex_tokens.json"), false);
    assert.equal(text.includes("token"), false);
    assert.equal(text.includes("api_key"), false);
  });

  test("includes non-secret provider error metadata when present", () => {
    const rows = providerTechnicalRows({
      ...connectedAccount,
      lastErrorCode: "codex_unavailable",
      lastErrorMessage: "Codex command is unavailable"
    });

    assert.deepEqual(rows.slice(-2), [
      { label: "Last error code", value: "codex_unavailable" },
      { label: "Last error message", value: "Codex command is unavailable" }
    ]);
  });
});
```

- [ ] **Step 2: Run metadata tests to verify failure**

Run:

```bash
cd crates/noema-core/web
bun test src/components/settings/providerMetadata.test.ts
```

Expected: FAIL because `providerMetadata.ts` does not exist.

- [ ] **Step 3: Implement provider metadata helpers**

Create `crates/noema-core/web/src/components/settings/providerMetadata.ts`:

```ts
import type { ProviderAccountsQuery } from "@/generated/graphql";

export type ProviderSettingsAccount = ProviderAccountsQuery["providerAccounts"][number];

export type ProviderMetadataRow = {
  label: string;
  value: string;
};

export function providerStatusLabel(status: ProviderSettingsAccount["status"]) {
  const labels: Record<ProviderSettingsAccount["status"], string> = {
    AUTHENTICATED: "Authenticated",
    CHECKING: "Checking",
    UNAUTHENTICATED: "Unauthenticated",
    UNAVAILABLE: "Unavailable",
    UNKNOWN: "Unknown"
  };
  return labels[status];
}

export function providerAuthMethodLabel(method: ProviderSettingsAccount["authMethod"]) {
  const labels: Record<string, string> = {
    external_manual: "External manual",
    none: "None",
    oauth_device_code: "OAuth device code",
    secret_input: "Secret input"
  };
  return labels[method] ?? method;
}

function yesNo(value: boolean) {
  return value ? "Yes" : "No";
}

function optionalRow(label: string, value: string | null | undefined): ProviderMetadataRow[] {
  return value ? [{ label, value }] : [];
}

export function providerTechnicalRows(account: ProviderSettingsAccount): ProviderMetadataRow[] {
  return [
    { label: "Provider kind", value: account.providerKind },
    { label: "Account key", value: account.accountKey },
    { label: "Auth method", value: providerAuthMethodLabel(account.authMethod) },
    { label: "Status", value: providerStatusLabel(account.status) },
    { label: "Active", value: yesNo(account.isActive) },
    { label: "Default", value: yesNo(account.isDefault) },
    ...optionalRow("Last checked", account.lastCheckedAt),
    ...optionalRow("Last authenticated", account.lastAuthenticatedAt),
    ...optionalRow("Last error code", account.lastErrorCode),
    ...optionalRow("Last error message", account.lastErrorMessage)
  ];
}
```

- [ ] **Step 4: Run metadata tests**

Run:

```bash
cd crates/noema-core/web
bun test src/components/settings/providerMetadata.test.ts
```

Expected: PASS.

- [ ] **Step 5: Commit metadata helpers**

Run:

```bash
git add crates/noema-core/web/src/components/settings/providerMetadata.ts crates/noema-core/web/src/components/settings/providerMetadata.test.ts
git commit -m "feat: add provider settings metadata helpers"
```

## Task 6: Settings Takeover Components

**Files:**
- Create: `crates/noema-core/web/src/components/settings/SettingsSidebar.tsx`
- Create: `crates/noema-core/web/src/components/settings/ProvidersSettingsPane.tsx`
- Create: `crates/noema-core/web/src/pages/SettingsPage.tsx`
- Create: `crates/noema-core/web/src/pages/SettingsPage.test.tsx`

- [ ] **Step 1: Write failing settings page tests**

Create `crates/noema-core/web/src/pages/SettingsPage.test.tsx`:

```tsx
import assert from "node:assert/strict";
import { describe, test } from "node:test";
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
    const markup = renderToStaticMarkup(<SettingsPage onClose={() => {}} />);

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
```

- [ ] **Step 2: Run settings component tests to verify failure**

Run:

```bash
cd crates/noema-core/web
bun test src/pages/SettingsPage.test.tsx
```

Expected: FAIL because settings components do not exist.

- [ ] **Step 3: Implement the local settings sidebar**

Create `crates/noema-core/web/src/components/settings/SettingsSidebar.tsx`:

```tsx
import { ServerCog } from "lucide-react";

export function SettingsSidebar() {
  return (
    <aside
      data-slot="settings-sidebar"
      aria-label="Settings sections"
      className="grid min-h-0 border-r border-[var(--border-subtle)] bg-[var(--pine-50)] px-4 py-5 max-[760px]:border-r-0 max-[760px]:border-b"
    >
      <div className="grid content-start gap-4">
        <div>
          <p className="m-0 font-mono text-[11px] tracking-[0.12em] text-[var(--text-accent)] uppercase">
            Settings
          </p>
          <h1 className="m-0 font-heading text-2xl leading-tight tracking-normal text-foreground">
            Settings
          </h1>
        </div>
        <nav aria-label="Settings" className="grid gap-1">
          <button
            type="button"
            aria-current="page"
            className="flex items-center gap-2 rounded-md bg-[color-mix(in_srgb,var(--pine-700)_10%,transparent)] px-2.5 py-2 text-left text-sm font-medium text-[var(--pine-700)]"
          >
            <ServerCog className="size-4" aria-hidden="true" />
            Providers
          </button>
        </nav>
      </div>
    </aside>
  );
}
```

- [ ] **Step 4: Implement the Providers pane**

Create `crates/noema-core/web/src/components/settings/ProvidersSettingsPane.tsx`:

```tsx
import { RefreshCw } from "lucide-react";
import { useQuery } from "@apollo/client/react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { ProviderAccountsDocument } from "@/generated/graphql";
import {
  providerStatusLabel,
  providerTechnicalRows,
  type ProviderSettingsAccount
} from "./providerMetadata";

export function ProvidersSettingsPane() {
  const result = useQuery(ProviderAccountsDocument, { fetchPolicy: "cache-and-network" });
  return (
    <ProvidersSettingsPaneContent
      accounts={result.data?.providerAccounts ?? []}
      loading={result.loading && !result.data}
      error={result.error?.message ?? null}
      onRetry={() => void result.refetch()}
    />
  );
}

export function ProvidersSettingsPaneContent({
  accounts,
  loading,
  error,
  onRetry
}: {
  accounts: readonly ProviderSettingsAccount[];
  loading: boolean;
  error: string | null;
  onRetry: () => void;
}) {
  const account = accounts[0] ?? null;

  if (loading) {
    return <p className="m-0 text-sm text-muted-foreground">Loading provider metadata...</p>;
  }

  if (error) {
    return (
      <div className="grid gap-3 rounded-md border border-[var(--border-subtle)] bg-white p-4">
        <p className="m-0 text-sm text-muted-foreground">{error}</p>
        <Button type="button" variant="outline" className="w-fit" onClick={onRetry}>
          <RefreshCw className="size-4" aria-hidden="true" />
          Retry
        </Button>
      </div>
    );
  }

  if (!account) {
    return (
      <div className="rounded-md border border-[var(--border-subtle)] bg-white p-4">
        <p className="m-0 text-sm text-muted-foreground">
          No provider accounts are available. Settings is open, but Noema did not return a connected provider account.
        </p>
      </div>
    );
  }

  const rows = providerTechnicalRows(account);

  return (
    <div className="grid gap-4">
      <div className="grid gap-2 rounded-md border border-[var(--border-subtle)] bg-white p-4">
        <p className="m-0 text-xs font-medium uppercase tracking-[0.12em] text-[var(--text-accent)]">
          Connected provider
        </p>
        <div className="flex flex-wrap items-center gap-3">
          <h2 className="m-0 font-heading text-2xl leading-tight tracking-normal text-foreground">
            {account.displayName}
          </h2>
          <Badge variant="outline">{providerStatusLabel(account.status)}</Badge>
        </div>
      </div>

      <div className="overflow-hidden rounded-md border border-[var(--border-subtle)] bg-white">
        <dl className="m-0 divide-y divide-[var(--border-subtle)]">
          {rows.map((row) => (
            <div key={row.label} className="grid grid-cols-[minmax(120px,220px)_1fr] gap-4 px-4 py-3 max-[760px]:grid-cols-1 max-[760px]:gap-1">
              <dt className="text-sm font-medium text-muted-foreground">{row.label}</dt>
              <dd className="m-0 min-w-0 break-words font-mono text-sm text-foreground">{row.value}</dd>
            </div>
          ))}
        </dl>
      </div>
    </div>
  );
}
```

- [ ] **Step 5: Implement the settings takeover page**

Create `crates/noema-core/web/src/pages/SettingsPage.tsx`:

```tsx
import { X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { ProvidersSettingsPane } from "@/components/settings/ProvidersSettingsPane";
import { SettingsSidebar } from "@/components/settings/SettingsSidebar";

export function SettingsPage({ onClose }: { onClose: () => void }) {
  return (
    <main
      data-slot="settings-page"
      className="grid h-dvh min-h-screen grid-cols-[240px_minmax(0,1fr)] overflow-hidden bg-background text-foreground max-[760px]:grid-cols-1 max-[760px]:grid-rows-[auto_minmax(0,1fr)]"
    >
      <SettingsSidebar />
      <section className="grid min-h-0 grid-rows-[auto_minmax(0,1fr)] overflow-hidden">
        <header className="flex items-center justify-between gap-3 border-b border-[var(--border-subtle)] bg-white/95 px-5 py-3">
          <div className="min-w-0">
            <p className="m-0 font-mono text-[11px] tracking-[0.12em] text-[var(--text-accent)] uppercase">
              Providers
            </p>
            <strong className="block truncate font-heading text-base tracking-normal">
              Provider account
            </strong>
          </div>
          <Button type="button" variant="ghost" size="icon" aria-label="Close settings" onClick={onClose}>
            <X className="size-4" aria-hidden="true" />
          </Button>
        </header>
        <div className="min-h-0 overflow-auto px-6 py-6 max-[760px]:px-5">
          <div className="grid max-w-3xl gap-5">
            <div className="grid gap-2">
              <h2 className="m-0 font-heading text-[32px] leading-[1.1] tracking-normal text-foreground">
                Providers
              </h2>
              <p className="m-0 max-w-[620px] text-sm text-muted-foreground">
                Review the provider account Noema uses for chat. Secret credential material stays outside the UI.
              </p>
            </div>
            <ProvidersSettingsPane />
          </div>
        </div>
      </section>
    </main>
  );
}
```

- [ ] **Step 6: Run settings component tests**

Run:

```bash
cd crates/noema-core/web
bun test src/pages/SettingsPage.test.tsx src/components/settings/providerMetadata.test.ts
```

Expected: PASS.

- [ ] **Step 7: Commit settings components**

Run:

```bash
git add crates/noema-core/web/src/components/settings crates/noema-core/web/src/pages/SettingsPage.tsx crates/noema-core/web/src/pages/SettingsPage.test.tsx
git commit -m "feat: add settings providers page"
```

## Task 7: App Integration And Onboarding Gate

**Files:**
- Modify: `crates/noema-core/web/src/App.tsx`
- Modify: `crates/noema-core/web/src/App.test.ts`

- [ ] **Step 1: Write failing app helper tests**

In `crates/noema-core/web/src/App.test.ts`, import the new helpers:

```ts
import {
  canSendMessage,
  shouldRefreshLocalStatusForConversationEvent,
  shouldRenderSettingsRoute
} from "./App";
```

Add this test:

```ts
describe("shouldRenderSettingsRoute", () => {
  test("renders settings only after onboarding", () => {
    assert.equal(
      shouldRenderSettingsRoute({
        route: { kind: "settings", section: "providers" },
        onboarded: true
      }),
      true
    );
    assert.equal(
      shouldRenderSettingsRoute({
        route: { kind: "settings", section: "providers" },
        onboarded: false
      }),
      false
    );
    assert.equal(
      shouldRenderSettingsRoute({
        route: { kind: "memory_home" },
        onboarded: true
      }),
      false
    );
  });
});
```

- [ ] **Step 2: Run app tests to verify failure**

Run:

```bash
cd crates/noema-core/web
bun test src/App.test.ts
```

Expected: FAIL because `shouldRenderSettingsRoute` does not exist.

- [ ] **Step 3: Implement app integration**

In `crates/noema-core/web/src/App.tsx`, import `SettingsPage` and `AppRoute`:

```ts
import { SettingsPage } from "./pages/SettingsPage";
import { useBrowserRoute, type AppRoute } from "./routes";
```

Add this exported helper near the other pure helpers:

```ts
export function shouldRenderSettingsRoute({
  route,
  onboarded
}: {
  route: AppRoute;
  onboarded: boolean;
}) {
  return onboarded && route.kind === "settings";
}
```

Update the route hook destructuring:

```ts
  const { route, navigate, closeSettings } = useBrowserRoute();
```

After the `if (!onboarding.isUserOnboarded)` block and before memory route branches, add:

```tsx
  if (shouldRenderSettingsRoute({ route, onboarded })) {
    return <SettingsPage onClose={closeSettings} />;
  }
```

Keep all existing onboarding checks before this block so unauthenticated users never reach Settings.

- [ ] **Step 4: Run app tests**

Run:

```bash
cd crates/noema-core/web
bun test src/App.test.ts src/routes.test.ts src/components/shell/AppShell.test.ts src/pages/SettingsPage.test.tsx
```

Expected: PASS.

- [ ] **Step 5: Commit app integration**

Run:

```bash
git add crates/noema-core/web/src/App.tsx crates/noema-core/web/src/App.test.ts
git commit -m "feat: render settings outside app shell"
```

## Task 8: Frontend IA Documentation

**Files:**
- Modify: `docs/frontend/navigation-workflows.md`
- Modify: `docs/frontend/current-contract.md`
- Modify: `docs/context/current.md`

- [ ] **Step 1: Update navigation workflow docs**

In `docs/frontend/navigation-workflows.md`, replace the current shell behavior paragraph with:

```md
Current shell behavior: the sidebar is a persistent navigation ground layer,
and route content sits above it as the active deck. Expanded desktop keeps the
ground-layer sidebar visible; collapsed desktop and mobile move the deck aside
to reveal the same single sidebar instance. Healthy setup, service, chat,
provider, and memory state are silent in the shell. The shell shows a single
compact attention item only when state is degraded or action-worthy; the active
page owns detailed recovery UI. A bottom-left cog opens Settings as a
temporary full-screen takeover rather than a primary navigation destination.
```

- [ ] **Step 2: Update frontend contract route matrix**

In `docs/frontend/current-contract.md`, replace the `/settings` row with:

```md
| `/settings` | Settings | provider account metadata from GraphQL, gated by onboarding | temporary full-screen utility takeover opened from the sidebar cog; initial section is Providers and shows only non-secret account metadata | Current |
```

Replace the current shell note with:

```md
Current shell note: after onboarding, the web UI uses a sidebar-first shell.
`Home` is the visible label for the durable primary conversation, not a
dashboard. `Memory` is the only other primary sidebar destination in the
current slice. A bottom-left cog opens Settings as a secondary utility
takeover. Unknown browser paths fall back to `Home`.
```

- [ ] **Step 3: Update durable current context**

In `docs/context/current.md`, add this settled decision near the existing web shell bullets:

```md
- The web shell exposes Settings as a bottom-left sidebar cog, not as a primary
  navigation destination. `/settings` renders as a full-screen utility takeover
  after onboarding and initially contains only the Providers section with
  non-secret provider account metadata.
```

- [ ] **Step 4: Run doc diff check**

Run:

```bash
git diff -- docs/frontend/navigation-workflows.md docs/frontend/current-contract.md docs/context/current.md
```

Expected: the diff only documents Settings as a utility takeover and provider metadata as non-secret.

- [ ] **Step 5: Commit documentation updates**

Run:

```bash
git add docs/frontend/navigation-workflows.md docs/frontend/current-contract.md docs/context/current.md
git commit -m "docs: document settings providers surface"
```

## Task 9: Full Validation And Final Commit Check

**Files:**
- No new source files beyond prior tasks.

- [ ] **Step 1: Run frontend typegen, lint, and build**

Run:

```bash
cd crates/noema-core/web
bun run gen:types
bun run lint
bun run build
```

Expected: all commands PASS.

- [ ] **Step 2: Run backend validation for GraphQL changes**

Run:

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

Expected: all commands PASS. If tests fail only because local socket binding is denied by sandboxing, rerun the same failing test command with escalated socket permissions and report the distinction.

- [ ] **Step 3: Run final ship checklist commands**

Run:

```bash
git status --short --branch
git diff --check
git diff --cached --stat
git diff --cached --name-status
```

Expected: working tree contains only intentional committed changes, `git diff --check` reports no whitespace errors, and there are no staged files after the task commits.

- [ ] **Step 4: Manually inspect route behavior in code**

Check these source facts:

```text
crates/noema-core/web/src/components/shell/AppShell.tsx still defines primary shell destinations as Home and Memory only.
crates/noema-core/web/src/components/shell/ShellSidebar.tsx renders one settings button with aria-label "Open settings".
crates/noema-core/web/src/App.tsx renders SettingsPage only after onboarding succeeds.
crates/noema-core/src/graphql/provider_accounts.rs does not expose provider_account_id in GraphqlProviderAccount.
```

Expected: all facts are true.

- [ ] **Step 5: Final status report**

Run:

```bash
git status --short --branch
```

Expected: clean working tree. Report branch ahead count, validation commands run, and any remaining untracked or unstaged files.
