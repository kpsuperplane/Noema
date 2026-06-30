# Route-Derived Shell Settings Design

## Status

Approved design for moving Settings back into the main app shell while making
the sidebar robust enough to support generic route-derived L0 to L1 navigation.
No implementation has been done in this spec. The next step is an
implementation plan.

## Context

Noema's current web shell uses a layered sidebar deck. Home and Memory render
inside the shell, while Settings renders as a full-screen utility takeover
outside `AppShell`. Settings currently owns its own sidebar for Providers,
Agents, MCPs, Trusted identities, Approvals, and Audit.

The desired direction is to make Settings feel like part of the same product
shell again. The settings cog should become a full bottom-anchored sidebar
menu item. Selecting it should transition the sidebar from the L0 app menu into
an L1 Settings submenu. Settings content should render in the same main deck
where Home chat and Memory render today.

This should not be implemented as a Settings-only branch. The shell should gain
a generic route-derived menu-level construct so future L1 menus can use the
same model.

## Goals

- Keep `/settings/...` as real, deep-linkable browser routes.
- Render Settings inside `AppShell`, not as a full-screen takeover.
- Turn the bottom settings cog into a full-width bottom sidebar menu item.
- Support a generic route-derived L0 to L1 sidebar navigation model.
- Make route state the source of truth for the active menu level and active
  leaf item.
- Animate L0 and L1 with a single-slot sidebar slide: L0 exits left, L1 enters
  from the right.
- Show Settings content in the existing main shell deck.
- Show a quiet breadcrumb in the deck header, for example `Settings / MCPs`.
- Preserve the existing Tauri desktop header offset that makes room for window
  controls.
- Preserve existing Settings pane GraphQL behavior and inline error handling.

## Non-Goals

- Do not add backend state, GraphQL fields, mutations, or subscriptions.
- Do not change provider, agent, MCP, approval, audit, or trusted identity data
  models.
- Do not build a persistent user preference system for sidebar state.
- Do not add new Settings sections.
- Do not add a general multi-level navigation stack beyond the route-derived
  L0 to L1 model.
- Do not redesign Home, Memory, chat transcript, memory graph, or onboarding.
- Do not inspect frontend behavior with browser tools unless explicitly
  requested during implementation.

## Product Decisions

- The URL route is the semantic source of truth.
- Local shell state may track animation and collapsed/mobile reveal state, but
  not semantic navigation level.
- The L0 sidebar contains Home, Memory, and a bottom Settings item.
- The Settings L1 sidebar contains Providers, Agents, MCPs, Trusted identities,
  Approvals, Audit, and a bottom Go back item.
- Only one sidebar level is visible and interactive at a time.
- The L0 Settings item and L1 Go back item occupy the same bottom sidebar
  position.
- `/settings` and `/settings/providers` open Settings L1 with Providers
  selected.
- `/settings/<section>` opens Settings L1 with the matching section selected,
  including after refresh or direct deep link.
- On collapsed desktop and mobile, selecting Settings keeps the sidebar
  revealed and waits for the user to select a leaf Settings item.
- Selecting a leaf Settings item closes the collapsed/mobile sidebar reveal.
- Go back uses browser history when the user entered Settings from another app
  route and falls back to Home on direct Settings loads.
- The deck header uses a quiet breadcrumb treatment: muted parent label, subtle
  separator, and stronger active leaf label.

## Architecture

`App` should render every onboarded route through `AppShell`. The current
Settings route branch that bypasses `AppShell` should be retired.

The shell should derive its navigation model from the active `AppRoute`:

```text
route
  -> active shell level: l0 | settings
  -> active sidebar item
  -> deck breadcrumb
  -> item behavior after click
```

This derived model should live in focused pure helper code, for example
`components/shell/shellNavigation.ts`. The helper should define small
structures such as:

- `ShellMenuLevel`
- `ShellMenuItem`
- `ShellBreadcrumb`
- `ShellMenuItemAction`
- route-to-level and route-to-breadcrumb functions

The model should stay deliberately small. It should support the current L0 and
Settings L1 needs without becoming a full router or arbitrary stack machine.

`deckNavigation.ts` can continue to own visual deck reveal behavior:

- mobile/collapsed sidebar reveal
- desktop sidebar collapse
- route-content surface visibility during deck movement
- reduced-motion behavior

It should not own whether Settings L1 is semantically active. That comes from
the route.

## Component Design

### `App`

Responsibilities:

- Keep onboarding and setup outside the product shell.
- Render onboarded Home, Memory, Memory Graph, and Settings routes through
  `AppShell`.
- Provide navigation callbacks, including history-aware Settings back behavior.
- Keep chat subscription and message behavior unchanged.

Settings route rendering should become:

```text
AppShell
  SettingsSurface(section)
```

### `AppShell`

Responsibilities:

- Derive the active shell navigation model from `route`.
- Render the persistent sidebar ground layer.
- Render one animated sidebar menu slot.
- Render the content deck for the current route.
- Render the quiet breadcrumb in the deck header.
- Preserve the existing Tauri desktop chrome offset behavior for header
  positioning.
- Decide whether a sidebar item click should keep or close the
  collapsed/mobile reveal.

The existing deck header transform that makes room for Tauri window controls
must remain intact. The breadcrumb should be inserted into that existing header
structure rather than replacing the offset logic.

### `ShellSidebar`

Responsibilities:

- Render a provided `ShellMenuLevel`.
- Render the attention item when the active level supports it.
- Render top menu items and one bottom menu item.
- Mark the active route item with `aria-current="page"`.
- Keep bottom item layout stable so L0 Settings and L1 Go back align.
- Render icons and labels consistently across L0 and L1.

`ShellSidebar` should not hard-code Settings-specific behavior. It should call
the item callback with enough metadata for `AppShell` to decide whether to
close the sidebar reveal.

### `SettingsSurface`

Responsibilities:

- Replace or refactor `SettingsPage` into a shell-contained settings content
  surface.
- Render section title/copy and the active section pane.
- Stop rendering its own full-screen layout and local settings sidebar.
- Keep existing section pane components and data behavior.

The current section pane components can remain:

- `ProvidersSettingsPane`
- `AgentsSettingsPane`
- `McpSettingsPane`
- `TrustedIdentitiesSettingsPane`
- `ApprovalsSettingsPane`
- `AuditSettingsPane`

## Route And Browser History

Routes remain:

```text
/settings
/settings/providers
/settings/agents
/settings/mcps
/settings/trusted-identities
/settings/approvals
/settings/audit
```

`/settings` and `/settings/providers` both map to Providers.

The route helper should continue remembering the most recent non-settings app
route. A history-aware Go back action should:

1. Use browser history when there is a prior non-settings app route from the
   current session.
2. Fall back to Home when the user directly loads a Settings URL.

The fallback must not rely on a local semantic navigation stack. It can use the
existing previous-app-route reference pattern, browser history metadata, or an
equivalent route helper.

## Interaction Details

### Expanded Desktop

- The sidebar ground layer remains visible.
- Clicking the bottom Settings item updates the route to `/settings/providers`.
- The sidebar menu slot animates L0 left and Settings L1 in from the right.
- The main deck renders Providers settings content.
- Selecting another Settings leaf changes the route and content without
  leaving the Settings L1 menu.
- Go back exits Settings and reverses the menu transition.

### Collapsed Desktop And Mobile

- Opening navigation reveals the sidebar ground layer as it does today.
- Tapping Settings keeps the sidebar revealed.
- The sidebar menu slot transitions from L0 to Settings L1 while the deck stays
  aside.
- The user selects a Settings leaf item.
- Leaf selection navigates to the matching route and closes the sidebar reveal.
- Go back exits Settings and closes the collapsed/mobile sidebar reveal after
  navigating to the previous non-settings route or Home fallback.

### Reduced Motion

Reduced-motion users should receive a shortened, simplified, or immediate menu
level swap. Route semantics and focus behavior should remain the same.

## Header Breadcrumb

Home and Memory routes may keep their current single label behavior.

Settings routes should show a quiet breadcrumb in the deck header:

```text
Settings / MCPs
```

The parent label should be muted, the separator subtle, and the active leaf
label stronger. This should not become a pill UI or a second tab bar. It should
fit inside the existing deck header height and preserve truncation behavior on
small screens.

The breadcrumb must live inside the existing header offset wrapper so Tauri
window controls keep their current clearance.

## Accessibility

- The sidebar region keeps a stable id and accessible label.
- The menu trigger keeps `aria-expanded` and `aria-controls`.
- Current leaf items use `aria-current="page"`.
- The active Settings section should be announced as the current page within
  the Settings L1 menu.
- The bottom Settings and Go back items need clear accessible names.
- Escape should continue closing a revealed collapsed/mobile sidebar.
- Pointer interaction with route content should remain blocked while the
  sidebar reveal is open.
- Focus should return predictably to the menu trigger after backdrop or Escape
  close.
- L0 to L1 animation should not leave offscreen items focusable.

## Error Handling

Settings pane query and mutation errors remain inside their existing panes.
The shell should not turn a Settings content error into a global attention item
unless a future product decision explicitly adds that behavior.

Onboarding remains the access gate. If the user opens a Settings URL before
onboarding is complete, the app should show the same setup/onboarding flow it
would show for any onboarded route.

Unknown Settings subpaths should continue falling back through the existing
route parser behavior.

## Testing

Frontend tests should cover the pure shell navigation model:

- L0 routes derive the L0 menu level.
- Settings routes derive the Settings L1 menu level.
- `/settings` and `/settings/providers` select Providers.
- Each Settings route selects its matching Settings leaf.
- Settings routes produce quiet breadcrumbs.
- Home and Memory routes keep expected labels.
- Settings item click keeps the collapsed/mobile reveal open.
- Settings leaf item click closes the collapsed/mobile reveal.
- Go back uses previous non-settings route when available.
- Go back falls back to Home on direct Settings loads.

Component tests should cover:

- Settings renders inside `AppShell`.
- The old full-screen Settings takeover is gone.
- The sidebar renders one visible menu level at a time.
- L0 Settings and L1 Go back occupy the same bottom item position by component
  structure.
- The quiet Settings breadcrumb appears in the deck header.
- The Tauri desktop header offset class/structure remains applied around the
  breadcrumb.
- Existing attention item behavior remains unchanged for L0 shell states.
- Existing Settings pane loading, error, empty, and success states still render
  inside the new content surface.

No backend tests are required unless implementation unexpectedly changes Rust,
GraphQL schema, or store code.

## Validation

For frontend-only implementation, run from `crates/noema-core/web`:

```bash
bun run lint
bun run build
```

Run targeted frontend tests for routes, shell navigation, AppShell, and
Settings panes. Do not run browser-based UI inspection unless explicitly
requested.

Before committing implementation work, run:

```bash
git status --short --branch
git diff --check
git diff --cached --stat
git diff --cached --name-status
```

## Documentation Updates

Implementation should update durable docs so they state:

- Settings renders inside the main shell.
- The shell supports route-derived L0 to L1 navigation.
- Settings is the first L1 menu.
- Settings routes are deep-linkable and restore the Settings submenu after
  refresh.
- The Settings content surface uses the main shell deck rather than a
  full-screen takeover.
