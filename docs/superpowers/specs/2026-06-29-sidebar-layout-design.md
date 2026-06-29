# Sidebar Layout Design

## Status

Approved design for replacing the current top app header with a sidebar-first
web shell. No implementation has been done in this spec. The next step is an
implementation plan.

## Context

Noema's current React web UI is a chat-first shell with a top `AppHeader`.
Routes already exist for the durable primary conversation at `/` and `/chat`,
memory management at `/memory`, and the memory graph at `/memory/graph`.

The current frontend IA still treats chat as the primary surface and memory as
a secondary management surface. This design keeps that product stance while
making navigation clearer: the durable primary conversation is labeled `Home`,
and memory gets a first-class sidebar destination.

## Goals

- Replace the top header/navigation chrome with a sidebar-first app shell after
  onboarding.
- Label the durable primary conversation as `Home`.
- Show a `Memory` destination for the memory management page.
- Use a persistent left sidebar on desktop.
- Use a drawer on narrow screens.
- Keep healthy status out of the shell; show attention only when action is
  needed.
- Retire `AppHeader` from the onboarded app.
- Keep the change frontend-only and preserve existing GraphQL behavior.

## Non-Goals

- Do not add new backend routes, GraphQL fields, or memory data contracts.
- Do not add future navigation entries such as Work, Tasks, Tools, Settings, or
  Inspect in this slice.
- Do not build a dashboard-style home page. `Home` means the current durable
  primary conversation.
- Do not expose healthy local service, chat, memory, or provider state as
  status pills.
- Do not change memory graph behavior beyond rendering it inside the shared
  shell.

## Product Decisions

- The onboarded app has one shared shell.
- Desktop uses a persistent left sidebar.
- Mobile uses a drawer opened from a small top bar.
- Primary destinations are exactly `Home` and `Memory`.
- `Home` routes to `/`, which remains the canonical path for the primary
  conversation.
- `/chat` remains accepted as a chat route but is not the sidebar label.
- `Memory` routes to `/memory`.
- `/memory/graph` remains addressable from inside the memory surface.
- Unknown browser paths fall back to `Home` instead of rendering a not-found
  page.
- Normal healthy status is silent. Attention appears only for degraded or
  action-worthy state.

## Architecture

Add a shared `AppShell` component for the onboarded app state. The shell owns
navigation structure, active destination highlighting, mobile drawer state, and
exception-only attention display. Chat, memory home, and memory graph pages
continue to own their content and data behavior.

`AppHeader` should be retired from onboarded routes. Setup and onboarding may
use smaller route-local brand/readiness treatment, but the product app after
onboarding should not keep a top header as primary chrome.

The existing route helper can remain lightweight. It should keep canonical
paths for supported routes and treat unknown paths as `Home`.

## Components

### `AppShell`

Responsibilities:

- Render persistent desktop sidebar.
- Render mobile drawer trigger and drawer contents.
- Close the mobile drawer after navigation.
- Highlight `Home` for chat routes.
- Highlight `Memory` for `/memory` and `/memory/graph`.
- Render exception-only attention state when needed.
- Provide a stable full-height content frame with correct overflow behavior.

### Sidebar Navigation

Navigation items:

| Label | Route | Active when |
| --- | --- | --- |
| `Home` | `/` | `/`, `/chat`, or any unknown path after fallback |
| `Memory` | `/memory` | `/memory` or `/memory/graph` |

Navigation should use the existing `navigate` helper so route changes do not
reload the app.

### Attention State

The shell should not show healthy status. It should show one compact attention
item only when state materially affects the app, for example:

- Provider connection is missing or expired.
- Chat connection is disconnected after the user is onboarded.
- Memory storage is unavailable on a memory route.
- Setup/readiness state blocks the current surface.

The active page still owns the detailed recovery UI. The shell attention item
is a signpost, not the full troubleshooting surface.

## Route And Data Flow

1. Browser path is parsed by the route helper.
2. Unknown paths resolve to `Home`.
3. The onboarded `App` renders `AppShell`.
4. `AppShell` highlights the active primary destination.
5. The current route's page renders inside the shell content area.
6. Chat continues to start or resume the primary conversation with the existing
   GraphQL flow.
7. Memory pages continue to use existing GraphQL queries and internal
   navigation.

No GraphQL queries, mutations, subscriptions, generated types, or Rust backend
code need to change for this design.

## Layout Behavior

Desktop:

- Fixed-width left sidebar.
- No top product header.
- Main content fills the remaining viewport.
- Chat composer remains anchored by the chat page layout.
- Memory graph keeps its large canvas/detail workspace inside the main content
  area.

Mobile:

- A compact route-local top bar contains the drawer trigger and current
  destination label.
- The drawer contains the Noema identity, attention item when present, and the
  `Home` and `Memory` links.
- Selecting a destination closes the drawer.
- Main content keeps the current page's scroll and overflow behavior.

## Edge Cases

- Unknown paths fall back to `Home` and should not show a not-found page.
- `/chat` remains accepted and displays the same primary conversation as `/`.
- `/memory/graph` highlights `Memory`, not a separate graph item.
- If the user is not onboarded, onboarding remains outside the new app shell.
- If attention state disappears, the sidebar returns to a quiet navigation-only
  state.
- If multiple issues are present, the shell should show the highest-priority
  compact attention item and let page-level recovery UI explain details.

## Testing

Frontend tests should cover:

- Route parsing maps unknown paths to chat/home behavior.
- `pathForRoute({ kind: "chat" })` still returns `/`.
- The shell labels the chat destination as `Home`.
- `Home` is active for `/` and `/chat`.
- `Memory` is active for `/memory` and `/memory/graph`.
- The mobile drawer closes after destination selection.
- The attention helper returns no item for healthy state.
- The attention helper returns an item for provider, chat, setup, or memory
  degraded state.
- Existing chat send behavior still works.

Validation should run from `crates/noema-core/web`:

```bash
bun test
bun run lint
bun run build
```

## Documentation Updates

Update frontend IA docs as part of implementation so the current visible IA
matches the new shell:

- The primary sidebar label is `Home`.
- `Home` means the durable primary conversation, not a dashboard.
- `Memory` is the only other primary sidebar destination in this slice.
- Healthy status is not persistent chrome; attention is exception-only.
