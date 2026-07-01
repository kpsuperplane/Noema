# Astryx Web UI Migration Design

## Summary

Migrate the Noema web UI from the current shadcn/Base UI/Tailwind foundation to
Astryx as the only component system.

This is a true replacement, not a compatibility-layer migration. The final web
app should remove shadcn, Base UI, Tailwind, shadcn config, Tailwind build
plugins, Tailwind utility classes, and shadcn-shaped primitive wrappers.

Noema should keep its product information architecture, routes, shell behavior,
GraphQL data flow, and Noema-specific domain components. Commodity framework
components should become Astryx-native, even when Astryx's component shape,
layout, or styling differs from the current shadcn/Base UI component.

## Context

The current web frontend lives in `crates/noema-core/web`. It is a React 19 app
with Apollo Client, GraphQL over `/graphql`, GraphQL subscriptions over
`/graphql/ws`, and a local web shell that is also bundled into the Tauri desktop
app.

Current UI foundation:

- shadcn/ui `base-rhea` component wrappers in `src/components/ui`.
- Base UI primitives under those wrappers.
- Tailwind CSS v4 utilities, `@tailwindcss/vite`, `tailwind-merge`,
  `class-variance-authority`, and `shadcn/tailwind.css`.
- Noema-owned CSS variables in `src/styles.css`.

Current product surfaces that must keep their behavior:

- `AppShell`, `ShellSidebar`, route-derived deck navigation, and shell surface
  visibility.
- Setup and onboarding readiness flow.
- Chat surface, composer, transcript replay, streaming transcript behavior,
  markers, structured cards, attachments, and error notices.
- Memory home and memory graph controls.
- Settings surfaces for providers, agents, MCPs, trusted identities, approvals,
  and audit.
- MCP setup and tool-permissions flows.

Astryx references used for this design:

- `https://astryx.atmeta.com/docs/getting-started`
- `https://astryx.atmeta.com/themes`
- `https://astryx.atmeta.com/components`

Astryx is currently Beta. The migration should use Astryx's documented package,
theme, component, and StyleX conventions, but implementation should verify the
installed package APIs directly because they may evolve.

## Decisions

Adopt Astryx as the web UI foundation.

Final desired dependency direction:

- Keep React, React DOM, Apollo Client, GraphQL, GraphQL WS, Vite, TypeScript,
  eslint, `lucide-react`, `@xyflow/react`, `d3-force`, and other
  product/domain dependencies that remain useful. Keep `sonner` only if Astryx
  does not provide a suitable toast path or if removing it would expand the
  migration beyond UI foundation work.
- Add Astryx core, Astryx Neutral theme support, Astryx CLI, and StyleX
  integration following current Astryx docs.
- Remove `@base-ui/react`, `@shadcn/react`, `shadcn`, `tailwindcss`,
  `@tailwindcss/vite`, `tailwind-merge`, `class-variance-authority` if unused,
  `tw-animate-css` if only shadcn/Tailwind-related, and any other dependency
  that exists only for the old UI stack.

The final source should not keep `components/ui` as a shadcn-style primitive
bucket. Prefer direct Astryx imports from product components. Keep a small local
Noema primitive only when all of these are true:

- Astryx does not provide the needed domain pattern.
- The component expresses Noema product semantics rather than a generic UI
  primitive.
- Its API is Noema-native, not a shadcn/Base UI compatibility API.

## Non-Goals

This migration does not:

- Redesign Noema's information architecture.
- Replace Apollo Client or the GraphQL product API.
- Change backend schema, daemon behavior, GraphQL operations, or generated
  GraphQL types.
- Change conversation persistence, transcript replay semantics, provider state,
  MCP setup behavior, memory graph data, or settings read/mutation contracts.
- Add migrations or compatibility layers for the old frontend stack.
- Preserve current shadcn visual details when Astryx provides a different
  native component shape.

## Architecture

The web UI should move to this layered architecture:

```text
crates/noema-core/web/src/
  styles.css            # Astryx reset/core/theme imports plus global app hooks
  theme/                # Noema-owned Astryx Neutral-derived theme files
  components/
    shell/              # Noema shell and route/deck navigation
    onboarding/         # setup and provider readiness
    transcript/         # Noema transcript/domain rendering
    memory/             # memory graph and memory detail UI
    settings/           # settings panes and MCP flows
    ...                 # other product-owned areas
  graphql/              # unchanged Apollo/transport code
  generated/            # unchanged generated GraphQL outputs
```

The old `components/ui` folder should be emptied and removed unless the
implementation finds a small Noema-owned domain primitive that belongs there
under a new, non-shadcn contract. Direct Astryx imports are preferred over a
new wrapper layer.

Noema-owned components should use:

- Astryx components for commodity controls: buttons, fields, dialogs, sheets or
  drawers, tabs, menus, badges, avatars, item/list rows, cards, separators,
  toasts, chat/message primitives, and layout helpers where they fit.
- StyleX for Noema custom layout, shell states, transcript states, composer
  layout, graph controls, domain cards, motion state classes, and responsive
  behavior.
- A small amount of global CSS for document roots, Tauri transparency, Astryx
  theme imports, unavoidable browser/platform quirks, and truly global
  animations.

## Theme

Start from Astryx Neutral and copy it into a Noema-owned editable theme file.
Then apply a light Noema refresh.

Current Noema identity is the baseline:

- Warm paper/ink neutrals.
- Pine primary color.
- Clay accent color.
- Red and blue status colors.
- `Bricolage Grotesque` display font.
- `Hanken Grotesk` body font.
- `JetBrains Mono` monospace font.

The final theme should express those choices through Astryx token names and
theme conventions. Old shadcn/Tailwind token aliases such as `--background`,
`--foreground`, `--muted`, `--ring`, and Tailwind `@theme` mappings should be
removed unless a Noema domain component needs a product semantic token that is
not covered by Astryx.

Light refresh is allowed, but the final result should remain recognizably
Noema: quiet, polished, chat-led, information-dense, inspectable, and warmer
than a generic gray admin console.

## Component Migration

Migrate from primitives upward.

For each old `components/ui` module, choose one of these outcomes:

1. Replace call sites directly with an Astryx component.
2. Move the behavior into a nearby Noema domain component and style it with
   StyleX.
3. Keep a small Noema-owned component only when it represents a Noema domain
   concept or fills a real Astryx gap.

Priority component areas:

- Button and icon-button usage.
- Textarea, input, select-like fields, and form labels.
- Badge/status chips.
- Card-like surfaces.
- Dialog, sheet/drawer, and close affordances.
- Tabs.
- Dropdown/menu actions.
- Avatar.
- Item/list rows.
- Separator/divider.
- Toast/notification handling.
- Attachment-like transcript cards.
- Chat/message/bubble/scroller pieces.
- Marker/status rows for memory, tools, and errors.

Do not preserve a shadcn API solely to reduce edits. If Astryx's native
component API or layout differs, migrate the product component to that native
shape.

## Product Surface Requirements

### Shell And Setup

Preserve:

- Route-derived L0/L1 navigation behavior.
- The persistent sidebar ground layer and route deck layout.
- Expanded desktop sidebar behavior.
- Collapsed desktop and mobile deck reveal behavior.
- `ShellSurfaceContext` visibility states and visible-only side effects.
- Settings as a bottom-anchored L0 item with route-derived L1 submenu behavior.
- Setup frame and onboarding readiness semantics.

Astryx may replace the underlying buttons, list items, tabs, dialogs, and
layout pieces, but route ownership and shell state remain Noema-owned.

### Chat And Transcript

Preserve:

- Composer behavior, including submit state, disabled state, text entry,
  textarea growth/sizing, and current mobile accommodations.
- GraphQL-backed conversation start/send behavior.
- Durable transcript replay from `conversation_items`.
- Live turn rendering from subscriptions.
- Streaming row arrival behavior and bottom follower behavior.
- Noema transcript entry model: user text, assistant text, activity rows,
  structured cards, errors, and completion events.
- Memory/tool markers and details.
- Attachment/structured-card semantics.

Use Astryx chat/message primitives only where they remain rendering primitives.
They must not become the state owner, transport owner, or persistence boundary.

### Settings And MCP Flows

Preserve:

- Provider and agent settings read models.
- MCP server setup flow.
- MCP tool permissions modal behavior.
- Trusted identities, approvals, and audit unavailable/current states.
- Existing GraphQL mutations, read models, safe error presentation, and
  authentication-required states.

Astryx components should replace commodity controls and visual structure, but
the setup/calibration product flow must remain Noema-owned.

### Memory Surfaces

Preserve:

- Memory home route behavior.
- Memory graph data flow and existing graph visualization behavior. Keep
  `@xyflow/react` unless Astryx or a focused replacement can preserve the same
  domain behavior with less code.
- Graph controls and detail panel semantics.
- Redaction/unavailable/error states.

The graph itself is a domain visualization, not an Astryx commodity component.
Use Astryx and StyleX around it without changing graph semantics.

## Styling Migration

Remove Tailwind from JSX, CSS, and build configuration.

Implementation should:

- Replace utility-class strings with StyleX styles or Astryx props.
- Remove `cn`/`tailwind-merge` usage when no longer needed.
- Remove `class-variance-authority` variants and replace them with Astryx
  variants, StyleX conditional arrays, or small explicit maps.
- Remove `@import "tailwindcss"`, `@import "shadcn/tailwind.css"`, and
  Tailwind `@theme` blocks from `styles.css`.
- Keep only global CSS that is genuinely global.
- Convert data-slot animation selectors to StyleX or focused global CSS based
  on whether they are component-local or document-wide.

This is expected to touch many files. Keep changes scoped to the web UI
migration and avoid unrelated frontend refactors.

## Migration Plan

Implement as one end-to-end effort with checkpointed milestones:

1. **Astryx Foundation**
   - Install Astryx packages and StyleX integration.
   - Add the Astryx reset/core CSS and Noema Neutral-derived theme.
   - Update Vite config for StyleX and remove Tailwind plugin setup once no
     longer needed.
   - Confirm the app can compile with the new foundation before broad surface
     rewrites.

2. **Primitive Replacement**
   - Replace low-level primitive usage in dependency order.
   - Remove or rewrite old `components/ui` modules as their call sites move.
   - Avoid a permanent compatibility shim.

3. **Shell And Setup**
   - Migrate shell/sidebar/setup/onboarding styling and commodity controls.
   - Preserve route-derived behavior and responsive deck semantics.

4. **Chat And Transcript**
   - Migrate composer, transcript rows, markers, structured cards, scroller,
     bottom follower, and error notices.
   - Preserve transcript model and GraphQL behavior.

5. **Settings And Memory**
   - Migrate settings panes, MCP setup, tool permissions modal, memory home,
     graph controls, and detail surfaces.
   - Preserve current/future/unavailable state boundaries.

6. **Cleanup**
   - Delete obsolete dependencies and lockfile entries.
   - Delete shadcn config and old primitive files.
   - Remove Tailwind imports/classes/plugins.
   - Remove unused helpers.
   - Update frontend docs and `docs/context/current.md` with the new settled
     Astryx direction.

Each milestone should be small enough to validate and commit independently,
even though this is one end-to-end migration mission.

## Validation

Default validation for the web migration:

```bash
cd crates/noema-core/web
bun run gen:types
bun run lint
bun run build
```

Run broader repository validation only where implementation touches Rust,
GraphQL schema exports, or shared backend/frontend contracts.

Before each commit:

```bash
git status --short --branch
git diff --check
git diff --cached --stat
git diff --cached --name-status
```

Per current project guidance, do not inspect frontend/UI work with browser
tools unless explicitly requested. The implementation agent should rely on code
validation, source review, and the final human visual review unless the user
opts into browser inspection later.

## Risks And Mitigations

| Risk | Mitigation |
| --- | --- |
| Astryx is Beta and APIs may change. | Verify installed package APIs and generated docs during implementation; avoid guessing from stale examples. |
| StyleX/Vite integration may need project-specific setup. | Land foundation first and validate compile/build before broad rewrites. |
| Chat scrolling and composer behavior are sensitive. | Migrate transcript/composer after primitives; preserve model and behavior tests/checks around replay, streaming, and submit states. |
| Removing Tailwind means many JSX class strings must be converted. | Work by surface and dependency order; treat lingering Tailwind utility strings as cleanup blockers. |
| Old shadcn API shape may linger through wrappers. | Delete `components/ui` modules as call sites migrate; only keep Noema-native domain primitives. |
| Theme refresh could drift away from Noema. | Start from current Noema colors/fonts and Astryx Neutral; keep refresh light and review final theme in the written spec/implementation diff. |

## Completion Criteria

The migration is complete when:

- The web app builds and lints without shadcn, Base UI, or Tailwind
  dependencies.
- No frontend source imports `@base-ui/react`, `@shadcn/react`, `shadcn`, or
  Tailwind-only helpers.
- No JSX relies on Tailwind utility classes.
- `components/ui` is removed or contains only Noema-owned non-shadcn domain
  primitives.
- Astryx theme CSS and StyleX are the styling foundation.
- Noema routes, shell behavior, chat behavior, settings flows, memory graph
  semantics, and GraphQL data flow remain intact.
- Frontend docs and durable context record that the web UI now uses an Astryx
  Neutral-derived Noema theme and StyleX instead of shadcn/Base UI/Tailwind.
