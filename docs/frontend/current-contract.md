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
- `/settings/system/desktop` in the Tauri desktop app only
- `/settings/system/clients`

Unknown paths fall back to Chat. Do not document proposed routes as current or
add disabled navigation merely to reserve future information architecture.

## Onboarding

Onboarding is a bounded readiness flow. It replaces the product shell until
the human confirms one complete model setup. It then opens Chat.

The first provider view presents Local, OpenRouter, and Codex as peer choices.
The human selects one provider to start and can add others later. OpenRouter
offers OAuth first and keeps API-key entry behind disclosure. Local setup shows
one compatible recommendation and its required download when available.
OpenAI remains a configuration path outside the first-run chooser.

After connection, the human reviews model assignments for Chat, Tasks, and
supporting work. Proposed assignments remain drafts until one confirmation
saves the complete selection. The same assignments remain editable later.

Use task and outcome language in onboarding. Keep paths, configuration,
transport, runtime, and policy internals outside the default flow. The current
shell restores Chat, Tasks, Memory, and Settings after setup. Do not hide these
destinations to preserve an obsolete staged rollout.

## State authorities

- SQLite owns durable conversations, transcript items, tasks, provider and tool
  metadata, policies, approvals, and client state.
- Native Markdown under `memory/human/` owns durable human memory; SQLite FTS is
  a rebuildable search projection.
- GraphQL queries provide scoped read models, mutations execute explicit
  commands, and subscriptions carry live changes.
- Each route composes one root query from shared fragments. Mutation payloads
  update normalized cache objects when they contain the changed state.
- A subscription invalidates its active root. Refetch after a mutation only
  when the payload cannot represent server-derived state.
- Daemon and WebSocket state is coordination state, not a second durable
  transcript or task authority.
- Browser code never receives credential material. Provider-auth redirects and
  callbacks remain server-governed flows.
- Tauri keeps the origin, client identifier, and rotating refresh credential
  in the operating system credential store. Access tokens remain in Rust
  memory. Its webview receives connection state and the validated server origin.

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

Unexpected render failures preserve the nearest stable human task. Provider or
runtime failures use the full-page recovery surface. Route failures keep the
product shell available. Transcript items, human intervention cards, detail
rails, sidebars, and the rich Task editor fail within their local surfaces.
Error boundaries do not replace explicit query, mutation, or connection errors.

The desktop app runs one embedded local instance or one connected remote server.
A connection link carries only the validated server origin. Rust opens the
system browser for OAuth with PKCE and recent passkey approval. Remote mode uses
HTTPS GraphQL and authenticated `graphql-transport-ws` through Tauri IPC.

Returning to local mode revokes the remote OAuth family and removes its local
credentials before restart. If revocation cannot reach the server, confirmed
forget removes only the local credentials. Failed startup shows retry,
local-mode, and confirmed-forget actions instead of the product surface.

## API integration UX

API Settings groups connections by provider, account, then API. Provider headers
own account creation. The add-account dialog selects compatible services before
it starts the provider access flow. One OAuth sign-in can authorize all selected
services through one compatible application. Account headers use an action menu. OAuth is
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
Web Chat groups OAuth setup by application for a new account and by grant for an existing account.
Compatible APIs share one account card and one OAuth attempt. API connection policies remain separate.

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

Task detail shows `Workspace` and `Transcript`.
Above 1200 pixels, Task detail shows both views side by side with a 600-pixel Transcript.
The wide layout keeps a top boundary and a vertical divider between the views.
The floating Task context card occupies the Transcript column in the wide layout.
The wide context card omits its ordinary Task status line.
At smaller widths, Task detail uses tabs and swipe navigation.
Workspace shows direct file buttons above the selected UTF-8 preview.
`Result` and `Task` appear first. Other files follow in path order.
The file row scrolls horizontally when needed and vertically with the preview.
A completed Task opens `RESULT.md`; another Task opens `TASK.md`.
Completion selects `RESULT.md` once when `TASK.md` was open.
Result previews preserve provider citations.

Web Task creation and Inbox editing use one shared Milkdown Crepe editor.
Inbox titles and descriptions expose adjacent pencil controls and edit in place without changing the surrounding layout.
Project and Executor editing remains in a focused settings dialog.
For a selected project, `PROJECT.md` is the first Tasks list row.
It opens in the existing detail rail and renders Markdown before editing starts.
Document edits autosave with revision and digest fences. Navigation waits for a pending save.
Conflicts preserve the local draft and offer the latest stored document explicitly.
Archived projects keep this document read-only until reopening.
The editor includes source mode and falls back to source when rich parsing fails.
iPhone and iPad use full-screen Markdown source editors for the same Task fields.
Both clients send the transient document digest with human updates.

Recurrence detail reads its separate template `TASK.md`.
Recurring titles and template descriptions use the same field-adjacent pencil controls.
The cadence has its own adjacent schedule control. The template always identifies its future-run scope.
Template saves affect only future occurrences and show local save or stale-draft status.
Occurrence history groups runs by local day. Template editing remains separate from schedule editing.
Task routes have no dormant text-search parameter or description-search behavior.

The Tasks list reads recurring authorities directly. A recurrence remains in
Scheduled when all of its Task instances are terminal. Instances provide run
history and do not control recurrence visibility.

Memory articles keep prose primary. One numeric citation represents one nearby
claim. Its hover or focus card shows every exact evidence source with type,
date, excerpt, and identifier.

Web and iOS use one provider citation contract for Chat messages and completed
Task results. They number unique URLs in first-use order. Each citation marker
follows its claim when the provider supplies a valid UTF-16 end offset. A marker
without a valid offset follows the complete message. Markers use plain,
non-interactive superscript numbers. One Sources control follows the cited
content and opens its Sources view. The control uses its own line in Chat
message bubbles. Each source shows its title, host, favicon, and exact URL.

Web renders every Sources control with Astryx Citation. Web and iOS show up to
three exact-host favicons in a compact transparent pill. Surface-colored rings
separate overlapping favicons. The first domain always appears without a leading
`www.` label. Extra domains use one `+N` suffix. Failed favicons disappear instead
of showing a fallback avatar. One source with no favicon uses a plain Sources
book icon. Sources without website hosts keep the generic Sources icon.

The backend decodes reserved source footnotes from completed Task results. It
returns readable text and the same structured provider citation metadata that
Chat uses. Web and iOS use the shared marker, icon, and Sources presentation.

Clients do not parse `[^noema-source-N]` markers or definitions.

## Validation

For frontend changes, run `bun run lint` and `bun run build` from `apps/web`.
Run focused existing tests when the changed logic has coverage. UI work is not
visually verified unless browser inspection is explicitly authorized.
