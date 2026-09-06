# Go server verification results

Started: 2026-09-05. Status: **In progress**.

The [suite](go-server-user-verification.md) defines the cases and required variants.
A passing unit test does not prove a complete user journey.
The case table records only evidence from this run.

## Build and environment

- Linux x86_64 with full access for the current run.
- Earlier browser checks used disposable Go homes and synthetic records.
- From the user’s latest instruction, checks use the instance at `https://noema.kevinpei.com`.
- Release source: `af0c239869c6cd3e8624bb0a4bd905ff300eb6ca`.
- Source snapshot and temporary logs: `/var/tmp/noema-suite-run-20260905`.
- Manual device and real-consent variants remain **Human later — Not run**.
- Native macOS and Windows execution still needs suitable runners.

## Initial checks

The full-access Go unit run passed all 604 tests.
The first sandboxed run could not create a Unix socket. The full-access rerun passed that check.
The [unit result list](evidence/2026-09-05-go-unit-results.json) records each outcome and source commit.
The Go vet check passed. The frontend lint check passed. The corrected frontend unit run passed all 70 tests.
Two menu expectations omitted Notifications and Clients. The current client contract includes both pages.
The test correction adds 19 lines and removes one line. It changes no production code.
The three protected relay scenarios also passed with full access.

The packaged Linux release has SHA-256 `e278abffc1d7b00aaf338a153a76c4c91b13b2c2c43049c7e682f7d7d2e7bc6b`.
The [browser script](evidence/2026-09-05-auth-check.mjs) uses synthetic passkeys and a disposable Unicode home.
Its [results](evidence/2026-09-05-auth-results.json) describe the exact assertions.
The script contains no credentials. Its paths identify this run's local setup.

The protected inspection preflight passed HTTP, a GraphQL query, mutation denial, and WebSocket acknowledgement.
That preflight used the existing development server. It does not count as isolated release evidence.

The [core browser and command run](evidence/2026-09-05-core-check.mjs) completed nine controlled assertions.
Its [results](evidence/2026-09-05-core-results.json) cover Task capture, stale saves, Project archive rules, and complete-home restore.
Command-level checks retain pending browser variants in the table.

The controlled HTTPS run uses a Linux network namespace with no route to real providers.
Private namespace DNS maps the fake service names to an address assigned only inside that namespace.
Noema keeps its public-address checks, pinned dialing, HTTPS verification, OAuth state checks, and PKCE checks.
Chromium trusts only the fixture certificate's public-key fingerprint for fake consent.
The [wire results](evidence/2026-09-05-wire-results.json) and [service receipts](evidence/2026-09-05-wire-receipts.json) record the tested paths.
The [runner](evidence/2026-09-05-wire-check.mjs) and [fake services](evidence/2026-09-05-fake-services.mjs) retain the exact setup.
Use the [replay notes](evidence/2026-09-05-replay.md) for certificate and namespace preparation.

Three defects were fixed during this run:

- API model tools omitted configured connection and account labels. Their descriptions now include current API, connection, and account identity.
- OAuth attachment responses omitted grant metadata present in normal queries. Mutations now reuse the complete query result.
- OAuth account renaming attempted to recreate the current token file. It now changes only account metadata and preserves the token.

The [fix evidence](evidence/2026-09-05-account-label-fix.json) records the patched artifact, source hashes, regression, and code sizes.
All 605 Go tests and Go vet pass after these changes.
The production patch adds five net lines. The regression adds 84 test lines. Generated GraphQL is unchanged.
The latest authored and inclusive Go ratios remain below 80 percent, at 44.95 and 76.23 percent.

The [OAuth response fix](evidence/2026-09-05-oauth-response-fix.json) records the latest release artifact and validation.
It removes seven net production lines and changes no generated code.
All 605 Go tests and Go vet pass on that patch.
The wire run now reads from both synthetic accounts and rejects saved calls after shared-grant revocation.
The [concurrent probe](evidence/2026-09-05-concurrent-api-probe.go.txt) uses production adapter calls against the same protected test home.
Two concurrent calls refresh the shared grant once. Both return the intended account records.
After revocation, two saved calls fail before reaching the fake API.

## Document, Lua, and target checks

All four [cross-builds](evidence/2026-09-05-cross-build-results.json) passed: Linux arm64, macOS amd64, macOS arm64, and Windows amd64.
These builds do not prove native startup or client behavior.
They use the source overrides recorded in the OAuth response fix.

The [document driver](evidence/2026-09-05-document-check.mjs) passed nine assertions before a later browser wait timed out.
The [document results](evidence/2026-09-05-document-results.json) record those completed assertions.
The [fixture preparation](evidence/2026-09-05-prepare-documents.py) reuses known document structures and independent expected text.
The release parsed DOCX, ODT, ODP, RTF, and Unicode text through Chat.
It rejected malformed DOCX, invalid UTF-8, parent traversal, and symbolic links.
Other formats and archive limits remain pending.

The first Lua call returned the expected result to the provider, but its final message was not visible within 30 seconds.
The [isolated Lua driver](evidence/2026-09-05-lua-check.mjs) passed both cases in a fresh conversation.
Its [results](evidence/2026-09-05-lua-results.json) cover calculations, JSON preservation, and prohibited execution.
The longer-transcript browser timeout remains unresolved. It is not recorded as a Lua failure or a complete Chat pass.
Both runs used the release artifact recorded in the OAuth response fix.

Validation reused `CGO_ENABLED=0 go test ./cmd/... ./internal/...`: 605 passed on the three recorded source overrides.
Validation also reused `CGO_ENABLED=0 go vet ./cmd/... ./internal/...`: passed on those same changes.
No application code changed in this evidence unit. Script syntax and evidence links received focused checks.

## Current instance

The user authorized test data and changes on `https://noema.kevinpei.com`.
The development socket and the domain serve the same home at `/var/lib/noema-dev`.
A test passkey now admits the browser through normal Noema authentication.
Its private key and browser session remain in a protected credential directory outside the repository.

Cloudflare Access intercepted the public recovery POST before it reached Noema.
The browser now resolves this domain to the local HTTPS origin with normal certificate verification.
This tests the requested instance. It does not prove access through the Cloudflare edge.
The [public-instance Task check](evidence/2026-09-05-public-task-check.mjs) saved a visible audit Task through the browser.
Its [result](evidence/2026-09-05-public-task-results.json) records the Task URL and server binary hash.
The Task remains in Inbox with exact Unicode Markdown.

## Live Task execution and Projects

The [Run Now driver](evidence/2026-09-05-public-run-check.mjs) submitted a calculation through the live browser.
A WebSocket subscription observed Task changes without polling the server.
The [run results](evidence/2026-09-05-public-run-results.json) show completed Planner, Executor, and Reviewer runs.
RESULT.md contains 42, 600, and the exact requested Unicode text. REVIEW.md accepts those requirements.
The [document check](evidence/2026-09-05-public-result-check.mjs) verified that reopening selects the result.
The request and review also render separately. The Transcript view opens with the latest tool activity.
The oldest Planner entry was outside the observed transcript viewport. Complete history navigation remains pending.

The [Project checks](evidence/2026-09-05-public-project-check.mjs) use the development socket on this same instance.
Their [results](evidence/2026-09-05-public-project-results.json) cover folder settings, current-document preservation, archive denial, and editing after reopen.
These command checks do not prove browser draft retention or shared-folder use by every Task role.
The audit Project and Task remain visible on the requested instance.

This unit changes no production code. Script syntax, evidence JSON, links, and Git whitespace passed focused checks.
The unchanged server sources reuse the Go test and vet results recorded above.

## Scheduling findings

The live fall-back preview listed one local minute twice. Preview now uses the existing local-minute key to omit duplicates.
The focused `CGO_ENABLED=0 go test ./internal/schedule` check passed.
Broad `CGO_ENABLED=0 go test ./cmd/... ./internal/...` and `CGO_ENABLED=0 go vet ./cmd/... ./internal/...` passed on this patch.
The live preview now skips the missing spring minute and lists the repeated fall minute once.
The patch adds five net production lines and 15 regression-test lines. Generated GraphQL is unchanged.
Authored and inclusive Go totals remain below the migration limits: 78,209 and 182,846 lines.

The live Run now check found a missing handoff from schedule release to the Planner queue.
The scheduler now calls the existing queue operation after occurrence documents are ready.
The [released Task results](evidence/2026-09-05-released-task-results.json) confirm that both previously stuck audit Tasks completed with one Planner run each.
A second defect rejected document-only recurrence updates. The GraphQL handler now passes the existing document-change flag.

The [fix record](evidence/2026-09-05-scheduling-fixes.json) records source hashes, focused checks, broad validation, and code sizes.
Both focused regressions passed. Broad Go test and vet checks passed on the combined patch.
The patch adds 30 production lines and 57 net test lines. Generated GraphQL and database schemas are unchanged.
Both migration ratios remain below 80 percent.

The [fresh deadline driver](evidence/2026-09-05-public-due-check.mjs) uses the live HTTPS origin and a Task event subscription.
Its [results](evidence/2026-09-05-public-due-results.json) confirm no run before the deadline, one Planner run afterward, and a completed result.
The Task entered the queue at 20:27:44.005 UTC for its 20:27:44 UTC deadline.

The [initial schedule driver](evidence/2026-09-05-public-schedule-check.mjs) retains the failed handoff assertion and its earlier setup.
Its [saved results](evidence/2026-09-05-public-schedule-results.json) cover the completed preview and pre-deadline checks.
The [recurrence driver](evidence/2026-09-05-public-recurrence-check.mjs) confirmed document-only saves and stale-save rejection after the fix.
It then paused, resumed, and skipped a slot. A manual run was correctly denied while the original occurrence remained pending.
The [continuation](evidence/2026-09-05-public-recurrence-finish-check.mjs) cancelled that audit occurrence before requesting a manual run.
Its [results](evidence/2026-09-05-public-recurrence-finish-results.json) confirm the updated template copy, unchanged original document, preserved normal cadence, and immutable ended template.
These scripts preserve this audit sequence. They are not an order-independent test framework.

## Live Artifact checks

A completed Task published HTML and two Markdown versions on the requested instance.
The [publication driver](evidence/2026-09-05-public-publish-check.mjs) and [result](evidence/2026-09-05-public-publish-results.json) record this Task.
Successful publication results previously appeared only as technical tool records in the Task transcript.
The mapper now adds the existing Artifact card after each successful publication result.
The card opens the existing preview panel. The technical record remains available.
The patch adds 26 frontend lines. It changes no server code or schema.

The [transport driver](evidence/2026-09-05-public-artifact-check.mjs) passed six groups of assertions.
Its [results](evidence/2026-09-05-public-artifact-results.json) cover exact bytes, ownership, external references, unsafe-file rejection, and authenticated downloads.
The [preview driver](evidence/2026-09-05-public-artifact-preview-check.mjs) opens the published HTML through the Task transcript.
Its [desktop](evidence/2026-09-05-public-artifact-preview-results-1280.json) and [phone](evidence/2026-09-05-public-artifact-preview-results-390.json) results verify HTML isolation and visible content.
Scripts, forms, handlers, navigation, and external loading are blocked.
Both Markdown versions retain their original bytes. Saved upload-action binding remains pending.
The browser uses the direct HTTPS origin. Cloudflare edge behavior remains outside this evidence.

The [follow-up driver](evidence/2026-09-05-artifact-followup.mjs) completed result-link downloads, version selection, and visible file-error checks.
Its [results](evidence/2026-09-05-artifact-followup-results.json) confirm these checks on the same published Task.
Altered, removed, and symlinked files show a load error. Restoring the original restores the preview.

The [fix record](evidence/2026-09-05-artifact-card-fix.json) records the patch, checks, and visual review.
Frontend lint and build passed on the mapper change. Generated-file checks passed with unchanged schemas and operations.
The server reuses the passing Go test and vet results from the scheduling fix. This frontend patch changes no server inputs.

The [format publication](evidence/2026-09-05-artifact-format-publish.mjs) completed another live Task with four exact [fixtures](evidence/2026-09-05-artifact-format-fixtures.json).
The [Task result](evidence/2026-09-05-artifact-format-publish-results.json) records completed execution and review.
The [format driver](evidence/2026-09-05-artifact-format-preview.mjs) verifies literal text, SVG and binary fallbacks, and exact downloads.
Its [results](evidence/2026-09-05-artifact-format-preview-results.json) retain each completed assertion.
Full Chromium displayed the PDF page with “Noema PDF audit 42.” The smaller headless browser runtime displayed a blank frame.
The [full Chromium result](evidence/2026-09-05-artifact-pdf-browser-results.json) also verifies the bundled KaTeX font.
The response policy now permits embedded fonts. Script sources remain restricted to the application origin.

The restart exposed a second defect: document recovery treated Artifact directories as Task document directories.
Recovery now skips valid `task_<id>` Artifact directories. It retains document-stage checks for Task directories.
Both focused regressions failed before their fixes and passed afterward.
The [fix record](evidence/2026-09-05-artifact-restart-fixes.json) includes source hashes, code sizes, and final Go test and vet results.
The patch adds five net production lines and seven test lines. Both migration ratios remain below 80 percent.

The [restart driver](evidence/2026-09-05-artifact-restart.mjs) uploads 40 KiB, the Task upload limit, before restart.
The [before](evidence/2026-09-05-artifact-restart-before.json) and [after](evidence/2026-09-05-artifact-restart-after.json) records show identical bytes and delivery headers.
The [process record](evidence/2026-09-05-artifact-restart-process.json) proves a new server process served the second download.
The existing development supervisor restarted its server watcher. No source content changed for this restart check.



## Live Task editing and workspace checks

The [two-browser driver](evidence/2026-09-05-task-edit-browser.mjs) verifies title edits, exact document saves, competing drafts, cancellation, and source/rich switching.
Its [results](evidence/2026-09-05-task-edit-browser-results.json) confirm reload persistence and preservation of the allocated Task directory.
The stale browser keeps its draft. Acknowledging current state does not silently replace that draft or the accepted server content.
The [fallback driver](evidence/2026-09-05-task-editor-fallback.mjs) blocks the rich-editor module request.
Its [result](evidence/2026-09-05-task-editor-fallback-results.json) confirms that the original source remains editable and saves exactly.
A parser-specific failure remains untested.

The [state driver](evidence/2026-09-05-task-state-browser.mjs) starts a Task while another browser holds an edit draft.
Its [results](evidence/2026-09-05-task-state-browser-results.json) confirm disabled stale saving, completion selection, and readable completed documents.
Completion opens the result once. Later request selection survives Transcript and Workspace navigation.
The [workspace driver](evidence/2026-09-05-task-workspace-browser.mjs) adds two controlled support files to the completed calculation Task.
Its [results](evidence/2026-09-05-task-workspace-browser-results.json) distinguish request, result, review, Markdown support, literal text, and the Planner transcript start.

The [visual driver](evidence/2026-09-05-task-edit-visual.mjs) opens the existing source editor at desktop and phone widths.
The [validation record](evidence/2026-09-05-task-browser-validation.json) records those widths, driver hashes, tested revision, and reused Go checks.
This unit changes no production code. Script syntax, JSON, evidence links, case IDs, and Git whitespace passed focused checks.

## Live Task views, Projects, and Chat capture

The [Task view driver](evidence/2026-09-05-task-views-browser.mjs) checks personal, Project, Scheduled, and terminal views.
Its [results](evidence/2026-09-05-task-views-browser-results.json) confirm selection and actions for Inbox, scheduled, completed, and cancelled Tasks.
Both temporary view Tasks were cancelled after the check.

The [Project driver](evidence/2026-09-05-project-browser.mjs) uses two browsers for competing edits, archive, and reopen.
Its [results](evidence/2026-09-05-project-browser-results.json) confirm exact persistence, retained drafts, readable archived content, and restored editing.
The [visual driver](evidence/2026-09-05-project-visual.mjs) opens the source editor at desktop and phone widths.
The source and editing controls remain visible without horizontal clipping. The Project remains open.

The [Chat driver](evidence/2026-09-05-chat-task-project.mjs) captures an exact Unicode request in the existing Project.
Its [results](evidence/2026-09-05-chat-task-project-results.json) contain this synthetic turn and its eight paired tool calls and results.
The new Task remains in Inbox with no runs. One visible Chat reference opens the correct request.
Project inspection includes the existing cancelled Task. Other model providers remain pending.
An earlier ambiguous prompt included trailing instructions in its request. The passing check uses explicit JSON values.

The live check found an API defect: Task detail omitted its stored conversation source.
The existing field now receives that source. Manual captures still return no conversation source.
The [validation record](evidence/2026-09-05-task-source-validation.json) records source hashes, focused regression results, broad Go checks, and code sizes.
The regression failed before the fix and passed after it. Full Go tests and vet passed on the final source.
Those results remain valid after these documentation changes. The patch changes one production line and adds 27 test lines.
Both migration ratios remain below 80 percent. These browser checks use the live HTTPS origin; Cloudflare Access remains outside their scope.

## Live Chat persistence and reconnect

The [Chat driver](evidence/2026-09-05-chat-reconnect-browser.mjs) sends one simple request and one exact rich Markdown request.
Its [results](evidence/2026-09-05-chat-reconnect-browser-results.json) contain only these synthetic turns.
The first turn retains one human message and one assistant response after reload.
During the second response, the driver drops a text delta and closes the browser WebSocket.
The client reconnects and recovers the exact response without duplicate saved messages. A tool-result disconnect remains pending.

Code, lists, links, Unicode, and emoji remain intact after reload and in a second browser at phone width.
The foreground request creates no Task. This evidence covers Codex and Chromium, not the remaining provider and client matrix.
The [visual driver](evidence/2026-09-05-chat-rich-visual.mjs) opens and expands the saved response at desktop and phone widths.
Its [results](evidence/2026-09-05-chat-rich-visual-results.json) confirm no horizontal page overflow. The final response line remains reachable.
The [validation record](evidence/2026-09-05-chat-reconnect-validation.json) records the tested revision and driver hashes.
No production code changed. Existing Go checks remain valid; script syntax, JSON, links, case IDs, and whitespace receive focused validation.

## Live Chat choices and initial free-text finding

The [single-choice driver](evidence/2026-09-05-chat-choice-browser.mjs) answers one exact prompt from the browser.
Its [results](evidence/2026-09-05-chat-choice-browser-results.json) show one stored selection, one resumed turn, and a rejected stale answer from another browser.
The [multiple-selection driver](evidence/2026-09-05-chat-many-browser.mjs) chooses options in reverse order and submits them together.
Its [results](evidence/2026-09-05-chat-many-browser-results.json) preserve prompt order and both selected labels. Reload retains the selection.

The [free-text driver](evidence/2026-09-05-chat-choice-free-text.mjs) found an incomplete path.
The enabled composer submits text while a choice waits. The server reports “The Chat turn could not be saved.”
The text is not stored. A later browser option click also failed to create a selection.
The browser cause remains unproven; this check does not establish successful browser recovery.
The [state check](evidence/2026-09-05-choice-state.mjs) confirmed the pending prompt and submitted its exact structured answer through GraphQL.
Its [results](evidence/2026-09-05-choice-state-results.json) preserve the before and after records.
The [final read](evidence/2026-09-05-chat-choice-free-text-results.json) confirms the resumed answer and selection after reload.
These results establish API recovery. CHAT-09 remains incomplete until the free-text path and browser follow-up are resolved.

The [validation record](evidence/2026-09-05-chat-choice-validation.json) records the tested revision, driver hashes, source inspection, and remaining limits.
The Rust and Go selection mutations both accept option IDs. That fact does not prove the intended ordinary-text continuation behavior.
That evidence unit changed no production code. The following correction resolves its open classification and browser findings.

## Pending-choice rejection correction

The Rust baseline rejects ordinary text when Chat waits for a human interaction.
Its `RuntimeActor::turn` returns that error before it starts a new text turn.
Thus, CHAT-09 has no supported free-text continuation in this migration contract. The structured selection path remains required.
The [fix record](evidence/2026-09-05-choice-rejection-fix.json) identifies the exact baseline source and final Go changes.

The Go server now explains its busy state and asks the human to finish the current interaction.
It retains the general save error for unrelated failures. It does not expose internal error details.
The existing continuation test now rejects ordinary text before resolving its original choice.
That regression failed before the change and passed afterward. Full Go tests and vet passed on the final source.
The patch adds four net production lines and 14 test lines. Both migration ratios remain below 80 percent.

The [recovery driver](evidence/2026-09-05-chat-choice-recovery.mjs) checks the new message and submits a structured answer through the browser.
Its [results](evidence/2026-09-05-chat-choice-recovery-results.json) confirm successful submission, one selection, the original resumed turn, stale rejection, and reload persistence.
The driver now scopes its radio control to the exact prompt and waits for the exact turn’s saved answer.
These checks resolved the earlier browser finding without a frontend change.
Other providers and native clients remain outside this evidence.

## Live interactive A2UI checks

The [keyboard driver](evidence/2026-09-05-a2ui-browser.mjs) renders a controlled form with all nine components in the current catalog.
Its [results](evidence/2026-09-05-a2ui-browser-results.json) preserve the submitted Unicode name, boolean, and choice in the resumed turn.
The [pointer driver](evidence/2026-09-05-a2ui-pointer.mjs) repeats the submission through visible checkbox and radio labels.
Its [results](evidence/2026-09-05-a2ui-pointer-results.json) confirm one accepted action, a matching answer, and persistence after reload.
Both checks reject forged context before submission. They reject stale actions afterward without changing the accepted turn.
Submitted controls become disabled. The original surface and answered surface remain separate transcript records.

The [invalid-input driver](evidence/2026-09-05-a2ui-invalid.mjs) supplies an unsupported Image component and active HTML.
Its [results](evidence/2026-09-05-a2ui-invalid-results.json) show both validation failures and no created surface.
The HTML does not execute. Chat remains editable after reload.
The [validation record](evidence/2026-09-05-a2ui-validation.json) records source revision, driver hashes, reused Go checks, and visual review.
Desktop and phone views keep the form controls visible and reachable. No production code changed.
Other providers and native clients remain outside this evidence.

## Live transcript pagination

The [pagination driver](evidence/2026-09-05-chat-pagination.mjs) captures a 100-item baseline before a new streamed response.
It compares three older pages and their adjacent head page against the same baseline.
Its [results](evidence/2026-09-05-chat-pagination-results.json) show 40 ordered items without gaps, duplicates, or changed content.
At least one older-page read occurs during text streaming.
The browser also loads older history through its own named GraphQL operation after wheel input.
The driver excludes its direct queries from that browser assertion.
After completion and reload, the same older pages retain their exact content.

The [validation record](evidence/2026-09-05-chat-pagination-validation.json) identifies the tested revision, comparison window, driver hashes, and reused checks.
No production code changed. This evidence covers the tested window in Codex and Chromium.

## Fresh actions and tool-result reconnect

The [new-action driver](evidence/2026-09-05-chat-new-action.mjs) deliberately requests another Task with an existing title and a different exact document.
Its [results](evidence/2026-09-05-chat-new-action-results.json) show one new Task, one new capture result, and one matching Chat reference.
The original Task retains its document, revision, state, and identity. The new Task remains in Inbox with no runs.
The Chat reference opens the new request in the detail panel. Reload preserves the saved Task.

The [tool reconnect driver](evidence/2026-09-05-chat-tool-reconnect.mjs) drops a live `project.read` result and closes its WebSocket.
Its [results](evidence/2026-09-05-chat-tool-reconnect-results.json) confirm a new connection and one matching call/result pair.
The answer reflects the Project’s exact name and document. Reload retains the same turn without duplicate entries.
Together with the earlier text-delta check, this completes the tested Chromium reconnect variants.
The [validation record](evidence/2026-09-05-chat-action-validation.json) records the source revision, driver hashes, Task identities, and reused Go checks.
No production code changed. Other providers and native clients remain outside this evidence.

## Live Memory updates, corrections, and citations

The [initial inspection](evidence/2026-09-05-memory-inspect.mjs) found an existing cited profile, preserved in the [before record](evidence/2026-09-05-memory-before.json).
The [update driver](evidence/2026-09-05-memory-update.mjs) adds synthetic stargazing and travel-note preferences through Chat, then uses the Memory update control.
The [addition result](evidence/2026-09-05-memory-addition-results.json) records two cited child articles while the root profile remains unchanged.
The [correction result](evidence/2026-09-05-memory-correction-results.json) replaces Spanish with French and cites the newer exact human message.
The stargazing article retains its original source. The correction changes status from idle to running to idle and advances the processed sequence.
The first update completed despite a driver wait error. Verification resumed without starting another update.

The [citation driver](evidence/2026-09-05-memory-citations.mjs) follows the related-article link and opens the nearby citation by pointer and keyboard.
Its [results](evidence/2026-09-05-memory-citations-results.json) verify source identity, type, date, and exact excerpt at desktop and phone widths.
Both source cards remain readable after their opening transition.
The [Chat read driver](evidence/2026-09-05-memory-chat-read.mjs) requests search and deeper article reads.
Its [results](evidence/2026-09-05-memory-chat-read-results.json) show successful Memory tools and the current French preference alongside stargazing.
The [validation record](evidence/2026-09-05-memory-validation.json) records source revision, driver hashes, reused checks, and remaining limits.
No production code changed. The synthetic preferences remain in the authorized test instance.
Empty-state, deep hierarchy, bounded root context, and remaining Memory failure cases are not established by these checks.

## Live Memory search reconstruction

The [rebuild driver](evidence/2026-09-05-memory-rebuild.mjs) runs the same two exact Memory searches before and after a server restart.
The [before](evidence/2026-09-05-memory-rebuild-before.json) and [after](evidence/2026-09-05-memory-rebuild-after.json) records contain matching search results, articles, citations, and source-file hashes.
The [process record](evidence/2026-09-05-memory-rebuild-process.json) confirms a new server process.
The existing development supervisor restarted its server watcher. Memory was idle before the restart and opened without an error afterward.
The browser’s existing authenticated session remained usable.

The Go Memory store builds its lexical search index in process memory from the Markdown tree.
Startup and publication rebuild that index. The older SQLite search description was stale and has been corrected in the current authorities.
The [validation record](evidence/2026-09-05-memory-rebuild-validation.json) identifies source hashes, compared queries, and reused Go checks.
No production code changed. This check covers the current three-article test tree.

## Duplicate Memory facts and assistant evidence

The [duplicate-evidence driver](evidence/2026-09-05-memory-duplicate.mjs) repeats the existing synthetic preferences and asks for an unrelated fictional occupation.
Its [results](evidence/2026-09-05-memory-duplicate-results.json) contain the new human message, assistant response, and Memory snapshots before and after update.
All three article paths and bodies remain unchanged. No duplicate article or fact appears.
Every retained citation is a human-message source. The assistant’s invented occupation does not become Memory evidence.
The update finishes without an error and advances its processed sequence. Reload preserves the root article.
The [validation record](evidence/2026-09-05-memory-duplicate-validation.json) identifies the tested revision, driver hashes, reused checks, and limits.
No production code changed. This result does not cover secret-bearing input or every Memory rejection path.

## Browser schedule controls

The [browser driver](evidence/2026-09-05-schedule-browser.mjs) changed timing on one synthetic Task at noema.kevinpei.com.
Reschedule saved the replacement instant without starting a run.
Unschedule returned the same Task to Inbox and preserved Unicode content after reload.
The phone control then started that Task before its future schedule.
The [completion driver](evidence/2026-09-05-schedule-completion.mjs) confirmed successful Planner, Executor, and Reviewer runs.
One Executor wrote `56` to `RESULT.md`. The reloaded desktop browser displayed that result.
The [results](evidence/2026-09-05-schedule-browser-results.json) retain exact timing, Task identity, and run records.
The [validation record](evidence/2026-09-05-schedule-browser-validation.json) records scope and reused checks.
No production change was required. TIME-02 passes for live Codex and Chromium.

## Recurrence drafts and schedule dialog fix

Opening the recurrence schedule dialog caused a React update loop and replaced the detail with an error.
The Astryx 0.1.9 text-measurement hook updated state when a heading ref detached.
A bounded Bun patch removes those updates. Disabled measurement returns inactive values.
The [browser driver](evidence/2026-09-05-recurrence-drafts.mjs) now opens the dialog on desktop and phone without diagnostic overrides.
Competing description drafts preserve the current template. Reloading preserves the stale draft for an explicit save.
A competing schedule save leaves the current timing unchanged and retains the entered draft.
The [cleanup driver](evidence/2026-09-05-recurrence-cleanup.mjs) ended the recurrence and cancelled its unstarted Task.
The ended template remains readable after reload, with editing controls absent.
The [results](evidence/2026-09-05-recurrence-drafts-results.json) record saved revisions and cleanup.
The [component check](evidence/2026-09-05-truncation-component-check.mjs) verifies ordinary Unicode text, disabled measurement, resizing, and rerendering.
Its [fixture](evidence/2026-09-05-truncation-component.jsx) uses the installed hook. The live dialog supplies the crash regression evidence.
The [validation record](evidence/2026-09-05-recurrence-drafts-validation.json) records passing frontend checks and the patch removal condition.
TIME-07 passes in Chromium. The later deadline check covers future execution under TIME-08.

## Recurrence history and manual execution

The [browser driver](evidence/2026-09-05-recurrence-history.mjs) created a future recurrence and cancelled its initial occurrence.
Run now created one separate manual occurrence without changing the next normal slot.
One Executor wrote `72`. Planner and Reviewer runs also completed.
After reload, the active recurrence stayed in Scheduled while both occurrences were terminal.
Desktop and phone history showed separate manual and scheduled entries.
The manual history link opened the correct result. The fixture was then ended.
The [results](evidence/2026-09-05-recurrence-history-results.json) retain occurrence identities, timing, completion, and cleanup.
The [validation record](evidence/2026-09-05-recurrence-history-validation.json) records scope and reused frontend checks.
TIME-05 and TIME-09 pass for live Codex and Chromium. The automatic template check follows below.

## Automatic recurrence template copying

The [socket driver](evidence/2026-09-05-recurrence-auto-template.mjs) edited a minute-based recurrence on the requested instance.
At 23:10 UTC, the live scheduler created an occurrence with the exact edited template and revision.
The driver read that initial file 32 milliseconds after the scheduled instant.
The earlier cancelled Task kept its sole `TASK.md` file with exact original Unicode content.
The recurrence was ended after the new occurrence appeared.
The [completion driver](evidence/2026-09-05-recurrence-auto-completion.mjs) confirmed successful Planner, Executor, and Reviewer runs.
One Executor produced `81`. The browser displayed that result after reload.
The [results](evidence/2026-09-05-recurrence-auto-template-results.json) retain exact files, timestamps, revisions, and completion.
The [validation record](evidence/2026-09-05-recurrence-auto-validation.json) records scope and reused checks.
TIME-06 passes for the live scheduler, Codex, and Chromium. No production change was required.

## Recurrence controls across deadlines

The [deadline driver](evidence/2026-09-05-recurrence-deadline-controls.mjs) controlled two minute-based recurrences through the development socket.
Pause and skip-next prevented work at 23:16 UTC.
Resume applied the configured missed-run policy and kept the next normal slot.
At 23:17 UTC, each recurrence created exactly one new occurrence.
Both recurrences were ended. Their records stayed unchanged across 23:18 UTC, with no extra occurrence.
The [completion check](evidence/2026-09-05-recurrence-control-completion.mjs) confirmed result `21` from each permitted occurrence.
Each had one Executor run and successful planning and review.
The [browser check](evidence/2026-09-05-recurrence-control-views.mjs) verified paused, skipped, and ended states at desktop and phone widths.
Ended templates remained readable, with Run now and Edit schedule controls absent.
The [results](evidence/2026-09-05-recurrence-deadline-controls-results.json) retain exact states before and after each deadline.
The [validation record](evidence/2026-09-05-recurrence-deadline-validation.json) records scope and reused checks.
TIME-08 passes. The TIME-04 restart checks below cover both missed-run policies.

## Stored daylight-saving transitions

The [scheduler test](../../internal/store/task_schedules_test.go) now exercises spring and fall transitions through production scheduling and queue functions.
It uses real SQLite with fixed instants in `America/New_York`.
The missing spring minute creates no Task or queued run. The next valid day queues one new Planner.
The repeated fall minute creates no duplicate occurrence or run. The next day again queues one new Planner.
Overlap is allowed so an active Task cannot hide a duplicate through overlap suppression.
The [validation record](evidence/2026-09-05-dst-store-validation.json) records instants, assertions, source hash, and passing focused and broad Go checks.
The test adds 71 lines. No production change was required.
TIME-11 passes for controlled scheduler time. The live server clock was unchanged.

## Recurrence overlap policy counts

The [overlap test](../../internal/store/task_schedules_test.go) keeps a Task active across two later slots using production store commands.
Skip creates no later Task. Allow creates one Task for each later slot.
Queue one initially failed: completing its catch-up Task released another Task for an older overlapped slot.
The release transaction now advances expired timing and preserves any future next slot.
Queue one releases exactly one catch-up Task. A further scheduler wake creates no extra Task.
A fourth case verifies preservation of a future slot set by Skip next.
The [validation record](evidence/2026-09-05-overlap-store-validation.json) contains the before-and-after results and passing broad Go checks.
The fix adds eight production lines and 114 test lines. Both Go migration ratios remain below 80 percent.
TIME-10 passes for controlled scheduler state. Provider execution remains covered by the separate live deadline checks.

## One-time schedule edits during due processing

The [race test](../../internal/store/task_schedules_test.go) covers edit-first, deadline-first, and concurrent calls through production store commands.
If the edit wins, the old deadline queues no run and the replacement deadline queues one Planner.
If due processing wins, the stale edit is rejected and the original timing remains accepted.
The concurrent case permits either valid transaction ordering. Repeated queue handoff creates no duplicate run.
The [validation record](evidence/2026-09-05-schedule-race-validation.json) records passing focused tests, broad Go tests, and vet.
This unit adds 82 test lines and no production code.
The recurrence race coverage follows below.

## Recurrence edits during occurrence creation

The [recurrence race test](../../internal/store/task_schedules_test.go) adds edit-first, deadline-first, and concurrent cases through production store commands.
An edit committed first suppresses the replaced slot.
An occurrence committed first remains a valid existing Task with its original title and revision.
Both orderings preserve the accepted future slot. Later scheduler calls cannot create more work from replaced timing.
The future occurrence uses the edited title and revision. Repeated queue handoff leaves one Planner per occurrence.
The [validation record](evidence/2026-09-05-recurrence-race-validation.json) records passing focused tests, broad Go tests, and vet.
This unit adds 94 test lines and no production code.
Together with the one-time race cases, this completes TIME-12 for controlled scheduler state.

## Missed deadlines across live restarts

The [one-time driver](evidence/2026-09-05-missed-restart.mjs) stopped the requested instance across a deadline with RUN_ONCE and SKIP Tasks.
The [recurrence driver](evidence/2026-09-05-missed-recurrence-restart.mjs) repeated the check for established recurrences.
Each driver confirmed app absence after the deadline. The development supervisor then restored the app with a new process ID.
RUN_ONCE executed once in both cases. SKIP cancelled the one-time Task and recorded the recurring slot without creating a Task.
The [one-time completion check](evidence/2026-09-05-missed-restart-completion.mjs) and [recurrence completion check](evidence/2026-09-05-missed-recurrence-completion.mjs) verified result `42` after browser reload.
Each permitted Task completed planning, execution, and review with one Executor.
Both recurrence templates ended after recovery. Their initial Tasks were cancelled before the tested deadline.
The [preservation check](evidence/2026-09-05-missed-restart-preservation.mjs) confirmed exact completed state and run identities after the second restart.
The [one-time results](evidence/2026-09-05-missed-restart-results.json) and [recurrence results](evidence/2026-09-05-missed-recurrence-restart-results.json) retain timings and observed state.
The [validation record](evidence/2026-09-05-missed-restart-validation.json) records scope, driver hashes, healthy processes, and reused Go checks.
TIME-04 passes for the live Codex and Chromium path. The host clock was unchanged.

## Task clocks and original request time

TIME-03 inspection found that Go Task roles received no current clock.
The [runtime fix](../../internal/runtime/task_execution.go) adds fresh clock context to the existing system message using Chat's clock formatter.
Schedule timezone takes priority over source timezone. Tasks without either timezone use UTC.
Each scheduled Task receives its occurrence cutoff. The original Chat timestamp remains separate Task data in its source timezone.
The [regression test](../../internal/runtime/task_execution_test.go) uses Los Angeles, Tokyo, and a scheduled occurrence with different source and schedule timezones.
It checks Planner, Executor, and Reviewer messages. Current timestamps must fall within the call interval, rather than match the old request date.
Original request timestamps and Unicode Task text remain intact.
The test failed before the fix. The [validation record](evidence/2026-09-06-task-clock-validation.json) records the checks and source hashes.
The live relative-time checks below complete the remaining capture coverage.

## Relative-time capture in browser timezones

The [browser driver](evidence/2026-09-06-relative-capture.mjs) requests tomorrow at 09:00 from Los Angeles and Tokyo browser contexts.
Both requests reached `noema.kevinpei.com` shortly after midnight UTC on September 6.
Los Angeles correctly saved September 6 at 09:00 PDT. Tokyo correctly saved September 7 at 09:00 JST.
Each request created one Task. The rendered Task details showed the expected date, time, and timezone.
Both synthetic Tasks were then cancelled with zero runs.
The [results](evidence/2026-09-06-relative-capture-results.json) retain the request text, turn evidence, saved schedules, and cancellation state.
The [validation record](evidence/2026-09-06-relative-capture-validation.json) links the controlled runtime checks for later role clocks and original request timestamps.
TIME-03 passes for this combined live and controlled scope.

## Direct Task delegation and optional enum conversion

The [live delegation driver](evidence/2026-09-06-direct-delegate.mjs) authorized a complete calculation request for direct execution.
The [first attempt](evidence/2026-09-06-direct-delegate-before-results.json) failed without creating a Task.
The model supplied both complexity fields. A specific error alone did not permit a successful correction.
Inspection found that [provider conversion](../../internal/provider/openrouter_tooling.go) required optional enum fields while excluding null from their allowed choices.
The converter now permits null for optional enums. Source enum rules remain unchanged, and optional null placeholders are removed before source validation.
The [provider regression test](../../internal/provider/openrouter_generation_test.go) failed before this fix and passed afterward.
The existing delegation test also verifies a specific correction message followed by successful direct authorization.
The [completion driver](evidence/2026-09-06-direct-delegate-completion.mjs) verified one Executor, one Reviewer, no Planner, and exact result `72` after browser reload.
The [live results](evidence/2026-09-06-direct-delegate-results.json) preserve the request, accepted intent, Task identity, and completed runs.
The [validation record](evidence/2026-09-06-direct-delegate-validation.json) contains source hashes and passing focused tests, broad Go tests, and vet.
RUN-02 passes for live Codex and Chromium. Other providers remain separate matrix cases.

## Reviewer correction with current Task files

The [runtime test](../../internal/runtime/task_execution_test.go) uses production Task execution and SQLite with controlled model responses.
The first Executor writes only one of two required values. The Reviewer receives that result and requests the missing value.
The correction Executor receives the current request, incomplete result, and exact review feedback.
The rejected Task remains incomplete until correction and approval. The second Reviewer receives the corrected result and the current request.
The final result preserves both required values, including Unicode text.
Five completed runs retain their exact parent links: Planner, Executor, Reviewer, correction Executor, and final Reviewer.
The correction runs use review round two. No production change was required.
The [validation record](evidence/2026-09-06-review-correction-validation.json) records passing focused and broad Go checks.
RUN-03 passes for controlled runtime responses. Live model review quality remains outside this deterministic check.

## Cancellation across Task states

The [store test](../../internal/store/task_execution_test.go) cancels queued, running, and waiting Tasks through production commands and SQLite.
Each cancellation advances the generation, clears active run and gate fields, and leaves one cancelled run.
A waiting clarification gate becomes superseded. Late planning completion and transcript writes return the stale-run error.
A later worker claim finds no work. Rejected late changes preserve the accepted cancellation revision and generation.
The [validation record](evidence/2026-09-06-cancel-states-validation.json) records passing focused tests, broad Go tests, and vet.
No production fix was needed. The test adds 76 lines.
The action-state checks below address stored outcome uncertainty. Runtime and rendered checks with a controlled external service remain pending.

## Cancelled external actions retain uncertainty

The [action test](../../internal/store/action_requests_test.go) creates an authorized Task action and claims it for execution before cancelling the Task.
Before the fix, the action remained marked executing. Recovery excluded its cancelled Task generation.
The [cancellation transaction](../../internal/store/task_lifecycle.go) now marks executing actions as outcome uncertain and saves the matching action event.
The exact action arguments remain intact, including Unicode text. Late success and a second claim are rejected.
Recovery preserves uncertainty. The Task remains cancelled with the accepted generation.
The [validation record](evidence/2026-09-06-cancel-action-validation.json) records the regression and passing focused and broad Go checks.
The fix adds 28 production lines and 72 test lines. No schema change was required.
The controlled MCP check below verifies the late runtime result. Displayed outcome coverage remains pending.

## Late MCP success after Task cancellation

The [runtime test](../../internal/runtime/task_execution_test.go) uses a local HTTP MCP service and the production Task MCP execution path.
The service receives one authorized write with exact Unicode arguments. It holds the response while the Task is cancelled.
After cancellation, the service completes the synthetic write and returns success.
The runtime cannot save that late success over the uncertain action outcome. The Task remains cancelled and has no completion timestamp.
The action retains its exact arguments and outcome-uncertain failure code. The service receives only one call.
The [validation record](evidence/2026-09-06-cancel-mcp-validation.json) records passing focused tests, broad Go tests, and vet.
This test adds 122 lines. No production change was required.
RUN-05 remains partial until the browser displays the uncertain outcome from a cancelled external action.

## Cancellation uncertainty in the Task transcript

Transcript inspection found another gap: the action was uncertain, but its tool call remained running with no result.
The [cancellation transaction](../../internal/store/task_lifecycle.go) now saves a failed tool result and closes the original call.
The result states: “Task cancelled. The external action may have completed.”
It retains the original call link, correlation, tool name, and round. Existing transcript fields carry the result to clients.
The [regression test](../../internal/store/action_requests_test.go) failed before the fix. It now verifies the closed call and exact uncertainty message.
The [validation record](evidence/2026-09-06-cancel-transcript-validation.json) records focused and broad Go checks.
This fix adds 20 production lines and 11 test lines. No frontend or schema change was needed.
The browser checks below complete rendered coverage of this saved result.

## Readable cancellation errors in the browser

The [browser driver](evidence/2026-09-06-cancel-transcript-browser.mjs) loads the real app on `noema.kevinpei.com` with controlled transcript responses.
The response uses the failed call and uncertain result shape verified by the store test. It changes no live Task data.
Before the fix, the expanded error showed only the tool name.
The Task mapper now unwraps the saved result. The shared error display prefers its readable message over its code.
At desktop and phone widths, expansion shows exactly one message: “Task cancelled. The external action may have completed.”
The [browser results](evidence/2026-09-06-cancel-transcript-browser-results.json) retain the fixture and visible text.
The [validation record](evidence/2026-09-06-cancel-browser-validation.json) records the scope and passing frontend checks.
Together with the store and controlled MCP checks, RUN-05 passes for controlled cancellation coverage.
The completed run header issue is fixed and verified below.

## Correct status in completed run headers

The Task transcript used a fixed Running label for every run header, including completed runs.
The [header fix](../../apps/web/src/components/chatDetail/task/TaskTranscript.tsx) reuses the existing status-label function.
The [live browser driver](evidence/2026-09-06-run-header-browser.mjs) opens the completed direct-execution Task on `noema.kevinpei.com`.
At desktop and phone widths, both Executor and Reviewer headers show Completed. The API independently reports both runs completed.
The check uses normal server responses and changes no Task data.
The [results](evidence/2026-09-06-run-header-browser-results.json) retain both viewport checks.
The [validation record](evidence/2026-09-06-run-header-validation.json) records passing generated-code checks, lint, and the production build.

## Executor continuation from a saved checkpoint

The [runtime test](../../internal/runtime/task_execution_test.go) uses production execution and SQLite with controlled model responses.
The first Executor saves a support file and records completed work plus exact remaining steps in TASK.md.
Continuation creates one child Executor. Its context contains the saved checkpoint, and its file read returns the earlier output.
The saved transcript contains one write for the completed first step. Final output preserves both required values, including Unicode text.
Planner, both Executors, and Reviewer complete. The first Executor is complete before its child starts.
The [validation record](evidence/2026-09-06-continuation-validation.json) records passing focused tests, broad Go tests, and vet.
The test adds 98 lines. No production change was required.
RUN-07 passes for controlled runtime responses.

## Reopening while an old provider response returns

The [runtime test](../../internal/runtime/task_execution_test.go) holds an old provider response through cancellation and reopening.
The response then proposes overwriting RESULT.md. The runtime closes the old session without accepting its tool call.
TASK.md and RESULT.md retain their exact Unicode content. The Task keeps its new current run and generation.
The old run remains cancelled, and no extra run appears.
The [validation record](evidence/2026-09-06-reopen-late-validation.json) records passing focused tests, broad Go tests, and vet.
The test adds 84 lines. No production fix was required.
The completed-Task check below completes the second reopening variant.

## Late review after reopening a completed Task

The [runtime test](../../internal/runtime/task_execution_test.go) completes planning, execution, and review, then reopens the Task.
It submits the old Reviewer decision again and checks current state plus all three document files.
Before the fix, REVIEW.md was rewritten and restored before the stale decision failed.
The [review completion path](../../internal/runtime/task_execution.go) now checks the current generation and run before touching REVIEW.md.
The stale decision leaves file contents and modification times unchanged. The new Executor and generation remain current.
The [validation record](evidence/2026-09-06-reopen-review-validation.json) records the regression and passing focused tests, broad Go tests, and vet.
The fix adds seven production lines and 69 test lines. Normal correction review remains covered by the focused checks.
Together with the cancelled-Task test, RUN-06 passes for controlled runtime reopening.

## Role recovery after SQLite reopening

The controlled store check closes and reopens SQLite during each Task role.
Planner, Executor, and Reviewer recover the same run and generation.
Saved replay items retain exact Unicode content, identifiers, and sequence.
Usage counts and parent links remain intact.
Repeated recovery creates no extra run, and a second worker cannot claim the leased run.
Each recovered role can finish. The Task reaches Done with three completed runs.

The [validation record](evidence/2026-09-06-role-restart-validation.json) records the test and checks.
This checks persistent store recovery. The runtime check below extends coverage to resumed provider context.

## Runtime recovery for each Task role

The controlled runtime check stops the worker during an active provider call in each role.
It closes SQLite, opens the saved database, and starts a new Task runtime.
The resumed provider receives the exact saved Task file read result.
Planner and Executor finish from their saved progress without another progress write.
Each Task reaches Done with exactly three completed runs.
The interrupted role keeps its run identifier. Task generation and exact Unicode files remain intact.

The [validation record](evidence/2026-09-06-role-runtime-restart-validation.json) records the test and checks.
RUN-08 passes for controlled runtime and database restart with synthetic provider responses.
This does not test abrupt process termination or every external model service.

## Remaining audit workflow

Run the remaining cases before applying batches of fixes.
Record failures with evidence, reproduction steps, and affected cases.
Group fixes by shared cause and subsystem. Recheck failed cases after each fix batch.
Run broad validation once for each completed code batch.
Interrupt the first pass only when a defect blocks further testing or risks test data.
Keep human-only cases marked for later testing.

### Deferred defects

| Defect | Cases | Observed failure | Fix batch | Evidence |
| --- | --- | --- | --- | --- |
| AUDIT-01 | RUN-04 | Review-limit recovery records Executor but queues Reviewer after Retry. | Task gates | [Result](evidence/2026-09-06-review-recovery-results.json), [reproduction patch](evidence/2026-09-06-review-recovery-reproduction.patch) |
| AUDIT-02 | RUN-14 | Executor and Reviewer reject another Task that primary Chat can read for the same owner. | Task reads | [Result](evidence/2026-09-06-task-first-pass-results.json), [reproduction patch](evidence/2026-09-06-task-first-pass-reproduction.patch) |
| AUDIT-03 | RUN-04 | Clarification and approval answers persist but are absent from resumed provider context. | Task gates | [Result](evidence/2026-09-06-task-limits-gates-results.json), [reproduction patch](evidence/2026-09-06-task-limits-gates-reproduction.patch) |
| AUDIT-04 | RUN-15; PROJECT-01 | All three roles reject permitted parent-relative shared reads inside the configured Project folder. | Task file access | [Result](evidence/2026-09-06-project-access-results.json), [reproduction patch](evidence/2026-09-06-project-access-reproduction.patch) |
| AUDIT-05 | RUN-04 | Live clarification Task detail has no visible question or response control in Workspace or Transcript at either width. | Task gate UI | [Browser result](evidence/2026-09-06-impossible-block-results.json) |
| AUDIT-06 | API-14 | Application deletion remains blocked after its only grant is revoked. | OAuth lifecycle | [Result](evidence/2026-09-06-oauth-flow-results.json), [reproduction patch](evidence/2026-09-06-oauth-flow-reproduction.patch) |
| AUDIT-07 | API-07 | Expanded OAuth consent grants the new scope, but the existing connection does not expose the new operation. | OAuth lifecycle | [Result](evidence/2026-09-06-oauth-expansion-results.json), [reproduction patch](evidence/2026-09-06-oauth-expansion-reproduction.patch) |
| AUDIT-08 | MCP-02 | Renewal omits the existing registered OAuth client and fails when dynamic registration is unavailable. | OAuth lifecycle | [Result](evidence/2026-09-06-mcp-oauth-results.json), [reproduction patch](evidence/2026-09-06-mcp-oauth-reproduction.patch) |

The reproduction patch contains a focused failing test. It is outside the normal test suite until the fix batch starts.
Clarification and approval context delivery fails under AUDIT-03. Missing browser gate controls are recorded under AUDIT-05.

## Task first-pass checks

The [controlled tool checks](evidence/2026-09-06-task-first-pass-results.json) cover cross-Task reads and Task file boundaries.
Executor and Reviewer reject cross-Task inspection. Primary Chat accepts the same target for the owner.
AUDIT-02 records this failure. The later fix must preserve the required run-scope rules.

Task file reads, writes, and deletes reject absolute paths, parent traversal, and an outside directory symlink.
The outside file remains exact. An allowed Task file read preserves Unicode content.
The Project check below finds that permitted shared reads fail. Outside reads remain denied.

Existing successful checks cover tool-limit finalization, saved progress audit recovery, and automatic retry exhaustion.
Their source assertions were reviewed. The recorded broad Go result remains valid because its relevant inputs are unchanged.
The separate limit checks below extend this evidence. Review-limit retry remains failed under AUDIT-01.

## Separate limits and human answers

The [controlled checks](evidence/2026-09-06-task-limits-gates-results.json) exercise continuation and active-time limits separately.
The continuation ceiling stops normal calls and permits one terminal-only finalization.
An exhausted active-time budget skips normal calls.
Active-time expiry also interrupts a blocked provider call and starts terminal-only finalization.
Each finalization receives the correct stop reason and saved Task text. Exact Unicode files remain intact without false completion.

Clarification and approval resolution each save one answer and start one new Planner run.
Submitting the same command twice creates no duplicate run or answer.
However, neither resumed provider receives the accepted answer. AUDIT-03 records this failure for the Task gate fix batch.

RUN-09 combines these checks with prior tool-limit, audit, retry, and review-limit evidence.
It covers limit enforcement with controlled runtime and store inputs. Retry after a review-limit gate remains failed under RUN-04.

## Shared Project access and current context

The [controlled Project check](evidence/2026-09-06-project-access-results.json) creates a Project and a linked Task inside its configured folder.
Each role receives exact current `PROJECT.md` text after a separate edit.
Planner, Executor, and Reviewer reject paths outside the Project and a symlink to an outside file.

However, all three roles also reject `../shared.md` inside the Project folder.
The [Task contract](../tasks.md) permits this shared read.
AUDIT-04 records the failure for the Task file-access batch.
Project creation and folder selection remain verified, but working-folder use fails this check.

## Task notices and completion suppression

The [controlled notice check](evidence/2026-09-06-task-notices-results.json) feeds saved Task events through the production primary Chat notice path.
Capture, waiting, recovery, and completion produce the expected saved Chat items.
Narration receives the exact gate question and current result. Tool execution and hosted search stay disabled during narration.
Saved items contain assistant text and the correct Task references. Run transcripts do not enter Chat.
Repeated processing adds no items or narration.

When `notify_human` is false, the Task reaches Done without a completion notice or completion narration call.
Both notice variants pass. These checks use controlled narration and store transitions.
Rendered transitions and an actual unchanged recurrence remain pending.

## Live limitation report and human-input block

The [browser driver](evidence/2026-09-06-impossible-block.mjs) creates two audit Tasks on `noema.kevinpei.com`.
The impossible-integer Task finishes with a correct mathematical limitation report.
Planner, Executor, and Reviewer complete. No human gate opens.
The missing-address Task asks for the complete delivery address and waits at a clarification gate.
It creates no result or label and does not claim completion.

The [results](evidence/2026-09-06-impossible-block-results.json) include saved states and desktop and phone observations.
The limitation report renders at both widths.
The waiting Task document explains that it needs the address.
However, neither Workspace nor Transcript shows the gate question or a response control.
The [Transcript inspection](evidence/2026-09-06-human-input-transcript.mjs) also shows “Waiting for approval” for this clarification gate.
AUDIT-05 records the missing gate controls. The user-input Task remains waiting for later fix verification.

- Completed limitation Task: `task:2c4aed88b9b5e0ddffdee0f99edbac59`.
- Waiting address Task: `task:f932f3c82add0b649c3ffc5d2328e179`.

## Controlled API pagination and response boundaries

The [TLS service check](evidence/2026-09-06-api-pagination-results.json) uses production adapter setup, approval, policy, calls, pagination, and response transforms.
A temporary test transport routes calls to the local fake service. It does not verify production network routing.

An approved no-auth connection exposes usable tools after its policy is saved.
Empty results remain arrays. Two pages retain exact Unicode items.
A failed next-page request preserves its continuation for a successful retry.
Changed arguments, another operation, and a consumed continuation fail before HTTP access.
Cross-account continuation checks remain pending.
Malformed JSON, wrong-type items, and an oversized string fail at the response boundary.
The returned data cannot bypass the reviewed output schema in these cases.

Direct-credential calls, browser setup, and complete OAuth flows remain pending.

## OAuth callback, shared refresh, and deletion checks

The [controlled OAuth check](evidence/2026-09-06-oauth-flow-results.json) runs token exchange and dependent API calls through a local TLS service.
Denial avoids token exchange. A successful callback exchanges once; replay cannot exchange again.
A later attempt lookup and subscriber receive the exact completed grant.
Two API connections share one grant without duplicate attachment.
Concurrent calls refresh once and preserve exact Unicode output. OAuth snapshots exclude token material.

Revocation removes both bindings and rejects saved calls. Active grants correctly block application deletion.
However, deletion remains blocked after the only grant is revoked.
AUDIT-06 records the remaining failure. API-14 is now failed because earlier evidence did not check deletion after disconnection.
Late service recovery passes, but browser background and foreground resumption remain pending.

## OAuth failure recovery

The [controlled failure checks](evidence/2026-09-06-oauth-failures-results.json) cover denial, expiry, token exchange failure, and application replacement during authorization.
Each attempt retains its exact terminal status and creates no grant or callable tools.
Rejected callbacks avoid token exchange, except the intentional token-endpoint failure.
A fresh valid attempt succeeds after each failure.
Browser cancellation, rendered next actions, and restart after configuration changes remain pending.

## Expanded OAuth operation access

The [controlled expansion check](evidence/2026-09-06-oauth-expansion-results.json) adds an operation to an existing account connection.
The authorization request includes both required scopes.
Partial consent correctly leaves the ungranted operation unavailable.
The original operation remains usable, and grant and connection identities stay unchanged.

Full consent grants the additional scope, but the existing connection still exposes only its original operation.
AUDIT-07 records this failure for the OAuth lifecycle batch.
Rendered confirmation of new operation benefits remains pending.

## API connection controls

The [controlled connection check](evidence/2026-09-06-api-controls-results.json) changes one of two APIs that share an OAuth grant.
Rename updates the model tool label and rejects the older binding.
Disable removes the connection from callable tools. Enable restores the exact API result.
Deletion remains effective after adapter service reconstruction.
The neighboring API stays usable, and the shared grant remains exactly unchanged.
This checks the production service path with a controlled TLS endpoint.

## API retries and uncertain writes

A controlled TLS service counted requests from the production adapter.
A disconnected safe read retried once and returned exact Unicode content.
HTTP 429 returned a failed result with its status after one request.
A write disconnect after receipt returned `ErrOutcomeUncertain` after one request.
API-12 passes this controlled service check. Browser presentation and production network routing remain outside its scope.
Evidence: [results](evidence/2026-09-06-api-outcomes-results.json) and [reproduction patch](evidence/2026-09-06-api-outcomes-reproduction.patch).
The temporary test is outside the normal suite. No production code changed.

## MCP discovery, policy, and deletion

Two controlled HTTP services used the production MCP service and real SQLite.
Both returned exact Unicode content through their own discovered bindings.
Policy override, reset, disable, and enable rejected old authority where required.
Read and review-required bindings rejected wrong-type input before remote execution.
Changed remote metadata blocked execution without affecting the other service.
Deletion survived service reconstruction. The deleted binding failed while the other service remained usable.
A disconnected service rejected calls and became unhealthy.
Browser presentation, complete review execution, invalid results, and pending authentication removal remain pending.
Evidence: [results](evidence/2026-09-06-mcp-lifecycle-results.json) and [reproduction patch](evidence/2026-09-06-mcp-lifecycle-reproduction.patch).
The temporary test is outside the normal suite. No production code changed.

## MCP process access and environment

Disabled stdio setup started no process and saved no server.
Enabled setup discovered and invoked the configured controlled process.
The child received exact ordinary Unicode values and its explicit protected credential binding.
The child did not inherit a parent-only environment variable.
Safe server metadata excluded the credential value and preserved ordinary Unicode.
The credential file used mode `0600` on Linux.
Evidence: [results](evidence/2026-09-06-mcp-stdio-results.json) and [reproduction patch](evidence/2026-09-06-mcp-stdio-reproduction.patch).
The temporary test is outside the normal suite. No production code changed.

## MCP OAuth completion, cancellation, and renewal

A controlled OAuth server supported a registered public client without dynamic registration.
Initial authorization completed and signaled the exact attempt. Callback replay failed without another exchange.
Cancellation rejected the late callback and performed no token exchange.
Renewal failed because its setup omitted the existing registered client.
The OAuth handler then selected unsupported dynamic registration. AUDIT-08 records this defect for the OAuth lifecycle batch.
Deletion during renewal remains pending because renewal setup failed first.
Browser next actions and complete pending-call resumption remain pending.
Evidence: [results](evidence/2026-09-06-mcp-oauth-results.json) and [reproduction patch](evidence/2026-09-06-mcp-oauth-reproduction.patch).
The temporary failing test is outside the normal suite. No production code changed.

## ACP coverage review

Existing controlled tests cover probe metadata, authentication, configuration changes, deletion, and references that prevent deletion.
Runtime checks cover exact ACP approval, result files, Reviewer completion, and terminal replay without relaunch.
Process checks cover cancellation, blocked prompt writes, protocol failures, and uncertainty after approval.
These checks passed in the recorded Go validation at `56f3a134`.
Tracked Go source, tests, and module dependencies remain unchanged. The successful results are reused; no new execution is claimed.
All four ACP cases remain partial. Their remaining checks are listed in the evidence map.
Evidence: [coverage map](evidence/2026-09-06-acp-coverage-review.json) and [reused validation](evidence/2026-09-06-role-runtime-restart-validation.json).

## ACP process-start recovery

A missing executable caused the real Task runtime to enter recovery.
The Task document and support file retained exact Unicode content.
After the agent configuration changed, Retry selected the corrected executable and connection revision.
Repeating the same Retry command created no duplicate run. The Task generation remained unchanged.
The corrected process completed through exact approval and controlled provider review.
The result matched exactly, the support file stayed unchanged, and the same generation had five runs.
Browser diagnostic review remains pending.
Evidence: [results](evidence/2026-09-06-acp-failure-results.json) and [reproduction patch](evidence/2026-09-06-acp-failure-reproduction.patch).
The temporary test is outside the normal suite. No production code changed.

## Live ACP settings controls

On noema.kevinpei.com, synthetic ACP entries were created, tested, edited, and deleted at desktop and phone widths.
The missing executable produced “ACP initialization failed” beneath the correct entry.
Unicode names remained visible. Both entries were deleted through the browser.
Successful process probing, authentication, and Task assignment after deletion remain pending.
Evidence: [results](evidence/2026-09-06-acp-settings-results.json) and [browser driver](evidence/2026-09-06-acp-settings.mjs).
Screenshots were inspected at both widths. Their hashes are recorded; image files remain outside the repository.

## Live ACP authentication with a controlled process

The browser successfully probed a controlled Python ACP service at desktop and phone widths.
Each authentication action invoked the selected `audit` method once.
After reload, the authentication action was no longer required.
Both synthetic entries were edited and deleted through the browser.
Task assignment after deletion remains pending. Real external authentication remains a later human check.
Evidence: [results](evidence/2026-09-06-acp-auth-settings-results.json), [browser driver](evidence/2026-09-06-acp-auth-settings.mjs), and [controlled process](evidence/2026-09-06-acp-browser-service.py).
Screenshots were inspected at both widths. Their hashes are recorded; image files remain outside the repository.

## Case results

**Partial** means that evidence covers only the named portion.
**Not run** means that this run has no complete case evidence yet.
A human variant remains pending even when its controlled counterpart passes.

| Case | Automatic result | Evidence or remaining work | Human variant |
| --- | --- | --- | --- |
| HOME-01 | Pass · Linux/Chromium | Packaged release opens fresh passkey setup. Other native platforms remain pending. | — |
| HOME-03 | Partial | Core restart restores passkeys, Tasks, Project documents, and model selections. Chat, Memory, integrations, and Artifacts remain pending. | — |
| HOME-04 | Not run | Controlled setup pending. | — |
| HOME-05 | Partial | Complete stopped-home copy and restore preserves access, Task content, Project context, and setup. Schedules and remaining data classes are pending. | — |
| HOME-06 | Partial | Release startup, passkey setup, recovery, and restart work in a Unicode home with spaces. Files and helpers remain pending. | — |
| HOME-07 | Not run | Controlled setup pending. | — |
| HOME-08 | Not run | Controlled setup pending. | — |
| HOME-09 | Not run | Controlled setup pending. | — |
| HOME-10 | Not run | Requires a native Windows runner. | — |
| AUTH-01 | Pass · Chromium virtual passkey | Initial claim admits authenticated GraphQL. Product setup follow-through remains in SETUP. | Not run |
| AUTH-02 | Not run | Controlled setup pending. | Not run |
| AUTH-03 | Partial | Authenticated session survives server restart. Browser close and reopen remain pending. | Not run |
| AUTH-04 | Partial | Final passkey removal returns 409. Additional-key management and recent-verification checks remain pending. | Not run |
| AUTH-05 | Pass · Chromium virtual passkey | Recovery enrolls a new key. Reusing the consumed recovery code returns 401. | Not run |
| AUTH-06 | Partial | Fresh login works after logout. Invalid, expired, and replayed browser ceremonies remain pending. | Not run |
| AUTH-07 | Partial | Logout denies reads and writes in another tab. Push cleanup and rendered Chat recovery remain pending. | Not run |
| AUTH-08 | Partial | Unclaimed reads and signed-out GraphQL, artifacts, and WebSocket are denied. Unclaimed WebSocket remains pending. | — |
| AUTH-09 | Pass · Chromium | Authenticated foreign-Origin requests return 403. Foreign-Host requests return 400. | — |
| AUTH-10 | Not run | Controlled setup pending. | Not run |
| SETUP-02 | Partial | Proposed client choices leave setup incomplete. One confirmation saves the complete selection. Browser draft controls remain pending. | — |
| SETUP-03 | Not run | Controlled setup pending. | Not run |
| SETUP-04 | Not run | Controlled setup pending. | Not run |
| SETUP-05 | Not run | Controlled setup pending. | — |
| SETUP-06 | Not run | Controlled setup pending. | — |
| SETUP-07 | Not run | Controlled setup pending. | — |
| SETUP-08 | Not run | Controlled setup pending. | — |
| SETUP-09 | Not run | Controlled setup pending. | — |
| SETUP-10 | Not run | Controlled setup pending. | — |
| CHAT-01 | Pass · live Codex/Chromium | One exact human message and one assistant response remain in order after reload. Other providers remain pending. | — |
| CHAT-02 | Partial · live Codex | Eight tool calls have eight matching successful results. The final answer matches stored Project and Task state. Other providers remain pending. Live Chat capture checks above. | — |
| CHAT-03 | Pass · live Codex/Chromium | Dropped text and tool-result events recover after socket closure. Saved text and call/result pairs remain exact without duplicates after reload. | — |
| CHAT-04 | Not run | Controlled setup pending. | — |
| CHAT-05 | Pass · live Codex/Chromium | Browser history loads during a new response. Forty adjacent items retain exact content and order without gaps or duplicates, including after reload. | — |
| CHAT-06 | Partial · live Chromium | Unicode, emoji, paragraphs, code, lists, and links survive reload and a second browser read. Native clients remain pending. | — |
| CHAT-07 | Not run | Controlled setup pending. | — |
| CHAT-08 | Pass · live Codex | The explicit foreground request returns its exact response in Chat and creates no Task. Other provider variants remain pending. | — |
| CHAT-09 | Pass · live Codex/Chromium | Single and multiple selections resume once and survive reload. Ordinary text receives a clear rejection; the Rust baseline also excludes free-text continuation. | — |
| CHAT-10 | Pass · live Codex/Chromium | Another browser cannot resubmit an answered prompt. The original selection and turn remain unchanged. Live choice checks above. | — |
| CHAT-11 | Pass · live Codex/Chromium | All nine catalog components render. Keyboard and pointer submissions preserve exact bound values. One action resumes the correct turn and survives reload. | — |
| CHAT-12 | Pass · live Codex/Chromium | Unsupported components and active HTML create no surface. Forged context and stale actions are rejected. Chat remains usable. | — |
| CHAT-13 | Not run | Controlled setup pending. | — |
| CHAT-14 | Not run | Controlled setup pending. | — |
| CHAT-15 | Pass · live Codex/Chromium | A repeated title creates a distinct requested Task with fresh tool evidence. The older Task remains unchanged. The new Chat reference opens the new document. | — |
| MODEL-01 | Not run | Controlled setup pending. | — |
| MODEL-02 | Not run | Controlled setup pending. | — |
| MODEL-03 | Not run | Controlled setup pending. | — |
| MODEL-04 | Not run | Controlled setup pending. | — |
| MODEL-05 | Not run | Controlled setup pending. | — |
| MODEL-06 | Not run | Controlled setup pending. | — |
| MODEL-07 | Not run | Controlled setup pending. | — |
| MODEL-08 | Not run | Controlled setup pending. | — |
| TASK-01 | Pass · live Chromium | Chat captures one exact Inbox request in the existing Project. The Task keeps its conversation source and opens from Chat. Live Chat capture checks above. | — |
| TASK-02 | Pass · live Linux/Chromium | Add to Inbox preserves the exact request without execution. Run Now starts a Task that completes through review. | — |
| TASK-03 | Pass · live Chromium | Browser title and exact Unicode Markdown edits survive reload. Renaming preserves the allocated directory. Live Task editing checks above. | — |
| TASK-04 | Pass · two live browser contexts | A competing save preserves the stale local draft and accepted server content. Acknowledgement and cancel do not overwrite either silently. Live Task editing checks above. | — |
| TASK-05 | Partial | Source/rich switching, cancel, stale acknowledgement, and rich-editor load failure preserve source editing. Parser-specific failure remains pending. Live Task editing checks above. | — |
| TASK-06 | Pass · live Chromium | Starting work invalidates the held edit and disables saving. Active and completed documents remain readable; completed editing controls are absent. Live Task editing checks above. | — |
| TASK-07 | Pass · live Chromium | Personal, Project, Scheduled, and terminal filters show the expected records. Selection and available actions match Task state. Live Task view checks above. | — |
| TASK-08 | Pass · live Chromium | Reopen selects the result. Request, review, support Markdown, and literal text remain distinct. Transcript navigation reaches the Planner start. Live Task editing checks above. | — |
| TASK-09 | Pass · live Chromium | Completion opens the result. A later request selection remains selected across Transcript and Workspace navigation. Live Task editing checks above. | — |
| TASK-10 | Partial | A repeated capture returns the original Task identity. Lost responses for other commands remain pending. | — |
| TASK-11 | Pass · Linux | Equal Unicode titles create distinct Task directories. Exact old and new documents remain intact. | — |
| RUN-01 | Pass · live calculation Task | Planner, Executor, and Reviewer all complete. Correct result and accepting review persist before the Task is done. | — |
| RUN-02 | Pass · live Codex/Chromium | Direct delegation preserves the request and completes with one Executor, one Reviewer, no Planner, and result 72. Optional enum conversion fixed. | — |
| RUN-03 | Pass · controlled runtime | Rejected result remains incomplete. Correction receives current request, result, and review files. A second review approves the corrected result. | — |
| RUN-04 | Fail · controlled runtime/live browser | AUDIT-01: wrong retry role. AUDIT-03: missing answer context. AUDIT-05: missing visible gate question and response controls. | — |
| RUN-05 | Pass · controlled store/runtime/browser | Cancellation rejects old work across three states. Late MCP success retains uncertainty. The saved result is readable at desktop and phone widths. | — |
| RUN-06 | Pass · controlled runtime | Cancelled and completed Tasks preserve current files after reopening. Late provider writes and Reviewer decisions cannot replace the new generation. | — |
| RUN-07 | Pass · controlled runtime | Continuation reloads the saved checkpoint and support file. The first completed write occurs once. A second Executor and final Reviewer complete the Task. | — |
| RUN-08 | Pass · controlled runtime | Restart during each role restores exact saved read results and progress. Files, generation, and run identity remain intact. Each Task finishes with three completed runs. | — |
| RUN-09 | Pass · controlled runtime/store | Continuation, tool, active-time, retry, review, and audit limits reach their recorded stop states. Progress remains intact. Gate resumption defects stay under RUN-04. | — |
| RUN-10 | Pass · live Codex/Chromium | Impossible request produces an honest completed limitation report. Missing address opens clarification without a fabricated label or false completion. | — |
| RUN-11 | Not run | Controlled setup pending. | — |
| RUN-12 | Partial | Saved capture, waiting, recovery, and completion notices pass. Repeated processing adds no output. Rendered transitions remain pending. | — |
| RUN-13 | Partial | A false notify_human decision suppresses completion narration and saved notices while the Task reaches Done. Actual unchanged recurrence remains pending. | — |
| RUN-14 | Fail · controlled tool handler | AUDIT-02: Executor and Reviewer reject another Task that primary Chat can read for the same owner. | — |
| RUN-15 | Fail · controlled tool handler | Outside reads, writes, and deletes are denied. AUDIT-04: all three roles also reject permitted shared Project reads. | — |
| TIME-01 | Pass · live UTC deadline | No run before the due time. One Planner queued five milliseconds afterward. The Task completed with the expected result. | — |
| TIME-02 | Pass · live Codex/Chromium | Browser reschedule replaces timing. Unschedule preserves the same Task in Inbox after reload. Run now completes that Task early with one Executor run. | — |
| TIME-03 | Pass · live Codex/Chromium and controlled runtime | Live requests resolve tomorrow at 09:00 in Los Angeles and Tokyo. Later roles receive fresh clocks, original request timestamps, and occurrence cutoffs. | — |
| TIME-04 | Pass · live Codex/Chromium | One-time and recurring RUN_ONCE execute once after restart. SKIP avoids execution. Completed state survives a second restart. | — |
| TIME-05 | Pass · live Codex/Chromium | The active recurrence remains in Scheduled after its manual occurrence completes and its initial occurrence is cancelled. History survives reload. | — |
| TIME-06 | Pass · live Codex/Chromium | The automatic slot copies the exact edited template and revision. The earlier Task file stays unchanged. The new Task completes the edited request. | — |
| TIME-07 | Pass · Chromium | Fixed the recurrence schedule dialog crash. Competing browser drafts preserve current text and timing. The stale description survives reload for an explicit save. | — |
| TIME-08 | Pass · live scheduler/Codex/Chromium | Pause and skip prevent work at the deadline. Resume permits one next-slot occurrence. End prevents later occurrences and preserves readable, immutable templates. | — |
| TIME-09 | Pass · live Codex/Chromium | Browser Run now creates one manual occurrence and preserves the next normal slot through completion. Its history link opens the correct result. | — |
| TIME-10 | Pass · controlled scheduler state | Skip creates no later Task while active. Queue one releases one catch-up Task. Allow creates both due occurrences. Fixed extra queue-one catch-up work. | — |
| TIME-11 | Pass · controlled scheduler time | Production scheduling skips the missing spring minute and creates no duplicate run for the repeated fall minute. The next valid day queues one occurrence. | — |
| TIME-12 | Pass · controlled scheduler state | One-time and recurrence races preserve accepted timing. Replaced slots create no later work. Existing occurrences retain their revision, and each occurrence queues once. | — |
| PROJECT-01 | Fail · shared file access | Live creation, rename, and folder selection pass. AUDIT-04: linked Task roles cannot read permitted shared files. | — |
| PROJECT-02 | Pass · live Chromium | Exact saves survive reload. A competing browser retains its draft. Acknowledgement and cancellation preserve accepted content. Live Project checks above. | — |
| PROJECT-03 | Pass · controlled role context | Each role receives exact current PROJECT.md text after separate edits. | — |
| PROJECT-04 | Pass · live Chromium | Archive preserves readable context and removes editing controls in both browsers. Reopen restores editing with the same content. Live Project checks above. | — |
| PROJECT-05 | Pass · live Codex | Chat reads the existing Project and lists its Tasks, including cancelled work. It captures a request without duplicating the Project. Live Chat capture checks above. | — |
| AGENT-01 | Not run | Controlled setup pending. | — |
| AGENT-02 | Not run | Controlled setup pending. | — |
| AGENT-03 | Not run | Controlled setup pending. | — |
| MEM-01 | Not run | The live tree already contains a cited profile. A controlled empty-state check remains pending. | — |
| MEM-02 | Pass · live Chromium | Browser update creates cited articles from exact synthetic human facts. The saved sequence advances and status returns to idle. | — |
| MEM-03 | Not run | Controlled setup pending. | — |
| MEM-04 | Pass · live Chromium/Chat | The newer preference replaces the old fact and cites its correction. Chat reads the current article and reports French. | — |
| MEM-05 | Pass · live Chat/Chromium | Repeated preferences leave article paths and bodies unchanged. An assistant-invented fictional occupation is not stored or cited as a human fact. | — |
| MEM-06 | Partial · live Chromium | Root and child articles open through exact related-article routes. Deep hierarchy and ancestor checks remain pending. | — |
| MEM-07 | Pass · live Chromium | Pointer hover and keyboard focus show the exact source type, date, excerpt, and identity at desktop and phone widths. | — |
| MEM-08 | Partial · live Chat | Search and deeper page reads return the corrected fact and unchanged leisure interest. Bounded root-context injection remains pending. | — |
| MEM-09 | Not run | Controlled setup pending. | — |
| MEM-10 | Pass · live Linux/Chromium | Restart rebuilds the current lexical index from Markdown. Exact searches, article content, citations, and source-file hashes remain unchanged. | — |
| MEM-11 | Not run | Controlled setup pending. | — |
| MEM-12 | Not run | Controlled setup pending. | — |
| ACTION-01 | Not run | Controlled setup pending. | — |
| ACTION-02 | Not run | Controlled setup pending. | — |
| ACTION-03 | Not run | Controlled setup pending. | — |
| ACTION-04 | Not run | Controlled setup pending. | — |
| ACTION-05 | Not run | Controlled setup pending. | — |
| ACTION-06 | Not run | Controlled setup pending. | — |
| ACTION-07 | Not run | Controlled setup pending. | — |
| ACTION-08 | Not run | Controlled setup pending. | — |
| ACTION-09 | Not run | Controlled setup pending. | — |
| INFO-01 | Not run | Controlled setup pending. | — |
| INFO-02 | Not run | Controlled setup pending. | — |
| INFO-03 | Not run | Controlled setup pending. | — |
| INFO-04 | Not run | Controlled setup pending. | — |
| INFO-05 | Not run | Controlled setup pending. | — |
| INFO-06 | Not run | Controlled setup pending. | — |
| API-01 | Partial | Chat tool calls propose two definitions. Explicit client review accepts each. Controlled OAuth then creates usable API connections. | — |
| API-02 | Partial | Approved no-auth tools execute against a controlled TLS service. Direct-credential calls and browser setup remain pending. | — |
| API-03 | Partial | One fake browser OAuth sign-in attaches two APIs to a shared grant. Both return the same synthetic account. Independent policy variants remain pending. | Not run |
| API-04 | Partial | Both synthetic accounts return their own records. Current labels identify model tools. OAuth attachment replies retain grant metadata. Write and identity-discovery variants remain pending. | Not run |
| API-05 | Partial | Denied, expired, failed, and superseded callbacks expose no grant or tools. Fresh attempts recover. Browser cancellation and next actions remain pending. | Not run |
| API-06 | Partial | Late attempt lookup and subscription recover the exact completed grant. Browser background and foreground resumption remain pending. | Not run |
| API-07 | Fail · controlled TLS service | Partial consent preserves access limits. AUDIT-07: full consent does not expose the newly granted operation on the existing connection. | Not run |
| API-08 | Pass · controlled HTTPS | Sequential browser-routed calls and concurrent production adapter calls refresh the shared token once and preserve the selected account. Real consent remains pending. | Not run |
| API-09 | Pass · controlled TLS service | Empty arrays, two exact Unicode pages, and failed next-page retry pass. Query bounds remain enforced. | — |
| API-10 | Partial | Changed arguments, another operation, and consumed continuation fail before HTTP. Cross-account checks remain pending. | — |
| API-11 | Pass · controlled TLS service | Malformed JSON, wrong-type items, and oversized output fail without bypassing the reviewed schema. | — |
| API-12 | Pass · controlled TLS service | Safe read disconnect retries once. HTTP 429 stays failed. A received write stays uncertain without automatic repetition. | Not run |
| API-13 | Pass · controlled TLS service | Rename, disable, enable, and delete pass. Deletion survives service restart. The shared grant and neighboring API remain intact. | — |
| API-14 | Fail · controlled TLS service | Shared-grant revocation stops dependent access. AUDIT-06: application deletion remains blocked after grant revocation. | — |
| API-15 | Partial | Application replacement rejects older OAuth attempts before token exchange. Restart, retained identity, and changed-approval checks remain pending. | — |
| MCP-01 | Partial | Two controlled HTTP services discover tools and return exact Unicode under their own bindings. Browser result association remains pending. | — |
| MCP-02 | Fail · controlled HTTP service | Completion, replay rejection, and cancellation pass. AUDIT-08: renewal loses the registered OAuth client. Browser next actions and pending-call resumption remain pending. | Not run |
| MCP-03 | Pass · controlled HTTP service | Changed remote metadata rejects saved authority before execution. The other service remains usable. | — |
| MCP-04 | Partial | Override, reset, disable, and enable enforce current authority. Wrong-type input fails for read and review-required bindings. Full review execution remains pending. | — |
| MCP-05 | Partial | Disconnect rejects the call and marks the service unhealthy. Invalid results and browser errors remain pending. | — |
| MCP-06 | Pass · controlled Linux process | Disabled setup starts no process. Enabled setup calls the configured service with explicit environment bindings and protected credentials. Parent-only values stay absent. | — |
| MCP-07 | Partial | Deletion survives service reconstruction and rejects saved calls. The other service remains usable. Pending authentication removal remains pending. | — |
| ACP-01 | Partial | Browser create, successful and failed probes, named authentication, edit, and delete pass at both widths. Post-deletion Task assignment remains pending. | Not run |
| ACP-02 | Partial | Controlled runtime covers ACP approval, result files, review, and saved terminal replay. Published artifacts and equivalent file boundaries remain pending. | — |
| ACP-03 | Partial | Controlled process cancellation and uncertain protocol failure pass existing checks. Late output after Task cancellation remains pending. | — |
| ACP-04 | Partial | Process-start failure preserves exact files. Corrected Retry completes through approval and review without duplicate runs. Browser diagnostic review remains pending. | — |
| WEB-01 | Not run | Controlled setup pending. | — |
| WEB-02 | Not run | Controlled setup pending. | — |
| WEB-03 | Not run | Controlled setup pending. | — |
| WEB-04 | Not run | Controlled setup pending. | — |
| WEB-05 | Not run | Controlled setup pending. | — |
| WEB-06 | Not run | Controlled setup pending. | — |
| WEB-07 | Not run | Controlled setup pending. | — |
| WEB-08 | Not run | Controlled setup pending. | — |
| WEB-09 | Not run | Controlled setup pending. | — |
| WEB-10 | Not run | Controlled setup pending. | — |
| WEB-11 | Not run | Controlled setup pending. | — |
| WEB-12 | Not run | Controlled setup pending. | — |
| WEB-13 | Not run | Controlled setup pending. | — |
| WEB-14 | Not run | Controlled setup pending. | — |
| WEB-15 | Not run | Controlled setup pending. | — |
| WEB-16 | Not run | Controlled setup pending. | — |
| FILE-01 | Not run | Controlled setup pending. | — |
| FILE-02 | Not run | Controlled setup pending. | — |
| FILE-03 | Partial | DOCX, ODT, and RTF preserve known structure through the packaged Chat tool. DOC remains pending. | — |
| FILE-04 | Partial | ODP preserves title, body, and speaker notes through Chat. PPT and PPTX remain pending. | — |
| FILE-05 | Not run | Controlled setup pending. | — |
| FILE-06 | Not run | Controlled setup pending. | — |
| FILE-07 | Not run | Controlled setup pending. | — |
| FILE-08 | Pass · controlled Linux | Unicode filenames and text survive Chat parsing. Invalid UTF-8 returns an explicit failure. | — |
| FILE-09 | Partial | Malformed DOCX, parent traversal, and symbolic links are rejected. Oversized and unsafe archives remain pending. | — |
| FILE-10 | Not run | Controlled setup pending. | — |
| CALC-01 | Pass · controlled Linux | Budget, percentage, structured JSON, nulls, and Unicode match independent values. | — |
| CALC-02 | Pass · controlled Linux | Unbounded Lua and file, process, environment, and network access fail within 15 seconds each. | — |
| ART-01 | Pass | Reopened the completed result and downloaded its published file. Owner, media type, filename, size, and exact bytes match. Live Artifact checks above. | — |
| ART-02 | Partial | External URL and conversation owner remain exact through GraphQL. Visible Chat reference remains pending. Live Artifact checks above. | — |
| ART-03 | Partial | Two versions download with distinct original bytes. The browser selector renders each version. Saved upload-action binding remains pending. Live Artifact checks above. | — |
| ART-04 | Partial | Markdown, literal text, and PDF render. Raster preview bytes match. Raster rendering and spreadsheet variants remain pending. Live Artifact checks above. | — |
| ART-05 | Pass | Live HTML preview preserves visible content and blocks active behavior at desktop and phone widths. Live Artifact checks above. | — |
| ART-06 | Pass | SVG and binary show Preview unavailable, expose no inline frame, and retain exact browser downloads. Live Artifact checks above. | — |
| ART-07 | Pass | Changed, missing, and symlink files fail delivery and show a preview load error. Restoring original bytes restores the preview. Live Artifact checks above. | — |
| ART-08 | Pass | Same URL returns 401 before login, 200 after passkey login, and 401 after logout. Authorized responses use no-store. Live Artifact checks above. | — |
| ART-09 | Pass | A 40 KiB binary retains exact bytes, filename, media type, length, and private caching across a verified server restart. Live Artifact checks above. | — |
| NOTE-01 | Not run | Controlled setup pending. | Not run |
| NOTE-02 | Not run | Controlled setup pending. | Not run |
| NOTE-03 | Not run | Controlled setup pending. | Not run |
| NOTE-04 | Not run | Controlled setup pending. | Not run |
| NOTE-05 | Not run | Controlled setup pending. | — |
| NOTE-06 | Not run | Controlled setup pending. | — |
| NOTE-07 | Not run | Controlled setup pending. | Not run |
| NOTE-08 | Not run | Controlled setup pending. | — |
| NOTE-09 | Not run | Controlled setup pending. | Not run |
| NOTE-10 | Not run | Controlled setup pending. | Not run |
| NOTE-11 | Not run | Controlled setup pending. | Not run |
| NOTE-12 | Not run | Controlled setup pending. | — |
| CLIENT-01 | Not run | Controlled setup pending. | Not run |
| CLIENT-02 | Not run | Controlled setup pending. | — |
| CLIENT-03 | Not run | Controlled setup pending. | — |
| CLIENT-04 | Not run | Controlled setup pending. | — |
| PWA-01 | Not run | Controlled setup pending. | Not run |
| PWA-02 | Not run | Controlled setup pending. | Not run |
| PWA-03 | Not run | Controlled setup pending. | Not run |
| PWA-04 | Not run | Controlled setup pending. | Not run |
| PWA-05 | Not run | Controlled setup pending. | Not run |
| PWA-06 | Not run | Controlled setup pending. | Not run |
| PWA-07 | Not run | Controlled setup pending. | Not run |
| NATIVE-01 | Not run | Controlled setup pending. | Not run |
| NATIVE-02 | Not run | Controlled setup pending. | Not run |
| NATIVE-03 | Not run | Controlled setup pending. | Not run |
| NATIVE-04 | Not applicable | Physical device journey. | Not run |
| NATIVE-05 | Not run | Controlled setup pending. | Not run |
| NATIVE-06 | Not run | Controlled setup pending. | Not run |
| DESKTOP-01 | Not run | Controlled setup pending. | Not run |
| DESKTOP-02 | Not run | Controlled setup pending. | Not run |
| DESKTOP-03 | Not run | Controlled setup pending. | Not run |
| UX-01 | Not run | Controlled setup pending. | Not run |
| UX-02 | Not run | Controlled setup pending. | Not run |
| UX-03 | Not run | Controlled setup pending. | — |
| UX-04 | Not run | Controlled setup pending. | — |
| UX-05 | Not run | Controlled setup pending. | — |
| DIAG-01 | Not run | Controlled setup pending. | — |
| DIAG-02 | Not run | Controlled setup pending. | — |
| OPS-01 | Not run | Controlled setup pending. | — |
| OPS-02 | Not run | Controlled setup pending. | — |
| OPS-03 | Not run | Controlled setup pending. | — |
| JOURNEY-01 | Not run | Controlled setup pending. | Not run |
| JOURNEY-02 | Not run | Controlled setup pending. | — |
| JOURNEY-03 | Not run | Controlled setup pending. | — |
| JOURNEY-04 | Not run | Controlled setup pending. | — |
| JOURNEY-05 | Not run | Controlled setup pending. | — |
| JOURNEY-06 | Not run | Controlled setup pending. | — |
| JOURNEY-07 | Not run | Controlled setup pending. | — |
| JOURNEY-08 | Not applicable | Physical device journey. | Not run |
| JOURNEY-09 | Not run | Controlled setup pending. | Not run |
| JOURNEY-10 | Not run | Controlled setup pending. | — |
| JOURNEY-11 | Not run | Controlled setup pending. | — |
| JOURNEY-12 | Not run | Controlled setup pending. | — |
