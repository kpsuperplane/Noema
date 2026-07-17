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

The Work header contains:

- a page title and the tab list: Board, List, Needs You, Activity, Completed;
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
independent page shells. A tab change focuses its panel heading only when
keyboard-initiated; pointer navigation preserves reading position.

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

Each card shows:

- task title and optional project label;
- stage label, elapsed age, and last-update time;
- a nullable current-run chip;
- attention badge(s) with text, icon, and non-color cue;
- only server-returned semantic action buttons.

| Stage | Typical valid controls |
| --- | --- |
| Inbox | Queue, edit title/description/project, Cancel |
| Queue | Cancel |
| Doing | Cancel |
| Waiting | Answer, Retry where allowed, Cancel |
| Review | Accept, Request Changes, Cancel |

The table is explanatory only. The card consumes validActions, so a recovery
gate or future policy change cannot leave a stale control on screen. There is
no drag-and-drop, generic move action, or direct edit outside Inbox.

## List

List is a dense representation of the same workTasks query, not a separate
task source. It supports text search, project, stage behavior, attention-only,
and active/terminal scope filters. Rows have title, project, stage, current
run, attention, updated time, and a compact action menu. Screen readers get
the same row content through a semantic table on wide screens and labelled
list items on narrow screens.

Search has a 250 ms debounce, resets pagination when its normalized value
changes, and writes q to the route only after the input settles. An empty
filter result says which filters are active and provides **Clear filters**.

## Needs You

Needs You is an action queue derived from needsYou, never a manually maintained
client inbox. It groups TaskAttention by:

1. Clarification
2. Approval
3. Recovery
4. Ready for acceptance

Each item names the task, optional project, concise safe reason, age, and
primary action. Clarification opens an answer form bound to the exact gate id.
Approval opens the existing approval interaction for the named gate, which
returns a structured Approved/Declined decision and non-empty message; it does
not render a new capability-policy UI. Recovery renders Answer/Retry only when
the server includes them in `validActions`; an invariant gate with no safe
continuation directs the user to detail and Cancel. Review Ready offers Accept
and Request Changes, with the review evidence available in the shared task
detail.

Answer and Request Changes use modal or inline forms that retain typed text on
a stale revision error. A successful mutation moves focus to the updated item
or its group heading; if the item leaves Needs You, focus returns to the next
item, then the group heading, then the page heading. This prevents a keyboard
user from losing their place when an action removes a row.

## Activity and Completed

Activity displays workActivity as a cursor-paginated timeline. Each entry uses
the stable event kind to render a compact sentence, timestamp, linked
task/project, and optional run role. Raw JSON remains hidden behind an advanced
inspection disclosure on task detail; it is not shown in the everyday
timeline. **Load more activity** follows the global event cursor and preserves
the reader's scroll anchor.

Completed displays completedTasks with a Completed/Cancelled filter, project
filter, search, completion date, accepted-result summary when available, and
Reopen. Reopen is a semantic action that begins a new Inbox cycle and keeps all
historic contracts/runs/reviews visible; the UI must not suggest that it
resumes a cancelled worker.

## Task detail

The existing chat task rail is split into reusable primitives and a
GraphQL-backed Work detail container. Both consume the same TaskDetail, cursor
hooks, live run mapper, transcript renderer, and semantic action components.
The new implementation removes current UI assumptions that task status is an
execution phase.

Task detail contains these ordered sections:

1. **Overview** — title, description, project, single stage, current run,
   attention, source, age, and valid actions.
2. **Current contract** — immutable request, optional execution plan, exact
   criteria, complexity, model and policy snapshots, workspace/project context
   snapshot.
3. **Attention and messages** — unresolved/current gate, human answer or
   change-request history, and resolved gate history.
4. **Contract revisions** — immutable contract-version list with parent
   contract and human-amendment links.
5. **Runs and transcripts** — Planner, Executor, and Reviewer runs with
   explicit role/status labels, lineage, usage, transcript paging, and safe
   errors.
6. **Evidence and review** — submissions, criterion evidence, reviewer
   outcomes, requested changes, and accepted review.
7. **Artifacts and result** — linked immutable artifact versions and the
   accepted result where one exists.
8. **Activity** — task-filtered work-event timeline.

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
broad management. It is not a miniature Board, transcript feed, or project
chat. Routine run transitions can update a current-run chip, but they do not
insert conversational messages.

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
- Provide keyboard access to every card, action menu, tab, timeline link, and
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
