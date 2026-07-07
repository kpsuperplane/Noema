# TanStack Router Code Splitting Design

## Context

The current web UI is a Vite-built React SPA served by the Noema Rust daemon and
bundled into the Tauri desktop app. Routing is owned by
`crates/noema-core/web/src/app/routes.ts`, while `App.tsx` statically imports all
top-level pages. The daemon build also sets `inlineDynamicImports: true` and the
Rust asset resolver serves only `app.js`, `styles.css`, `index.html`, and
`noema-mark.svg`, so route chunks cannot currently be emitted or loaded.

The chat route is the primary first-load surface. Memory graph and Settings are
secondary surfaces and include heavier dependencies such as React Flow, D3, and
MCP setup/calibration UI. Those surfaces should load on demand.

## Decision

Adopt TanStack Router with file-based routes and the Vite router plugin. Keep
the app client-first and keep Noema's existing Rust GraphQL/Tauri architecture.
Do not introduce TanStack Start, SSR, React Server Components, or a JavaScript
server.

Use route files for the existing public routes:

- `/`
- `/memory`
- `/memory/graph`
- `/settings`
- `/settings/agents`
- `/settings/tools/web`
- `/settings/tools/mcps`
- `/settings/safety/usage`
- `/settings/safety/approvals`
- `/settings/safety/identities`
- `/settings/system/providers`

Enable TanStack Router automatic code splitting so non-chat route components are
lazy-loaded. The shell and chat route remain in the initial bundle because chat
is the default product surface.

## Architecture

Add a router-owned root component that wraps the existing boot boundary and
shell behavior. Route components consume shared app state through TanStack
Router context instead of a hand-rolled `useBrowserRoute` hook. Existing shell
navigation keeps its domain `AppRoute` type, but navigation delegates to the
router.

Split the current `App.tsx` responsibilities:

- Boot and chat runtime state remain in a focused app provider/root module.
- URL parsing, push-state handling, and settings back navigation move to
  TanStack Router.
- Page rendering moves into route files.

Update the daemon asset resolver before removing `inlineDynamicImports`.
Development builds may continue reading emitted files from
`target/web-assets`. Release builds should embed a generated asset directory or
manifest so all emitted route chunks can be served by exact path. Asset paths
under `/assets/` must remain static-only and missing chunks must return 404.

The Tauri build already emits chunks under `dist-tauri/assets`, so it should
only need the same router/plugin configuration and relative asset paths.

## Error Handling

Route chunk loading should use a router pending component for lazy route loads
and a root error component for route-load failures. The fallback should be
shell-compatible and avoid resetting chat state for non-chat navigation
failures. The existing Vite preload error reload behavior is optional and
should not be the first implementation unless chunk cache invalidation becomes
visible during development.

## Testing And Validation

Keep route behavior covered with focused TypeScript tests for route conversion
helpers that remain in the shell navigation layer. Add Rust tests for daemon
asset serving that prove emitted chunk-like paths under `/assets/` are served
when present and missing paths still miss.

Run:

- `bun run gen:types`
- `bun run lint`
- `bun run build`
- `cargo fmt --all --check`
- targeted Rust tests for daemon web asset routing

Do not add frontend browser screenshot validation for this migration because it
is routing and bundling work, not a visual redesign.

## Scope Boundaries

Do not redesign shell IA, Settings IA, chat behavior, onboarding, GraphQL
operations, transcript rendering, or memory graph UX. Do not add backwards
compatibility for retired routes. Do not move to TanStack Start or another
full-stack framework in this slice.
