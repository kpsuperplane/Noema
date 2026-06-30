# Settings Agents Tab Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a read-only `Agents` tab to the full-screen Settings takeover that lists the agents in the system, starting with the primary agent, and shows safe technical metadata.

**Architecture:** Add a focused store list method and GraphQL `agents` read model over existing `AgentRecord` data. Extend the Settings route from a single Providers page to section-aware Settings routing, then render `Providers` and `Agents` as local Settings tabs while keeping Settings onboarding-gated.

**Tech Stack:** Rust, async-graphql, SurrealDB store APIs, React, Apollo Client, TypeScript, Bun, lucide-react, existing shadcn/base UI primitives.

---

## File Structure

- Modify `crates/noema-core/src/store/agents.rs`
  - Adds `NoemaStore::list_agents`.
  - Sorts `agent:primary` first, then named agents by display name, then unnamed agents by id.
- Modify `crates/noema-core/src/store/tests.rs`
  - Covers agent listing and deterministic ordering.
- Create `crates/noema-core/src/graphql/agents.rs`
  - Owns the safe agent GraphQL object and resolver.
  - Converts `AgentRecord` into `agentId`, `displayName`, and `isPrimary`.
- Modify `crates/noema-core/src/graphql.rs`
  - Registers the new GraphQL module.
- Modify `crates/noema-core/src/graphql/schema.rs`
  - Exposes `agents` on `QueryRoot`.
  - Adds resolver and SDL tests for the read model.
- Modify `crates/noema-core/web/src/graphql/operations.ts`
  - Adds the frontend `Agents` query.
- Regenerate `crates/noema-core/web/src/generated/schema.graphql` and `crates/noema-core/web/src/generated/graphql.ts`
  - Produced by `bun run gen:types`; do not hand-edit generated files.
- Modify `crates/noema-core/web/src/routes.ts`
  - Extends Settings routing to `providers | agents`.
  - Maps `/settings` to Providers and `/settings/agents` to Agents.
- Modify `crates/noema-core/web/src/routes.test.ts`
  - Covers Settings section parsing and canonical paths.
- Modify `crates/noema-core/web/src/components/settings/SettingsSidebar.tsx`
  - Renders Providers and Agents tabs.
  - Marks the active tab and navigates within Settings.
- Create `crates/noema-core/web/src/components/settings/agentMetadata.ts`
  - Formats agent labels, primary state, and metadata rows.
- Create `crates/noema-core/web/src/components/settings/agentMetadata.test.ts`
  - Covers unnamed display fallback and safe metadata output.
- Create `crates/noema-core/web/src/components/settings/AgentsSettingsPane.tsx`
  - Loads the `Agents` GraphQL query.
  - Passes query state into the content component.
- Create `crates/noema-core/web/src/components/settings/AgentsSettingsPaneContent.tsx`
  - Renders loading, error, empty, and success states.
- Modify `crates/noema-core/web/src/pages/SettingsPage.tsx`
  - Switches between Providers and Agents content.
  - Passes active section and navigation into `SettingsSidebar`.
- Modify `crates/noema-core/web/src/pages/SettingsPage.test.tsx`
  - Covers section switching and Agents pane rendering.
- Modify `crates/noema-core/web/src/App.tsx`
  - Passes the active Settings section and route navigation into `SettingsPage`.
  - Keeps Settings inaccessible until onboarding has completed.
- Modify `crates/noema-core/web/src/App.test.ts`
  - Extends existing route access helper coverage if Settings section gating is tested there.
- Modify `docs/frontend/navigation-workflows.md`
  - Documents Providers and Agents as Settings tabs.
- Modify `docs/frontend/current-contract.md`
  - Documents `/settings`, `/settings/agents`, and read-only Agents scope.
- Modify `docs/context/current.md`
  - Adds the settled decision that Settings now includes read-only Providers and Agents tabs.

## Task 1: Store Agent Listing

**Files:**
- Modify `crates/noema-core/src/store/agents.rs`
- Modify `crates/noema-core/src/store/tests.rs`

- [ ] **Step 1: Add failing store tests**

Add tests near the existing agent store coverage:

```rust
#[tokio::test]
async fn list_agents_returns_primary_first() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("default actors");
    store
        .create_agent(NewAgent {
            agent_id: "agent:zeta".to_string(),
            display_name: Some("Zeta".to_string()),
        })
        .await
        .expect("zeta agent");
    store
        .create_agent(NewAgent {
            agent_id: "agent:alpha".to_string(),
            display_name: Some("Alpha".to_string()),
        })
        .await
        .expect("alpha agent");

    let agents = store.list_agents().await.expect("agents");

    let ids = agents
        .iter()
        .map(|agent| agent.agent_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(ids, vec!["agent:primary", "agent:alpha", "agent:zeta"]);
}

#[tokio::test]
async fn list_agents_sorts_unnamed_agents_by_id_after_named_agents() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("default actors");
    store
        .create_agent(NewAgent {
            agent_id: "agent:unnamed-b".to_string(),
            display_name: None,
        })
        .await
        .expect("unnamed b");
    store
        .create_agent(NewAgent {
            agent_id: "agent:named".to_string(),
            display_name: Some("Named".to_string()),
        })
        .await
        .expect("named");
    store
        .create_agent(NewAgent {
            agent_id: "agent:unnamed-a".to_string(),
            display_name: None,
        })
        .await
        .expect("unnamed a");

    let agents = store.list_agents().await.expect("agents");

    let ids = agents
        .iter()
        .map(|agent| agent.agent_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        ids,
        vec![
            "agent:primary",
            "agent:named",
            "agent:unnamed-a",
            "agent:unnamed-b"
        ]
    );
}
```

- [ ] **Step 2: Run focused store tests to verify failure**

```bash
cargo test -p noema-core store::tests::list_agents --lib
```

Expected: FAIL because `NoemaStore::list_agents` does not exist yet.

- [ ] **Step 3: Implement `NoemaStore::list_agents`**

Add this method to `crates/noema-core/src/store/agents.rs`:

```rust
pub async fn list_agents(&self) -> Result<Vec<AgentRecord>> {
    let mut response = self
        .db
        .query(
            r#"
            SELECT
                agent_id,
                display_name
            FROM agents
            "#,
        )
        .await?;
    let mut agents: Vec<AgentRecord> = response.take(0)?;
    agents.sort_by(|left, right| {
        let left_primary = left.agent_id == "agent:primary";
        let right_primary = right.agent_id == "agent:primary";

        right_primary
            .cmp(&left_primary)
            .then_with(|| left.display_name.is_none().cmp(&right.display_name.is_none()))
            .then_with(|| left.display_name.cmp(&right.display_name))
            .then_with(|| left.agent_id.cmp(&right.agent_id))
    });
    Ok(agents)
}
```

- [ ] **Step 4: Verify focused store tests pass**

```bash
cargo test -p noema-core store::tests::list_agents --lib
```

## Task 2: Backend Agents GraphQL Read Model

**Files:**
- Create `crates/noema-core/src/graphql/agents.rs`
- Modify `crates/noema-core/src/graphql.rs`
- Modify `crates/noema-core/src/graphql/schema.rs`

- [ ] **Step 1: Add failing GraphQL tests**

Add a resolver test in `crates/noema-core/src/graphql/schema.rs`:

```rust
#[tokio::test]
async fn agents_query_returns_safe_agent_metadata() {
    use crate::{NewAgent, store::tests::test_store};

    let store = test_store().await;
    store.ensure_default_actors().await.expect("default actors");
    store
        .create_agent(NewAgent {
            agent_id: "agent:unnamed".to_string(),
            display_name: None,
        })
        .await
        .expect("unnamed agent");

    let schema = build_schema(GraphqlState::for_tests_with_store(store));
    let response = schema
        .execute(async_graphql::Request::new(
            r#"
            {
              agents {
                agentId
                displayName
                isPrimary
              }
            }
            "#,
        ))
        .await;

    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().expect("json");
    let agents = data["agents"].as_array().expect("agents array");
    assert_eq!(agents[0]["agentId"], "agent:primary");
    assert_eq!(agents[0]["displayName"], "Noema");
    assert_eq!(agents[0]["isPrimary"], true);
    assert_eq!(agents[1]["agentId"], "agent:unnamed");
    assert_eq!(agents[1]["displayName"], serde_json::Value::Null);
    assert_eq!(agents[1]["isPrimary"], false);

    let json_text = serde_json::to_string(&data).expect("agent json");
    assert!(!json_text.contains("prompt"));
    assert!(!json_text.contains("memory"));
    assert!(!json_text.contains("runtime"));
    assert!(!json_text.contains("credential"));
    assert!(!json_text.contains("conversation"));
}
```

Also extend the SDL test:

```rust
assert!(sdl.contains("agents"));
assert!(sdl.contains("type GraphqlAgent"));
assert!(sdl.contains("agentId"));
assert!(sdl.contains("displayName"));
assert!(sdl.contains("isPrimary"));
```

- [ ] **Step 2: Run focused GraphQL tests to verify failure**

```bash
cargo test -p noema-core graphql::schema::tests::agents_query_returns_safe_agent_metadata --lib
```

Expected: FAIL because the `agents` query does not exist yet.

- [ ] **Step 3: Add the GraphQL agents module**

Create `crates/noema-core/src/graphql/agents.rs`:

```rust
use async_graphql::{Result, SimpleObject};

use crate::AgentRecord;

use super::{errors::graphql_error, schema::GraphqlState};

#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlAgent {
    pub agent_id: String,
    pub display_name: Option<String>,
    pub is_primary: bool,
}

impl From<AgentRecord> for GraphqlAgent {
    fn from(agent: AgentRecord) -> Self {
        let is_primary = agent.agent_id == "agent:primary";
        Self {
            agent_id: agent.agent_id,
            display_name: agent.display_name,
            is_primary,
        }
    }
}

pub(super) async fn agents(state: &GraphqlState) -> Result<Vec<GraphqlAgent>> {
    let store = state.store()?;
    let agents = store.list_agents().await.map_err(graphql_error)?;
    Ok(agents.into_iter().map(Into::into).collect())
}
```

- [ ] **Step 4: Register the module**

In `crates/noema-core/src/graphql.rs`, add:

```rust
pub mod agents;
```

In `crates/noema-core/src/graphql/schema.rs`, import and expose the query:

```rust
use super::agents::{self, GraphqlAgent};
```

```rust
async fn agents(&self, ctx: &Context<'_>) -> Result<Vec<GraphqlAgent>> {
    agents::agents(ctx.data_unchecked::<GraphqlState>()).await
}
```

- [ ] **Step 5: Verify focused GraphQL tests pass**

```bash
cargo test -p noema-core graphql::schema::tests::agents_query_returns_safe_agent_metadata --lib
cargo test -p noema-core graphql::schema::tests::schema_sdl_exposes_initial_noema_fields --lib
```

## Task 3: Frontend GraphQL Operation And Generated Types

**Files:**
- Modify `crates/noema-core/web/src/graphql/operations.ts`
- Regenerate `crates/noema-core/web/src/generated/schema.graphql`
- Regenerate `crates/noema-core/web/src/generated/graphql.ts`

- [ ] **Step 1: Add the frontend query**

Add this operation to `crates/noema-core/web/src/graphql/operations.ts`:

```ts
export const AgentsDocument = gql`
  query Agents {
    agents {
      agentId
      displayName
      isPrimary
    }
  }
`;
```

- [ ] **Step 2: Regenerate frontend GraphQL artifacts**

```bash
cd crates/noema-core/web
bun run gen:types
```

- [ ] **Step 3: Inspect generated output**

Confirm `AgentsQuery`, `AgentsQueryVariables`, and the generated `AgentsDocument` exist in `crates/noema-core/web/src/generated/graphql.ts`.

## Task 4: Settings Section Routing

**Files:**
- Modify `crates/noema-core/web/src/routes.ts`
- Modify `crates/noema-core/web/src/routes.test.ts`

- [ ] **Step 1: Add failing route tests**

Extend route tests to cover:

```ts
expect(routeFromPathname("/settings")).toEqual({
  kind: "settings",
  section: "providers",
});
expect(routeFromPathname("/settings/agents")).toEqual({
  kind: "settings",
  section: "agents",
});
expect(pathForRoute({ kind: "settings", section: "providers" })).toBe("/settings");
expect(pathForRoute({ kind: "settings", section: "agents" })).toBe("/settings/agents");
expect(routeFromPathname("/settings/unknown")).toEqual({ kind: "home" });
```

- [ ] **Step 2: Extend the route model**

In `crates/noema-core/web/src/routes.ts`, add a shared section type:

```ts
export type SettingsSection = "providers" | "agents";
```

Update the Settings route variant:

```ts
| { kind: "settings"; section: SettingsSection }
```

Update route parsing:

```ts
if (pathname === "/settings") {
  return { kind: "settings", section: "providers" };
}
if (pathname === "/settings/agents") {
  return { kind: "settings", section: "agents" };
}
```

Update route path generation:

```ts
case "settings":
  return route.section === "agents" ? "/settings/agents" : "/settings";
```

- [ ] **Step 3: Update existing callers**

Replace existing `{ kind: "settings" }` construction with:

```ts
{ kind: "settings", section: "providers" }
```

Use this for the sidebar cog and any tests that open Settings from the app shell.

- [ ] **Step 4: Run route tests**

```bash
cd crates/noema-core/web
bun test src/routes.test.ts
```

## Task 5: Settings Sidebar Tabs

**Files:**
- Modify `crates/noema-core/web/src/components/settings/SettingsSidebar.tsx`
- Modify tests that render `SettingsSidebar` through `SettingsPage`

- [ ] **Step 1: Update the sidebar contract**

Change `SettingsSidebar` to receive:

```ts
import type { SettingsSection } from "@/routes";

type SettingsSidebarProps = {
  activeSection: SettingsSection;
  onSelectSection: (section: SettingsSection) => void;
};
```

- [ ] **Step 2: Render both tabs**

Render Providers and Agents as two local navigation buttons. Use `ServerCog` for Providers and `Bot` for Agents from `lucide-react`.

Each button should:

- set `type="button"`
- call `onSelectSection("providers")` or `onSelectSection("agents")`
- set `aria-current="page"` only when active
- preserve the existing selected/unselected visual language

- [ ] **Step 3: Keep the component compact**

If the selected/unselected class names become noisy, add a small helper such as:

```ts
function sectionButtonClass(isActive: boolean) {
  return cn(
    "flex h-9 items-center gap-2 rounded-md px-3 text-left text-sm font-medium tracking-normal",
    isActive
      ? "bg-accent text-accent-foreground"
      : "text-muted-foreground hover:bg-accent/60 hover:text-foreground",
  );
}
```

Use the existing `cn` helper if it is already available in the web app.

## Task 6: Agent Metadata Helpers

**Files:**
- Create `crates/noema-core/web/src/components/settings/agentMetadata.ts`
- Create `crates/noema-core/web/src/components/settings/agentMetadata.test.ts`

- [ ] **Step 1: Add helper tests**

Cover:

- named agent label returns the display name
- blank or missing display name returns `Unnamed agent`
- primary agents get a primary badge label
- metadata contains only `Agent id`

Example:

```ts
expect(agentDisplayName({ displayName: "Noema" })).toBe("Noema");
expect(agentDisplayName({ displayName: null })).toBe("Unnamed agent");
expect(agentDisplayName({ displayName: "   " })).toBe("Unnamed agent");
expect(agentMetadataRows({ agentId: "agent:primary" })).toEqual([
  { label: "Agent id", value: "agent:primary" },
]);
```

- [ ] **Step 2: Implement helpers**

Keep helper inputs narrow so they can accept generated GraphQL rows without importing generated types:

```ts
type AgentLike = {
  agentId: string;
  displayName?: string | null;
  isPrimary?: boolean;
};

export function agentDisplayName(agent: Pick<AgentLike, "displayName">) {
  const displayName = agent.displayName?.trim();
  return displayName ? displayName : "Unnamed agent";
}

export function agentBadgeLabel(agent: Pick<AgentLike, "isPrimary">) {
  return agent.isPrimary ? "Primary" : null;
}

export function agentMetadataRows(agent: Pick<AgentLike, "agentId">) {
  return [{ label: "Agent id", value: agent.agentId }];
}
```

## Task 7: Agents Settings Pane

**Files:**
- Create `crates/noema-core/web/src/components/settings/AgentsSettingsPane.tsx`
- Create `crates/noema-core/web/src/components/settings/AgentsSettingsPaneContent.tsx`
- Modify `crates/noema-core/web/src/pages/SettingsPage.test.tsx`

- [ ] **Step 1: Add content component tests**

In `SettingsPage.test.tsx`, add tests for `AgentsSettingsPaneContent` or a dedicated nearby test file if existing settings tests are already split:

- named agent renders its display name
- unnamed agent renders `Unnamed agent`
- primary agent renders `Primary`
- agent id metadata is visible
- loading state renders
- error state renders
- empty state renders

Use generated type-compatible objects:

```ts
const agents = [
  {
    __typename: "GraphqlAgent" as const,
    agentId: "agent:primary",
    displayName: "Noema",
    isPrimary: true,
  },
  {
    __typename: "GraphqlAgent" as const,
    agentId: "agent:unnamed",
    displayName: null,
    isPrimary: false,
  },
];
```

- [ ] **Step 2: Implement query wrapper**

In `AgentsSettingsPane.tsx`, use Apollo Client with the generated query:

```tsx
import { useQuery } from "@apollo/client";

import {
  AgentsDocument,
  type AgentsQuery,
} from "@/generated/graphql";

import { AgentsSettingsPaneContent } from "./AgentsSettingsPaneContent";

export function AgentsSettingsPane() {
  const { data, error, loading } = useQuery<AgentsQuery>(AgentsDocument);

  return (
    <AgentsSettingsPaneContent
      agents={data?.agents ?? []}
      error={error}
      loading={loading}
    />
  );
}
```

- [ ] **Step 3: Implement content states**

Render compact Settings-style content:

- loading: `Loading agents...`
- error: `Agent metadata could not be loaded.`
- empty: `No agents were found.`
- success: a vertical list of rows or cards

Each success item should show:

- display name or `Unnamed agent`
- `Primary` badge when `isPrimary`
- metadata row with `Agent id`

Use small, scannable typography consistent with `ProvidersSettingsPaneContent`. Keep cards at `rounded-md` or less.

## Task 8: Settings Page Section Switching

**Files:**
- Modify `crates/noema-core/web/src/pages/SettingsPage.tsx`
- Modify `crates/noema-core/web/src/App.tsx`
- Modify `crates/noema-core/web/src/pages/SettingsPage.test.tsx`
- Modify `crates/noema-core/web/src/App.test.ts` if route gating helper tests need updates

- [ ] **Step 1: Update `SettingsPage` props**

Use the route section as the source of truth:

```ts
import type { AppRoute, SettingsSection } from "@/routes";

type SettingsPageProps = {
  section: SettingsSection;
  onNavigate: (route: AppRoute) => void;
  onClose: () => void;
};
```

- [ ] **Step 2: Route local tab clicks through browser routing**

Inside `SettingsPage`, wire:

```ts
const handleSelectSection = (nextSection: SettingsSection) => {
  onNavigate({ kind: "settings", section: nextSection });
};
```

Pass `section` and `handleSelectSection` into `SettingsSidebar`.

- [ ] **Step 3: Switch title, description, and content**

Use section-specific copy:

Providers:

```text
Review the provider account Noema uses for chat. Secret credential material stays outside the UI.
```

Agents:

```text
Review the agents currently registered in Noema. This tab is read-only for now.
```

Render:

```tsx
{section === "agents" ? <AgentsSettingsPane /> : <ProvidersSettingsPane />}
```

- [ ] **Step 4: Pass route data from `App.tsx`**

When the current route is Settings, render:

```tsx
<SettingsPage
  section={currentRoute.section}
  onNavigate={navigate}
  onClose={closeSettings}
/>
```

Keep the existing onboarding gate so unauthenticated users remain in onboarding and never see Settings or `/settings/agents`.

- [ ] **Step 5: Add or update UI tests**

Cover:

- Providers remains the default for `/settings`
- Agents page title renders for `section="agents"`
- selecting the Agents tab calls route navigation with `{ kind: "settings", section: "agents" }`
- selecting the Providers tab from Agents calls route navigation with `{ kind: "settings", section: "providers" }`
- app route access helper still treats Settings as inaccessible before onboarding

Run:

```bash
cd crates/noema-core/web
bun test src/pages/SettingsPage.test.tsx src/routes.test.ts
```

## Task 9: Documentation And Durable Context

**Files:**
- Modify `docs/frontend/navigation-workflows.md`
- Modify `docs/frontend/current-contract.md`
- Modify `docs/context/current.md`

- [ ] **Step 1: Update frontend IA docs**

Document:

- Settings is a full-screen utility takeover.
- Settings contains `Providers` and `Agents`.
- `/settings` opens Providers.
- `/settings/agents` opens Agents.
- Agents is read-only in this slice.
- Agent ids are visible as safe local technical metadata.

- [ ] **Step 2: Update current project context**

Append a concise note that this implementation adds the read-only Agents Settings tab, backed by a dedicated `agents` GraphQL read model, with no agent management actions yet.

## Task 10: Full Validation And Commit

**Files:**
- All changed implementation and documentation files

- [ ] **Step 1: Run frontend validation**

```bash
cd crates/noema-core/web
bun run gen:types
bun run lint
bun run build
```

- [ ] **Step 2: Run Rust validation**

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

If a test fails because local socket binding is blocked by sandbox permissions, rerun the same test command with the required socket permissions and report the distinction.

- [ ] **Step 3: Run ship checks**

```bash
git status --short --branch
git diff --check
git diff --cached --stat
git diff --cached --name-status
```

- [ ] **Step 4: Commit the implementation**

Stage only files changed for this feature, inspect staged changes, then commit:

```bash
git add \
  crates/noema-core/src/store/agents.rs \
  crates/noema-core/src/store/tests.rs \
  crates/noema-core/src/graphql/agents.rs \
  crates/noema-core/src/graphql.rs \
  crates/noema-core/src/graphql/schema.rs \
  crates/noema-core/web/src/graphql/operations.ts \
  crates/noema-core/web/src/generated/schema.graphql \
  crates/noema-core/web/src/generated/graphql.ts \
  crates/noema-core/web/src/routes.ts \
  crates/noema-core/web/src/routes.test.ts \
  crates/noema-core/web/src/components/settings/SettingsSidebar.tsx \
  crates/noema-core/web/src/components/settings/agentMetadata.ts \
  crates/noema-core/web/src/components/settings/agentMetadata.test.ts \
  crates/noema-core/web/src/components/settings/AgentsSettingsPane.tsx \
  crates/noema-core/web/src/components/settings/AgentsSettingsPaneContent.tsx \
  crates/noema-core/web/src/pages/SettingsPage.tsx \
  crates/noema-core/web/src/pages/SettingsPage.test.tsx \
  crates/noema-core/web/src/App.tsx \
  crates/noema-core/web/src/App.test.ts \
  docs/frontend/navigation-workflows.md \
  docs/frontend/current-contract.md \
  docs/context/current.md
git diff --cached --stat
git diff --cached --name-status
git commit -m "feat: add settings agents tab"
```

## Rollback Plan

If implementation needs to be backed out before commit, remove the new `agents` GraphQL module, the new frontend Agents pane files, and the Settings route section changes, then regenerate GraphQL types from the previous operation set. If already committed, revert the single feature commit rather than partially removing files.

## Success Criteria

- `/settings` still opens Providers.
- `/settings/agents` opens Agents.
- Settings remains inaccessible before onboarding completes.
- Agents tab shows all stored agents with primary first.
- Unnamed agents display `Unnamed agent`.
- Primary agent displays a `Primary` badge.
- Agent id is visible; prompts, memory, runtime state, credentials, and conversations are not exposed.
- Frontend lint/build/type generation passes.
- Rust fmt/check/clippy/tests pass.
