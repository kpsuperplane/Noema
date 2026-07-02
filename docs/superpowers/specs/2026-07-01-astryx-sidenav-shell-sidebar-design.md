# Astryx SideNav Shell Sidebar Design

## Summary

Complete the shell sidebar's Astryx adoption by making `ShellSidebar` use the
installed Astryx `SideNav` primitives as the real sidebar navigation structure,
while keeping Noema's existing layered deck shell.

This is not an Astryx `AppShell` migration. Noema keeps ownership of the
Arc-style deck movement, desktop collapse/reveal behavior, mobile reveal,
Tauri chrome offset handling, route-derived L0/L1 menu state, and
`ShellSurfaceContext`.

## Context

The current shell already uses Astryx `SideNav` and `SideNavItem` inside
`crates/noema-core/web/src/components/shell/ShellSidebar.tsx`.

Noema-specific shell behavior lives outside Astryx:

- `AppShell.tsx` owns the persistent sidebar ground layer and content deck.
- `deckNavigation.ts` owns reveal/collapse state and surface visibility.
- `shellNavigation.ts` derives L0 and Settings L1 menu levels from routes.
- `ShellSidebar.tsx` animates menu-level transitions between L0 and Settings.
- Tauri desktop chrome offset and drag-region spacing are Noema shell concerns.

Astryx exposes:

- `SideNav` as the sidebar navigation container with `header`, `topContent`,
  scrollable `children`, `footer`, `footerIcons`, optional resize, and optional
  collapse.
- `SideNavItem` as the navigation item primitive with selected, disabled, icon,
  link, nested, and end-content support.
- `AppShell` and `LayoutPanel` as broader layout-frame primitives.

The requested slice is approach 1: deeper `SideNav` adoption inside the current
Noema shell, not replacing the shell frame with Astryx `AppShell`.

## Goals

- Make `ShellSidebar` read as a clean adapter from Noema `ShellMenuLevel` data
  to Astryx `SideNav` zones.
- Keep `SideNav` as the only sidebar navigation container.
- Keep `SideNavItem` as the only ordinary sidebar item primitive.
- Preserve the current single persistent sidebar ground layer.
- Preserve route-derived L0 to Settings L1 behavior and transition animation.
- Preserve the bottom item behavior: L0 Settings and L1 Go back.
- Preserve Noema attention rendering and primary-agent avatar behavior.
- Keep Noema deck collapse/reveal state authoritative.

## Non-Goals

This slice does not:

- Replace Noema `AppShell` with Astryx `AppShell`.
- Replace the layered deck shell with `LayoutPanel`.
- Enable Astryx `SideNav` collapse or resize behavior.
- Add new routes, destinations, sections, or navigation hierarchy.
- Change `deckNavigation.ts`, `ShellSurfaceContext`, route parsing, GraphQL,
  chat, memory, or settings pane behavior.
- Add browser inspection or UI tests unless explicitly requested.

## Design

### Shell Ownership

`AppShell.tsx` remains the shell owner. It should keep rendering:

```text
shell root
  aside shell-sidebar-ground
    ShellSidebar
  content deck
    deck header
    ShellSurfaceProvider
```

Do not pass Noema's shell through Astryx `AppShell` in this slice. Astryx
`AppShell` owns responsive mobile drawer behavior and the overall application
layout frame, which conflicts with Noema's current layered deck model.

### `ShellSidebar`

Keep `ShellSidebar` as a Noema adapter with these responsibilities:

- Receive a `ShellMenuLevel`.
- Maintain transition frames for route-derived L0/L1 animation.
- Render each transition frame as one Astryx `SideNav`.
- Pass the menu label to `SideNav` via `aria-label`.
- Use `SideNav.header` for the Noema drag spacer and attention area.
- Use the scrollable `SideNav.children` area for the current menu level's main
  items.
- Use `SideNav.footer` for the level's bottom item.
- Keep `SideNav.collapsible` disabled or omitted.
- Keep `SideNav.resizable` disabled or omitted.

The component can be cleaned up by naming the mapping boundaries explicitly,
for example:

- `ShellSidebarMenuFrame`
- `ShellSidebarNav`
- `ShellSidebarNavItem`
- `ShellSidebarHeader`

The exact names can follow the existing file style.

### Items

`ShellSidebarNavItem` should keep using `SideNavItem` with:

- `label` from the route-derived menu item, except the Home item can continue
  showing the primary agent display name once named.
- `icon` from the route-derived item, except Home can continue showing the
  scaled Noema agent avatar once named.
- `isSelected` from `ShellMenuLevel.activeItemId`.
- `isDisabled={!interactive}` for non-interactive exiting transition frames.
- `onClick={() => onSelectItem(item)}`.

Keep Noema's `data-slot`, `data-shell-menu-item`, and `data-current`
attributes around items if tests or deck styling depend on them.

### Header And Attention

The sidebar header remains Noema-owned because it includes:

- Tauri drag-region spacer.
- Compact attention rendering.
- Empty placeholder when the current level does not support attention or no
  attention exists.

The header can be passed to `SideNav.header`; do not introduce Astryx branding
heading unless Noema later decides to show explicit app identity in the sidebar.

### Collapse And Mobile Reveal

Do not enable `SideNav`'s `collapsible` prop in this slice. Noema currently has
two different concepts that should not be conflated:

- Noema deck collapse/reveal: the content deck covers or reveals the persistent
  sidebar ground layer.
- Astryx SideNav collapse: the nav itself becomes an icon-only toolbar.

Noema's deck state remains the source of truth for desktop collapse and mobile
reveal.

### Styling

Keep StyleX only for Noema-specific integration:

- Transition frame positioning and non-interactive exiting frames.
- Full-height viewport constraints inside the sidebar ground layer.
- Transparent `SideNav` background/border reset needed by the deck ground.
- Header drag-region spacing.
- Primary agent avatar scale.
- Item data wrappers if needed for test hooks or layout stability.

Prefer Astryx `SideNav` defaults for ordinary sidebar spacing, item selected
state, disabled state, icon alignment, and footer placement.

## Validation

Implementation should validate with:

- `git status --short --branch`
- `git diff --check`
- Source scan that `ShellSidebar` still imports and uses `SideNav` and
  `SideNavItem`.
- Source scan that `AppShell.tsx`, `deckNavigation.ts`,
  `ShellSurfaceContext.tsx`, and `shellNavigation.ts` are unchanged unless an
  import-only adjustment is required.
- `bun run lint` from `crates/noema-core/web`.
- `bun run build` from `crates/noema-core/web`.

Per current project guidance, do not inspect with browser tools or add UI tests
unless explicitly requested.

## Open Follow-Ups

- Revisit Astryx `SideNav` controlled collapse only if Noema decides to add a
  true icon-only sidebar mode separate from deck reveal.
- Revisit Astryx `AppShell` only as a larger shell architecture project.
