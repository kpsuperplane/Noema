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
  in the operating system credential store. Access tokens remain in its native
  process. Its webview receives connection state and the validated server origin.

## Interaction contracts

Chat reconstructs its transcript from durable pages and merges live events by
stable item identity. After reconnect, the client refetches active reads and
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
When no API exists, it shows one route to Chat instead of empty data sections.
Client import occurs only when a structured API connection action requires it.

Chat presents one pending human intervention at a time with queue navigation.
Chat omits approval-request activity notices. The tool marker shows the activity,
and the pending intervention card provides approval controls.
Tasks and dedicated queue surfaces can show the complete pending list.
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

Task detail uses one Astryx tab bar below the title for files and `Transcript`.
Above 1200 pixels, Task detail shows both views side by side with a 600-pixel Transcript.
The wide layout keeps a vertical divider between the views.
Task details share the outer top border with the task list.
The floating Task context card occupies the Workspace column in the wide layout.
Task and recurring task action bars share the document’s 760-pixel width limit and centered side margins.
The Task loading placeholder uses the same width and placement.
Transcript fills the right column’s height.
The action bar shows task state above the latest live activity at every width.
Human attention takes priority and shows its summary. Inactive tasks omit old run activity.
Activity stays on one line; full content remains in Transcript. Agent identity stays with the avatar.
At smaller widths, Task detail uses tabs and swipe navigation.
File tabs select the UTF-8 preview. In wide layouts, Transcript stays visible beside the selected file and has no tab.
`Result` and `Task` appear first. Other files follow in path order.
The combined tab bar stays above scrolling content and scrolls horizontally when needed.
Its first label aligns with the title text; the bar offsets the tabs’ built-in horizontal padding.
A completed Task opens `RESULT.md`; another Task opens `TASK.md`.
Completion selects `RESULT.md` once when `TASK.md` was open.
Result previews preserve provider citations.
Task, Result, Review, and support-file previews share a 760-pixel reading container with 24-pixel side margins.

Artifact detail previews Markdown and plain text directly. It uses the shared
file parser for supported spreadsheets. Raster images and PDFs use authorized
inline routes. HTML runs only in a sandbox after Noema removes active elements,
navigation, event handlers, and external resources. SVG remains download-only.

Task and project documents share the Markdown viewer and Milkdown editor.
Task creation uses `/tasks/new` and the normal detail area beside the Tasks list.
New tasks and Inbox tasks share the title header, document layout, timing summary, and editable body styling.
The new task title sits above a full-width divider. Project and timing controls precede the instructions.
The title and instructions have accessible names without visible field labels.
The instructions fill the remaining height in rich text and source modes.
Project and Schedule controls use visible labels. Add to Inbox and Run now remain visible together.
Opening Schedule in capture enables scheduling immediately. Remove schedule at the bottom disables scheduling and closes the popover.
Capture omits working-folder controls. Creation buttons stay compact and align to the right.
Capture and task detail use one `TaskActionBar` component for the floating frame, width, border, shadow, and bottom spacing.
Creation actions use the detail bar’s compact icon style. Run now uses its filled green Play icon.
A pending creation action replaces only its icon with a spinner. Its label stays visible.
Above 1200 pixels, capture reserves the same 600-pixel right panel as task details, with no content before creation.

Editable task and recurrence titles and instructions show a light hover background that extends 12 pixels horizontally and 4 pixels vertically beyond the field.
Body editors add 8 pixels of internal vertical padding, giving the highlight a 12-pixel inset on every side.
A non-interactive pseudo-element draws the highlight without changing layout. Focus replaces its background with a light border.
Hover and focus colors use the micro transition; reduced motion removes the transition.
Task body editors omit the block drag handle and the manual Markdown source switch.
Task titles have no focus underline. Read-only and busy fields keep their normal appearance.
Saving and saved states use a muted spinner and checkmark in a fixed field gutter, with screen-reader announcements and no layout shift.
Error and retry messages remain visible.
Existing task and recurrence fields save on blur. Both reuse the capture title and Markdown editor.
The editor reads current text before saving, changing source mode, starting, or leaving a task.
Failed saves preserve drafts and prevent navigation. Stale saves require acknowledgement before retry.
Document saves retain revision and digest fences. Active tasks follow server-provided edit availability.
Project documents retain their existing explicit editing controls.
The source editor remains available when rich parsing fails. iOS retains its native source editor.

Task titles use smaller text and wrap to show the complete title.
Task details keep Close on mobile and the combined file/Transcript tabs above scrolling content.
In the wide layout, the title stays above Workspace within its column.
Task loading uses the same columns, title position, tabs, and context placement.
Startup uses a Tasks placeholder when the current route opens a task.
The header and selected tab remain stable from Inbox through Queued and Running.
Project and Task agent choices appear in the Workspace body. Working folder and repeated revision/source metadata are omitted.
Timing appears beside Schedule or Reschedule. Lifecycle controls remain in the existing floating bar.
An Inbox task says Ready when you are. No separate Progress section appears.
Task list groups show their names and counts without explanatory description rows.

Recurrence detail reads its separate template `TASK.md`. Edits apply to future runs.
Above 1200 pixels, recurring task details show Run history in a 600-pixel right panel.
The title, instructions, settings, and schedule actions stay in the left panel.
Both panels scroll independently. At smaller widths, Run history follows the settings.
History reuses task list cards and linked task status. Skipped slots do not imply an existing task.
A recurring template remains selected when it creates a run. Opening that run remains separate navigation.
Schedule editing recognizes supported presets and preserves unmatched cron expressions.
Task confirmations and schedule dialogs reuse task cards. Footers keep equal-width actions reachable, with the primary action on the right.
Cancellation does not ask for a reason. Start task remains a direct action.

Agent and task model settings keep model, reasoning, and Fast controls visible.
Task models use provider availability without a separate enable setting.
Device notification settings precede server delivery setup. Provider detail headers align with the list header.

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

## Assistant message bubbles

The server saves separate paragraphs in completed Chat replies as separate messages.
Web and native clients render those saved boundaries without splitting text.
Adjacent web messages from the same speaker and turn share one visual group.
Progress text and final answers keep that group while retaining their text styles.
Blank lines and standalone three-dash separators create boundaries outside fenced code.
Code fences retain their internal blank lines.
Citations use each paragraph's source range.
The first paragraph retains the complete provider message for model history.
Later paragraphs are display records and do not duplicate model history.

A message streams in one record until its completion supplies stable boundaries.
Existing saved messages remain unchanged.
Human messages and readable reasoning remain unchanged.
