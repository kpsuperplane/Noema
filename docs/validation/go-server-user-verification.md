# Go server user verification suite

Prepared: 2026-09-05. Status: **Not run**.

This suite checks user outcomes after the Rust-to-Go server replacement.
It is a verification specification, not an execution report.
Previous acceptance results do not prove that the Go build passes.

## Scope and authorities

The baseline is the current product contract on `codex/go-server-migration`.
The migration record identifies Rust baseline commit `a007a4fa984f0d2eaeb2c101337dbbe7881d9379`.
Current contracts take precedence over older acceptance records.

- [Migration scope and release gates](../plans/2026-09-04-go-server-migration.md)
- [Current product context](../context/current.md)
- [Product and complete-home backup](../project.md)
- [Client surfaces and interactions](../frontend/current-contract.md)
- [Installed web application](../frontend/pwa.md)
- [iPhone and iPad client](../../apps/ios/README.md)
- [Authentication and public access](../server-security.md)
- [Tasks](../tasks.md) and [Memory](../memory.md)
- [Capabilities](../harness/capabilities.md), [action requests](../harness/action-governance.md), and [information handling](../harness/security.md)
- [Interactive browsing](../harness/web-browsing.md)
- [Shared API schema](../../graphql/schema.graphql)

The Go server requires a fresh Go home. Rust-home conversion is explicitly unsupported.
Backup and restore use a stopped server and a complete Go-home copy.
This suite does not add backup administration or promise database downgrade support.

These capabilities remain outside the current product scope:

- Shared workspace administration, secondary Noema human accounts, and collaborative Task assignment.
- Task dependencies, subtasks, configurable workflows, and Task text search.
- Direct Memory editing, Memory page history, private Memory scopes, and vector search.
- Offline execution or queued offline writes.
- Binary previews in the Task file browser. Supported Artifact previews have separate checks below.
- General export or deletion controls that have no current product path.
- Apple Foundation Models and its Swift bridge, removed by the current provider cleanup.

External participants can appear in local-owner Tasks. That does not imply shared Noema accounts.
If a listed check has no current path, record a contract discrepancy. Do not create a feature to pass it.

## Execution and evidence

Each row is one case. Its ID remains stable when results are recorded elsewhere.
All cases start as **Not run**, including cases covered by older Rust evidence.

| Priority | Meaning |
| --- | --- |
| P0 | Essential access, execution, authorization, or data preservation. Run first. |
| P1 | Supported daily capability or recovery path. Required for its declared release scope. |
| P2 | Secondary presentation or operator experience. Record failures before release. |

Use these result states: **Not run**, **Pass**, **Fail**, **Blocked**, **Not applicable**, and **Waived**.
A missing account, device, or service makes a required case Blocked.
Use Not applicable only for a capability excluded from that specific supported configuration.
A waiver requires an explicit owner, reason, and follow-up. A waiver is not a pass.

For each run, record:

| Field | Required evidence |
| --- | --- |
| Build | Server commit, release artifact, client version, and run date. |
| Environment | OS, architecture, browser or device, and local or remote server mode. |
| Configuration | Provider, model, account label, browser route, timezone, and relevant policy. |
| Case | ID, input, setup, exact variant, expected result, and actual result. |
| Proof | Durable Task, conversation, action, or artifact IDs; safe screenshots or receipts when useful. |
| Outcome | Result state, defect link, tester, and any explicit waiver. |

Store evidence in `docs/validation/` or the existing issue tracker.
Use one result row per case and configuration. Do not combine different provider outcomes into one pass.
An assistant success statement alone does not prove an external action occurred.
For external effects, inspect the destination record and count the actual effects.

## Setup and coverage matrix

Use an isolated Go home and controlled external accounts for execution.
Use synthetic private records, harmless documents, and reversible external actions.
Record a fixed clock and source cutoff for time-sensitive cases.
Use controlled failure injection for restart, timeout, and uncertain-action cases.
This document authorizes no test execution, account connection, or external message delivery.

Prepare these reusable records:

- Two browser sessions and two native client registrations for independent access checks.
- Two external accounts for one provider, plus two APIs sharing one account grant.
- A Personal Task, a Project with shared files, and a completed Task with citations and artifacts.
- A recurrence with completed history and an editable future template.
- Distinct current, replaced, conflicting, and duplicate source facts.
- Text containing Unicode, emoji, non-Latin names, exact URLs, and ordinary technical identifiers.
- A controlled service with empty results, multiple pages, expired access, rate limits, and lost write responses.
- Documents with known text, page order, table values, dates, currency, formulas, and OCR content.

| Dimension | Required variants |
| --- | --- |
| Server | Native Linux, macOS, and Windows. Include filesystem paths with spaces and Unicode. |
| Release architecture | Linux amd64/arm64, macOS amd64/arm64, Windows amd64. Record native execution separately from cross-builds. |
| Browser | Supported Chromium, Firefox, and Safari configurations. |
| Installed web | Installed standalone app, including iPhone installation and offline recovery. |
| Desktop | Packaged local server and remote HTTPS mode on each desktop OS. |
| Native | Physical iPhone and iPad. Include background, foreground, and locked-device transitions. |
| Model provider | OpenAI, OpenRouter, Codex, and compatible local GGUF. |
| Browser provider | Obscura, Playwright, and Kernel. Include route switching. |
| Integration | HTTP no-auth, direct credentials, OAuth, MCP remote, enabled MCP stdio, and ACP Executor. |
| Time | UTC, a non-UTC zone, a half-hour offset, and daylight-saving transitions. |
| Stored state | Empty Go home, populated Go home, supported Go upgrade, and restored complete Go home. |

Run the P0 access, Chat, Task, approval, and reconnect paths on every shipped client.
Run the provider cases once per supported provider and model capability.
Run format cases once per named format. Run notification cases on real configured devices.
Run representative cross-device journeys after individual cases pass.
Unsupported provider features need an explicit capability result, not a silent fallback or fabricated output.

## 1. Installation, startup, and stored data

| ID | Priority | Action or setup | Expected user result |
| --- | --- | --- | --- |
| HOME-01 | P0 | Start the release with an empty Go home. | Setup opens. No prior data or credentials are required. |
| HOME-03 | P0 | Restart a populated Go home. | Passkeys, conversations, Tasks, Projects, Memory, settings, and artifacts remain available. |
| HOME-04 | P0 | Upgrade a supported earlier Go schema using a copied home. | Supported records and files survive. Documented retirements apply. Fresh creation and upgrade expose the same current capabilities. |
| HOME-05 | P0 | Stop the server and copy its complete Go home. Restore that copy before startup. | Data, credentials, integrations, and files return together. Scheduled work resumes under its missed-run policy. |
| HOME-06 | P1 | Use a custom `NOEMA_HOME` with spaces and Unicode. | Setup, files, downloads, restart, and helper processes use that home correctly. |
| HOME-07 | P0 | Stop or terminate the server during active work in a test home. | Reopening preserves committed records. Interrupted work recovers without duplicate external effects. |
| HOME-08 | P1 | Make the test home unwritable or fill its disk during a save. | The operation reports failure. Existing content remains readable after storage recovery. |
| HOME-09 | P1 | Start and stop the packaged app repeatedly. | The server and owned helper processes stop correctly. The next launch has no stale port or file lock. |
| HOME-10 | P1 | Run concurrent Task and Chat activity on native Windows. | Writes complete without lost records. Cold reopen preserves data and passes the existing SQLite integrity check. |

## 2. Passkeys, recovery, and browser access

| ID | Priority | Action or setup | Expected user result |
| --- | --- | --- | --- |
| AUTH-01 | P0 | Claim a fresh instance with a passkey. | The first valid claim succeeds without a recovery code. Product onboarding follows. |
| AUTH-02 | P0 | Complete two competing initial claims. | Exactly one claim wins. The other browser gains no access. |
| AUTH-03 | P0 | Sign in, close the browser, and restart the server. | A valid stored session remains usable. Signed-out access still requires authentication. |
| AUTH-04 | P0 | Add and remove a passkey through the supported controls. | Recent verification is required. Normal settings cannot remove the final passkey. |
| AUTH-05 | P0 | Recover access with the current recovery code. | Recovery permits passkey setup. The consumed code cannot be reused. |
| AUTH-06 | P1 | Submit an invalid, expired, or replayed passkey ceremony. | Access is refused. A fresh valid ceremony still works. |
| AUTH-07 | P0 | Log out while another tab has Chat open. | Protected reads and writes require login. Session-bound push registrations are removed. |
| AUTH-08 | P0 | Open GraphQL, subscriptions, and private artifacts without authentication. | Private data and commands remain inaccessible, including before the initial claim. |
| AUTH-09 | P0 | Send authenticated browser requests from an unapproved origin or host. | The server rejects them without changing state. |
| AUTH-10 | P1 | Cancel login or recovery, then use browser Back and Forward. | The correct form and entered state return. Recovery does not falsely report success. |

## 3. Onboarding, providers, and model settings

| ID | Priority | Action or setup | Expected user result |
| --- | --- | --- | --- |
| SETUP-02 | P1 | Change proposed assignments before confirming onboarding. | Draft choices do not become active early. Confirmation saves the complete selection together. |
| SETUP-03 | P1 | Cancel authentication, reload setup, or return from a failed callback. | Setup remains recoverable and does not claim a connected account. |
| SETUP-04 | P1 | Connect OpenAI through provider settings. | The account and eligible models become available without repeating completed onboarding. |
| SETUP-05 | P0 | Add, replace, clear, and delete a provider credential. | Later model calls use current access. Removed access is unavailable. Reads never return the credential. |
| SETUP-06 | P1 | Configure two accounts for the same provider. | Account labels remain distinct. Each selected model uses its intended account. |
| SETUP-07 | P1 | Change Chat, Planner, Reviewer, and Executor complexity-tier assignments. | Later turns or runs use the saved assignment. Settings survive reload and restart. |
| SETUP-08 | P1 | Change Memory, action reviewer, progress audit, and fetch summarizer assignments. | The corresponding operation uses its own saved selection. Unavailable choices report a useful error. |
| SETUP-09 | P1 | Change supported reasoning, profile, fast-mode, or default-model preferences. | Eligible choices persist and affect later requests. Ineligible combinations are rejected clearly. |
| SETUP-10 | P1 | Expire provider access during work. | The failure names the affected provider and recovery step. It does not invent a successful result. |

## 4. Chat, transcript, and human interaction

| ID | Priority | Action or setup | Expected user result |
| --- | --- | --- | --- |
| CHAT-01 | P0 | Send a simple message and reopen Chat. | One human message and one complete assistant response remain in the correct order. |
| CHAT-02 | P0 | Send a request that uses several tools. | Progress and results remain attached to the correct calls. The final answer reflects actual results. |
| CHAT-03 | P0 | Disconnect during streaming, then reconnect. | The durable transcript fills missing content without duplicate messages or tool results. |
| CHAT-04 | P0 | Restart during a turn, including a turn with pending action review. | The turn recovers or shows an explicit recoverable state. Saved actions do not silently disappear. |
| CHAT-05 | P1 | Open older transcript pages during a new response. | Older content remains stable. Page boundaries have no gaps or duplicate items. |
| CHAT-06 | P1 | Send Unicode, emoji, multiline text, code, lists, and links. | Text remains intact and readable after streaming, reload, and cross-client reads. |
| CHAT-07 | P1 | Continue a long conversation until compaction occurs. | Current instructions, material facts, source links, and pending decisions remain usable. |
| CHAT-08 | P1 | Request work explicitly in foreground Chat. | The agent honors foreground execution when supported. Automatic Task advice does not override the request. |
| CHAT-09 | P1 | Answer a multiple-choice prompt, including its supported free-text path. | The selected answer resumes the correct interaction once. Refresh preserves the recorded response. |
| CHAT-10 | P0 | Submit an old or already-resolved choice from another client. | The stale response is rejected without starting duplicate work. |
| CHAT-11 | P1 | Request a supported interactive A2UI surface and submit it. | Controls render from the supported catalog. The correct surface receives the action once. |
| CHAT-12 | P0 | Supply invalid A2UI components, active HTML, or a stale surface action. | Untrusted content cannot execute code or authorize a stale action. Chat remains usable. |
| CHAT-13 | P1 | Create several pending human interventions. | Chat shows one current intervention with queue navigation. Each answer affects only its recorded request. |
| CHAT-14 | P1 | Exhaust a configured turn limit or trigger repeated tool failure. | The response explains the stop reason and completed progress. It does not claim unfinished actions succeeded. |
| CHAT-15 | P1 | Ask for a new action resembling an older successful action. | Historical results are not presented as proof that the new action occurred. |

## 5. Provider response behavior

Run these cases for every eligible provider in the coverage matrix.
Hosted search applies only where the selected provider and model support it.

| ID | Priority | Action or setup | Expected user result |
| --- | --- | --- | --- |
| MODEL-01 | P0 | Complete text-only and tool-assisted requests in Chat and Tasks. | Text, tool arguments, call order, and final outcomes remain correct. |
| MODEL-02 | P1 | Use hosted search and open the resulting Sources view. | Claims retain working source URLs and titles in Chat and completed Task results. |
| MODEL-03 | P1 | Continue across several provider requests and a transport reconnect. | Prior tool outputs and response order remain correct. No call executes twice. |
| MODEL-04 | P1 | Change current request settings during a reusable provider session. | Later requests honor current settings without losing earlier context. |
| MODEL-05 | P1 | Expire a continuation containing provider-hosted web state. | Noema reports the unavailable continuation. It does not silently invent replacement web evidence. |
| MODEL-06 | P0 | Return tool input that violates the source schema. | The tool does not execute. The failure remains attributable to the proposed call. |
| MODEL-07 | P1 | Trigger a rate limit, timeout, malformed stream, or abrupt stream end. | Noema reports a bounded failure or supported recovery. Partial output does not become false completion. |
| MODEL-08 | P2 | Inspect provider reasoning, usage, and run diagnostics where exposed. | Reported data belongs to the correct turn or run. Unsupported data is not fabricated. |

## 6. Local models and retired provider selections

| ID | Priority | Action or setup | Expected user result |
| --- | --- | --- | --- |
| LOCAL-01 | P1 | Discover compatible local models during setup and in Settings. | Compatibility and required download are clear. Unsupported models are not presented as ready. |
| LOCAL-02 | P1 | Install a recommended GGUF model and activate it. | Progress reaches a verified installation. Chat uses the selected local model. |
| LOCAL-03 | P1 | Interrupt, resume, and cancel a model download. | Supported resume preserves progress. Cancellation leaves no model falsely marked ready. |
| LOCAL-04 | P0 | Import an invalid or damaged GGUF file. | Validation fails before activation. Existing installed models remain usable. |
| LOCAL-05 | P1 | Trigger low disk space or runtime startup failure. | The failure is actionable. Retry works after the cause is removed. |
| LOCAL-06 | P1 | Remove an installed model and restart. | Storage and availability reflect removal. Assignments do not silently use missing weights. |
| LOCAL-07 | P0 | Upgrade a copied Go home containing retired Apple model accounts and selections. | Retired choices disappear. Historical conversations and Tasks remain readable. A supported model can be selected for later work. |
| LOCAL-08 | P1 | Terminate a local model runtime during generation. | The user sees recovery or failure. The next supported attempt does not inherit a broken process. |

## 7. Task capture, organization, and documents

| ID | Priority | Action or setup | Expected user result |
| --- | --- | --- | --- |
| TASK-01 | P0 | Capture work from Chat. | One durable Task appears with the correct request and a working Chat reference. |
| TASK-02 | P0 | Create through `/tasks/new` using Run and Save to Inbox separately. | Run authorizes execution. Save to Inbox captures work without starting it. |
| TASK-03 | P1 | Edit an Inbox title and `TASK.md`. | Saved content survives reload. Renaming does not move the allocated Task directory. |
| TASK-04 | P0 | Save competing Task drafts from two clients. | The stale save preserves the local draft and current server content. No silent overwrite occurs. |
| TASK-05 | P1 | Use rich Markdown, source mode, cancel, and stale reload. | Source remains intact. Cancel discards only the intended draft. Parsing failure retains source editing. |
| TASK-06 | P1 | Open Task documents outside Inbox. | Editing follows the current state rules. A stale editable view cannot overwrite active work. |
| TASK-07 | P1 | Browse Personal, Project, Scheduled, and terminal Task views. | Records appear in the correct view. Selection and valid actions match current stored state. |
| TASK-08 | P1 | Inspect `TASK.md`, `RESULT.md`, `REVIEW.md`, and support files. | Workspace and Transcript show distinct current content. Completed Tasks open the result by default. |
| TASK-09 | P1 | Complete a Task while its request document is selected. | The result opens once. Later user selection remains respected. |
| TASK-10 | P0 | Retry a capture or command after its response is lost. | The original accepted result returns without duplicate Tasks or state changes. |
| TASK-11 | P1 | Create Tasks with colliding titles and Unicode names. | Each receives a distinct usable directory. Existing files remain intact. |

## 8. Task execution, review, and recovery

| ID | Priority | Action or setup | Expected user result |
| --- | --- | --- | --- |
| RUN-01 | P0 | Queue a Task requiring planning, execution, and review. | The Task progresses through current stages and completes only after Reviewer approval. |
| RUN-02 | P1 | Delegate a complete request eligible for direct execution. | Execution can start without unnecessary planning. The request and authorization remain intact. |
| RUN-03 | P1 | Submit a result missing one explicit requirement. | Review requests correction. The next Executor uses current request, result, and review files. |
| RUN-04 | P0 | Trigger clarification, approval, and recovery gates separately. | Each gate explains the required response. Answer or retry resumes the recorded role once. |
| RUN-05 | P0 | Cancel queued, running, and waiting Tasks. | No new work starts for the cancelled generation. Any possible external effect remains explicitly uncertain. |
| RUN-06 | P0 | Reopen a completed or cancelled Task while old work returns late. | Current files remain available. Old results cannot change the reopened Task. |
| RUN-07 | P1 | Continue a long Executor run across a saved checkpoint. | The next run uses current files and exact remaining steps without repeating completed actions. |
| RUN-08 | P1 | Restart during planning, execution, and review separately. | Each role recovers from durable state. The Task does not remain falsely active forever. |
| RUN-09 | P1 | Reach continuation, tool, active-time, retry, review, and audit limits separately. | Each enforced limit produces its correct stop or recovery behavior with preserved progress. |
| RUN-10 | P1 | Request an impossible outcome and a human-resolvable block separately. | An honest limitation report differs from a request for human help. Neither falsely claims the requested outcome. |
| RUN-11 | P1 | Execute several Tasks concurrently. | Results, artifacts, policies, and notices remain attached to their correct Tasks. |
| RUN-12 | P1 | Observe capture, waiting, recovery, and completion in primary Chat. | Durable Task references update without repeated cards or full run-transcript flooding. |
| RUN-13 | P1 | Complete an unchanged recurring result that forbids repeated updates. | The Task completes while its redundant completion notice remains suppressed. |
| RUN-14 | P1 | Read another authorized Task from an Executor or Reviewer. | Exact current documents and identity are preserved. Unrelated access remains blocked by its scope. |
| RUN-15 | P0 | Attempt file access beyond a Task or Project boundary, including symlinks. | Access fails without exposing or modifying outside files. Permitted shared Project reads still work. |

## 9. Scheduling and recurrence

| ID | Priority | Action or setup | Expected user result |
| --- | --- | --- | --- |
| TIME-01 | P0 | Schedule one Task at a known local time. | It stays unstarted before its due time and starts once when due. |
| TIME-02 | P1 | Reschedule, unschedule, and run a pending Task now separately. | Reschedule replaces future timing. Unschedule returns Inbox. Run now executes that same Task early. |
| TIME-03 | P1 | Capture relative-time requests in different timezones. | The original request time resolves relative terms. Later runs receive the correct current clock. |
| TIME-04 | P0 | Restart across a missed due time with `run_once` and `skip`. | Each policy produces its documented outcome without duplicate execution. |
| TIME-05 | P1 | Create a repeating Task and inspect its history. | The recurrence stays visible in Scheduled after its occurrences become terminal. |
| TIME-06 | P0 | Edit a recurrence template, then create its next occurrence. | Only future occurrences copy the new exact template. Existing Task files remain unchanged. |
| TIME-07 | P0 | Save competing recurrence drafts. | The stale draft remains recoverable. Current template and schedule are not overwritten. |
| TIME-08 | P1 | Pause, resume, skip next, and end a recurrence separately. | Future runs follow each command. An ended template remains readable and immutable. |
| TIME-09 | P1 | Run an established recurrence now. | One manual occurrence starts. The normal cadence remains distinct. |
| TIME-10 | P1 | Keep an occurrence active across the next slot under each overlap policy. | `skip`, `queue_one`, and `allow` produce their documented occurrence counts. |
| TIME-11 | P0 | Cross spring-forward and fall-back transitions in the authoring zone. | Missing local minutes do not run. A repeated local minute runs at most once. |
| TIME-12 | P1 | Edit a schedule while its due transition is being processed. | One current schedule wins. No obsolete occurrence executes afterward. |

## 10. Projects and Agent configuration

| ID | Priority | Action or setup | Expected user result |
| --- | --- | --- | --- |
| PROJECT-01 | P1 | Create a Project, edit its name, and configure its working folder. | The Project remains selectable. Its Tasks and shared files use the correct folder. |
| PROJECT-02 | P0 | Edit `PROJECT.md`, including competing saves. | Successful content persists. Stale saves preserve both the draft and current document. |
| PROJECT-03 | P1 | Update Project context between Task roles. | Each later Planner, Executor, and Reviewer reads the current Project context. |
| PROJECT-04 | P1 | Archive and reopen a Project. | Archived context stays readable and read-only. Reopening restores the supported edit actions. |
| PROJECT-05 | P1 | Ask Chat to find or create a Project and inspect its Tasks. | Tools return the intended Project and current Task state without creating unnecessary duplicates. |
| AGENT-01 | P1 | Open Agent settings and start work with configured Agents. | Names and identities stay stable across Chat, Tasks, reload, and restart. |
| AGENT-02 | P1 | Disable a Task model-pool entry and start an eligible Task. | Selection respects the enabled pool and complexity tier. Missing eligible models produce a clear failure. |
| AGENT-03 | P1 | Change execution policy during an existing run. | The active run keeps its recorded limits. Later runs use the applicable saved policy. |

## 11. Memory capture, reading, and evidence

| ID | Priority | Action or setup | Expected user result |
| --- | --- | --- | --- |
| MEM-01 | P1 | Open Memory before any durable facts exist. | The empty state remains usable and does not invent a personal profile. |
| MEM-02 | P0 | State a stable personal fact and request a Memory update. | The article records the fact with exact eligible evidence. Status reaches a clear completion state. |
| MEM-03 | P1 | Trigger a Memory update through conversation compaction. | Eligible new evidence is processed once. The visible tree and update status refresh. |
| MEM-04 | P0 | Correct a stored fact and update again. | Later retrieval uses the current fact. Evidence preserves the distinction between old and new claims. |
| MEM-05 | P1 | Supply duplicate evidence, then an unsupported assistant claim. | Duplicate sources do not create duplicate facts. Assistant text alone does not become claim evidence. |
| MEM-06 | P1 | Open root, child, and deeply nested Memory articles. | Titles, icons, breadcrumbs, related articles, and exact routes match the stored hierarchy. |
| MEM-07 | P0 | Open a citation by pointer and keyboard. | The evidence type, date, excerpt, and exact source identity support the nearby claim. |
| MEM-08 | P1 | Search and read deeper Memory from Chat. | Relevant authorized facts remain intact. A new ordinary turn receives the bounded root context. |
| MEM-09 | P0 | Restart during Memory publication or return invalid update output. | Readers see complete valid pages. Failed updates do not skip unprocessed evidence. |
| MEM-10 | P1 | Rebuild the disposable search index from the stored Markdown tree. | Search returns the same source facts without changing article content or evidence. |
| MEM-11 | P1 | Update a tree too large to load every article body. | Relevant articles remain usable. Unread article bodies are not silently rewritten. |
| MEM-12 | P0 | Mix stable preferences with temporary tool errors and secret-bearing setup data. | Stable eligible facts remain available. Operational noise and secrets do not become personal Memory. |

## 12. Action requests, approvals, and information handling

Use both Chat and Tasks for ACTION-01 through ACTION-06.

| ID | Priority | Action or setup | Expected user result |
| --- | --- | --- | --- |
| ACTION-01 | P0 | Request an action under each supported connection review policy. | Immediate execution, model review, and human review follow the saved policy and current authorization. |
| ACTION-02 | P0 | Approve one pending action from two clients or repeat its submission. | Only the saved exact action executes, at most once. The second decision cannot reuse approval. |
| ACTION-03 | P0 | Decline a proposed external action. | The action does not execute. Chat or Task continuation reports the decline accurately. |
| ACTION-04 | P0 | Change the tool, account access, policy, or Task generation before approval executes. | The stale action cannot run under obsolete authority. The user sees its current outcome. |
| ACTION-05 | P0 | Restart while waiting for approval or authentication. | The pending request returns with its exact input and decision state. Resume does not duplicate the effect. |
| ACTION-06 | P0 | Lose the response after a controlled external write commits. | The outcome remains uncertain until checked. Noema does not automatically repeat the write. |
| ACTION-07 | P0 | Make the action reviewer unavailable or return invalid review output. | Noema requests a human decision. It does not silently permit execution. |
| ACTION-08 | P1 | Ask to use a disabled tool, then approve Enable tool. | The exact tool policy becomes enabled. Unrelated tools remain unchanged. |
| ACTION-09 | P0 | Attempt to approve through ambiguous free text instead of the recorded decision control. | Free text cannot consume a saved approval decision. Structured action identity remains authoritative. |
| INFO-01 | P0 | Use an authorized private document in a private answer. | Names, values, and relevant details remain intact within the authorized purpose and scope. |
| INFO-02 | P0 | Ask to disclose only selected private fields to a controlled external recipient. | The reviewed disclosure matches the authorized fields and recipient. The original source remains intact. |
| INFO-03 | P0 | Place instructions inside a webpage, tool result, or document. | Those instructions cannot grant access, change policy, or authorize unrelated actions. |
| INFO-04 | P0 | Configure credentials through their protected setup paths. Inspect ordinary output using safe checks. | Credential values do not enter model context, transcripts, Memory, logs, artifacts, or exports. |
| INFO-05 | P0 | Use ordinary IDs, paths, hosts, ports, model names, and fields containing `secret` or `authorization`. | Non-secret values remain intact. Field spelling alone does not conceal information. |
| INFO-06 | P0 | Attempt an unauthorized Task, artifact, or external-account read. | The boundary denies access without deleting or modifying the original private data. |

## 13. HTTP APIs, credentials, and OAuth accounts

| ID | Priority | Action or setup | Expected user result |
| --- | --- | --- | --- |
| API-01 | P1 | Add a supported API through its structured Chat setup path. | The reviewed tools become available only after definition and connection setup complete. |
| API-02 | P1 | Connect a no-auth API and a direct-credential API separately. | Both expose usable tools under their own policies. Credential fields remain write-only. |
| API-03 | P1 | Connect compatible APIs with one OAuth sign-in. | The correct account appears once. Each API keeps its own enabled tools and review policy. |
| API-04 | P0 | Connect a second account for the same provider. | Reads and writes use the selected account. Labels and account identity prevent silent mixing. |
| API-05 | P1 | Cancel, deny, expire, or fail an OAuth attempt. | Setup remains incomplete or partial with a clear next action. No false connected state appears. |
| API-06 | P1 | Complete OAuth while the originating client is backgrounded. | Foreground recovery finds the exact attempt result and resumes the correct setup. |
| API-07 | P1 | Add access for more operations to an existing account. | Confirmation explains new operation benefits. Only granted operations become callable. |
| API-08 | P0 | Expire a shared grant while two dependent APIs request data. | Refresh preserves the correct account and resumes authorized calls without losing access through competing refreshes. |
| API-09 | P1 | Return empty results, two pages, and a failed next-page request. | Empty lists stay empty. Continuation preserves query bounds and can retry the failed page safely. |
| API-10 | P0 | Reuse a continuation with another account, operation, or changed arguments. | The invalid continuation is rejected. Results cannot cross account or query boundaries. |
| API-11 | P1 | Return malformed, oversized, or wrong-type data. | A useful failure replaces invalid data. Raw responses do not bypass the reviewed output boundary. |
| API-12 | P0 | Return a rate limit or disconnect after a possible write. | Safe read retries follow policy. A possibly completed write remains uncertain and is not repeated automatically. |
| API-13 | P1 | Rename, disable, re-enable, and delete one API connection. | The connection reflects each change. Deleting it preserves the account grant and unrelated APIs. |
| API-14 | P0 | Disconnect a shared account, then try each dependent API. | All dependent access stops. Application deletion requires its grants to be disconnected first. |
| API-15 | P1 | Restart after a supported managed definition or application change. | Compatible connections retain identity. Changed access requirements remain visible and cannot reuse stale approvals. |

## 14. MCP and ACP

| ID | Priority | Action or setup | Expected user result |
| --- | --- | --- | --- |
| MCP-01 | P1 | Add a supported remote MCP service and invoke a read tool. | Setup discovers current tools. Results appear under the correct service and call. |
| MCP-02 | P1 | Complete, cancel, skip, and renew MCP authentication separately. | Each flow has an accurate next action. Successful authentication resumes only the intended pending call. |
| MCP-03 | P0 | Change tool metadata after policy was approved. | Stale metadata cannot retain callable authority without the required current classification. |
| MCP-04 | P0 | Override, reset, disable, and enable an MCP tool policy. | Current policy governs later calls. Review and direct execution both validate source input. |
| MCP-05 | P1 | Disconnect the service or return an invalid tool result during work. | Noema reports a service-specific failure. Other configured services remain usable. |
| MCP-06 | P0 | Configure stdio MCP with process access disabled, then explicitly enabled in a test setup. | Disabled mode starts no process. Enabled mode runs the configured service with the protected environment boundary. |
| MCP-07 | P1 | Delete an MCP service and restart. | Its tools and pending access cannot remain silently active. |
| ACP-01 | P1 | Create, test, authenticate, edit, and delete an ACP Executor. | Settings report the real process and authentication status. Removed Executors cannot receive later Tasks. |
| ACP-02 | P0 | Execute a Task through ACP with files, tools, artifacts, and review. | The result follows the same Task boundaries and completion rules as built-in execution. |
| ACP-03 | P0 | Cancel an ACP Task or terminate its process mid-run. | Work stops or enters explicit recovery. Late output cannot complete a cancelled generation. |
| ACP-04 | P1 | Trigger an ACP protocol or process-start failure. | The Task preserves progress and a useful diagnostic. Retry starts a valid current run. |

## 15. Search, fetch, and interactive browsing

| ID | Priority | Action or setup | Expected user result |
| --- | --- | --- | --- |
| WEB-01 | P1 | Search current public sources through the configured search provider. | Results contain usable titles and URLs. The answer distinguishes source evidence from inference. |
| WEB-02 | P1 | Fetch a known page and summarize it with the configured model. | Relevant content and source identity survive. Truncation or unavailable content is stated accurately. |
| WEB-03 | P1 | Change search, fetch, and ordered browser-provider settings. | Later calls use the saved settings. Search and direct fetch remain distinct from interactive browsing. |
| WEB-04 | P1 | Open, inspect, interact, wait, navigate history, and close with each browser provider. | Current page content and action results remain coherent. Closing releases the owned session. |
| WEB-05 | P1 | Select Obscura with no installed worker, then reuse it without network access. | Initial verified installation succeeds from public assets. The installed worker remains reusable. |
| WEB-06 | P1 | Make the current browser provider fail, including its first open. | The agent can request a route switch. A failed switch preserves the current route for retry. |
| WEB-07 | P0 | Switch providers after entering page state. | The new browser starts fresh. Credentials, cookies, and form state do not transfer silently. |
| WEB-08 | P0 | Submit an interaction using an old page snapshot. | No interaction occurs against a changed target. Noema obtains current page state before another attempt. |
| WEB-09 | P0 | Review a form submission with visible, hidden, password, and file controls. | Review shows destination, method, and visible submitted values. Protected control values remain excluded. |
| WEB-10 | P0 | Decline a browser effect, then retry it with changed element references only. | The equivalent declined effect remains blocked within that Task generation. |
| WEB-11 | P0 | Return a main-document error after a controlled form commit. | The effect becomes uncertain. Read-only status checks reconcile it before any further write. |
| WEB-12 | P1 | Continue a Task after recording an active browser session. | The Executor inspects that session before reopening. Existing POST results and filled forms remain available when the session survives. |
| WEB-13 | P0 | Upload one exact Task artifact through the supported browser route. | Review binds its version, filename, and byte count. The destination receives those exact bytes once. |
| WEB-14 | P1 | Attempt upload while Obscura is active or Kernel is unavailable. | The supported switch or retry outcome is explicit. Noema does not claim an upload occurred. |
| WEB-15 | P0 | Navigate or redirect toward private network addresses or local files. | Network policy blocks prohibited destinations at every navigation. Normal public pages remain accessible. |
| WEB-16 | P1 | Hang the browser process past its command deadline. | The call terminates with a useful diagnostic. Later work can start a healthy session. |

## 16. Files, document parsing, OCR, and calculation

Prepare one known document for each format named below.
Record extracted values against the known source, including any stated parser limits.

| ID | Priority | Action or setup | Expected user result |
| --- | --- | --- | --- |
| FILE-01 | P1 | Download a permitted public file into a Task. | The saved file has the expected name, bytes, and Task location. Review applies where required. |
| FILE-02 | P0 | Download through an unsafe redirect or escaping output path. | The download fails without writing outside the permitted location. |
| FILE-03 | P1 | Parse DOC, DOCX, ODT, and RTF separately. | Known text, paragraph order, and supported tables remain usable without invented content. |
| FILE-04 | P1 | Parse PPT, PPTX, and ODP separately. | Known slide content preserves reading order and source identity within supported extraction limits. |
| FILE-05 | P1 | Parse PDF and EPUB separately. | Known text and structural order remain useful. Missing or unsupported content is reported accurately. |
| FILE-06 | P1 | Parse XLS, XLSX, and ODS separately. | Sheets and cells preserve known values. Dates, currency, and cached formula results retain supported display meaning. |
| FILE-07 | P1 | Read a scanned document through the configured OCR path. | Known image text is recovered with source identity. Poor recognition is not presented as certain evidence. |
| FILE-08 | P1 | Parse Unicode text and documents with non-Latin filenames. | Names and text remain intact. Encoding failures produce clear errors. |
| FILE-09 | P0 | Parse damaged, oversized, or unsafe archive-based documents. | Parsing stops within its limits. Existing files remain intact and extracted paths cannot escape. |
| FILE-10 | P1 | Remove a required conversion or OCR executable. | The operation identifies unavailable processing. Unrelated formats and Chat remain usable. |
| CALC-01 | P1 | Use Lua for a known budget, percentage, date-independent calculation, and structured transformation. | Results match independently calculated expected values. JSON arrays, objects, nulls, and Unicode retain their meaning. |
| CALC-02 | P0 | Run unbounded Lua or attempt file, process, environment, or network access. | The sandbox rejects prohibited access or terminates within limits. No partial success is fabricated. |

## 17. Artifacts, previews, and downloads

| ID | Priority | Action or setup | Expected user result |
| --- | --- | --- | --- |
| ART-01 | P0 | Publish a local Task artifact and reopen it from its result. | The artifact retains its source Task, metadata, exact bytes, and working authorized download. |
| ART-02 | P1 | Create a conversation artifact referencing an external URL. | The exact URL and source remain visible. Noema does not claim to own a local copy. |
| ART-03 | P0 | Publish a newer artifact version while an older version is selected for upload. | Versions remain distinguishable. A saved action cannot silently substitute different bytes. |
| ART-04 | P1 | Preview Markdown, text, raster images, PDFs, and supported spreadsheets separately. | Supported content renders through its authorized preview path. Download remains usable. |
| ART-05 | P0 | Preview HTML containing scripts, navigation, handlers, and external resources. | The sandbox prevents active behavior and external loading. Safe visible content remains useful. |
| ART-06 | P1 | Open an SVG or unsupported preview format. | SVG remains download-only. Unsupported previews provide an honest fallback. |
| ART-07 | P0 | Alter, remove, or redirect a stored artifact file through a symlink. | Integrity or path checks reject unsafe delivery. The UI does not present replacement content as the original artifact. |
| ART-08 | P0 | Copy a private artifact URL into a signed-out browser. | Access requires authorization. Private downloads do not become publicly cached content. |
| ART-09 | P1 | Download a large permitted file, then restart and download again. | Filename, content type, length, and bytes remain correct. Authorized delivery survives restart. |

## 18. Notifications, presence, and Live Activities

Production APNs checks require signed device builds, matching entitlements, and a configured APNs provider.
Installed web push requires the configured HTTPS public origin.

| ID | Priority | Action or setup | Expected user result |
| --- | --- | --- | --- |
| NOTE-01 | P1 | Enable installed web notifications through the explicit control. | The browser requests permission only after that action. Registration reports its actual status. |
| NOTE-02 | P1 | Produce a final answer and a new human intervention with the app backgrounded or closed. | Each supported event delivers the correct notification and navigation target. |
| NOTE-03 | P1 | Keep primary Chat focused on one registered device. | That device suppresses its alert. Another eligible device still receives the notification. |
| NOTE-04 | P1 | Open Tasks or Settings instead of Chat. | The client does not falsely suppress new Chat notifications. |
| NOTE-05 | P0 | Restart with old transcript items and pending deliveries. | Historical items do not generate fresh alerts. Pending delivery follows its durable retry state. |
| NOTE-06 | P1 | Return expired-subscription, rate-limit, network, and server failures. | Expired registration is removed. Retryable failures follow the supported schedule without an endless queue. |
| NOTE-07 | P0 | Disable notifications, log out, or revoke the owning native client. | The affected registration stops delivery. Other authorized clients remain independent. |
| NOTE-08 | P0 | Configure, replace, and remove APNs provider settings. | Status reflects current configuration. Reads and diagnostics never expose the private key. |
| NOTE-09 | P1 | Register iPhone and iPad push notifications and tap delivered alerts. | Each device receives its authorized alerts and opens the intended Noema surface. |
| NOTE-10 | P1 | Start several Tasks with Live Activities enabled. | One aggregate activity reflects active Tasks and their current status. Terminal work leaves the active set. |
| NOTE-11 | P1 | Dismiss, disable, restart, and re-register Live Activities separately. | Activity state reconciles without duplicate or permanently stale activities. |
| NOTE-12 | P0 | Inspect delivered notification content for reviewed external actions. | Payloads contain the permitted preview, not secret credentials, raw arguments, results, or hidden form values. |

## 19. Client recovery, offline behavior, and navigation

| ID | Priority | Action or setup | Expected user result |
| --- | --- | --- | --- |
| CLIENT-01 | P0 | Open the same Chat and Task on web, desktop, iPhone, and iPad. | Text, state, valid actions, citations, and result identity agree across clients. |
| CLIENT-02 | P1 | Open direct Task, Project, Memory, Settings, and artifact links. | Supported links restore the intended surface. Unknown web paths use the documented Chat fallback. |
| CLIENT-03 | P1 | Change a setting or resolve a decision on another client. | Active views refresh from current state without a full manual reload. |
| CLIENT-04 | P1 | Interrupt the live connection during an active Task. | Reconnect restores durable Task and transcript state before later live completion is accepted. |
| PWA-01 | P1 | Install the web app after authentication and visit core surfaces. | The complete release installs. Current views and supported drafts can be saved for offline use. |
| PWA-02 | P0 | Relaunch the installed app offline after saving data. | Saved reads and editable drafts remain available. Mutations and approvals cannot run or queue offline. |
| PWA-03 | P0 | Reconnect after another client changes the same records. | Reconciliation replaces stale reads before writes unlock. Old responses cannot overwrite newer state. |
| PWA-04 | P0 | Make the reachable server require login during recovery. | Cached private content hides behind authentication. Successful login permits reconciliation. |
| PWA-05 | P1 | Deploy a complete new release while the installed app is offline. | Reconnection updates automatically at a safe point. Route, search parameters, and drafts survive. |
| PWA-06 | P1 | Fail an update download or exhaust browser storage. | The previous complete release and saved snapshot remain usable. Failures do not corrupt durable server data. |
| PWA-07 | P0 | Erase installed private local data using its supported control. | Saved reads and drafts disappear from that installation. Server data remains governed by its existing source. |
| NATIVE-01 | P0 | Connect iPhone, iPad, and remote desktop through a connection link. | The system browser completes passkey-approved OAuth. The link contains only the validated server origin. |
| NATIVE-02 | P0 | Cancel OAuth or use an invalid return, expired code, or wrong PKCE verifier. | Native access is not granted. The app offers a clean retry. |
| NATIVE-03 | P0 | Interrupt credential refresh after the server accepts it. | Request-bound clients recover with their saved request identity. Supported legacy desktop retries follow their documented window. Credentials remain protected. |
| NATIVE-04 | P1 | Lock, background, suspend, and reopen iOS during activity. | Refresh respects active and unlocked state. Foreground recovery restores current reads and subscriptions. |
| NATIVE-05 | P1 | Remove connectivity, then separately stop a reachable server. | The client distinguishes network loss from server failure. Recovery clears the banner only after authenticated readiness. |
| NATIVE-06 | P0 | Disconnect or revoke one client, including while it is offline. | Its access stops when checked. Local protected cache and credentials clear through the documented disconnect path. |
| DESKTOP-01 | P0 | Launch each packaged desktop build in local mode. | The bundled Go server starts without a development toolchain. Closing the app stops owned processes. |
| DESKTOP-02 | P1 | Connect remote HTTPS, restart desktop, then return to local mode. | Remote access persists securely. Return to local mode revokes the remote family and removes local credentials. |
| DESKTOP-03 | P1 | Make remote revocation or startup unavailable. | Retry, local mode, and confirmed forget follow the documented recovery flow. Failure does not falsely report server revocation. |

## 20. User controls, accessibility, and diagnostics

These checks verify existing interactions. They do not request a UI redesign.

| ID | Priority | Action or setup | Expected user result |
| --- | --- | --- | --- |
| UX-01 | P1 | Complete login, capture, editing, and approval with keyboard or native accessibility controls. | Controls have names, focus remains visible, and essential actions remain reachable. |
| UX-02 | P1 | Use narrow screens, large text, and the on-screen keyboard. | The current decision, document, and primary action remain readable and reachable. |
| UX-03 | P1 | Open Sources for text containing Unicode and repeated source URLs. | Markers attach to the correct claims. Source numbering, titles, and exact URLs agree across clients. |
| UX-04 | P2 | Fail a favicon or one local content renderer. | Missing decoration or one failed item does not hide the surrounding conversation or Task controls. |
| UX-05 | P1 | Trigger query, save, authentication, and provider failures separately. | Each failure shows its correct recovery action without losing the current user draft. |
| DIAG-01 | P2 | Inspect a completed, failed, waiting, and cancelled Task run. | Tool outcomes, timing, usage, and errors explain that run. Terminal work does not remain falsely active. |
| DIAG-02 | P0 | Inspect technical details after an authentication or browser error. | Safe provider diagnostics and ordinary identifiers remain useful. Credential values remain absent. |
| OPS-01 | P1 | Open public callback and consent pages without an authenticated application bundle. | Required setup pages render and complete their bounded flow. Private application data remains inaccessible. |
| OPS-02 | P0 | Start with GraphiQL, local GraphQL socket, and stdio MCP disabled. | Those optional access paths remain closed. Public access cannot select local development authority. |
| OPS-03 | P1 | Explicitly enable each supported development access path in an isolated setup. | The selected path works within its documented boundary. Other flags remain independent. |

## 21. Complete user journeys

Each journey combines capabilities that can pass separately but fail together.
Use current Go evidence and bounded, populated sources.
Do not accept an empty result as proof that source reading works.

| ID | Priority | User request and setup | Expected complete outcome |
| --- | --- | --- | --- |
| JOURNEY-01 | P0 | Set up a fresh instance, connect a model, and ask a question. | Passkey setup, onboarding, model selection, streaming, and restart lead to one durable usable conversation. |
| JOURNEY-02 | P1 | Prepare a daily brief from messages, calendar, Tasks, and Memory. | The brief uses every populated source, preserves time bounds, identifies missing evidence, and makes no external writes. |
| JOURNEY-03 | P1 | Plan a week with recurring events, all-day events, Tasks, and travel time. | The plan respects local times, recurrence identity, overlaps, capacity, and known travel constraints. |
| JOURNEY-04 | P0 | Delegate research into a Project with an explicit deliverable. | Planning, current Project context, search, files, result citations, review, and the final artifact form one complete Task. |
| JOURNEY-05 | P0 | Prepare a private document packet with calculations and one approved upload. | Source facts and calculations remain exact. Review binds the selected artifact and destination. One upload produces a verified receipt. |
| JOURNEY-06 | P0 | Submit a controlled booking or form whose final response is lost. | Read-only reconciliation establishes its actual state. No second booking or submission occurs. |
| JOURNEY-07 | P1 | Run a recurring monitor across unchanged and changed source records. | Unchanged results remain quiet when requested. A material change produces one useful update with evidence. |
| JOURNEY-08 | P1 | Start a Task on desktop, answer its question on iPhone, and inspect its result on web. | All clients share one decision, Task state, result, and artifact set. |
| JOURNEY-09 | P0 | Resume work after an integration requires renewed authentication. | The correct account regains access. The original pending work resumes without repeating completed external actions. |
| JOURNEY-10 | P1 | Correct a personal preference, update Memory, and request a later plan. | The new plan uses the corrected fact with eligible evidence. The old fact does not silently control the answer. |
| JOURNEY-11 | P0 | Restore a complete stopped-server Go backup, then reopen a pending decision and scheduled Task. | Access, files, policies, and pending state remain coherent. Decisions and due work execute at most once. |
| JOURNEY-12 | P1 | Prepare an audience-specific Project update from conflicting source records. | The update distinguishes confirmed facts, conflicts, and missing evidence. Private internal details stay within the authorized audience. |

Reuse detailed inputs from these earlier packages. Their old pass results do not transfer to this suite.

- [Milestone 1: daily assistance, research, and Memory](personal-assistant-milestone-1-acceptance.md)
- [Milestone 2: recurring monitoring and follow-through](personal-assistant-milestone-2-acceptance.md)
- [Milestone 3: private packets, calculation, and artifacts](personal-assistant-milestone-3-acceptance.md)
- [Milestone 4: browser actions and uncertain outcomes](personal-assistant-milestone-4-browser-acceptance.md)
- [Milestone 5: multi-person external cases](personal-assistant-milestone-5-audit.md)
- [Later lifecycle evidence](personal-assistant-remaining-acceptance-2026-08-31.md)
- [Broader personal-assistant inventory](../difficult-digital-personal-assistant-tasks.md)

## Exit criteria

1. Every P0 case passes for each applicable shipped configuration.
2. Every P1 case passes or has an explicit release waiver with an owner and follow-up.
3. Every remaining case has a result, defect, exclusion reason, or explicit waiver.
4. No unresolved failure permits unauthorized access, secret exposure, lost data, or repeated external effects.
5. Every shipped provider, browser route, document format, and client has its required evidence.
6. Native platform execution is recorded separately from compilation and unit-test results.
7. Credential or hardware gaps remain Blocked until tested or explicitly waived.
8. The migration record's separate packaging, public asset, license, race-test, and size gates are satisfied.

Final release summary template:

```text
Server commit and release:
Client versions:
Execution dates and owners:
Configurations covered:
Pass / Fail / Blocked / Not run / Not applicable / Waived counts:
P0 failures:
Data, authorization, or repeated-action defects:
Remaining defects and explicit waivers:
Evidence links:
Release decision and decision owner:
```
