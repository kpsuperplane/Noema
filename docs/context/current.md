# Current Noema Context

This brief contains active direction, current constraints, and open loops.
Durable contracts belong in subsystem documents. Git owns completed history.

## Active direction

Noema is an always-on, self-hosted personal agent. Chat is the primary surface.
Tasks, Memory, integrations, governance, and settings appear when needed.

The current product has one server with web and desktop shells. It also has a
native SwiftUI iPhone and iPad client. New work should be a small vertical
slice or a net-negative reduction.

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
- A Task continuation requires `TASK.md` to be the last completed tool action.
  Progress-audit checkpoints keep file tools available until that write succeeds.
- An Executor can submit an honest limitation report for an impossible outcome.
  Human-resolvable blocks and per-run ceilings do not qualify as system limitations.
- Every Task role run receives a fresh current clock.
  Captured request time remains separate data for interpreting the original request.
- A final task transaction finishes active run items and open debug spans.
  Unknown external outcomes are not retried automatically.
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
- Each capability binding owns its Task checkpoint policy.
  Browser provider switches do not require a Task checkpoint.
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

- Apollo iOS 2.3 code generation and native builds require macOS.
  Linux can validate the authored GraphQL operations against the shared schema.
- Live Gmail and Calendar OAuth acceptance still needs interactive Google
  account consent after native generation.
- Production iOS notifications need enabled entitlements, regenerated signing
  profiles, and an APNs provider configured in browser Settings.
- Production Tasks Live Activities also need the widget App ID in regenerated
  signing profiles.
- Product decisions remain open for integration substrates, full-parity clients,
  task roles, and secondary vertical systems.

## Validation defaults

- Rust: `cargo fmt --all --check`, `cargo check-workspace`, `cargo gate-lint`,
  and `cargo gate-test`. Use `cargo validate` for focused commands.
- Web: run `bun run lint` and `bun run build` from `apps/web`.
- Run unit tests only unless smoke or fixture tests are explicitly requested.
