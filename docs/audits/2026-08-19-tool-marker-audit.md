# Recent tool marker audit

- **Date:** 2026-08-19
- **Status:** Static evidence snapshot with open recommendations
- **Source revision:** `6f7425d87cb4`
- **Runtime verification:** Not performed

This audit reviews saved tool calls and the shared Chat and Task marker.
It does not define current product behavior.

## Outcome

The marker is compact, but it often hides the action, target, outcome, and failure reason.

The Task path provides less structured display data than Chat.
The web client adapter builds only `display.name` from the raw tool name.

The latest 250 global calls contain 13 exact tool names.
Eleven of these 13 tool types omit a useful target from the primary line.

Fifty-two calls failed.
The primary line never shows failure text.

Two cancelled calls have no results.
They appear pending instead of cancelled.

Every sampled Task detail leads with a correlation identifier.
Most also show complete input and output JSON.
This order makes technical evidence more prominent than the human outcome.

Consecutive calls collapse into groups.
A closed group hides its count and any earlier failure.

## Scope and method

The source was `.noema-dev/db/noema.sqlite3`.
All queries used read-only SQLite access.

Chat calls use `conversation_items`.
Task run calls use `agent_run_items`.
Both tables store calls with `kind = 'tool_call'`.

No global sequence exists across both tables.
The sample used saved time and SQLite insertion order.

The frozen global sample ends at Task call row `11937`.
It covers `2026-08-18T20:57:25.571Z` through `2026-08-19T00:56:32.767Z`.

All 250 newest calls came from Task runs.
They cover six Tasks and 27 runs.

The database held 723 Chat calls and 3,031 Task run calls at the cutoff.
Checked action requests were excluded because they do not include every marker call.

I inspected field names, types, counts, and marker rules.
I did not copy saved arguments, results, or private values into this audit.

### Chat control sample

The global sample did not exercise Chat display metadata.
I therefore checked the latest 250 Chat calls as a control sample.

The Chat sample covers `2026-08-11T14:36:00.346Z` through `2026-08-18T20:46:47.287Z`.
It contains 36 exact tool names.

Chat hides three internal `enable.*` calls.
The remaining 247 calls contain 34 visible tool names.

The control sample does not change the global frequency table.
It confirms the shared marker and grouping defects.

## Human job and information order

| Question | Answer |
| --- | --- |
| Human | A person reviewing agent progress or investigating a failure. |
| Job | Understand what happened and decide whether action is necessary. |
| Focal point | The current meaningful action, outcome, or failure. |
| Now | Status, action, safe target, and concise outcome. |
| Next | Purpose, provider, fallback, and useful result evidence. |
| Later | Exact arguments, output, identifiers, and runtime debug data. |
| Omit | Duplicate state, raw screenshot data, and lifecycle handshakes. |
| Density | Compact, because markers sit inside dense transcripts. |

## Current rendering path

1. [Task run item mapping](../../apps/web/src/components/chatDetail/task/taskRunItemMapper.ts#L123) creates shared transcript activities.
   It sets `detail_mode` to `complete` and copies the raw tool name into `display.name`.
2. [The render model](../../apps/web/src/components/transcript/renderModel.ts#L407) pairs calls with results.
   It also groups consecutive calls from the same turn and agent.
3. [The marker model](../../apps/web/src/components/transcript/markerModel.ts#L24) selects the primary line.
   It prefers `display.description`, then web subjects, first-party names, and `display.name`.
4. [The marker component](../../apps/web/src/components/transcript/ToolMarker.tsx#L235) renders groups, status icons, and disclosure.
5. [The detail attachment](../../apps/web/src/components/transcript/ToolDetailAttachment.tsx#L30) renders screenshots and detail rows.
6. [Chat replay](../../crates/noema-api/src/graphql/replay.rs#L36) hides internal tool enablement calls.

Chat and Task both enable consecutive marker grouping.
Task enters the shared component through a separate adapter.

## Shared marker behavior

| State or structure | Current display |
| --- | --- |
| Running | Spinner and primary line. No visible status word. |
| Completed | Green check and primary line. No visible outcome. |
| Failed | Red X and primary line. Failure summary stays in hover text or disclosure. |
| Cancelled or skipped | Green check with a result, or pending clock without one. |
| Closed group | One current or retained call. Count and aggregate status stay hidden. |
| Open group | Wrench icon, `N tool calls`, and every child marker. |
| Expanded title | `Used <identity>`, including failed calls. |
| Task detail | Correlation, complete input JSON, complete output JSON, and optional screenshot. |
| Static marker | Disabled button when no detail exists. |

Status icons are hidden from assistive technology.
Completed, failed, cancelled, and pending rows can have the same accessible name.

## Latest 250 global calls

| Exact tool | Calls | Outcome | Current primary line |
| --- | ---: | --- | --- |
| `web.browse.open` | 98 | 46 completed, 50 failed, 2 cancelled | `Browser interaction` |
| `task.files.write` | 37 | 37 completed | `task.files.write` |
| `task.files.read` | 29 | 28 completed, 1 failed | `task.files.read` |
| `web.search` | 25 | 25 completed | Exact saved query with a search icon |
| `web.browse.close` | 19 | 19 completed | `Browser interaction` |
| `task.files.list` | 13 | 13 completed | `task.files.list` |
| `task.continue_execution` | 7 | 7 completed | `task.continue_execution` |
| `task.finish_execution` | 6 | 6 completed | `task.finish_execution` |
| `task.finish_review` | 6 | 6 completed | `task.finish_review` |
| `task.finish_planning` | 5 | 5 completed | `task.finish_planning` |
| `task.list` | 3 | 3 completed | `task.list` |
| `web.browse.interact` | 1 | 1 failed | `Browser interaction` |
| `web.fetch` | 1 | 1 completed | Page title, hostname, or URL with a web icon |
| **Total** | **250** | **196 completed, 52 failed, 2 cancelled** | |

### Current expansion by tool

| Exact tool | Current expanded title | Current expanded evidence |
| --- | --- | --- |
| `web.browse.open` | `Used Browser interaction` | Correlation, URL, wait mode, reason, browser state, page data, and screenshot data |
| `task.files.write` | `Used task.files.write` | Correlation, path, complete file content, and result path |
| `task.files.read` | `Used task.files.read` | Correlation, path, complete file content, or raw failure data |
| `web.search` | `Used Web Search` | Correlation, query, status, and raw result JSON |
| `web.browse.close` | `Used Browser interaction` | Correlation, empty input, browser state, page data, and screenshot data |
| `task.files.list` | `Used task.files.list` | Correlation, path, and complete entries JSON |
| `task.continue_execution` | `Used task.continue_execution` | Correlation plus empty input and output objects |
| `task.finish_execution` | `Used task.finish_execution` | Correlation plus empty input and output objects |
| `task.finish_review` | `Used task.finish_review` | Correlation, decision, and complete feedback |
| `task.finish_planning` | `Used task.finish_planning` | Correlation, complexity input, and complexity output |
| `task.list` | `Used task.list` | Correlation, empty input, complete Task collection, and page state |
| `web.browse.interact` | `Used Browser interaction` | Correlation, revision, element reference, action, value, and raw failure data |
| `web.fetch` | `Used Fetched Web Page` | Correlation, URL, fetch status, and raw result JSON |

Failed calls still use the word `Used` in the expanded title.
Each value is capped at 88 pixels and scrolls when necessary.

Every sampled call contains an arguments object and a correlation identifier.
Two cancelled browser calls have no linked result.
The other 248 calls have one linked result.

Task run items store no display object.
The web client Task adapter creates only `display.name` from the exact tool name.

Browser Task output can include encoded screenshot data.
The detail then renders the image and the complete output JSON.

## Tool recommendations

These examples define information order, not final copy.
The producer should supply structured display values.
The frontend must not infer meaning from English phrases.

| Exact tool | Recommended primary line | Supporting detail |
| --- | --- | --- |
| `web.browse.open` | `Opening <host>`, `Opened <title>`, or `Could not open <host>` | Show the short failure reason after failure. Keep URL, wait mode, and screenshot in disclosure. |
| `task.files.write` | `Wrote <file>` | Show path, size, and replacement outcome. Put full content behind `File contents`. |
| `task.files.read` | `Read <file>` or `Could not read <file>` | Show path and size. Put full content behind `File contents`. |
| `web.search` | `Searched web for “<query>”` | If a result count exists, show it after completion. Keep provider and fallback as evidence. |
| `web.browse.close` | `Closed browser` | Fold this cleanup into a browser session summary when no decision depends on it. |
| `task.files.list` | `Listed files in <folder> · <count>` | Show entry names in disclosure. Omit raw collection JSON from the first expansion. |
| `task.continue_execution` | No separate marker | Fold continuation into the next Task run boundary. Keep the call in technical history. |
| `task.finish_execution` | No separate marker | Use the existing Executor completion boundary and submitted result. |
| `task.finish_review` | `Review approved`, `Changes requested`, or `Human decision needed` | Show feedback below the decision. Keep identifiers in technical details. |
| `task.finish_planning` | `Plan ready · <complexity>` | Merge this outcome with the Planner completion boundary. |
| `task.list` | `Checked current Task` | Show current stage and relevant next action. This scoped tool does not list all Tasks. |
| `web.browse.interact` | `Clicked <element>`, `Filled <field>`, or `Pressed <key>` | Use a structured accessible element name. Do not show entered values by default. |
| `web.fetch` | `Read <page title> — <host>` | Keep the exact URL, content size, provider, and fallback in disclosure. |

The default line should use one stable structure:

```text
<state-aware action> <safe target> · <short outcome>
```

The target can contain authorized private information.
Its omission is a hierarchy choice, not a secrecy transformation.

Secrets must remain absent.
Technical disclosure should preserve ordinary identifiers and authorized private evidence.

## Control sample findings

The visible Chat control sample contains 247 calls.
Custom descriptions drive 156 primary lines.
Generic fallback labels drive the other 91 lines.

This split gives one tool inconsistent wording across calls.
Some descriptions state agent intent instead of the completed outcome.

The calls collapse into about 102 groups.
Fifty-nine groups contain two through ten calls.

The closed group shows no count.
It also hides an earlier failure when the retained call completed.

Thirty-two visible results failed.
Four failed markers have no `Error` detail row.

All failed items have a saved summary.
The primary row exposes that summary only through native hover text.

One cancelled result shows a green completion check.
Three cancelled calls without results show a pending clock.

Every visible Chat call is expandable.
The details produce 216 Input rows and 207 Output rows.

Only four details use the saved Result row.
Most saved result strings only say `Completed`, `Done`, or `Failed`.

Fifty-one browser calls contain valid screenshots.
The marker can render those images.

Two Notion connections use the same `Notion fetch` label.
The marker ignores provider or account context when identical actions become ambiguous.

## Ranked changes

### P1 — Correct status truth

Map completed, failed, cancelled, interrupted, skipped, pending, and running states explicitly.
Reconcile call and result state before choosing the marker state.

### P1 — Expose failure and accessible status

Show a short failure reason in the primary row.
Add status text to the accessible name or description.
Do not depend on color, icon shape, hover text, or expansion.

### P1 — Make closed groups honest

Show call count and aggregate severity while the group is closed.
An earlier child failure must outrank a later success.
Keep the latest active action visible while work runs.

### P2 — Reuse one structured display contract

Keep the existing display metadata as the presentation authority.
Fill it for Task run items from structured tool arguments and results.

Use action, target, outcome, provider, and fallback fields.
Do not add tool-specific JSX or direct phrase matching.

### P2 — Put human detail before technical detail

Lead with purpose, target, outcome, and useful evidence.
Place full input, output, correlation, and runtime fields under `Technical details`.

Exclude screenshot bytes from JSON previews when the image is already visible.
Keep complete source data reachable through the technical record.

### P2 — Add useful producer outcomes

Start with browser open, Task file tools, Task list, Calendar lists, and Gmail lists.
These high-volume tools cause most generic rows.

### P2 — Add context only when it resolves ambiguity

Show a reviewed account, connection, calendar, mailbox, or workspace label when identical actions could be confused.
Never show internal server identifiers as the default context.

### P3 — Fold lifecycle-only Task calls

Use existing Task run boundaries for planning, continuation, execution, and review transitions.
Keep exact calls available in technical history.

## Acceptance scenarios for a future change

1. A running browser open names its host and reports `Running` to assistive technology.
2. A failed browser open shows the host and short error without expansion.
3. A cancelled call never shows a completion check or pending clock.
4. A mixed group shows its count and child failure while closed.
5. A file write names its file without showing file content by default.
6. A review marker names its decision and keeps feedback adjacent.
7. Two connector accounts remain distinguishable without exposing internal identifiers.
8. A screenshot appears once and keeps encoded bytes out of ordinary detail.
9. Exact arguments, output, and correlation remain reachable under technical disclosure.

## Validation notes

The static trace covered Chat replay, Task mapping, marker grouping, status, names, and details.
No browser, device, screen-reader, or touch inspection occurred.

The focused Task mapper test currently disagrees with the marker model.
The [test expects a Success row](../../apps/web/src/components/chatDetail/task/taskRunItemMapper.test.ts#L69).
The [complete detail builder omits that row](../../apps/web/src/components/transcript/markerModel.ts#L154).

Run a fresh visual and accessibility review before implementing these recommendations.
