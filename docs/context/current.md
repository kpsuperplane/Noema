# Current Noema Context

This brief contains active direction, current constraints, and open loops. Durable contracts belong in subsystem documents. Git owns completed history.

## Active direction

Rust prompt text is restored from `4d29f6ba`. See [prompt parity](../validation/rust-prompt-parity.md) for exact reference checks, provider delivery checks, and context-history limits.

The [local CLI](../cli.md) uses the private socket for GraphQL, Tasks, and streaming Chat.
Linux and macOS enable the socket by default. Public authentication remains separate.

Personal-assistant acceptance now follows [the live case plan](../plans/2026-09-06-personal-assistant-live-cases.md).
Use direct GraphQL interaction through a thin CLI and examine each outcome before advancing.
First verify agent-generated Gmail API and Notion MCP connections, then retain all 100 original assistant cases.
The earlier 100-case replay completion report is invalid: successful process exits concealed blocked outcomes.
No new acceptance pass is established by that report.

The Go provider fixture now exposes Gmail, Notion MCP, and Calendar contracts under
`noema-provider-fixtures-v3.service` at fixture revision `2026-09-08-gmail-v1-notion-mcp-v8`. It uses synthetic OAuth and two accounts.
Gmail and Notion proposal setup, OAuth callbacks, account boundaries, and live
read paths work. Calendar proposal acceptance, nested-body writes, and reads
also work. Full setup recovery and delegated reuse remain open.
PA-001 through PA-008 pass. Detailed cases and evidence remain in the active acceptance ledger below.
`ConversationAuthorizationContext` now follows continuation trigger items to
the original human turn, with a focused store regression test.
The memory updater now omits nested browser screenshots and internal Task-list results. A live update completed at sequence 1881 and saved the 16:00 weekday
work-stop preference. Focused and broad Go checks pass at `c1f9ecbe`.
The active ledger is [live acceptance evidence](../validation/evidence/personal-assistant-live/README.md).

The Go server source replacement is complete. The server links no Rust or CGo.
The budget plan is `docs/plans/2026-09-06-audit-completion-budget.md`. Browser safety blockers are fixed and broad Go checks pass at a677fa4e. Controlled automatic coverage is complete. Keep the results file limited to necessary context and the case table. Maintain defects in `docs/validation/go-server-user-verification-fix-backlog.md`. Concurrent store approval and execution claims each admit one winner; completed approval reuse fails. Task workers now claim two runs concurrently and preserve separate result files. Browser-client approval remains pending. Controlled ACTION-07 and ACTION-09 checks pass: reviewer failures preserve exact pending approval, and ambiguous Chat text cannot consume that approval. Structured decline resumes the matching failed tool result. Concurrent-client decisions and full process recovery remain pending. JOURNEY-05 passes through the supported Task Artifact route; Chat safely refuses the unsupported Artifact route. AUDIT-22 records this route boundary. NOTE-09 reuses unchanged passing ownership, retry, revocation, and routing units. Physical iPhone and iPad delivery and alert taps remain human tests. NATIVE-05 also requires human testing on Apple devices; this Linux host lacks Swift and Xcode. DESKTOP-01 passes for the Linux Debian package: model setup renders without development tools in PATH, and normal close stops the owned Go server. The extracted AppImage also passes these checks. Native FUSE launch is denied by this container. FUSE-capable Linux, macOS, and Windows launch checks remain for humans. AUDIT-20 is fixed; failed Task queries show a Retry control. AUDIT-21 is fixed; forward and reverse Tab navigation reaches the Task capture controls at both widths; reverse Tab reaches the title. Reverse traversal completes keyboard capture and title saving at both widths; saved titles survive reload. Keyboard login passes with the existing virtual passkey at both widths. Both isolated sessions log out. Controlled keyboard approval and decline preserve the action ID and revision at both widths. Reduced-viewport Task editing preserves drafts and reachable Save controls, including controlled text enlargement. Approval and decline also remain usable at 320×480 after a controlled save failure. Native zoom and keyboard checks remain pending. Controlled failed saves preserve exact drafts at both browser widths. Forced socket closure reconnects and refreshes the waiting Task and transcript at both widths. A live calculation Task also recovers missed Planner completion and later displays the completed Reviewer and exact result 323 without reload. JOURNEY-10 passes: a later plan reads current Memory and uses the corrected French preference with the saved stargazing interest. JOURNEY-12 passes with two conflicting native Task records: the customer draft preserves uncertainty and excludes internal details. NOTE-04 passes controlled web presence checks: Tasks and Settings keep Chat presence inactive at both widths. JOURNEY-02 and JOURNEY-03 pass with populated loopback MCP messages and calendar, native Tasks, and Memory. Exact bounds, recurrence identity, leave, capacity, travel conflicts, and missing evidence survive synthesis. The temporary integration and service are removed. JOURNEY-06 controlled receipt reads preserve one write after a lost booking response; browser and model recovery remain pending. JOURNEY-04 passes live Project research with updated context, search, exact citations, one review correction, and verified Artifact bytes. Eight existing desktop units pass for protected profiles, revocation policy, connection parsing, callbacks, and transport. Packaged and physical-client checks remain open. Defer defects unless they block testing or risk test data. AUDIT-01 records incorrect retry-role selection after the review limit. AUDIT-02 records rejected cross-Task reads. AUDIT-03 records saved human answers missing from resumed provider context. Separate continuation and active-time limit checks pass, including expiry during a provider call. Controlled Task file boundary checks pass. AUDIT-04 records rejected shared Project reads. All three roles receive exact updated Project context. Controlled Chat notice processing preserves Task identity and avoids duplicate output. Disabled completion notices stay suppressed; browser notice transitions and unchanged recurrences remain pending. Live impossible and missing-address Tasks distinguish limitation reports from clarification. AUDIT-05 records missing Task gate controls at desktop and phone widths. See `docs/validation/go-server-user-verification-results.md`. Checks now use noema.kevinpei.com. The live calculation Task completed planning, execution, and review. Released schedules now enter the Planner queue. Document-only recurrence edits work. A live deadline test passed. Published Task files now open through Artifact cards. HTML isolation and authorized downloads passed controlled checks. Artifact directories no longer block startup recovery. Bundled KaTeX fonts load. Exact downloads survive restart. Browser Task and Project edits preserve stale drafts. Task filters, Project archive controls, and Chat capture passed. Task detail now retains its conversation source. Live Chat recovers a dropped text delta. Rich text and foreground requests pass in Chromium. The audit is paused for the requested choice simplification. Successful choice displays now finish the Chat turn without another model response. Web caches option labels within each question. Web and iOS omit successful choice tool markers. A selection sends ordinary user text and saves selected-option metadata. Choice queues, pause recovery, and provider resumption are removed. Migrations 34–35 release existing questions and preserve selections. Controlled runtime, transaction, upgrade, and live selection checks pass; the assistant reply is a normal message in the next turn. A2UI forms preserve submitted values. Invalid surfaces, forged context, and stale actions are rejected. Browser history loads during streaming; adjacent pages retain exact content after reload. Tool-result reconnect and fresh capture despite an older matching title pass. Memory updates, corrections, citations, and deeper Chat retrieval pass. Memory search reconstruction preserves exact results and source files across restart. Repeated facts do not duplicate articles; assistant fiction stays out of Memory. Synthetic preferences remain on the test instance. Browser reschedule, unschedule, and early Task completion pass. Recurrence drafts preserve current text and timing. Manual recurrence execution preserves normal cadence; history remains visible after completion. Automatic occurrences copy edited templates exactly and preserve earlier Task files. Pause, resume, skip-next, and end follow actual recurrence deadlines. Controlled-time store tests verify spring gaps and repeated fall minutes without duplicate runs. Overlap policy tests found and fixed extra queue-one catch-up Tasks. Future skipped slots remain intact. Live restart checks verify both missed-run policies for one-time Tasks and recurrences. RUN_ONCE executes once; SKIP avoids execution. Completed state survives another restart. One-time schedule races preserve the accepted deadline and queue one run. Recurrence races preserve accepted future timing and the revisions of already-created occurrences. A Bun patch fixes an Astryx text-measurement loop that crashed the recurrence schedule dialog. Task roles now receive fresh clocks, occurrence cutoffs, and separate original Chat timestamps. Controlled role clocks pass. Live relative-time capture resolves tomorrow correctly in Los Angeles and Tokyo. Direct delegation now completes without a Planner. Optional enum conversion now permits null while preserving source rules. Controlled correction checks verify current request, result, and review files across rejection and final approval. Cancellation rejects old run changes across queued, running, and waiting states. Cancellation now marks executing actions as outcome uncertain in the same transaction. A controlled MCP write returns late success without replacing uncertainty or cancellation. Cancellation now saves an uncertain tool result and closes the transcript call. Desktop and phone checks show the uncertainty message after result unwrapping. Completed run headers now use stored status. Live desktop and phone checks show Completed. Controlled continuation reloads saved progress and support files without repeating the completed write. Reopening a cancelled Task rejects late provider file writes and preserves current files. Completed-Task reopening rejects late review before file writes. Both reopening variants preserve current files and generation. SQLite reopening preserves all three Task roles and saved progress without duplicate runs. Controlled runtime restarts restore saved read results for all three roles and reach completion with unchanged files and run identity. Other providers remain pending.
Controlled TLS API checks pass empty results, pagination retry, operation and argument bounds, and invalid-output rejection. Safe read retries, rate-limit responses, and uncertain writes pass controlled TLS checks. Cross-account continuation remains pending. Controlled OAuth exchange, callback replay rejection, late attempt lookup, and shared refresh pass. Denied, expired, failed, and superseded callbacks expose no tools; fresh attempts recover. AUDIT-06 records blocked application deletion after grant revocation. AUDIT-07 records missing operation access after expanded OAuth consent. Partial consent preserves access limits and account identity. API rename, disable, enable, and deletion pass controlled calls. Deletion survives service reconstruction without changing the shared grant or neighboring API. Real consent remains pending. API model tools include current account labels. Account renaming preserves tokens. Mutation replies retain OAuth metadata. Real device and consent variants remain pending. Controlled MCP discovery, policy controls, stale metadata rejection, deletion, and disconnect checks pass. MCP OAuth completion, replay rejection, and cancellation pass. AUDIT-08 records renewal losing the registered OAuth client. Deletion during renewal remains pending. Disabled stdio starts no process. Enabled stdio preserves explicit ordinary and protected environment bindings without inheriting parent-only values. Browser presentation, full review execution, invalid results, and pending authentication removal remain pending. ACP coverage now reuses unchanged successful unit and runtime checks. ACP process-start failure preserves files and enters recovery. Retry selects corrected configuration once and completes through approval and review with exact saved progress. Live ACP create, failed probe, edit, and delete pass at both browser widths. Synthetic entries were deleted. Controlled ACP probing and named authentication also pass at both widths and survive reload. Deleted ACP Executors reject Task creation and reassignment before and after database reopening. Controlled ACP-01 passes. Download checks preserve exact Unicode bytes and reject escaping paths, private redirects, and replacement of existing files. AUDIT-09 records the missing Executor file.download capability. Task download placement and review remain pending. Legacy DOC output matches its complete expected text through the production parser. Controlled FILE-03 now passes with earlier DOCX, ODT, and RTF evidence. Controlled PDF and EPUB parsing preserves exact text and order. Malformed PDF fails explicitly without changing source bytes. XLS numeric parsing and XLSX/ODS date, currency, and cached formula displays pass. XLS display variants remain pending. Installed OCR recovers clear text. Missing OCR fails explicitly while unrelated text parsing works. Poor recognition and Chat behavior remain pending. Document archive limits reject oversized input, oversized referenced parts, excess entries, and parent paths. Valid control and source preservation pass. Controlled FILE-09 passes. PPT and PPTX match complete expected structure and notes through the production parser. Earlier ODP evidence completes controlled FILE-04 coverage. Default public search returns usable results; direct fetch preserves source and content. Live configured Chat completes search and fetch, cites source facts, and labels inference. WEB-01 and WEB-02 pass. Browser coverage now reuses unchanged controlled lifecycle, installation, stale-revision, uncertainty, and upload checks. Real-provider paths and complete effect verification remain pending. AUDIT-10 is fixed: reviewed browser actions now retain the provider response ID, and continuation accepts its running turn state. The exact live example.com lifecycle completes through link, history, and close. Controlled failed switching preserves the usable session and route. Retry selects the recovered next provider and closes successfully. Artifact and file boundaries, late output after cancellation, and recovery diagnostics remain pending. AUDIT-11 is fixed; changed browser targets are rejected under saved references. AUDIT-12 is fixed; hidden and password values stay out of browser snapshots. AUDIT-13 is fixed; declined effects remain unclaimable even when references change under a permissive review. AUDIT-14 is fixed; eligible pending-action notifications are queued. AUDIT-15 is fixed; emulated denial updates permission state. Real prompts remain pending. Direct web links pass at desktop and phone widths. AUDIT-16 is fixed; an active web client refreshes saved settings without reload. The original provider is restored. Emulated installed-browser checks activate the real worker and save an authenticated snapshot. AUDIT-17 is fixed; saved drafts survive navigation and offline reload. AUDIT-18 is fixed; authenticated recovery exits offline mode. Saved Chat reads return offline and Send stays disabled. Installed-browser authentication, local erasure, and controlled quota checks pass. Physical installation and conflict reconciliation remain pending. Optional access flags and public authority checks pass; full configured startup remains pending. AUDIT-19 is fixed; Desktop consent cancellation reaches its callback.

The Go server uses a fresh home. It does not open or convert a Rust home. INFO-02 passes through a controlled reviewed MCP disclosure binding. JOURNEY-05 passes through the supported Task Artifact route; the Chat route still refuses because it cannot create or publish Artifacts.
The Go server is the production authority. Rust remains only for retained support targets.
Rust behavior is the default target; the [divergence review](../validation/2026-09-08-rust-go-divergence-review.md) records exceptions and test gaps.
The Go server includes authentication, onboarding, Chat, Projects, Agents, Artifacts, Task lifecycle, integrations, and notifications.
OpenAI, OpenRouter, and Codex preserve text, tools, hosted search, replay, reasoning, citations, usage, and current model assignments.
OpenAI and Codex reuse bounded Responses WebSocket sessions within each Chat turn or Task run.
Migration 33 removes Apple model accounts and selections. Historical conversation and Task records remain intact.
Primary Chat supports durable recovery, context admission, compaction checkpoints, A2UI, and bounded tool loops.
It reviews exact `file.download` calls, pauses for decisions, and resumes from known or uncertain outcomes.
Task placement, schedules, recurrences, occurrence documents, and due release now use Go authorities.
Task runs snapshot and enforce continuation, tool, active-time, retry, review, and audit limits.
Primary Chat receives durable Task capture, waiting, recovery, completion, and integration-ready notices without repeated cards.
Native Memory owns bounded reads, search, citations, hierarchy, model updates, crash-safe publication, checkpoints, and root prompt context.
Artifact storage owns safe local files, external URLs, versions, metadata, integrity checks, and authorized delivery.
Spreadsheet previews support XLS, XLSX, and ODS through bounded parsers.
Excelize owns XLSX parsing and display formatting. Archive and output limits remain local.
`file.parse` uses isolated conversion for DOC, DOCX, PPT, PPTX, ODT, ODP, RTF, PDF, EPUB, and spreadsheet formats.
Web Push and APNs own protected keys, client registrations, presence, durable retries, and primary Chat final answers.
Built-in Tasks run all roles and Task tools, while preserving artifact sources, capture time zones, and Executor-only hosted web access.
MCP, bounded Lua, ACP Task runs, Agent naming, HTTP adapters, direct credentials, adapter OAuth, and Task runtime events now use Go.
Explicit web search, fetch, settings, SPA assets, GraphiQL, and the private GraphQL socket now use Go.

## Current constraints

### Security and storage

- [Security](../harness/security.md) owns the three information classes.
  Secrets never enter model context or ordinary persistence. Authorized private
  information and ordinary technical values remain intact.
- `${NOEMA_HOME:-$HOME/.noema}` is the durable home. SQLite at
  `noema.sqlite3` owns stored structured state and is server-only. Schema
  changes append immutable forward-only migrations.
- Native Markdown under `memory/human/` owns durable human memory. Search rebuilds
  its lexical index in process memory. Version-two pages contain claim-level evidence groups.
- Durable chat comes from conversation items. Live subscriptions and daemon
  state coordinate work but do not replace stored state.
- Provider assistant text uses one conversation item. Readable text is primary.
  The same row stores provider text only when projection changes it.
- Current task state is transactional. Task events support audit and
  invalidation. They do not provide a second replay authority.
- Task stages use one code-owned personal workflow. SQLite does not store workflow definitions.
- A successful command commits its state, audit event, notification, and
  idempotency receipt together when applicable.

### Runtime and governance

- Each action request stores one exact call, review, decision, execution state,
  and result. Immediate and reviewed calls use the same source input check.
- A reviewer model classifies authorization and risk. Current policy can
  execute a reviewed external write when that classification permits it.
- A human decision approves or declines one exact action request. Approval is
  one-shot and is consumed during execution admission.
- A correction Executor reads current `TASK.md`, `RESULT.md`, and `REVIEW.md` files.
- Task state changes use a current-run check. Old generations and stale worker
  claims cannot change the current task.
- `TASK.md` is the mutable Task request, plan, notes, progress, and questions authority.
- `PROJECT.md` is the mutable project context authority. SQLite stores project metadata, but not its Markdown.
- Every Planner, Executor, and Reviewer run reloads its linked project's current `PROJECT.md` before Task files.
- SQLite stores Task and recurrence titles, but it stores no duplicate Task prose.
- Recurrence templates use `${NOEMA_HOME}/recurrences/<recurrence-id-suffix>/TASK.md` and seed future occurrences exactly.
- Human Task and template saves use transient SHA-256 fences. Stale saves preserve both the draft and current data.
  `RESULT.md` is the mutable submitted result authority.
  `REVIEW.md` contains current Reviewer feedback when feedback exists.
- Planner, Executor, and Reviewer handoffs use current Task files.
  Noema does not store content snapshots for those handoffs.
- An Executor continuation receives bounded tool actions after the latest successful `TASK.md` save.
  A tool call without a result has an uncertain outcome.
- An Executor can submit an honest limitation report for an impossible outcome.
  Human-resolvable blocks and per-run ceilings do not qualify as system limitations.
- Every Task role run receives a fresh current clock.
  Captured request time remains separate data for interpreting the original request.
- A final task transaction finishes active run items and open debug spans.
  An uncertain external action returns to an Executor for status checks.
  An equivalent action request cannot run again.
- Reviewer approval owns the Task completion update decision.
  A no-new-information result suppresses that update when the Task forbids repeated content.
- Provider request settings do not authorize returned tool input. Every returned
  call must pass the source input check before invocation.
- [Provider generation sessions](../development/provider-sessions.md) own transport efficiency.
  Complete local replay preserves ordered provider output and owns correctness.
- A healthy provider continuation can exceed the local replay admission limit.
  Provider-hosted web state fails closed if the provider continuation expires.
- Provider wrappers forward active-continuation state. The runtime does not
  compact replay while the inner provider session remains active.
- Finalization makes one provider request without tools. Its instructions state the exact stop reason.
- Historical tool results can support valid facts. They do not prove a requested
  current-turn action occurred.
- An explicit human request for foreground execution overrides automatic
  delegation advice.
- Responses WebSocket requests use the provider timeout. Current request
  settings can change while the earlier response identifier continues provider
  state.
- A full provider conversion preserves every source rule. Optional null
  placeholders are restored before policy, review, or storage.

### Capabilities and integrations

- Hosted providers, local models, MCP, native HTTP adapters, and browser tools
  keep distinct transport and security ownership.
- Hosted web clients own typed credentials. Shared credential access supplies them without ordinary-string conversion.
- Hosted search and fetch share URL and response normalization. Each provider owns its wire protocol and status mapping.
- TinyFish and Firecrawl provide hosted search and fetch. Firecrawl also has a permanent credential-free account.
- Hosted web remains the preferred page reader. `file.download` stores public
  non-HTML resources, and `file.parse` returns bounded local content.
- Humans can upload private files into Task-owned artifacts.
  The artifact store assigns the owner, immutable version, byte size, and internal integrity digest.
- `file.parse` returns bounded text from saved email messages and supported raster images.
- Agents can run bounded Lua 5.4 over read-only JSON input.
  The sandbox has no file, network, process, module, clock, or random access.
- Generated artifacts keep their owner, creation scope, and immutable version IDs.
  Task results cite an artifact ID and a precise locator when the artifact supports a claim.
- Artifact detail previews PDF, spreadsheet, email, raster image, and isolated HTML artifacts.
  Binary previews use authorized inline routes. SVG remains download-only.
- Artifact creation does not apply a hidden HTML accessibility gate.
  A Task or standard renderer must own accessibility requirements for its deliverable.
- File downloads use the same URL policy and action review as fetch and browser
  open. Primary chats keep a durable working directory.
- Each model preference owns its speed. Codex and OpenAI support Standard and
  Fast. Durable request snapshots preserve that choice.
- Adapter manifests use schema version 9. Definitions and OAuth objects have
  filesystem authorities. SQLite adapter projections are disposable.
- Adapter installation is direct. No scheduler or scheduled adapter work remains.
- An adapter definition ID identifies the service. A semantic digest identifies
  one immutable reviewed revision.
- OAuth profiles, applications, external accounts, grants, and connections are
  separate authorities. Protected generations contain client secrets and
  tokens.
- Web Chat groups pending OAuth work by application or grant. Compatible APIs
  share one attempt, while each API keeps its connection policy.
- The compiled catalog exposes only operations covered by current OAuth scopes.
  Existing covered operations stay active during scope expansion.
- A disabled operation remains non-callable. Its enablement tool creates a human
  action request when current policy permits enablement.
- Connector proposals expand into one complete manifest before compilation,
  review, and persistence.
- Native Project-to-Task reads support Project status and audience updates.
  Executors can list bounded workspace Tasks. Executors and Reviewers can inspect one exact Task and its current documents.
- Delegated daily briefs, planning, and weekly reviews passed live with these reads.
- The reviewed event adapter exposes one bounded instance operation.
  It requires both time bounds and preserves recurrence, all-day dates, attendees, locations, and continuation. The current synthetic Calendar revision returns up to six events per page.
- The reviewed message adapter uses an explicit empty JSON array for absent message results.
  Empty results and a two-page continuation passed live after revision.
- All 14 Milestone 1 main paths pass in the current provider setup.
  The provider-neutral package is `docs/validation/personal-assistant-milestone-1-acceptance.md`.
- The human waived second-provider and exact fixed-route portability for Milestone 1.
  The accepted evidence verifies outcomes but does not prove portability.

### Clients and product surfaces

- [Server authentication and public access](../server-security.md) is the
  implemented server contract. Development and local-access features fail
  closed by default.
- Codex `noema-build` supports Git, caches, sockets, and complete read-only development-home inspection. See [development permissions](../development/codex-permissions.md).
- `./attach` owns the public development supervisor through one tmux session.
  Tmux runs a guardian that gives the supervisor a parent-death signal. The
  supervisor then stops its child watchers during session shutdown.
- iOS and desktop use browser OAuth with S256 PKCE, passkey approval, short
  access tokens, and rotating refresh credentials. Legacy pairing routes and
  stored bearer credential fields are absent.
- Browser sessions survive server restarts. A protected cookie key and stored
  session digests preserve authority without placing cookie values in SQLite.
- The first visitor to a fresh instance can create the initial passkey without
  a recovery code. One atomic insert selects the winner during concurrent claims.
- After the initial claim, adding a passkey requires recent passkey verification
  or a recovery-code setup session.
- Native refresh rotation binds recovery to a client-saved request identifier.
  The exact response remains recoverable while its direct successor is active.
  Noema desktop 0.1.x retains a 60-second retry until its request-bound release.
- iOS runs refresh only while active. One connection service owns refresh
  scheduling, serialization, Keychain reloads, and current transport access.
- The Tauri app defaults to its embedded host and can connect to one remote
  HTTPS server. Rust owns OAuth, credentials, transport, and local return.
- Interactive browser sessions belong to one conversation or task generation.
  The human configures an ordered provider route. Obscura remains the default.
  Go downloads upstream Obscura v0.2.2 with pinned archive digests.
  The CDP adapter shares Kernel snapshot logic and preserves trusted actions and main-document failure checks.
- The agent changes providers only through `web.browse.switch_provider`.
  A switch starts fresh and never transfers browser state.
- A failed initial browser open keeps route state. The agent can switch providers without a snapshot revision.
- A switch to the exact failed navigation URL reuses that live session's URL
  authorization. A different URL follows normal review.
- Switch recovery includes the current snapshot revision when one exists.
- Browser failures retain typed recovery and safe provider diagnostics through
  model results, persistence, governed actions, and diagnostics.
- Reviewed form actions bind visible submitted values, method, and declared destination.
  Password, hidden, and file controls stay outside review context.
- A main-document 5xx response after browser interaction becomes `outcome_uncertain`.
  A returned uncertain snapshot remains available for reconciliation.
- An uncertain Task action keeps normal read tools available before Noema requests human help.
- A continued Executor snapshots a recorded active browser session before reopening a URL.
- The Task Reviewer checks every explicit requirement against current evidence.
- Reviewed browser actions can upload one exact Task artifact.
  The action request binds its Task, artifact version, filename, and byte count.
  The artifact store verifies the file digest internally.
- Obscura cannot select local files.
  The current browser route uses Kernel for upload actions and returns `retry_later` when that switch fails.
- Reviewed actions do not require a Task document save.
  Review and approval remain the action safety boundaries.
- One coordinator owns the active backend and public snapshot revisions.
  Every navigation reruns network and SSRF checks.
- Browser commands have a 30-second deadline. A timed-out process is discarded.
- Web Push registrations belong to browser sessions. Browser logout removes
  session-bound registrations. Installed mode can erase its private local data.
- The iOS client stores normalized reads in one protected per-client cache. It
  clears that cache during disconnect and does not queue offline writes.
- Frontend route and interaction truth is in
  [the current frontend contract](../frontend/current-contract.md). UI changes
  follow [product design guidance](../frontend/product-design.md).
- Task creation uses `/tasks/new` and the normal Task detail area.
  Project creation uses a name-only row below Personal in the Tasks sidebar.
- Task and project Markdown documents share explicit edit, save, cancel, source,
  error, and stale-reload behavior.
- Each web route owns one composed GraphQL root read. Shared fragments and normalized mutation payloads update the Apollo cache.
- Apollo abstract-type metadata comes from the generated GraphQL schema. A cache
  schema version change discards incompatible installed-PWA snapshots.
- Subscriptions invalidate one active root. A mutation refetches only when its payload cannot represent server-derived state.
- The backend formats built-in tool markers from saved tool facts during reads
  and live delivery. It does not store marker text.
- Web, iOS, and Live Activities consume the same action, outcome, and status data.
  Raw built-in payloads remain under technical disclosure.
  Connected tools keep the generalized marker path.
- Optimize total system simplicity. Follow
  [engineering simplicity](../development/simplicity.md) before nontrivial
  architecture, workflow, harness, or testing-policy work.

## Open loops

- The current calculation and artifact paths passed focused live regression.
  Task `task:18d07a17681adac7353` completed with a reviewed accessible HTML artifact.
- Browser review compares parsed interaction meaning across snapshots and optional input shapes.
  The unchanged retry returned `human_declined_equivalent_action` before a second action request.
- Current reviewer policy can execute an authorized medium-risk write.
  Do not add mandatory review, decline notes, or a general consent registry now.
- The Milestone 3 evidence record is `docs/validation/personal-assistant-milestone-3-acceptance.md`.
- Task `task:18d0fabfeac8347e2c05` passed automatic flight reconciliation.
  It observed `PROCESSING`, used read-only checks, and reached `CONFIRMED` with one commit.
- All six Milestone 4 acceptance families pass their shared gates.
  Tasks 41, 43, 45, 77, and 95 now have complete current main paths.
- The Milestone 4 evidence record is `docs/validation/personal-assistant-milestone-4-browser-acceptance.md`.
- Milestone 5 excludes shared Noema workspaces and secondary Noema human accounts.
  Multi-person cases use local-owner Tasks, external participant records, and reviewed external actions.
- All 14 Milestone 5 tasks now pass current controlled main paths.
  Follow-up records closed the four partial paths and ten missing lifecycle checks.
- The Milestone 5 evidence record is `docs/validation/personal-assistant-milestone-5-audit.md`.
- The 100-task roadmap now records 80 Verified, 16 Extend, and 4 Build tasks.
  The remaining 20 tasks need complete operational main paths.
- The consolidated follow-up evidence is `docs/validation/personal-assistant-remaining-acceptance-2026-08-31.md`.
- The uncertain Build evidence is `docs/validation/personal-assistant-uncertain-build-acceptance-2026-08-31.md`.
- Apollo iOS 2.3 code generation and native builds require macOS.
  Linux can validate the authored GraphQL operations against the shared schema.
- Synthetic Gmail and Notion OAuth callbacks have been exercised through the
  live hosted fixture. Real provider consent remains outside this acceptance.
- Production iOS notifications need enabled entitlements, regenerated signing
  profiles, and an APNs provider configured in browser Settings.
- Production Tasks Live Activities also need the widget App ID in regenerated
  signing profiles.
- Both Go size measures must remain below 80%. A protection needs a current failure, concrete threat, retained capability, or client contract.
- Go owns schema version 38, Playwright browsing, Kernel sessions, diagnostics, and local-model execution.
- Desktop launches a Go sidecar. The former Rust backend and evaluation dependency closure are removed.
- Obscura uses upstream releases without Cargo. Public Linux installation, executable reuse, direct worker lifecycle, and stalled-worker recovery pass. The exact live Chat `example.com` lifecycle now passes after reviewed continuations preserve the provider response and continuation tool rounds accept their running state. AUDIT-11 records an old snapshot executing a changed button. AUDIT-12 records protected control values in snapshots. AUDIT-13 records a repeated declined effect becoming claimable. Use synthetic form values during further checks. Product selection remains pending. Native Windows WAL stress remains a cutover gate.

## Validation

Follow [AGENTS.md](../../AGENTS.md#validation) for check scope, commands, retries, and result reuse.
Use focused checks during implementation. The integrating agent owns broad validation for each completed server or native code unit.
Commits, pushes, and report edits do not invalidate successful checks with unchanged inputs.
