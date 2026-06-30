# Settings Agents Tab Design

## Status

Approved design for adding a read-only Agents tab to the existing Settings
takeover. No implementation has been done in this spec. The next step is an
implementation plan.

## Context

Noema's web shell now exposes Settings as a bottom-left sidebar cog. Settings
renders as a full-screen utility takeover after onboarding and currently has a
Providers section backed by a dedicated GraphQL provider-account read model.

Agents already exist as durable rows in the embedded store. The current store
supports creating an agent, reading one agent by id, and updating an agent's
display name. The current frontend can read the primary agent display name
through `localStatus`, but there is no Settings-facing agent list API yet.

## Goals

- Add a second Settings tab called `Agents`.
- Keep the Agents tab read-only in this slice.
- Show the agents currently in the system, starting with the primary agent.
- Display each agent's user-facing name when present and a clear fallback when
  unnamed.
- Show safe technical metadata, including the agent id.
- Keep Settings gated behind onboarding.
- Preserve Providers as the default Settings section.

## Non-Goals

- Do not add create, rename, delete, enable/disable, policy, model, prompt, or
  tool-access controls.
- Do not expose agent memory internals, runtime state, prompts, provider
  credentials, tool grants, or conversation history in this tab.
- Do not add a primary shell navigation item for Agents.
- Do not build an agent detail page in this slice.
- Do not add compatibility layers or migrations.

## Product Decisions

- Settings has two tabs in this slice: `Providers` and `Agents`.
- `/settings` continues to default to Providers.
- `/settings/agents` opens the Agents tab directly.
- The Agents tab is an inspection surface, not a management surface.
- The primary agent appears first and receives a `Primary` badge.
- Agents without a display name render as `Unnamed agent`.
- The agent id is shown as technical metadata because this is a local
  owner/admin utility surface.

## Architecture

Add a dedicated GraphQL read model for agents rather than stretching
`localStatus`. The read model should be small and settings-specific:

```text
agents {
  agentId
  displayName
  isPrimary
}
```

The store should gain a focused `list_agents` method over the existing
`agents` table. It should return `AgentRecord` values sorted predictably:

1. `agent:primary`
2. remaining agents by display name when present, then by agent id

The GraphQL resolver converts store records into `GraphqlAgent` objects and
derives `isPrimary` from `agent_id == "agent:primary"`.

The frontend route model should extend the Settings route section:

```text
{ kind: "settings", section: "providers" | "agents" }
```

Route behavior:

- `/settings` maps to `{ kind: "settings", section: "providers" }`.
- `/settings/agents` maps to `{ kind: "settings", section: "agents" }`.
- Unknown Settings subpaths continue to fall back to Home.

`SettingsPage` owns the active section and switches between the existing
Providers pane and the new Agents pane. `SettingsSidebar` renders both tabs and
marks the active tab with `aria-current="page"`.

## Components

### `SettingsSidebar`

Responsibilities:

- Render the Settings title.
- Render `Providers` and `Agents` as local settings tabs.
- Mark the active section.
- Navigate through the existing browser route helper.
- Keep tabs visually secondary to the app shell's Home and Memory navigation.

### `AgentsSettingsPane`

Responsibilities:

- Load the read-only `agents` GraphQL query.
- Render loading, error, empty, and success states.
- Render agent rows/cards using existing Noema visual language.
- Show an agent display name or `Unnamed agent`.
- Show `Primary` for the primary agent.
- Show agent id as technical metadata.

### Agent Metadata Helpers

A small helper may format:

- display label
- primary badge state
- predictable row metadata

This keeps rendering components simple and makes the unnamed/primary cases easy
to test.

## UI Behavior

The Agents tab should show a compact header explaining that agents are
read-only for now, followed by a scannable list.

For each agent:

- title: display name, or `Unnamed agent`
- badge: `Primary` when `isPrimary` is true
- metadata: `Agent id`

If the query is loading, show a small pane-level loading state. If the query
fails, show an inline error with retry when the data layer supports retry. If
the list is empty, show a bounded inconsistency message because bootstrap
normally creates the primary agent.

## Error And Access Rules

- Unauthenticated users remain in onboarding and cannot access Settings or the
  Agents tab.
- If agent metadata fails to load after onboarding, show an inline Settings
  error rather than falling back to onboarding.
- Agent ids may be shown in this local Settings tab, but no prompt, memory,
  runtime status, provider credential, or conversation data should be included.

## Testing

Store tests should cover:

- `list_agents` returns the primary agent and additional agents.
- The primary agent sorts first.
- Other agents have deterministic ordering.

GraphQL tests should cover:

- `agents` exposes `agentId`, `displayName`, and `isPrimary`.
- The primary agent has `isPrimary: true`.
- The query does not expose runtime state, prompts, memory, provider
  credentials, or conversations.

Frontend tests should cover:

- `/settings` maps to Providers.
- `/settings/agents` maps to Agents.
- `pathForRoute` returns `/settings/agents` for the Agents section.
- Settings sidebar renders Providers and Agents.
- Settings sidebar marks the active tab.
- Agents pane renders named and unnamed agents.
- Agents pane shows `Primary` for the primary agent.
- Agents pane shows loading, error, and empty states.
- Settings remains onboarding-gated.

## Validation

Run from `crates/noema-core/web` for frontend changes:

```bash
bun run gen:types
bun run lint
bun run build
```

Run Rust validation because this slice adds store and GraphQL code:

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

Before committing implementation work, run:

```bash
git status --short --branch
git diff --check
git diff --cached --stat
git diff --cached --name-status
```

## Documentation Updates

Update frontend IA docs during implementation so they state:

- Settings contains Providers and Agents.
- Agents is a read-only Settings tab in this slice.
- `/settings` defaults to Providers.
- `/settings/agents` opens the Agents tab.
- Agent management controls are intentionally out of scope for this slice.
