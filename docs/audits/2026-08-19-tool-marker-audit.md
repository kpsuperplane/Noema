# Tool marker audit

- **Date:** 2026-08-19
- **Status:** Built-in marker changes implemented; connected-tool work deferred
- **Audit baseline:** `6ea0a1f7`
- **Visual verification:** Not performed

This audit reviews saved tool calls and the shared Chat and Task marker.
It documents current behavior and proposes clearer marker content.

## Outcome

The refreshed latest sample contains 300 calls and 15 exact tool names.
The complete stored history contains 3,933 marker calls and 93 named exact tools.

The complete history also contains seven unnamed legacy Task calls.
Three internal enablement calls are stored but hidden from Chat.

The current `web.search` marker is already clear.
It shows a magnifying glass and the exact saved query.

The current `web.fetch` marker shows a globe and the page title or host.
Most other Task markers show raw exact tool names.

Browser actions share the text `Browser interaction`.
This text hides whether the agent opened, clicked, waited, or captured a page.

Task status conversion was the largest correctness problem.
Four cancelled Chat results showed a red failure icon because GraphQL compressed their state.
Nine skipped Task results showed a green completion check.
Fifty-four calls without results showed a pending clock.

Closed groups hide their call count and earlier failures.
Expanded Task details put correlation and complete JSON before the human outcome.

## Corrections from the first audit

The first audit covered only `kind = 'tool_call'` Chat rows.
Chat also stores 98 legacy `activity` rows that render as tool markers.

Those legacy rows contain 93 `web.search` calls and five `web.fetch` calls.
Their current markers now have explicit coverage below.

The latest sample now contains 300 calls instead of 250 calls.
It adds `web.browse.snapshot` and `web.browse.wait` to the recent catalog.

The complete catalog adds every older stored tool type.
It includes browser navigation, downloads, connectors, memory, artifacts, and Task lifecycle tools.

## Implemented built-in scope

Built-in markers now use structured action, target, outcome, and exact status data.
This applies to web, browser, Task, project, file, artifact, adapter, memory, naming, and service connection tools.

The backend formats this marker data during reads and live delivery.
Saved conversation and Task run records do not contain the formatted text.
Web, iOS, and Live Activities consume the same backend result.

Web search keeps its magnifying glass and exact query.
Web fetch keeps its globe and page title or host.

Cancelled, interrupted, skipped, failed, pending, running, and completed states now remain distinct.
Each status icon now has an accessible label.

Closed groups now show their call count and aggregate severity.
Built-in payloads and correlation data now appear under `Technical details`.
Rendered screenshots no longer repeat encoded image data in the JSON preview.

Task lifecycle-only calls now fold into existing Task run boundaries.
The exact records remain in saved Task run history.

Connected tools, such as Gmail and Notion, keep their existing generalized display path.
Dedicated `noema.*` presentation and terminal tools keep their existing non-marker surfaces.

## Scope and method

The source was `.noema-dev/db/noema.sqlite3`.
All database access was read-only.

Chat marker calls come from two stored shapes:

- 723 `conversation_items` rows with `kind = 'tool_call'`.
- 98 legacy rows with `kind = 'activity'` and `activity_kind = 'tool_call'`.

Task markers come from 3,112 `agent_run_items` rows with `kind = 'tool_call'`.

The complete inventory covers 3,933 calls.
It covers `2026-08-01T20:09:10.297Z` through `2026-08-19T01:19:55.397Z`.

I paired each call with its saved result when one existed.
I then applied the current marker functions to every pair.

The check recorded marker identity, icon kind, detail labels, screenshots, and displayed status.
It did not copy saved arguments, results, or private values into this document.

The current marker functions reported these totals:

| Surface | Calls | Visible | Expandable | Screenshots |
| --- | ---: | ---: | ---: | ---: |
| Chat | 821 | 818 | 722 | 51 |
| Task | 3,112 | 3,112 | 3,112 | 247 |
| **Total** | **3,933** | **3,930** | **3,834** | **298** |

Chat hides three `enable.*` calls during replay.
All 98 successful legacy web calls lack expansion content.

One other Chat call also lacks useful detail.
Every Task call expands because the detail always includes correlation.

## Human job and information order

| Question | Answer |
| --- | --- |
| Human | A person reviewing agent progress or investigating a failure. |
| Job | Understand what happened and decide whether action is necessary. |
| Focal point | The current meaningful action, outcome, or failure. |
| Now | Status, action, safe target, and concise outcome. |
| Next | Purpose, provider, fallback, and useful result evidence. |
| Later | Exact arguments, output, identifiers, and runtime debug data. |
| Omit | Duplicate state, screenshot bytes, and lifecycle handshakes. |
| Density | Compact, because markers sit inside dense transcripts. |

## Current rendering path

1. [Task run item mapping](../../apps/web/src/components/chatDetail/task/taskRunItemMapper.ts#L123) creates shared transcript activities.
   It sets `detail_mode` to `complete` and copies the exact tool name into `display.name`.
2. [The render model](../../apps/web/src/components/transcript/renderModel.ts#L407) pairs calls with results.
   It also groups consecutive calls from the same turn and agent.
3. [The marker model](../../apps/web/src/components/transcript/markerModel.ts#L24) selects the primary line.
   It prefers a saved description, then a web subject, then the saved name.
4. [The marker component](../../apps/web/src/components/transcript/ToolMarker.tsx#L235) renders groups, icons, status, and disclosure.
5. [The detail attachment](../../apps/web/src/components/transcript/ToolDetailAttachment.tsx#L30) renders screenshots and detail rows.
6. [Chat replay](../../crates/noema-api/src/graphql/replay.rs#L36) hides internal enablement calls.

Chat and Task use the same marker after their different adapters run.
This shared endpoint can hide important producer differences.

## Baseline shared behavior

### Primary line

Every marker starts with one status icon.
Only `web.search` and `web.fetch` get a second tool-type icon.

| Tool kind | Type icon | Primary text |
| --- | --- | --- |
| `web.search` | Magnifying glass | Exact saved query when available |
| `web.fetch` | Globe | Page title, hostname, URL, or saved target |
| `web.browse.*` | None | `Browser interaction` unless Chat has a saved description |
| Other Chat tool | None | Saved description, then saved display name |
| Other Task tool | None | Raw exact tool name |

Of 818 visible Chat calls, 435 use a saved description.
Ninety-eight use the special web subject rules.
The remaining 285 use a saved display name or fallback name.

### Status

| Saved outcome | Calls | Current marker |
| --- | ---: | --- |
| Completed result | 3,487 | Green check |
| Failed result | 379 | Red X |
| Cancelled Chat result | 4 | Red X |
| Skipped Task result | 9 | Green check |
| No result | 54 | Pending clock |

The 54 resultless calls include five cancelled calls and 49 failed Task calls.
The baseline marker does not show these saved states correctly.

Running calls show a spinner.
No status icon supplies an accessible status label.

Failed rows keep the failure summary in hover text or disclosure.
The primary line does not show the short failure reason.

### Grouping

Consecutive calls from the same turn and agent form one marker group.
The closed group shows one retained or active child.

The closed group does not show its call count.
It also does not show an aggregate failure state.

The open group shows a wrench and `N tool calls`.
Each child then uses its normal marker row.

### Expansion

Chat usually shows concise input and output previews.
It can also use saved result, provider, fallback, and display rows.

Task always requests complete detail.
The detail starts with correlation, then complete input and output values.

Expanded titles use `Using <identity>` while pending.
All other titles use `Used <identity>`, including failed and cancelled calls.

Each text value has an 88-pixel maximum height.
Long values scroll inside that area.

Browser results can include a rendered screenshot.
The same result JSON can still contain the encoded screenshot bytes.

## Verified web behavior

### `web.search`

The collapsed row shows a magnifying-glass icon.
The row text is the exact saved query when the query exists.

The row does not prefix the query with `Searched web for`.
The status icon appears before the magnifying glass.

The 93 Chat calls use the legacy activity shape.
They completed and have no expansion rows.

The 134 Task calls all expand.
Their title is `Used Web Search`.

Task detail contains correlation, input, and complete output.
The output can contain the complete search result data.

Recommendation: keep the current magnifying glass and query.
Add accessible status text and an optional result count.

Do not repeat the query in a second visible label.
Keep provider and fallback evidence in disclosure.

### `web.fetch`

The collapsed row shows a globe icon.
A successful row prefers the returned page title.

If no title exists, it shows the hostname, URL, or saved target.
A failed fetch prefers the requested host or target.

The five Chat calls completed and have no expansion rows.
The 33 Task calls expand as `Used Fetched Web Page`.

Recommendation: keep the title or host and globe.
Add a short failed state directly to the row.

Show content size and fallback evidence in disclosure.
Do not show complete fetched content in the first detail level.

### `web.browse.*`

Every browser operation uses `Browser interaction` without a saved Chat description.
The marker does not show a browser type icon.

This rule applies to open, navigate, interact, snapshot, wait, history, and close.
The expansion title is `Used Browser interaction`.

Recommendation: use the actual action and a safe target.
Keep the generic browser session as supporting context.

## Refreshed latest 300 calls

The latest sample ends at Task call row `12172`.
It covers `2026-08-18T21:02:18.437Z` through `2026-08-19T01:19:55.397Z`.

All 300 calls came from Task runs.
They contain 243 completed results, 55 failed results, and two resultless calls.

| Exact tool | Calls | Saved outcome | Current collapsed marker |
| --- | ---: | --- | --- |
| `web.browse.open` | 123 | 72 completed, 49 failed, 2 without results | `Browser interaction` |
| `task.files.write` | 41 | 41 completed | `task.files.write` |
| `web.search` | 31 | 31 completed | Magnifying glass and exact query |
| `task.files.read` | 24 | 23 completed, 1 failed | `task.files.read` |
| `task.files.list` | 15 | 15 completed | `task.files.list` |
| `web.browse.close` | 15 | 15 completed | `Browser interaction` |
| `web.browse.interact` | 15 | 11 completed, 4 failed | `Browser interaction` |
| `task.finish_execution` | 8 | 8 completed | `task.finish_execution` |
| `task.finish_review` | 8 | 8 completed | `task.finish_review` |
| `task.continue_execution` | 7 | 7 completed | `task.continue_execution` |
| `task.finish_planning` | 5 | 5 completed | `task.finish_planning` |
| `task.list` | 3 | 3 completed | `task.list` |
| `web.fetch` | 3 | 3 completed | Globe and page title or host |
| `web.browse.snapshot` | 1 | 1 failed | `Browser interaction` |
| `web.browse.wait` | 1 | 1 completed | `Browser interaction` |
| **Total** | **300** | **243 completed, 55 failed, 2 without results** | |

## Complete logical tool catalog

`C/T` means recorded Chat calls and Task calls.
Connected-tool identifiers are grouped by operation here.

The exact 93-name inventory follows this catalog.

### Web and browser tools

| Logical tool | C/T | Current collapsed marker | Recommended collapsed marker |
| --- | ---: | --- | --- |
| `web.search` | 93/134 | Magnifying glass and exact query | Keep it; add accessible status and optional result count |
| `web.fetch` | 5/33 | Globe and title, host, URL, or target | Keep it; add failure text and content size |
| `web.browse.open` | 20/458 | Chat description or `Browser interaction`; Task generic | `Opening <host>`, `Opened <title>`, or `Could not open <host>` |
| `web.browse.navigate` | 0/162 | `Browser interaction` | `Navigated to <title> — <host>` |
| `web.browse.interact` | 35/129 | Chat description or `Browser interaction`; Task generic | `Clicked <element>`, `Filled <field>`, or `Pressed <key>` |
| `web.browse.snapshot` | 32/40 | Chat description or `Browser interaction`; Task generic | `Captured <page title>` |
| `web.browse.wait` | 7/7 | Chat description or `Browser interaction`; Task generic | `Waited for <condition>` or fold into the next action |
| `web.browse.history` | 0/1 | `Browser interaction` | `Checked browser history` |
| `web.browse.close` | 3/26 | Chat description or `Browser interaction`; Task generic | `Closed browser` or fold into session completion |

Browser interactions must use a structured accessible element name.
Entered values must not appear in the collapsed row.

### Task and Task-file tools

| Logical tool | C/T | Current collapsed marker | Recommended collapsed marker |
| --- | ---: | --- | --- |
| `task.list` | 77/52 | Chat description or `List`; Task raw name | `Checked current Task · <stage>` |
| `task.capture` | 3/0 | Chat description or `Capture` | `Created Task <title>` |
| `task.delegate` | 89/0 | Chat description or `Delegate` | `Delegated <Task> to <agent>` |
| `task.queue` | 2/0 | Chat description or `Queue` | `Queued <Task>` |
| `task.answer` | 2/0 | Chat description or `Answer` | `Answered Task question` or `Could not answer` |
| `task.cancel` | 8/0 | Chat description or `Cancel` | `Cancelled <Task>` |
| `task.retry` | 5/0 | Chat description or `Retry` | `Retrying <Task>` or `Retried <Task>` |
| `task.reopen` | 4/0 | Chat description or `Reopen` | `Reopened <Task>` |
| `task.report_blocked` | 0/29 | Raw exact name | `Reported Task blocked · <short reason>` |
| `task.recurrence.update` | 8/0 | Chat description or `Update` | `Updated schedule for <Task>` |
| `task.recurrence.run_now` | 1/0 | Chat description or `Run now` | `Started scheduled Task now` |
| `task.schedule.run_now` | 2/0 | Chat description or `Run now` | `Started scheduled Task now` |
| `task.files.list` | 0/37 | Raw exact name | `Listed <folder> · <count>` |
| `task.files.read` | 0/96 | Raw exact name | `Read <file>` or `Could not read <file>` |
| `task.files.write` | 0/120 | Raw exact name | `Wrote <file>` |
| `task.read_artifact` | 0/5 | Raw exact name | `Read artifact <title>` |
| `task.read_submission_evidence` | 0/36 | Raw exact name | `Checked submission evidence · <count>` |
| `task.continue_execution` | 0/24 | Raw exact name | Fold into the next Task run boundary |
| `task.finish_planning` | 0/9 | Raw exact name | `Plan ready · <complexity>` |
| `task.finish_execution` | 0/25 | Raw exact name | Fold into Executor completion |
| `task.finish_review` | 0/25 | Raw exact name | `Review approved`, `Changes requested`, or `Human decision needed` |
| `task.submit_plan` | 0/61 | Raw exact name | Fold into Planner completion |
| `task.submit_result` | 0/158 | Raw exact name | Fold into submitted Result |
| `task.submit_review` | 0/156 | Raw exact name | Fold into Reviewer completion |

File details should show path, size, and outcome first.
Put complete file content behind a named `File contents` disclosure.

Task lifecycle calls should remain available in technical history.
They should not compete with the existing run boundary.

### Gmail tools

| Logical tool | C/T | Current collapsed marker | Recommended collapsed marker |
| --- | ---: | --- | --- |
| `get_profile` | 5/25 | Chat description or `Get profile`; Task raw exact name | `Checked Gmail account <label>` |
| `search_messages` | 28/140 | Chat description or `Search messages`; Task raw exact name | `Searched mail for “<query>” · <count>` |
| `list_messages` | 9/0 | Chat description or `List messages` | `Listed <mailbox> · <count>` |
| `get_message` | 89/513 | Chat description or `Get message`; Task raw exact name | `Read <subject> — <sender>` |
| `get_thread` | 7/0 | Chat description or `Get thread` | `Read <subject> · <message count>` |

Show the reviewed account label when two Gmail accounts could be confused.
Keep message identifiers and complete bodies in disclosure.

### Google Calendar and Docs tools

| Logical tool | C/T | Current collapsed marker | Recommended collapsed marker |
| --- | ---: | --- | --- |
| `get_account_identity` | 2/26 | Chat `Get account identity`; Task raw exact name | `Checked Calendar account <label>` |
| `get_primary_calendar_identity` | 7/2 | Chat description or saved name; Task raw exact name | `Checked primary calendar <label>` |
| `list_calendars` | 4/11 | Chat `List calendars`; Task raw exact name | `Listed calendars · <count>` |
| `list_primary_events` | 1/0 | Chat description or saved name | `Listed primary events · <date range> · <count>` |
| `list_events` | 70/69 | Chat description or `List events`; Task raw exact name | `Listed events · <date range> · <count>` |
| `search_events` | 22/6 | Chat description or `Search events`; Task raw exact name | `Searched events for “<query>” · <count>` |
| `get_event` | 24/97 | Chat description or `Get event`; Task raw exact name | `Read <event title> · <time>` |
| `get_event_timing` | 1/1 | Chat description or saved name; Task raw exact name | `Checked timing for <event>` |
| `create_event` | 24/1 | Chat description or `Create event`; Task raw exact name | `Created <event title> · <time>` |
| `update_event` | 5/0 | Chat description or `Update event` | `Updated <event title>` |
| `update_event_details` | 5/1 | Chat description or saved name; Task raw exact name | `Updated details for <event>` |
| `update_event_location` | 1/0 | Chat description or saved name | `Updated location for <event>` |
| `move_timed_event` | 2/0 | Chat description or saved name | `Moved <event> to <time>` |
| Google Docs `get_document` | 2/0 | Chat description or `Get document` | `Read <document title>` |

Show the reviewed account or calendar label only when it resolves ambiguity.
Keep event identifiers, recurrence rules, and complete document content in disclosure.

### Notion and MCP tools

| Logical tool | C/T | Current collapsed marker | Recommended collapsed marker |
| --- | ---: | --- | --- |
| `mcp.connect_service` | 11/0 | Chat description or `Connect service` | `Connected <service>` or `Connection needs attention` |
| Notion `search` | 6/49 | Chat description or `Notion search`; Task raw exact name | `Searched Notion for “<query>” · <count>` |
| Notion `fetch` | 15/203 | Chat description or `Notion fetch`; Task raw exact name | `Read <page title>` |
| Notion `create_pages` | 2/15 | Chat description or saved name; Task raw exact name | `Created <page title>` or `Created <count> pages` |
| Notion `update_page` | 3/4 | Chat description or saved name; Task raw exact name | `Updated <page title>` |
| Notion `list_private_pages` | 3/0 | Chat `Notion list private pages` | `Listed private pages · <count>` |
| Notion `list_recent_pages` | 2/0 | Chat `Notion list recent pages` | `Listed recent pages · <count>` |
| Notion `list_shared_pages` | 3/0 | Chat `Notion list shared pages` | `Listed shared pages · <count>` |

Three Notion connections can produce the same Chat label.
Use a reviewed workspace or account label when this causes ambiguity.

Do not show MCP server identifiers in the collapsed row.
Keep those identifiers in technical details.

### Adapters, artifacts, memory, and other tools

| Logical tool | C/T | Current collapsed marker | Recommended collapsed marker |
| --- | ---: | --- | --- |
| `adapter.definition_template` | 28/49 | Chat description or saved name; Task raw name | `Loaded definition template for <service>` |
| `adapter.propose_definition` | 33/56 | Chat description or saved name; Task raw name | `Proposed definition for <service>` |
| `artifact.create_local_file` | 2/4 | Chat description or saved name; Task raw name | `Created <file>` |
| `file.download` | 0/2 | Raw exact name | `Downloaded <file> from <host>` or `Could not download <file>` |
| `read_memory_page` | 2/1 | Chat description or saved name; Task raw name | `Read memory page <title>` |
| `search_memory` | 0/7 | Raw exact name | `Searched memory for “<query>” · <count>` |
| `project.list` | 1/0 | `List` | `Listed projects · <count>` |
| `update_own_name` | 3/0 | Description or `Saved name` | `Saved name as <name>` |
| Internal `enable.*` | 3/0 | Hidden from Chat replay | Keep hidden; use the action request card |
| Unnamed legacy Task call | 0/7 | `Tool activity` with pending clock | Repair producer data; keep unnamed records in technical history |

Definition markers should show validation outcome before complete schema data.
Artifact and download markers should link to the resulting file when possible.

## Exact recorded name inventory

This table lists every named exact tool stored at the audit cutoff.
It includes connection identifiers because those values are stored tool names.

| Exact tool | Chat | Task | Total |
| --- | ---: | ---: | ---: |
| `adapter.definition_template` | 28 | 49 | 77 |
| `adapter.propose_definition` | 33 | 56 | 89 |
| `artifact.create_local_file` | 2 | 4 | 6 |
| `enable.google_calendar_personal-c8f323ff.get_event_timing` | 2 | 0 | 2 |
| `enable.google_calendar_personal-c8f323ff.move_timed_event` | 1 | 0 | 1 |
| `file.download` | 0 | 2 | 2 |
| `gmail_read_search_drafts_personal-4d245d42.get_message` | 48 | 0 | 48 |
| `gmail_read_search_drafts_personal-4d245d42.get_profile` | 2 | 2 | 4 |
| `gmail_read_search_drafts_personal-4d245d42.search_messages` | 21 | 2 | 23 |
| `gmail_read_search_drafts_personal-c3fa4f56.get_message` | 7 | 16 | 23 |
| `gmail_read_search_drafts_personal-c3fa4f56.search_messages` | 1 | 11 | 12 |
| `gmail_read_search_drafts_personal-fcd78377.get_message` | 14 | 497 | 511 |
| `gmail_read_search_drafts_personal-fcd78377.get_profile` | 0 | 21 | 21 |
| `gmail_read_search_drafts_personal-fcd78377.search_messages` | 6 | 127 | 133 |
| `google_calendar_personal-6d25522b.list_primary_events` | 1 | 0 | 1 |
| `google_calendar_personal-b28b8445.create_event` | 4 | 0 | 4 |
| `google_calendar_personal-b28b8445.list_events` | 8 | 1 | 9 |
| `google_calendar_personal-b28b8445.search_events` | 9 | 0 | 9 |
| `google_calendar_personal-b28b8445.update_event` | 5 | 0 | 5 |
| `google_calendar_personal-c8f323ff.create_event` | 1 | 0 | 1 |
| `google_calendar_personal-c8f323ff.get_event_timing` | 1 | 1 | 2 |
| `google_calendar_personal-c8f323ff.get_primary_calendar_identity` | 7 | 2 | 9 |
| `google_calendar_personal-c8f323ff.list_calendars` | 3 | 0 | 3 |
| `google_calendar_personal-c8f323ff.list_events` | 23 | 18 | 41 |
| `google_calendar_personal-c8f323ff.move_timed_event` | 2 | 0 | 2 |
| `google_calendar_personal-c8f323ff.update_event_location` | 1 | 0 | 1 |
| `google_calendar_personal-cb23736e.list_events` | 2 | 0 | 2 |
| `google_calendar_personal-d18e1a08.create_event` | 3 | 0 | 3 |
| `google_calendar_personal-d18e1a08.list_events` | 14 | 0 | 14 |
| `google_calendar_personal-e6f8079a.create_event` | 16 | 1 | 17 |
| `google_calendar_personal-e6f8079a.get_account_identity` | 2 | 26 | 28 |
| `google_calendar_personal-e6f8079a.get_event` | 24 | 97 | 121 |
| `google_calendar_personal-e6f8079a.list_calendars` | 1 | 11 | 12 |
| `google_calendar_personal-e6f8079a.list_events` | 23 | 50 | 73 |
| `google_calendar_personal-e6f8079a.search_events` | 13 | 6 | 19 |
| `google_calendar_personal-e6f8079a.update_event_details` | 5 | 1 | 6 |
| `google_docs_personal-4429c835.get_document` | 2 | 0 | 2 |
| `google_gmail_personal-435d851e.get_profile` | 3 | 0 | 3 |
| `google_gmail_personal-435d851e.list_messages` | 8 | 0 | 8 |
| `google_gmail_personal-64915e51.get_message` | 4 | 0 | 4 |
| `google_gmail_personal-64915e51.get_thread` | 7 | 0 | 7 |
| `google_gmail_readonly_personal-ba24c034.get_message` | 16 | 0 | 16 |
| `google_gmail_readonly_personal-ba24c034.get_profile` | 0 | 2 | 2 |
| `google_gmail_readonly_personal-ba24c034.list_messages` | 1 | 0 | 1 |
| `mcp.connect_service` | 11 | 0 | 11 |
| `mcp.mcp_server:09ed8c491fa0ec327cfa243c962f0483.notion-create-pages` | 2 | 15 | 17 |
| `mcp.mcp_server:09ed8c491fa0ec327cfa243c962f0483.notion-fetch` | 14 | 200 | 214 |
| `mcp.mcp_server:09ed8c491fa0ec327cfa243c962f0483.notion-list-private-pages` | 3 | 0 | 3 |
| `mcp.mcp_server:09ed8c491fa0ec327cfa243c962f0483.notion-list-recent-pages` | 2 | 0 | 2 |
| `mcp.mcp_server:09ed8c491fa0ec327cfa243c962f0483.notion-list-shared-pages` | 3 | 0 | 3 |
| `mcp.mcp_server:09ed8c491fa0ec327cfa243c962f0483.notion-search` | 5 | 45 | 50 |
| `mcp.mcp_server:09ed8c491fa0ec327cfa243c962f0483.notion-update-page` | 3 | 4 | 7 |
| `mcp.mcp_server:278dbeab677cc7ec84cebdff00a7b207.notion-fetch` | 1 | 1 | 2 |
| `mcp.mcp_server:278dbeab677cc7ec84cebdff00a7b207.notion-search` | 0 | 1 | 1 |
| `mcp.mcp_server:2da43974641428805b48e0f8c98fb79e.notion-fetch` | 0 | 2 | 2 |
| `mcp.mcp_server:2da43974641428805b48e0f8c98fb79e.notion-search` | 1 | 3 | 4 |
| `project.list` | 1 | 0 | 1 |
| `read_memory_page` | 2 | 1 | 3 |
| `search_memory` | 0 | 7 | 7 |
| `task.answer` | 2 | 0 | 2 |
| `task.cancel` | 8 | 0 | 8 |
| `task.capture` | 3 | 0 | 3 |
| `task.continue_execution` | 0 | 24 | 24 |
| `task.delegate` | 89 | 0 | 89 |
| `task.files.list` | 0 | 37 | 37 |
| `task.files.read` | 0 | 96 | 96 |
| `task.files.write` | 0 | 120 | 120 |
| `task.finish_execution` | 0 | 25 | 25 |
| `task.finish_planning` | 0 | 9 | 9 |
| `task.finish_review` | 0 | 25 | 25 |
| `task.list` | 77 | 52 | 129 |
| `task.queue` | 2 | 0 | 2 |
| `task.read_artifact` | 0 | 5 | 5 |
| `task.read_submission_evidence` | 0 | 36 | 36 |
| `task.recurrence.run_now` | 1 | 0 | 1 |
| `task.recurrence.update` | 8 | 0 | 8 |
| `task.reopen` | 4 | 0 | 4 |
| `task.report_blocked` | 0 | 29 | 29 |
| `task.retry` | 5 | 0 | 5 |
| `task.schedule.run_now` | 2 | 0 | 2 |
| `task.submit_plan` | 0 | 61 | 61 |
| `task.submit_result` | 0 | 158 | 158 |
| `task.submit_review` | 0 | 156 | 156 |
| `update_own_name` | 3 | 0 | 3 |
| `web.browse.close` | 3 | 26 | 29 |
| `web.browse.history` | 0 | 1 | 1 |
| `web.browse.interact` | 35 | 129 | 164 |
| `web.browse.navigate` | 0 | 162 | 162 |
| `web.browse.open` | 20 | 458 | 478 |
| `web.browse.snapshot` | 32 | 40 | 72 |
| `web.browse.wait` | 7 | 7 | 14 |
| `web.fetch` | 5 | 33 | 38 |
| `web.search` | 93 | 134 | 227 |

Seven additional Task calls have no saved tool name.
They render as `Tool activity` with a pending clock and correlation detail.

## Ranked baseline changes

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

Keep display metadata as the presentation authority.
Fill it for Task run items from structured tool arguments and results.

Use action, target, outcome, provider, and fallback fields.
Do not add tool-specific JSX or English phrase matching.

### P2 — Preserve the current web search marker

Keep the magnifying glass and exact query.
Add accessible status and an optional structured result count.

### P2 — Put human detail before technical detail

Lead with purpose, target, outcome, and useful evidence.
Place full input, output, correlation, and runtime fields under `Technical details`.

Exclude screenshot bytes from JSON previews when the image is already visible.
Keep complete source data reachable through the technical record.

### P2 — Add useful producer outcomes

Start with browser actions, Task files, Gmail, Calendar, and Notion.
These tools have the highest volume or the least useful Task labels.

### P2 — Add context only when it resolves ambiguity

Show a reviewed account, calendar, mailbox, workspace, or connection label when identical actions could be confused.
Never show internal server identifiers as default context.

### P3 — Fold lifecycle-only Task calls

Use existing Task run boundaries for planning, continuation, execution, and review transitions.
Keep exact calls available in technical history.

## Acceptance scenarios for the built-in change

1. A web search keeps its magnifying glass and exact query.
2. A running web search reports `Running` to assistive technology.
3. A failed browser open shows the host and short error without expansion.
4. A cancelled call never shows a completion check or pending clock.
5. A mixed group shows its count and child failure while closed.
6. A file write names its file without showing file content by default.
7. A review marker names its decision and keeps feedback adjacent.
8. Two connector accounts remain distinguishable without internal identifiers.
9. A screenshot appears once and omits encoded bytes from ordinary detail.
10. Exact arguments, output, and correlation remain reachable under technical disclosure.
11. An unnamed tool never appears as an indefinitely pending current action.
12. A successful legacy web search remains concise without an empty chevron.

## Validation notes

The saved-state check covered all 3,933 stored marker calls.
The static trace covered replay, Task mapping, pairing, grouping, status, names, icons, and details.

The check applied current marker functions to every saved call and result pair.
It found 298 valid rendered screenshots.

All 25 focused marker, pairing, and Task mapper tests passed.
All 13 focused Rust replay and transcript persistence tests passed.

Generated-file checks, TypeScript, ESLint, and both web builds passed.
Workspace Rust checking and formatting passed.

The full web suite passed 67 tests.
Two unrelated shell-navigation tests failed on existing Notifications and Clients entries.

Workspace lint found unrelated documentation warnings in live-activity and notification store code.
The full Rust test gate stopped when the API test compiler received signal 9.

No browser inspection was authorized, so the new layout was not visually verified.
The [test expects a Success row](../../apps/web/src/components/chatDetail/task/taskRunItemMapper.test.ts#L69).

The [complete detail builder omits that row](../../apps/web/src/components/transcript/markerModel.ts#L154).
This existing mismatch is outside the documentation change.

The exact-name inventory matches all 93 named database values.
Its counts total 821 Chat calls and 3,105 named Task calls.

The logical catalog totals 821 Chat calls and all 3,112 Task calls.
The Task total includes seven unnamed calls.

No browser, device, screen-reader, or touch inspection occurred.
Run visual and accessibility review before implementing these recommendations.
