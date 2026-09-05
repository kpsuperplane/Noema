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
| CHAT-01 | Partial | Controlled OpenAI-compatible HTTPS transport saves an answer and restores it after reload. Other providers and transcript count assertions remain pending. | — |
| CHAT-02 | Not run | Controlled setup pending. | — |
| CHAT-03 | Not run | Controlled setup pending. | — |
| CHAT-04 | Not run | Controlled setup pending. | — |
| CHAT-05 | Not run | Controlled setup pending. | — |
| CHAT-06 | Partial | Controlled provider text preserves Unicode, emoji, paragraphs, and an exact URL through reload. Rich text and cross-client variants remain pending. | — |
| CHAT-07 | Not run | Controlled setup pending. | — |
| CHAT-08 | Not run | Controlled setup pending. | — |
| CHAT-09 | Not run | Controlled setup pending. | — |
| CHAT-10 | Not run | Controlled setup pending. | — |
| CHAT-11 | Not run | Controlled setup pending. | — |
| CHAT-12 | Not run | Controlled setup pending. | — |
| CHAT-13 | Not run | Controlled setup pending. | — |
| CHAT-14 | Not run | Controlled setup pending. | — |
| CHAT-15 | Not run | Controlled setup pending. | — |
| MODEL-01 | Not run | Controlled setup pending. | — |
| MODEL-02 | Not run | Controlled setup pending. | — |
| MODEL-03 | Not run | Controlled setup pending. | — |
| MODEL-04 | Not run | Controlled setup pending. | — |
| MODEL-05 | Not run | Controlled setup pending. | — |
| MODEL-06 | Not run | Controlled setup pending. | — |
| MODEL-07 | Not run | Controlled setup pending. | — |
| MODEL-08 | Not run | Controlled setup pending. | — |
| TASK-01 | Not run | Controlled setup pending. | — |
| TASK-02 | Pass · live Linux/Chromium | Add to Inbox preserves the exact request without execution. Run Now starts a Task that completes through review. | — |
| TASK-03 | Partial | Command edits preserve exact content in the allocated directory. Browser edit controls remain pending. | — |
| TASK-04 | Partial | The server rejects a stale competing save and preserves current content. Browser draft recovery remains pending. | — |
| TASK-05 | Not run | Controlled setup pending. | — |
| TASK-06 | Not run | Controlled setup pending. | — |
| TASK-07 | Not run | Controlled setup pending. | — |
| TASK-08 | Partial | Completed Task opens RESULT.md. Request and review render separately. Transcript opens; full history and support-file checks remain pending. | — |
| TASK-09 | Not run | Controlled setup pending. | — |
| TASK-10 | Partial | A repeated capture returns the original Task identity. Lost responses for other commands remain pending. | — |
| TASK-11 | Pass · Linux | Equal Unicode titles create distinct Task directories. Exact old and new documents remain intact. | — |
| RUN-01 | Pass · live calculation Task | Planner, Executor, and Reviewer all complete. Correct result and accepting review persist before the Task is done. | — |
| RUN-02 | Not run | Controlled setup pending. | — |
| RUN-03 | Not run | Controlled setup pending. | — |
| RUN-04 | Not run | Controlled setup pending. | — |
| RUN-05 | Not run | Controlled setup pending. | — |
| RUN-06 | Not run | Controlled setup pending. | — |
| RUN-07 | Not run | Controlled setup pending. | — |
| RUN-08 | Not run | Controlled setup pending. | — |
| RUN-09 | Not run | Controlled setup pending. | — |
| RUN-10 | Not run | Controlled setup pending. | — |
| RUN-11 | Not run | Controlled setup pending. | — |
| RUN-12 | Not run | Controlled setup pending. | — |
| RUN-13 | Not run | Controlled setup pending. | — |
| RUN-14 | Not run | Controlled setup pending. | — |
| RUN-15 | Not run | Controlled setup pending. | — |
| TIME-01 | Pass · live UTC deadline | No run before the due time. One Planner queued five milliseconds afterward. The Task completed with the expected result. | — |
| TIME-02 | Partial | Fixed released schedules that never entered the queue. Reschedule, unschedule, and early execution pass through commands. Browser controls remain pending. | — |
| TIME-03 | Not run | Controlled setup pending. | — |
| TIME-04 | Not run | Controlled setup pending. | — |
| TIME-05 | Not run | Controlled setup pending. | — |
| TIME-06 | Partial | A manual occurrence copies the updated template. Existing Task content remains unchanged. Automatic future-slot copy remains pending. | — |
| TIME-07 | Partial | Document-only saves now work. Stale updates preserve current text. Browser draft recovery remains pending. | — |
| TIME-08 | Partial | Pause, resume, and skip next update schedule state. End preserves readable content and rejects edits. Future execution variants remain pending. | — |
| TIME-09 | Partial | One manual occurrence starts with the updated document and preserves the next normal slot. Browser history remains pending. | — |
| TIME-10 | Not run | Controlled setup pending. | — |
| TIME-11 | Partial | Fixed duplicate fall-back previews. Spring and fall preview assertions pass. Actual scheduled transition checks remain pending. | — |
| TIME-12 | Not run | Controlled setup pending. | — |
| PROJECT-01 | Partial | Live Project creation, rename, and shared folder settings pass. Task role use of the folder remains pending. | — |
| PROJECT-02 | Partial | Competing Project saves reject stale content. The current document survives reload and restore. Browser draft recovery remains pending. | — |
| PROJECT-03 | Not run | Controlled setup pending. | — |
| PROJECT-04 | Partial | Live archive preserves readable context and denies edits. Reopen permits a new saved edit. Browser controls remain pending. | — |
| PROJECT-05 | Not run | Controlled setup pending. | — |
| AGENT-01 | Not run | Controlled setup pending. | — |
| AGENT-02 | Not run | Controlled setup pending. | — |
| AGENT-03 | Not run | Controlled setup pending. | — |
| MEM-01 | Not run | Controlled setup pending. | — |
| MEM-02 | Not run | Controlled setup pending. | — |
| MEM-03 | Not run | Controlled setup pending. | — |
| MEM-04 | Not run | Controlled setup pending. | — |
| MEM-05 | Not run | Controlled setup pending. | — |
| MEM-06 | Not run | Controlled setup pending. | — |
| MEM-07 | Not run | Controlled setup pending. | — |
| MEM-08 | Not run | Controlled setup pending. | — |
| MEM-09 | Not run | Controlled setup pending. | — |
| MEM-10 | Not run | Controlled setup pending. | — |
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
| API-02 | Not run | Controlled setup pending. | — |
| API-03 | Partial | One fake browser OAuth sign-in attaches two APIs to a shared grant. Both return the same synthetic account. Independent policy variants remain pending. | Not run |
| API-04 | Partial | Both synthetic accounts return their own records. Current labels identify model tools. OAuth attachment replies retain grant metadata. Write and identity-discovery variants remain pending. | Not run |
| API-05 | Partial | Fake consent denial creates no token and records a denied attempt. Cancel, expiry, and other callback failures remain pending. | Not run |
| API-06 | Not run | Controlled setup pending. | Not run |
| API-07 | Not run | Controlled setup pending. | Not run |
| API-08 | Pass · controlled HTTPS | Sequential browser-routed calls and concurrent production adapter calls refresh the shared token once and preserve the selected account. Real consent remains pending. | Not run |
| API-09 | Not run | Controlled setup pending. | — |
| API-10 | Not run | Controlled setup pending. | — |
| API-11 | Not run | Controlled setup pending. | — |
| API-12 | Not run | Controlled setup pending. | — |
| API-13 | Partial | Connection and account labels update model descriptions. Disable, re-enable, and deletion variants remain pending. | — |
| API-14 | Pass · controlled HTTPS | Shared-grant revocation removes both tools and rejects both saved calls before delivery. Other-account access remains available. Active grants block application deletion. | — |
| API-15 | Not run | Controlled setup pending. | — |
| MCP-01 | Not run | Controlled setup pending. | — |
| MCP-02 | Not run | Controlled setup pending. | Not run |
| MCP-03 | Not run | Controlled setup pending. | — |
| MCP-04 | Not run | Controlled setup pending. | — |
| MCP-05 | Not run | Controlled setup pending. | — |
| MCP-06 | Not run | Controlled setup pending. | — |
| MCP-07 | Not run | Controlled setup pending. | — |
| ACP-01 | Not run | Controlled setup pending. | Not run |
| ACP-02 | Not run | Controlled setup pending. | — |
| ACP-03 | Not run | Controlled setup pending. | — |
| ACP-04 | Not run | Controlled setup pending. | — |
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
| ART-01 | Pass | Reopened the completed result and downloaded its published file. Owner, media type, filename, size, and exact bytes match. | Live Artifact checks above. |
| ART-02 | Partial | External URL and conversation owner remain exact through GraphQL. Visible Chat reference remains pending. | Live Artifact checks above. |
| ART-03 | Partial | Two versions download with distinct original bytes. The browser selector renders each version. Saved upload-action binding remains pending. | Live Artifact checks above. |
| ART-04 | Partial | Markdown, literal text, and PDF render. Raster preview bytes match. Raster rendering and spreadsheet variants remain pending. | Live Artifact checks above. |
| ART-05 | Pass | Live HTML preview preserves visible content and blocks active behavior at desktop and phone widths. | Live Artifact checks above. |
| ART-06 | Pass | SVG and binary show Preview unavailable, expose no inline frame, and retain exact browser downloads. | Live Artifact checks above. |
| ART-07 | Pass | Changed, missing, and symlink files fail delivery and show a preview load error. Restoring original bytes restores the preview. | Live Artifact checks above. |
| ART-08 | Pass | Same URL returns 401 before login, 200 after passkey login, and 401 after logout. Authorized responses use no-store. | Live Artifact checks above. |
| ART-09 | Pass | A 40 KiB binary retains exact bytes, filename, media type, length, and private caching across a verified server restart. | Live Artifact checks above. |
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
