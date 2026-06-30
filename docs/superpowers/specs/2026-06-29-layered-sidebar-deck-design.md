# Layered Sidebar Deck Design

## Status

Approved design for replacing the current same-level sidebar/content shell with
an Arc-inspired layered shell. No implementation has been done in this spec.
The next step is an implementation plan.

## Context

Noema's current onboarded web shell renders a persistent desktop
`ShellSidebar` beside the content pane and a separate mobile `ShellSidebar`
inside a `Sheet`. This makes the sidebar and content feel like peers on
desktop, while mobile behaves like a conventional drawer placed above the
content.

The desired direction reverses that hierarchy: navigation is the ground layer,
and route content is the active deck above it. The same navigation instance
should serve desktop, collapsed desktop, and mobile states so the shell has one
mental model and one source of navigation truth.

## Goals

- Render exactly one `ShellSidebar` inside a persistent navigation ground layer.
- Make the content area read as an elevated deck above that ground layer.
- Remove the mobile `Sheet` path from `AppShell`.
- Support expanded and collapsed desktop sidebar states.
- Make collapsed desktop and mobile reveal navigation through the same deck
  movement pattern.
- Keep the change frontend-only and preserve existing routes, GraphQL behavior,
  chat behavior, memory pages, and onboarding flow.
- Keep the implementation small enough to fit the current shell slice without
  creating a broad layout framework.

## Non-Goals

- Do not add backend state, GraphQL fields, mutations, or subscriptions.
- Do not introduce a persisted settings or preference system in this slice.
- Do not add new navigation destinations.
- Do not redesign `ShellSidebar` content beyond any controls needed for the
  layered shell.
- Do not change chat composer behavior, transcript behavior, memory home, or
  memory graph data behavior.
- Do not add compatibility paths for the retired mobile `Sheet` layout.

## Product Decisions

- The sidebar is a ground layer, not a peer pane.
- The route page is a deck above the ground layer.
- Desktop starts expanded by default.
- Desktop can collapse the sidebar for a focused deck view.
- Mobile is always collapsed by default.
- Opening navigation from mobile or collapsed desktop moves the deck aside to
  reveal the same sidebar ground layer.
- Desktop collapse state is component-local for now. It can become persisted
  later when Noema has an explicit frontend preference home.
- Selecting a navigation item closes the revealed navigation state.
- Onboarding remains outside this shell.

## Architecture

`AppShell` should render one layered shell tree:

```text
AppShell
  shell root
    navigation ground layer
      ShellSidebar
    content deck
      deck header / controls
      route content
```

The navigation ground layer is always mounted. It owns the single
`ShellSidebar` instance and remains visually behind the deck. The content deck
is the active route surface. Its layout changes based on viewport and shell
state.

Keep the behavior readable by extracting a small local shell behavior helper,
for example `useDeckNavigation`, near `AppShell`. This helper should own only
the shell state and actions:

- `navOpen`
- `sidebarCollapsed`
- `openNav`
- `closeNav`
- `toggleSidebarCollapsed`
- `navigateFromShell`

This should not become a general layout state machine. Route parsing, attention
state, and navigation item definitions can remain with the existing
`AppShell` module unless implementation pressure shows a clearer split.

## Layout Behavior

### Expanded Desktop

- The sidebar ground layer is visible on the left.
- The content deck is offset to the right.
- The deck has an inset, border, rounded corners, and shadow so it reads as the
  active surface above the shell background.
- Sidebar navigation remains interactive.
- The deck does not shift when navigating between current routes.

### Collapsed Desktop

- The content deck covers the sidebar navigation content while leaving only the
  shell background/inset visible.
- A deck header exposes a menu control for revealing navigation.
- A deck header control toggles the desktop collapse state.
- Opening navigation moves the deck aside to reveal the same sidebar ground
  layer used by expanded desktop.
- Selecting a destination routes normally and closes the reveal state.

### Mobile

- Mobile uses the collapsed deck model by default.
- The deck covers the navigation ground layer while navigation is closed.
- The deck header exposes the menu trigger and current destination label.
- Opening navigation translates the deck right and scales it down slightly to
  make the sidebar feel like the underlying layer.
- Tapping outside the revealed sidebar area closes navigation.
- Selecting a destination routes normally and closes navigation.

## Accessibility And Interaction

- The menu trigger should expose `aria-expanded` and `aria-controls` for the
  sidebar region.
- The sidebar ground layer should have a stable id and accessible label.
- Escape should close a revealed navigation state.
- Pointer interaction with route content should be blocked while mobile or
  collapsed-desktop navigation is revealed.
- Focus should return to the menu trigger after Escape or backdrop close.
- Reduced-motion users should receive shortened or disabled deck transform
  transitions.
- The collapse toggle should have a clear accessible name that reflects the
  next action.
- The current destination label should remain visible in collapsed/mobile deck
  headers.

## Components

### `AppShell`

Responsibilities:

- Render the layered shell root.
- Render the single `ShellSidebar` ground layer.
- Render the content deck and route children.
- Render deck-level menu and collapse controls.
- Apply responsive deck layout classes for expanded desktop, collapsed desktop,
  mobile closed, and navigation revealed states.
- Keep setup/onboarding outside the layered shell.

### `useDeckNavigation`

Responsibilities:

- Track whether navigation is revealed.
- Track whether the desktop sidebar is collapsed.
- Close revealed navigation after route changes.
- Provide stable callbacks for menu, backdrop, Escape, collapse toggle, and
  navigation selection.

This helper should stay local to the shell unless more surfaces need it later.

### `ShellSidebar`

Responsibilities remain mostly unchanged:

- Render Noema identity.
- Render exception-only attention state.
- Render primary navigation items.
- Call the provided navigation callback.

The component should not need to know whether it is being shown on desktop,
collapsed desktop, or mobile. Any shell-specific spacing or reveal behavior
belongs to `AppShell`.

## Route And Data Flow

1. Browser path is parsed by the existing route helper.
2. `AppShell` derives the active destination and attention state as it does
   today.
3. `AppShell` renders the persistent sidebar ground layer.
4. `AppShell` renders the current route's children inside the content deck.
5. Menu/collapse controls update shell-local state only.
6. Navigation clicks call the existing navigation callback and then close any
   revealed navigation state.
7. Chat and memory pages continue to own their GraphQL queries, subscriptions,
   and page-specific content behavior.

## Edge Cases

- `/chat` should still highlight `Home`.
- `/memory` and `/memory/graph` should still highlight `Memory`.
- Unknown paths should continue to fall back to `Home`.
- If setup or provider readiness blocks the app, onboarding/recovery behavior
  remains outside this shell change.
- If attention state disappears, the sidebar returns to quiet navigation-only
  presentation.
- If multiple degraded states exist, the existing attention priority helper
  continues to choose one compact attention item.
- Memory graph should keep a usable canvas/detail workspace inside the deck and
  must not be squeezed by nested card styling beyond the shell deck frame.

## Testing

Frontend tests should cover:

- `AppShell` renders one primary sidebar/navigation instance, not separate
  desktop and mobile sidebar copies.
- `Home` remains active for `/` and `/chat`.
- `Memory` remains active for `/memory` and `/memory/graph`.
- Healthy state still produces no shell attention item.
- Provider, chat, setup, and memory degraded states still produce the expected
  attention item.
- The mobile/collapsed menu trigger exposes expanded state.
- Opening navigation changes shell state.
- Selecting a navigation item closes revealed navigation.
- Escape closes revealed navigation.
- The desktop collapse toggle changes shell state and accessible label.

Validation should run from `crates/noema-core/web`:

```bash
bun test
bun run lint
bun run build
```

Because this is a UI layout change, implementation verification should also run
the local app and inspect desktop and mobile viewports for spacing, overflow,
safe area behavior, content interaction blocking while navigation is revealed,
and memory graph usability inside the deck.

## Documentation Updates

Update frontend IA docs during implementation to reflect:

- The sidebar is now the navigation ground layer.
- Route content is the elevated deck.
- Desktop supports expanded and collapsed sidebar states.
- Mobile and collapsed desktop reveal the same single sidebar by moving the
  deck aside.
- Healthy status remains quiet; attention is exception-only.
