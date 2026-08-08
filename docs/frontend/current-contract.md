# Current Frontend Contract

This document records the frontend boundaries that are not obvious from the
implementation. Route and GraphQL details remain code-generated authorities:

- `apps/web/src/app/routes.ts` owns route parsing and canonical paths;
- `apps/web/src/app/routes/` owns TanStack Router composition;
- `apps/web/src/graphql/operations.ts` owns authored web operations;
- `graphql/schema.graphql` is the generated API schema;
- `apps/web/src/generated/graphql.ts` is generated and must not be hand-edited.

## Product surfaces

The primary destinations are Chat (`/`), Work (`/work`), Memory (`/memory`),
and Settings. The current settings roots are:

- `/settings/agents`
- `/settings/models`
- `/settings/memory`
- `/settings/tools/web`
- `/settings/tools/apis` and connection detail paths below it
- `/settings/tools/mcps` and connection detail paths below it
- `/settings/safety/privacy`
- `/settings/safety/usage`
- `/settings/system/providers`
- `/settings/system/notifications`
- `/settings/system/clients`

Unknown paths fall back to Chat. Do not document proposed routes as current or
add disabled navigation merely to reserve future information architecture.

## State authorities

- SQLite owns durable conversations, transcript items, Work, provider and tool
  metadata, policies, approvals, and client state.
- Native Markdown under `memory/human/` owns durable human memory; SQLite FTS is
  a rebuildable search projection.
- GraphQL queries provide scoped read models, mutations execute explicit
  commands, and subscriptions carry live changes.
- Daemon and WebSocket state is coordination state, not a second durable
  transcript or Work authority.
- Browser code never receives credential material. Provider-auth redirects and
  callbacks remain server-governed flows.

## Interaction contracts

Chat reconstructs its transcript from durable pages and merges live events by
stable item identity. After reconnect, the client refetches active reads and
reconciles durable transcript state before treating later live completion as
authoritative.

The installed PWA follows [pwa.md](pwa.md): only complete releases and complete
Apollo snapshots become offline authorities, and mutations remain locked until
reconciliation completes.

A2UI renders only Noema's bounded component catalog. Provider HTML, scripts,
styles, unknown components, and arbitrary Markdown never become UI authority.
Interactive submissions carry the durable interaction and surface revision
fences required by the runtime.

Settings panes own their data loading and mutations directly unless a shared
controller has multiple production consumers. Editing is either inline without
a separate Save button or performed in a focused dialog with explicit Save.

## UI implementation

Noema is a dense task-first product surface. Follow
[product-design.md](product-design.md) for hierarchy, grouping, Astryx usage,
spacing, responsive composition, and visual review. Reuse the existing shell,
detail, transcript, settings, and domain patterns before introducing a new
surface abstraction.

The web app uses Astryx and StyleX. Generated GraphQL types are the client
contract; do not add hand-maintained mirrors for generated query shapes.

## Validation

For frontend changes, run `bun run lint` and `bun run build` from `apps/web`.
Run focused existing tests when the changed logic has coverage. UI work is not
visually verified unless browser inspection is explicitly authorized.
