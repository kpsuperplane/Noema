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

When the public address is missing and no passkey exists, the first visitor
confirms the current browser address. The address uses bold monospace text and the primary theme color.
To select another domain, the visitor must open Noema at that domain first.

Before product onboarding, the first visitor creates the initial passkey
without a recovery code. When a passkey already exists, the login surface keeps
recovery-code entry behind the `Recover access` action.

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

Supporting flows reuse `SetupFrame`, `SetupCard`, and the existing animated avatar.
The avatar centre meets the card edge. The title position stays fixed.
`theme/supporting.css` owns the shared card geometry, wash, spacing, and actions.
Desktop action pairs use equal columns. The primary action follows the secondary action.
Provider choices reuse `ListCardButton`. Model review groups resolved model names by their jobs.
The Customize disclosure keeps all nine assignments and their existing preference controls.

`internal/publicpage` renders consent and callback pages without authenticated JavaScript.
Its stylesheet resolves the current Vite entry CSS through `/assets/supporting.css`.
This avoids a second theme snapshot. Public pages use a static avatar fallback.
Run `bun scripts/render-public-buttons.tsx` from `apps/web` after an Astryx upgrade.
This regenerates public form controls from the installed Button component.

Recovery keeps its own state during native authorization and PWA reauthentication.
Browser Back restores passkey focus. Forward restores recovery field focus.
Native handoff does not claim that token exchange has completed.
A saved adapter grant with failed activation remains a partial setup result.
Human intervention cards remain part of their existing Chat flow.

## State authorities

- SQLite owns durable conversations, transcript items, tasks, provider and tool
  metadata, policies, approvals, and client state.
- Native Markdown under `memory/human/` owns durable human memory. Search uses
  a rebuildable in-process index.
- GraphQL queries provide scoped read models, mutations execute explicit
  commands, and subscriptions carry live changes.
- Each route composes one root query from shared fragments. Mutation payloads
  update normalized cache objects when they contain the changed state.
- A subscription invalidates its active root. Refetch after a mutation only
  when the payload cannot represent server-derived state.
- Daemon and WebSocket state is coordination state, not a second durable
  transcript or task authority.
- Client read models omit saved credential values. Setup forms can accept new
  credentials. Provider-auth redirects and callbacks remain server-governed flows.
- Tauri keeps the origin, client identifier, and rotating refresh credential
  in the operating system credential store. Access tokens remain in its native
  process. Its webview receives connection state and the validated server origin.

## Interaction contracts

Chat reconstructs its transcript from durable pages and merges live events by
stable item identity. Tool activities use their conversation record IDs.
Results reference the call record through `parent_item_id`. Runtime and storage
use the same record-ID function before live delivery and saving. The API reads
these identities directly, including for existing history. Provider call IDs
remain separate protocol values. After reconnect, the client refetches active reads and
reconciles durable transcript state before treating later live completion as
authoritative.

Multiple-choice option IDs belong to one question. Web caches each option inside its question or selection.
A successful question display finishes the Chat turn without another model response.
The saved question keeps usage information. Later selections arrive as normal user messages.
Web and iOS omit the question tool marker unless it failed.

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

The desktop app runs one packaged Go sidecar or one connected remote server.
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
The `Connect API` action opens a service picker like `Add provider`.
Both pickers use the shared `ServiceChoice` component for service choices.
The API picker includes Gmail, Google Calendar, and a route to Chat for other APIs.
While setup opens, a spinner replaces the selected item’s arrow without changing its size.
Selecting a service opens account setup directly. There is no separate Connect step.
Unconnected reviewed APIs remain in Finish setup with a Setup action and settings icon.
This action remains available before and after application import.
After selection, missing Google setup opens the protected application import dialog.
Successful import resumes the selected connection's account setup.
When no API exists, the page shows a short empty state.
Client import occurs only when a structured API connection action requires it.

Chat presents one pending human intervention at a time with queue navigation.
Chat omits approval-request activity notices. The tool marker shows the activity,
and the pending intervention card provides approval controls.
Tasks and dedicated queue surfaces can show the complete pending list.
Browser page approvals name the website and show the full URL and Noema's stated reason.
The `Open page once` control approves one request. Review details retain the saved action and assessment.
Browser submission approvals show the declared destination, method, and visible
submitted values before the decision controls. Hidden, password, and file values remain omitted.
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

Transcript styles use unnamed size queries against the nearest container.
StyleX 0.19 runtime injection retains only the first rule for each named container query.

Task detail keeps documents and Transcript together.
At container widths above 1200 pixels, Transcript occupies a separate 600-pixel column.
At smaller widths, tabs and swipe navigation select the view.
A completed Task opens `RESULT.md`; other Tasks open `TASK.md`.
Completion selects the result once when the request was open.
File tabs keep Result and Task first, then other files in path order.

The floating Task bar shows state and current activity. Human attention takes priority.
Inactive Tasks omit old activity. Full content remains available in Transcript.
Capture and detail reuse `TaskActionBar`. Layout values belong in the components.

Artifact detail previews Markdown and plain text directly. It uses the shared
file parser for supported spreadsheets. Raster images and PDFs use authorized
inline routes. HTML runs only in a sandbox after Noema removes active elements,
navigation, event handlers, and external resources. SVG remains download-only.

Task and Project documents share the Markdown viewer and Milkdown editor.
Task capture uses `/tasks/new` inside the normal Tasks surface.
Add to Inbox and Run now remain available together.
Capture omits working-folder controls.

Existing Task and recurrence titles and instructions save on blur.
The editor reads current text before saving, starting work, or leaving the Task.
Failed saves preserve drafts and prevent navigation. Stale saves require acknowledgement before retry.
Document saves retain revision and digest checks. Active Tasks use server-provided edit availability.
Project documents retain their explicit editing controls.
The source editor remains available when rich parsing fails. iOS uses its native source editor.

Recurrence detail edits the template `TASK.md`; changes affect future runs.
Run history remains separate from the selected template.
Skipped slots do not imply an existing Task.
Schedule editing preserves cron expressions that do not match a preset.
Cancellation does not require a reason. Starting a Task remains a direct action.

Agent and Task model settings keep model, reasoning, and Fast controls visible.
Task models use provider availability without a separate enable control.
Device notification settings precede server delivery setup.

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
UI requests authorize browser inspection unless the user restricts it.
Follow [browser inspection](browser-inspection.md) at desktop and phone widths.
Report when a relevant state could not be inspected. Do not add UI tests unless requested.

## Assistant message bubbles

The server saves separate paragraphs in Chat replies as separate messages during streaming.
Web and native clients render those saved boundaries without splitting text.
Adjacent web messages from the same speaker and turn share one visual group.
The backend supplies `presentation: bubble | marker` in Chat metadata and each Task output section.
Main Chat omits reasoning summaries from saved-page and live API content. Stored summaries remain intact.
Task transcripts retain reasoning-summary markers. Progress updates, full reasoning traces, and responses use bubbles.
Saved pages and live snapshots use the same choice. Clients do not classify provider phases.
Task reasoning summaries and tool calls use one marker renderer with shared typography and spacing.
MCP markers show a returned resource title or an explicit input reference.
Service icons use the shared favicon service with the saved MCP website domain.
If the website URL is absent, markers use the server icon domain.
If the icon is unavailable, the marker uses a plug icon.
Consecutive markers from the same turn and agent collapse into one expandable group.
The collapsed group shows its latest item and a count. Bubbles separate groups.
Reasoning markers keep the brain icon and have no individual disclosure.
Hover shows the complete summary when its row is truncated. Groups still expand to show their markers.
Reasoning markers separate adjacent bubble groups. Final answers retain message bubbles.
Blank lines and standalone three-dash separators create boundaries outside fenced code.
Code fences retain their internal blank lines.
Citations use each paragraph's source range.
The first paragraph retains the complete provider message for model history.
Later paragraphs are display records and do not duplicate model history.

Streaming and completed replies use the same message boundaries and record IDs.
An unfinished separator line stays hidden until more text resolves it.
Ordinary text streams without waiting for a complete line.
Existing saved messages remain unchanged.
Human messages and full reasoning traces retain bubbles.
Provider summary flags control presentation. Text length does not select a type.
Saved Codex sections without that flag use their recorded summary/content section ranges.
This read rule remains necessary while those saved sections exist.
