# Noema Work: User Interface Contract

**Status:** implementation specification
**Owns:** Work information architecture, routes, shared task detail, live UI
behavior, and accessibility
**Read with:** [the product contract](00-product-contract.md),
[the GraphQL contract](06-graphql-contract.md), and
[the existing frontend contract](../frontend/current-contract.md)

Work is the durable management surface for agent-executed tasks. Chat remains
Noema's home surface; Work makes queues, attention, history, and task evidence
easy to inspect without turning routine worker progress into chat noise.

The client renders one authoritative **stage** and its derived projections. It
does not maintain a frontend lifecycle enum, infer worker state from text, or
offer a generic task-state control.

## Routes and navigation

| Route | Purpose |
| --- | --- |
| /work | Work home, defaulting to the Board tab. |
| /work?view=board, list, needs-you, activity, or completed | One Work shell with route-validated tab state. |
| /work/tasks/$taskId | Deep-linked task detail in the Work shell. |

The /work search schema also accepts optional project, q, and terminal-view
filters. Invalid view values normalize to Board; invalid project/task ids
produce the normal unavailable or empty state without exposing another owner's
object. Query parameters are additive filter state, not a second source of
task truth.

The shell gains a first-level **Work** item between Home and Memory. Personal
is implicit in the shell and never displayed as a redundant workspace picker.
The page toolbar has an optional project filter, a bounded project-management
menu, and **New task**. New task creates an Inbox item through the semantic
capture mutation; it never creates a local-only draft card.

Opening a task from a board card, list row, attention item, activity event, or
chat card navigates to /work/tasks/$taskId. On wide screens the route can
render the same detail component as a rail beside retained Work context. On
narrow screens it is a full-width route. Back returns to the prior route and
filter when browser history has one, otherwise /work.

## Shared data and rendering rules

All Work components consume generated GraphQL types. The local presentation
layer may format timestamps, status labels, and safe activity text, but it
must not redefine a TaskStatus union or calculate a stage from run/gate fields.

| GraphQL field | UI use |
| --- | --- |
| task.stage | Column, status label, filters, and terminal grouping. |
| task.currentRun | Small live activity chip such as Planner queued or Reviewer running. |
| task.activeGate and task.attention | Needs You grouping and attention badge. |
| task.latestReview | Review-ready summary and Accept/Request Changes eligibility. |
| task.validActions | The only authority for visible action controls. |
| task.revision and task.generation | Mutation preconditions; they are never shown as task state. |
| workEvents | Invalidation, timeline updates, and reconnect backfill. |

Board/list cards use TaskSummary only. They do not query a full task separately
for every card. Opening a task loads TaskDetail and its bounded connections
lazily. The board bootstrap uses workOverview, then pages its selected column
through workTasks when needed.

Work uses one workspace-level workEvents subscription while its root is
mounted. A task detail uses filtered taskEvents for high-frequency run and
transcript updates. Event receipt invalidates/refetches the affected bounded
query or detail connection; it never locally synthesizes a new stage. On
reconnect, the client resumes from its saved cursor and refetches visible
queries after subscription readiness.

## Work shell

The Work surface has one compact toolbar containing:

- an accessible page title and the tab list: Board, List, Needs You, Activity,
  Completed;
- project filter with **All work**, active projects, and an explicit archived
  choice only where historic work is being viewed;
- search on List and Completed, with debounce reflected in the route;
- **New task**, which opens a minimal capture form for title, description, and
  optional project;
- project create, rename, archive, and reopen controls in a small menu.

Project controls manage containers only. They do not create scoped chat,
select an agent, change a model pool, fetch project memory, or grant
capabilities. Archiving a project does not cancel or hide its tasks; active
tasks remain reachable through direct links and project filtering.

The tab bar is a semantic tablist with route-backed selected state. Board,
List, Needs You, Activity, and Completed panels are labelled regions, not five
independent page shells. Arrow, Home, and End keys use roving tab focus; Tab
then enters the selected view.

## Board

The Board renders the five seeded active workflow stages returned by
workOverview.activeColumns in workflow display order:

1. Inbox
2. Queue
3. Doing
4. Waiting
5. Review

The renderer uses stage ids and server-provided labels. The ordered names above
describe the initial workflow, not a client-side mapping that future workflow
customization could bypass.

Each column has a visible count, a bounded vertical list, and a Load more
control where its TaskConnection has a next page. On a narrow viewport the
columns become horizontally scrollable labelled sections; they do not become
drag targets.

Each card shows the task title, one compact context line (attention, current run,
or project), and relative update time. The lane already communicates stage, so
cards do not repeat it. Descriptions, evidence, metadata grids, and action walls
do not appear on the board; opening the card reveals the shared detail rail and
its server-returned semantic actions. There is no drag-and-drop, generic move
action, or direct edit outside Inbox.

## List

List is a dense representation of the same workTasks query, not a separate
task source. It supports text search, project, stage behavior, attention-only,
and active/terminal scope filters. Rows have title, optional attention note,
project, stage, and updated time. Commands stay in detail. Narrow rows retain
project and stage as compact metadata rather than dropping the context.

Search has a 250 ms debounce, resets pagination when its normalized value
changes, and writes q to the route only after the input settles. An empty
filter result says which filters are active and provides **Clear filters**.

## Needs You

Needs You is a single dense priority queue derived from `needsYou`, never a
manually maintained client inbox. Each row names the attention kind, task,
concise safe reason, project, age, and required action. Opening a row preserves
the queue behind the shared detail rail, where Answer, Retry, Accept, Request
Changes, or Cancel is rendered only when the server includes it in
`validActions`. This keeps exact gate/revision checks and typed drafts in one
interaction instead of duplicating command forms across the queue.

## Activity and Completed

Activity displays `workActivity` as a cursor-paginated timeline. Each entry uses
the stable event kind to render a compact sentence, timestamp, and linked task.
Raw JSON is not shown in the everyday UI. **Load more** follows the global event
cursor.

Completed displays `completedTasks` with a Completed/Cancelled filter, project
filter, search, project, stage, and completion date. Opening a row exposes the
accepted result and Reopen action in detail. Reopen begins a new Inbox cycle
and keeps historic runs/reviews visible; the UI must not suggest that it resumes
a cancelled worker.

## Task detail

Work extends the pre-existing chat task rail instead of introducing a second
detail system. Chat and `/work/tasks/$taskId` render the same query adapter,
compact status/section primitives, run conversation view, transcript renderer,
and semantic action component. At 980 px and wider, Work context remains
visible beside a 440 px rail; below that breakpoint, detail becomes a full
surface.

Detail uses progressive disclosure in this order:

1. current human attention in a visually prominent top card, with its response
   field and Answer action embedded in that same card;
2. compact derived status, remaining server-returned semantic actions, and the
   original request;
3. the Planner/Executor/Reviewer timeline, with a selected run opening the
   existing conversation-style transcript in place;
4. exact acceptance criteria and current evidence;
5. proposed or approved result and task artifacts;
6. durable human/task updates and a safe failure notice where applicable;
7. low-priority task metadata.

Presentation labels may combine structured stage behavior, current run kind,
and latest review verdict, but this adapter is not persisted and never becomes
a second task-state authority.

The top task-control row is capability-driven and uses icon controls with
accessible labels and hover tooltips. Cancel and Retry appear only when the
server includes those semantic actions in `validActions`; Open in Work appears
only outside the Work route, and Close appears when the host surface supplies a
close action. When human attention is active, this row lives inside the top of
the decision card instead of being duplicated in the rail header.

| Action | Interaction |
| --- | --- |
| Queue | Confirms the current Inbox version and submits its revision/generation. |
| Answer | Requires a non-empty response and the explicit gate id; an Approval gate also sends Approved or Declined. |
| Retry | Offers optional guidance and names the recovery gate. |
| Accept | Sends task revision/generation; the command verifies the current approved review. |
| Request Changes | Requires non-empty feedback and creates a new contract revision. |
| Cancel | Uses a confirmation dialog because it fences active work. |
| Reopen | Explains that historic work remains archived and queues no worker until Queue is selected. |

The Chat detail rail retains its compact form and opens Work for broad
management. Chat cards and rail controls use the same mutation hooks as Work,
so a user cannot receive different action availability or concurrency behavior
depending on entry point.

## Chat-side Work panel

The primary chat shell includes a compact Work panel with a bounded list of
active task summaries and a bounded Needs You section. It uses the same
`TaskSummary`, `TaskAttention`, task-detail primitives, semantic action hooks,
and event invalidation as `/work`; it does not maintain a smaller task state
model or query one detail per row. Personal remains implicit.

The panel prioritizes Needs You, then recently updated active tasks, and links
to the shared task detail or the corresponding `/work` tab for pagination and
broad management. These sections are mutually exclusive: a task with current
structured attention appears in Needs You and is omitted from Active. It is not
a miniature Board, transcript feed, or project chat. Routine run transitions
can update a current-run chip, but they do not insert conversational messages.

## Loading, failure, and live states

| State | Required behavior |
| --- | --- |
| Initial loading | Use column/row/timeline skeletons that preserve layout; do not show fake task state. |
| Empty workspace | Explain that tasks appear when captured or delegated and offer New task. |
| Empty column/group/filter | State the relevant stage or filter and provide one useful next action. |
| Query error | Preserve any loaded data, show a retry action, and avoid replacing the whole shell with an error. |
| Missing/unavailable detail | Show a neutral unavailable page with Back to Work; do not distinguish authorization from deletion. |
| Mutation pending | Disable only the submitted action/form, show progress, and retain other readable content. |
| Stale revision/generation | Refetch authoritative data, preserve unsent answer/feedback/edit text, and explain that the task changed elsewhere. |
| Missing execution configuration | Render the server attention/error and link to existing Task Executor settings; do not invent model choices in Work. |
| Subscription reconnecting | Show a small non-blocking Updating Work status, leave cached data readable, and refetch after ready. |
| Terminal task | Stop high-frequency detail subscription after its final event, while keeping historic query pagination available. |

No state is hidden solely by color. A status chip always has text, a badge
uses an icon and label, and errors identify a retry or safe next step.

## Accessibility and responsive behavior

Work follows the existing Atryx and StyleX conventions and keeps source files
below the project's size threshold by separating route, data hook, shared
card/detail primitive, and view components.

- Use landmarks for Work navigation, toolbar, main view, task detail, and
  activity timeline; headings follow a single visible hierarchy.
- Use buttons for commands, links for navigation, labelled forms for capture,
  answer, feedback, and project edits, and native dialog focus management.
- Announce mutation success/failure and reconnect state through a polite live
  region; do not announce every worker transcript event.
- Preserve focus when a task detail rail opens/closes and when an action moves
  a task between views. Escape closes only the current dialog or rail.
- Provide keyboard access to every card, detail action, tab, timeline link, and
  Load more control. Board columns remain navigable without pointer dragging.
- Respect reduced motion and avoid using animation to conceal a data refresh.
- On narrow screens, toolbar controls wrap, board columns scroll horizontally
  with visible labels, list switches to labelled cards, and detail becomes a
  full route with a stable Back action.

## UI acceptance checks

The UI is ready when:

- /work and /work/tasks/$taskId are direct, route-safe entry points;
- Board, List, Needs You, Activity, Completed, and task detail are projections
  of one generated GraphQL contract and never each maintain task state;
- cards show one stage plus derived current-run/attention information, with
  semantic controls only;
- chat and Work share task detail/actions, while routine run progress remains
  in Work;
- pagination, reconnect recovery, stale mutations, empty states, and missing
  configuration are understandable without a reload;
- keyboard, focus, screen-reader, narrow-screen, and reduced-motion behavior
  remain intact;
- frontend validation uses code generation, TypeScript, ESLint, and production
  build checks without adding a new UI test suite.
