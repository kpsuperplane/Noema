# Current Frontend Contract

This document records the frontend boundaries that are not obvious from the
implementation. Route and GraphQL details remain code-generated authorities:

- `apps/web/src/app/routes.ts` owns route parsing and canonical paths;
- `apps/web/src/routes/` owns TanStack Router composition;
- `apps/web/src/graphql/tasksOperations.ts` owns authored Tasks operations;
- `graphql/schema.graphql` is the generated API schema;
- `apps/web/src/generated/graphql.ts` is generated and must not be hand-edited.

## Product surfaces

The primary destinations are Chat (`/`), Tasks (`/tasks`), Memory (`/memory`),
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

- SQLite owns durable conversations, transcript items, tasks, provider and tool
  metadata, policies, approvals, and client state.
- Native Markdown under `memory/human/` owns durable human memory; SQLite FTS is
  a rebuildable search projection.
- GraphQL queries provide scoped read models, mutations execute explicit
  commands, and subscriptions carry live changes.
- Daemon and WebSocket state is coordination state, not a second durable
  transcript or task authority.
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

## API integration UX

API Settings groups connections by provider, account, then API. Provider headers
contain access-method actions. Account headers use an action menu. OAuth is
one supported access method; credential and no-auth connections keep the same
provider hierarchy. MCP Settings keeps its service-first list. Both use the same
connection detail. The detail keeps connection policy and tool controls visible.
Tool counts appear in the Tools section, not in list rows or the detail header.
When no API exists, it shows one route to Chat instead of empty data sections.
Client import occurs only when a structured API connection action requires it.

Chat presents one pending human intervention at a time with queue navigation.
Tasks and dedicated queue surfaces can show the complete pending list.
An agent request to enable a disabled tool uses the existing action request
card. The primary action says `Enable tool` because approval changes persistent
tool policy. Chat omits the related internal `enable.*` call and result markers.

An OAuth application is reusable provider setup. An authorization grant is one
account's access. An API connection keeps its own tool policy and lifecycle.

Web and iOS use the structured server-selected next action. They do not infer
setup work from status copy. OAuth completion uses the exact attempt event and
foreground recovery query.

Settings also receives exact connection actions for every compatible account
and application. It excludes grants already attached to that definition.
Added-access confirmation shows operation benefits before scopes. A new
attachment continues directly to the existing connection-policy editor.

Connection deletion keeps the account authorization. Account disconnection
removes tokens and disables every dependent API. Application deletion requires
all grants to be disconnected first.

OAuth application secrets and account tokens never enter client read models.
Exact scopes and public client metadata stay behind technical disclosure.

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
