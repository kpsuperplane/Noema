# Settings Providers Page Design

## Status

Approved design for adding a settings cog and an initial Providers settings
surface. No implementation has been done in this spec. The next step is an
implementation plan.

## Context

Noema's onboarded web shell currently has a layered sidebar deck. The sidebar
contains primary destinations for Home and Memory, while route content sits in
an elevated deck. Provider authentication is already a first-run gate: when the
active provider account is not authenticated, the user sees onboarding instead
of the product shell.

The current frontend can read provider readiness through onboarding status, but
settings should not depend on onboarding as its long-term provider-account API.
Provider credential material lives under `NOEMA_HOME/providers/...` and must
not be exposed through the UI.

## Goals

- Add a simple cog affordance to the bottom-left corner of the app sidebar.
- Open Settings as a temporary full-screen app takeover, not as another primary
  shell destination.
- Add an initial Settings sidebar with one tab: Providers.
- Show the current connected provider account and safe non-secret technical
  metadata.
- Keep Settings inaccessible while provider onboarding is incomplete.
- Preserve the current Home and Memory primary navigation model.

## Non-Goals

- Do not add provider connect, reconnect, logout, or credential-editing actions
  in this slice.
- Do not expose secret files, token values, API keys, credential paths, raw
  provider CLI output, or credential-home contents.
- Do not add additional settings tabs beyond Providers.
- Do not make Settings a primary sidebar destination.
- Do not add persisted frontend preferences for the sidebar or settings state.

## Product Decisions

- The sidebar cog is a utility control anchored at the bottom of the existing
  sidebar ground layer.
- Settings is available only after the app is onboarded.
- Clicking the cog routes to `/settings` and renders a full-screen Settings
  takeover outside `AppShell`.
- Settings hides the normal app shell while it is open.
- Settings has its own left sidebar and right content pane.
- The only initial settings tab is `Providers`.
- A close/back control returns the user to the previous app route when known,
  otherwise to Home.
- The Providers tab shows both a plain connected-provider summary and safe
  non-secret account metadata.

## Architecture

The route model should add a settings route, for example:

```text
{ kind: "settings", section: "providers" }
```

`/settings` maps to the Providers section. Future settings sections can extend
the route type without changing the initial Providers behavior.

`ShellSidebar` receives an explicit settings callback or route item for the
bottom utility button. The button should use a cog icon, an accessible label,
and the existing shell navigation callback so mobile/collapsed reveal state
closes after navigation.

`App` continues to enforce onboarding before rendering onboarded surfaces. If
onboarding is incomplete, Settings is not rendered. Once onboarded, `App`
renders the settings route directly:

```text
App
  SettingsPage
    SettingsSidebar
    ProvidersSettingsPane
```

Other onboarded routes continue to render through `AppShell`.

## Data Source

The preferred implementation is a dedicated GraphQL provider-account read model
for settings. It should return safe metadata from the existing provider account
registry/store, such as:

- provider account id for stable client keys, not rendered by default in the
  initial Providers UI
- provider kind
- account key
- display name
- auth method
- status
- active/default flags if already modeled
- last checked timestamp
- last authenticated timestamp
- last non-secret error code
- last non-secret error message

If implementation pressure favors a smaller first step, the frontend may reuse
the existing onboarding status fields for display name, provider kind, account
key, auth method, and status. That reuse should be treated as a temporary
bridge, not the long-term settings API.

The API must not return token values, API keys, credential file contents,
secret environment values, raw provider CLI output, or provider credential home
contents.

## Components

### Sidebar Settings Button

Responsibilities:

- Render an icon-only cog button at the bottom-left of the sidebar.
- Use an accessible label such as `Open settings`.
- Navigate to `/settings`.
- Stay visually separate from primary Home and Memory navigation.
- Work with the existing layered sidebar reveal behavior.

### `SettingsPage`

Responsibilities:

- Render a full-screen settings takeover.
- Hide the normal `AppShell` while active.
- Provide a local settings sidebar.
- Provide a close/back control.
- Render the active settings section.
- Keep layout usable on desktop and mobile.

### `SettingsSidebar`

Responsibilities:

- Render Settings as the local surface title.
- Render a single selected `Providers` tab.
- Reserve a straightforward pattern for future settings sections without
  showing disabled future entries.

### `ProvidersSettingsPane`

Responsibilities:

- Load provider account metadata.
- Render a compact connected provider summary.
- Render safe technical metadata in a scannable detail list.
- Show loading, error, and empty states.

## UI Behavior

The Providers tab should show:

- Display name, for example `Codex`.
- Provider status, using the same status vocabulary as onboarding.
- Provider kind.
- Account key.
- Auth method.
- Last checked and last authenticated timestamps when available.
- Last non-secret error code/message when present.

Loading should use a small content-pane loading state. Query failure should show
an inline settings error with a retry action if the data layer supports retry.
An empty provider list is a backend/setup inconsistency because Settings is
gated behind onboarding.

## Error And Access Rules

- Unauthenticated users remain in onboarding and cannot access Settings.
- If a user opens `/settings` while onboarding is incomplete, the app should
  render the same onboarding flow it would render for any other onboarded
  route.
- If provider metadata fails to load after onboarding, show an inline Settings
  error rather than falling back to onboarding.
- If multiple provider accounts exist later, the initial Providers pane can list
  all safe account summaries, but this slice only needs to handle the current
  active connected provider.

## Testing

Frontend tests should cover:

- `/settings` parses to the settings Providers route.
- `pathForRoute` returns `/settings` for the Providers settings route.
- The sidebar renders a bottom utility settings button separate from Home and
  Memory.
- Clicking the settings button navigates through the shell navigation callback.
- Settings renders outside `AppShell` after onboarding.
- Settings is not rendered while onboarding is incomplete.
- Providers metadata renders safe fields.
- Providers metadata does not render credential paths, token values, API keys,
  provider account ids, or raw credential content fields.

If a backend GraphQL read model is added, Rust tests should cover:

- The resolver returns only non-secret provider account metadata.
- Missing provider accounts return a bounded empty or error result.
- Existing provider-account status fields are mapped correctly.

## Validation

For frontend-only implementation, run from `crates/noema-core/web`:

```bash
bun run gen:types
bun run lint
bun run build
```

If Rust GraphQL/backend code changes, also run the default Rust validation:

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
```

## Documentation Updates

Update frontend IA docs during implementation so they state:

- Settings is a utility takeover opened from the sidebar cog.
- Settings is secondary to Home and Memory.
- The initial Settings section is Providers.
- Settings is gated behind provider onboarding.
- Provider settings expose only safe non-secret account metadata.
