# Current Noema Context

This brief contains active direction, current constraints, and open loops.
Durable contracts belong in subsystem documents. Git owns completed history.

## Active direction

Noema is an always-on, self-hosted personal agent. Chat is the primary surface.
Tasks, Memory, integrations, governance, and settings appear when needed.

The current product has one server with web and desktop shells. It also has a
native SwiftUI iPhone and iPad client. New work should be a small vertical
slice or a net-negative reduction.

The approved server direction is a complete Go replacement. The new server
keeps all production capabilities and uses no Rust or CGo.

The Go server uses a fresh home. It does not open or convert a Rust home.
The Rust server remains the production authority until the final cutover.

The Go evidence gate passed. The replacement now uses Go schema version 17.
It includes authentication, provider onboarding, primary Chat, Projects, Agent settings, Artifacts, Task lifecycle, and notifications.
OpenRouter and Codex preserve text, tools, replay, reasoning, citations, usage, and current model assignments.
Primary Chat supports durable recovery and bounded `task.inspect`, `file.parse`, and Memory tool loops.
Task placement, schedules, recurrences, occurrence documents, and due release now use Go authorities.
Native Memory owns bounded reads, lexical search, citations, hierarchy, state, crash-safe publication, and root prompt context.
Artifact storage owns safe local files, external URLs, versions, metadata, integrity checks, and authorized delivery.
Spreadsheet previews support XLS, XLSX, and ODS through bounded parsers.
`file.parse` uses isolated conversion for DOC, DOCX, PPT, PPTX, ODT, ODP, RTF, PDF, EPUB, and spreadsheet formats.
Web Push and APNs own protected keys, client registrations, presence, durable retries, and primary Chat final answers.
Task execution, governance, remaining tools, Task Live Activity delivery, OCR, and Memory consolidation remain migration units.

## Current constraints

### Security and storage

- [Security](../harness/security.md) owns the three information classes.
  Secrets never enter model context or ordinary persistence. Authorized private
  information and ordinary technical values remain intact.
- `${NOEMA_HOME:-$HOME/.noema}` is the durable home. SQLite at
  `db/noema.sqlite3` owns stored structured state and is server-only. Schema
  changes append immutable forward-only migrations.
- Native Markdown under `memory/human/` owns durable human memory. SQLite FTS is
  rebuildable. Version-two pages contain claim-level evidence groups.
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
- Agents can run bounded Luau over read-only JSON input.
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
  It requires both time bounds and preserves recurrence, all-day dates, attendees, locations, and continuation.
- The reviewed message adapter uses an explicit empty JSON array for absent message results.
  Empty results and a two-page continuation passed live after revision.
- All 14 Milestone 1 main paths pass in the current provider setup.
  The provider-neutral package is `docs/validation/personal-assistant-milestone-1-acceptance.md`.
- The human waived second-provider and exact fixed-route portability for Milestone 1.
  The accepted evidence verifies outcomes but does not prove portability.
- Active-run restart and explicit-reopen authentication recovery reached reviewer-approved terminal success.
- The development watcher now stops its prior server process during a file-watch restart.
  The live acceptance runner retries read-only socket checks during restart downtime.
- The `noema-dev` service owns Task roots under `/var/lib/noema-dev/tasks`.
  Repository paths and `/root` ACLs do not grant service access.

### Clients and product surfaces

- [Server authentication and public access](../server-security.md) is the
  implemented server contract. Development and local-access features fail
  closed by default.
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
- Browser worker commands have a 30-second deadline. A timed-out worker is discarded.
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

- All 28 Milestone 3 paths reached reviewer-approved completion.
  Later acceptance raised the provider-neutral score to 80/100 Verified.
- The dedicated calculator is now bounded Luau.
  Upload source manifests and the hidden HTML validator are removed.
- The current calculation and artifact paths passed focused live regression.
  Task `task:18d07a17681adac7353` completed with a reviewed accessible HTML artifact.
- The return packet received test receipt `M3-0006`.
- The travel packet corrected one malformed source link before test receipt `M3-0007`.
- The human waived affected-user review for the controlled accessibility fixture.
  This waiver does not prove usability for an affected user.
- The Milestone 5 withdrawal retest passed as `task:18d0cf496635f93b1bd`.
  The human declined `action:18d0cf588c5a34b03bc` for the final send.
- Browser review compares parsed interaction meaning across snapshots and optional input shapes.
  The unchanged retry returned `human_declined_equivalent_action` before a second action request.
- The fixture kept consent withdrawn with no receipt, zero deliveries, and zero attempts.
  The Task Reviewer approved the result.
- Current reviewer policy can execute an authorized medium-risk write.
  Do not add mandatory review, decline notes, or a general consent registry now.
- The Milestone 3 evidence record is `docs/validation/personal-assistant-milestone-3-acceptance.md`.
- The first Milestone 4 browser acceptance slice used controlled bank and flight fixtures.
  The bank path passed with one receipt and one commit.
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
- The six uncertain Build cases reached reviewer-approved terminal success.
  Task 96 passed. Tasks 46, 47, 48, 51, and 94 now have Extend gaps.
- The 100-task roadmap now records 80 Verified, 16 Extend, and 4 Build tasks.
  The remaining 20 tasks need complete operational main paths.
- The consolidated follow-up evidence is `docs/validation/personal-assistant-remaining-acceptance-2026-08-31.md`.
- The uncertain Build evidence is `docs/validation/personal-assistant-uncertain-build-acceptance-2026-08-31.md`.
- Apollo iOS 2.3 code generation and native builds require macOS.
  Linux can validate the authored GraphQL operations against the shared schema.
- Live Gmail and Calendar OAuth acceptance still needs interactive Google
  account consent after native generation.
- Production iOS notifications need enabled entitlements, regenerated signing
  profiles, and an APNs provider configured in browser Settings.
- Production Tasks Live Activities also need the widget App ID in regenerated
  signing profiles.
- The Go migration keeps all current production capabilities and client outcomes.
  Native Windows WAL stress remains a cutover gate.

## Validation defaults

- Rust: `cargo fmt --all --check`, `cargo check-workspace`, `cargo gate-lint`,
  and `cargo gate-test`. Use `cargo validate` for focused commands.
- Go: use `go test ./cmd/... ./internal/...`, `go vet`, `staticcheck`, and
  `govulncheck`. Use `CGO_ENABLED=0` for shipped builds.
- Web: run `bun run lint` and `bun run build` from `apps/web`.
- Run unit tests only unless smoke or fixture tests are explicitly requested.
