# Current Noema Context

This is the active working brief for Codex sessions. Keep it below 300 lines.
Durable subsystem contracts belong in their closest document; Git history owns
completed milestone detail.

## Active Direction

Noema is an always-on, self-hosted personal agent operating system for humans,
agents, conversations, workspaces, projects, tasks, tools, memory, and governed
automation. Chat remains the primary surface; deeper management and inspection
appear when backed state and the human's current job require them.

The current product foundation is:

- one continuously available Noema server with web and desktop shells;
- SQLite-backed structured state owned exclusively by the server process;
- Noema-owned Codex OAuth plus first-party local GGUF inference;
- a native local-human Markdown memory tree with rebuildable lexical search;
- persisted conversations, transcript items, tools, artifacts, approvals, and
  provider/model settings;
- Work as the durable workspace/project/task and supervised-agent system;
- a React/Astryx frontend that starts from chat and progressively reveals
  memory, Tasks, settings, and inspection.

New slices should be vertical, user-visible, and small. Reuse or simplify the
current authority before adding types, ports, fixtures, or future-facing
frameworks. Follow `docs/development/simplicity.md` for budgets, tests,
subagents, reviews, and size measurement.

## Current Architectural Constraints

### Home and storage

- `${NOEMA_HOME:-$HOME/.noema}` is the durable home. SQLite lives at
  `db/noema.sqlite3`; model blobs, downloads, object-owned files, sidecar data,
  and rebuildable system state follow `docs/project.md`.
- The Noema server is the only process that opens the canonical SQLite
  database. Web, desktop, and future mobile clients use Noema APIs.
- Forward-only SQLite migrations preserve databases from the exact v9 baseline
  onward. Applied migrations are immutable; schema changes append a migration
  and advance `PRAGMA user_version`. There is no SurrealDB or older/nonmatching
  SQLite migration path.
- Concrete object rows remain canonical. Actor/principal, governable scope,
  provenance source, and transcript item are behavior contracts implemented by
  concrete objects rather than universal parent tables.
- Filesystem paths hold durable object-owned bytes. SQLite owns metadata,
  relationships, policies, and immutable version records. `system/` state is
  derived and rebuildable.

### Browser authentication

- Browser access authenticates the existing `human:local` principal with one persisted FIDO
  passkey; there is no account, role, or multi-human login model. Initial enrollment requires the
  one-use setup URL printed by the server, so the first public visitor cannot claim an empty system.
- WebAuthn ceremony state is short-lived, one-use, session-bound, and server-side. Assertions create
  private process-local sessions, so daemon restart requires authentication again.
- `web.rp_id` is explicit credential scope. `web.public_origin` supplies the exact Host/Origin checks
  and Secure-cookie policy; HTTPS terminates at a same-host proxy while Noema stays loopback-bound.

### Conversations and runtime

- Durable chat history is reconstructed from `conversation_items`; daemon
  WebSocket state and `agent_status` are live coordination state only.
- Browser GraphQL subscriptions retry with capped backoff. After reconnect,
  clients refetch active queries and backfill durable transcript pages before
  trusting later live completion events.
- OpenAI and Codex are separate Responses dialects. Codex uses Noema-owned OAuth
  homes, exact ChatGPT workspace/client identity, and the provider catalog; it
  does not silently import Codex CLI authentication.
- Runtime execution, prompts, context, tools, persistence, task workers, and
  transport-neutral events live in `noema-runtime`. Host composition lives in
  `noema-host`; GraphQL lives in `noema-api`; server and desktop remain process
  shells.
- New Chat turns and Planner, Executor, and Reviewer runs record safe wall-clock
  spans in `runtime_debug_spans`, owned by exactly one turn or run. The shared
  transcript Debug dialog reads this durable profile, polls only while running,
  and keeps provider rounds, tool execution, persistence, and uninstrumented
  gaps distinct without storing prompts, arguments, results, or raw errors.

### Providers and local models

- Provider selections persist exact provider instance, account, model/profile,
  and reasoning effort. Foreground work resolves current selections; admitted
  background runs retain their exact snapshots.
- Every generation request also carries the effective `ProviderToolTransport`
  selected from that pinned provider/model capability. Request lowering,
  response normalization, and runtime continuations use that same value, so
  native calls and the legacy Noema envelope cannot be inferred from tool
  counts or from whichever fields a provider happened to return.
- First-party local inference is the `local_models` provider over a supervised,
  loopback-only pinned llama.cpp runtime. Runtime lookup never trusts ambient
  `PATH`; packaged assets and explicit development overrides are authoritative.
- Verified GGUFs are content-addressed, downloads are resumable, and public
  imports require immutable provenance plus SHA-256. Activation, removal,
  process leases, and in-flight requests must remain race-safe.
- The bundled catalog contains only qualified model/build combinations. Gemma
  4 E4B IT remains the current measured default; qualification reports under
  `evals/local-models/results/` own historical measurements.
- Built-in and MCP tools share one typed request catalog. Local grammar lowering
  may relax llama.cpp-incompatible schema constraints, but canonical runtime
  handlers still validate authoritative payload schemas.
- Local generation is priority-arbitrated so foreground chat precedes memory,
  tasks, audits, compaction, and task-originated web summaries. Prompt-cache and
  memory-observation scheduling must not regress foreground latency.

### Memory

- Markdown under `memory/human/` is the only memory authority for `human:local`.
  Frontmatter owns page metadata, generated Lucide icon keys, and provenance;
  the filesystem owns hierarchy. Iconless legacy pages receive read-time
  fallbacks without rewriting canonical bytes.
- Canonical pages use a compact Wikipedia editorial form: a single generated
  title, concise lead, coherent sections, inline citations, and collected
  references. The root is the human overview, developed roots require at least
  two thematic sections, and children are focused topic articles.
- `root.md` is bounded to 750 words and enters every ordinary turn. Native page
  reads and lexical search retrieve deeper detail; their page text is ephemeral
  while durable tool results retain references and hashes only.
- Memory updates read one captured range from the primary conversation using
  its existing `sequence_index`. Completed human text is evidence; assistant
  text may provide context but is not independent evidence.
- Context compaction and the Memory-page action schedule the same separate,
  single-flight background job through the selected Memory model. Publication
  recovers forward from `.pending`, rebuilds FTS, and advances `.state.md` last.
- Memory source arrivals and job transitions invalidate a GraphQL subscription
  that refills the canonical tree/status snapshot without browser polling.
- Direct editing, history, private memory, additional scopes, vectors, and
  migration from the retired service remain outside the first slice.

### Capabilities, MCP, and artifacts

- Capability eligibility, approval, invocation, and audit authority remain
  explicit and fail closed. Do not infer semantic intent from English phrases,
  prefixes, or field names.
- `noema-capabilities-mcp` owns MCP contracts and transports. Stdio and rmcp
  Streamable HTTP are the supported transports; deprecated HTTP+SSE stays
  removed.
- MCP secrets use atomic, path-safe storage. Transport input, process trees,
  timeouts, cancellation, OAuth state, and per-server mutations remain bounded
  and race-safe.
- Each MCP provider owns two independent choices: whether otherwise-safe calls
  may receive context automatically, and how unsafe calls are approved. A tool
  is risky when it is destructive, or when it both mutates and is open-world;
  every call is unsafe when either risky or covered by `review_every_call`.
  Safe calls execute directly, while unsafe calls follow only the provider's
  unsafe-action policy. Reviewer failure creates a durable approval request
  instead of executing.
- A completed reviewer assessment is composed by one global authorization/risk
  policy: explicit/substantive authorization with low/medium risk, or weak
  authorization with low risk, may execute automatically; all other pairs
  require approval. Approval is exact, revision-fenced, payload-bound, and
  consumed once. Foreground and Work
  task actions use the same authority; waiting task runs release their lease
  and resume through a pinned child run after the exact action outcome is
  recorded.
- A foreground approval request completes the proposing generation without a
  failed tool result. Resolution records one terminal result under the original
  provider call identity and starts an idempotent bounded continuation without
  synthesizing another human message or retrying the external action.
- Primary actions review a seven-message human/assistant excerpt ending at the
  source human item. Human entries alone create authority; assistant entries
  only resolve later human references. Agent-created tasks snapshot that
  excerpt, while manual creation or a human Inbox body edit stores the task
  title and description. Work actions use that task-owned snapshot, never the
  rendered task prompt or a later conversation reread. Task actions remain
  lease/generation fenced, and TaskReviewer has no export tools.
- MCP tools persist effective read-only, idempotent, destructive, and open-world
  hints with provenance. Complete annotations make a tool ready immediately;
  missing hints block only that tool while background classification runs, and
  classification failure applies pessimistic defaults. Invocation revalidates
  the provider policy, tool policy revision, and metadata fingerprint before
  execution; reviewer- and human-mediated reads use the same exact governed
  binding and approval lifecycle as mutations.
- Web search is a trusted query to the configured search provider and does not
  require an approval prompt. Search-result URLs and links extracted from
  fetched HTML enter the local `observed_urls` authority; a later exact fetch
  may skip review when its remaining arguments are local-only controls, while
  every request still reruns current URL, DNS, and SSRF checks. Approved web
  replay is destination- and digest-bound; observations do not expire.
- The web app surfaces pending actions above the primary composer, in task
  detail, and in Work Needs You. The persisted action/event authority is
  delivery-neutral so another client can project the same attention state.
- Governed artifacts are versioned outputs owned by concrete contexts. Local
  writes use artifact storage helpers that reject traversal and symlinks;
  external versions accept validated HTTP(S) URLs only.
- The primary agent creates local files through `artifact.create_local_file`.
  Chat renders typed artifact references and opens local text/Markdown versions
  in the generic detail rail.

### Work

- Work is the durable workspace/project/task system. V1 seeds one Personal
  workspace and one executable workflow. Optional projects organize tasks
  without changing execution policy.
- `tasks.stage_id` is the workflow-state authority. Current run, gate, review,
  attention, and completion labels are derived projections, not parallel
  status fields.
- The six stage behaviors are Intake, Dispatch, Active, HumanGate,
  TerminalSuccess, and TerminalCancelled. Done is terminal success and remains
  available through task history; there is no separate task Archive stage.
- Semantic commands are revision- and generation-fenced, idempotent SQLite
  transactions. State updates, audit events, required notifications, and
  command receipts commit together.
- Planner, Executor, and Reviewer runs are bounded and supervised. Human gates,
  contracts, submissions, reviews, cancellation, retry, and reopen preserve
  generation and lease authority. Reviewer approval completes a task directly;
  Reopen requires new human direction and queues a fresh generation.
- Simple contracts default to two short phases and compact results; execution
  policy values remain runaway-work ceilings. Reviewers are bound to submitted
  evidence without task-list or web tools. Executor and Reviewer terminal tools
  enumerate exact criterion ids and allow one in-conversation payload repair
  before a non-retryable recovery gate.
- The global `work_events` ledger is an audit and invalidation surface. Do not
  turn it into a second state authority or add replay infrastructure without a
  concrete runtime requirement.
- The primary chat stores task cards as `task_reference` message items whose
  payload contains only the task id. GraphQL hydrates each card from the live
  task projection and a Work-event subscription keeps it current; the primary
  agent narrates durable task updates naturally, while `/work` provides denser
  management. Both reuse the same task detail and server-owned action vocabulary.
- Task detail uses one padded chronological transcript as the primary surface;
  its compact summary header groups server-authorized task command icons beside
  the Tasks and information controls. Planner, Executor, and Review runs are
  marked inline with role, revision, status, and duration; their persisted items reuse the shared
  response and activity lanes, and each immutable executor submission renders
  once as the durable result in that chronological stream. A compact floating
  task card carries the title and info trigger, an optional needs-input row,
  and a validation summary row on active tasks and completed-task Transcript
  tabs; the completed-task Result tab omits it. The info popover exposes
  compact metadata only, while validation details remain available through the
  row disclosure.
- Current Work behavior and `docs/workspaces/README.md` are authoritative. The
  completed multi-agent implementation packets remain in Git history and should
  not drive new implementation.

### Frontend

- Noema product UI follows `docs/frontend/product-design.md` and the repo-local
  `noema-product-ui` skill.
- Design starts from the human's job, focal action, information priority, and
  semantic grouping. Productive surfaces use Astryx components and spacing
  tokens before one-off controls or raw values.
- Chat, Tasks, Memory, and Settings share one compact navigation band in the
  shell chrome above the white content deck. Memory pages and Settings sections
  use the same labeled navigation rail on the shell's left. On mobile, primary
  navigation lands on each root page with the rail closed, and a shared page-title
  trigger opens it; Memory omits both controls until it has more than one article.
  Returning to another surface uses the persistent navigation. Memory page icons
  appear in its rail, article title, and Related Articles cards while titles remain
  the accessible labels.
- Chat, detail rails, task transcripts, settings, and domain objects reuse
  existing Noema presentation patterns. Evidence and internals stay available
  through progressive disclosure instead of flattening every field into the
  default view.
- Stubbed actions and controls remain hidden until real backend operations
  exist. Backend field availability is not a requirement to display a field.
- Frontend generated GraphQL and route artifacts are generated, never edited by
  hand.
- Finite frontend motion uses the shared critically damped `micro`, `standard`,
  and `surface` presets plus sampled CSS/Astryx tokens. Periodic work signals are
  the only time-based exception and stop under reduced motion; gesture and
  scrolling springs remain interruptible and preserve semantic state authority.

## Open Loops

- Complete third-party MCP installation/auth management, approval decision
  mutations, and richer audit persistence without reviving deprecated
  transports or implicit approval.
- Evaluate native-memory recall, citation accuracy, page churn, secret-copy
  behavior, and root growth before adding scopes, vectors, or editing.
- Decide which export formats ship first and how preview/redaction works.
- Add signing, notarization, updates, and production distribution after the
  unsigned developer desktop build is stable.
- Implement the approved system-error log contract from
  `docs/superpowers/specs/2026-07-02-system-errors-log-design.md` when it becomes
  the active slice.
- Continue simplifying Work, Store, Runtime, Providers, and their test fixtures
  under measured net-negative slices. Do not start another repository-wide
  horizontal rewrite.

## Codex Workflow

- Keep exploration, planning, implementation, review, and shipping distinct.
- Preserve unrelated changes. Work on main, commit finished units, and push only when asked.
- Use `docs/development/simplicity.md` for budgets, size reporting, tests, and review severity.
- Prefer one vertical implementer and one read-only review unless the work has independent slices.
- Treat raw `~/.codex/sessions` as private; read only when requested and never quote without permission.
- Train references are welcome when they fit naturally.

## Validation Defaults

- Rust: `cargo fmt --all --check`, `cargo check-workspace`, `cargo gate-lint`, and `cargo gate-test`.
- Frontend: run `bun run gen:types`, `bun run lint`, and `bun run build` from `apps/web`; do not add
  frontend tests or use browser inspection unless requested.
- Before commit or push, check status and diffs, then inspect the staged stat and name-status.
