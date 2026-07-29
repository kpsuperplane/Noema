# Current Noema Context

This is the active working brief for Codex sessions. Keep it below 300 lines. Durable subsystem
contracts belong in their closest document; Git history owns completed milestone detail.

## Active Direction

Noema is an always-on, self-hosted personal agent operating system for humans,
agents, conversations, workspaces, projects, tasks, tools, memory, and governed
automation. Chat remains the primary surface; deeper management and inspection
appear when backed state and the human's current job require them.

The foundation is one continuously available server with web and desktop shells,
server-owned SQLite state, Noema-owned Codex OAuth, local GGUF inference, native
Markdown memory, durable conversations/tools/artifacts/approvals/Work, and a
React/Astryx frontend that progressively reveals deeper management surfaces.

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
- Filesystem paths hold durable object-owned bytes. SQLite owns metadata, relationships, policies,
  and immutable version records. Adapter definitions, exact source bytes, and provenance instead
  use canonical `${NOEMA_HOME}/adapters/` files plus a disposable body-free SQLite projection.

### Browser authentication

- Browser access authenticates only `human:local` with one persisted FIDO
  passkey; initial enrollment requires the printed one-use setup URL. WebAuthn
  ceremony state is short-lived, one-use, session-bound, and server-side;
  process-local sessions require authentication again after restart.
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
- `noema-runtime` admits every context-bearing agent request against the selected
  model's complete reconstructed input, instructions, provider-visible tools,
  hosted-search overhead, output reserve, and safety reserve. It iteratively
  compacts only provider-consumed history; an oversized active suffix fails
  locally before dispatch. Host composition lives in `noema-host`; GraphQL lives
  in `noema-api`; server and desktop remain process shells.
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
- Memory updates read one captured primary-conversation range and send either
  every lean page or a whole-tree catalog with FTS-ranked editable bodies;
  catalog-only page content is protected from mutation. Human text is evidence.
- The single Memory-model update may emit compact icon metadata patches; the
  server merges them with canonical page content before atomic publication,
  FTS rebuild, and `.state.md` checkpoint advancement.
- Memory source arrivals and job transitions invalidate a GraphQL subscription
  that refills the canonical tree/status snapshot without browser polling.
- Direct editing, history, private memory, additional scopes, vectors, and
  migration from the retired service remain outside the first slice.

### Capabilities, MCP, and artifacts

- Capability eligibility, review, invocation, and audit authority remain explicit and fail closed. The immutable catalog retains the exact target, all four tool-behavior hints, one `execute_immediately | human_review | llm_review` decision, ownership scope, and optional revision-fenced connection destination; duplicate names invalidate it. Role access is derived separately from scope and whether the tool is external. No operation-level result privacy, model-route, model-payload, or provider-retention policy exists: a configured model provider is trusted to receive tool results.
- `noema-capabilities` owns the source-neutral integration-management policy vocabulary: structured API/MCP keys, data-sharing and unsafe-action choices, provenance-bearing four-hint tool policies, readiness, pessimistic defaulting, bounded metadata-only classification, and the original three-decision resolver. Source crates retain authentication, transport, identity, persistence, and async classification scheduling; they do not define parallel policy enums or resolvers.
- GraphQL composes those source authorities behind structured `API | MCP` management reads and exact-revision policy/tool commands; it does not infer ownership from IDs or names. Settings exposes sibling APIs and MCPs definition groups plus shared connection-detail routes for sharing, approvals, enabled state, all four behavior hints, provenance, and decision previews. API groups keep their reviewed definition as connection authority even when a newer draft awaits review. API and MCP setup remain source-specific, and adding a sibling connection reuses only the explicitly selected immutable definition revision with fresh credentials and policy.
- A capability has one execution/model output. Connection-backed MCP and native adapter invokers bound responses to 1 MiB; successful adapter responses use one authority that either decodes bounded JSON or runs an exact reviewed Luau transform on Tokio's blocking pool with a closed media-type/output-schema contract. The resulting value is recursively redacted once, then reused for the model, delayed continuation, transcript, action history, and replay. Native adapters retain bounded JSON details from remote HTTP rejections so the agent can explain actionable provider failures, and binding sanitizers remain only for lifecycle-specific storage.
- `noema-capability-adapters` owns provider-neutral public-HTTP manifest v3 definitions, exact imported sources, filesystem-canonical connection-v2 descriptors/schedules, and private credential/cursor generations under `${NOEMA_HOME}/adapters/`; SQLite projections are disposable. Manifest operations carry the same four provenance-bearing behavior hints as MCP tools, while each connection owns its sharing/unsafe-action policy, enabled operations, and exact-revision human overrides. Startup rewrites validated v1/v2 definitions and connection-v1 descriptors conservatively, preserves credential/cursor/schedule generations, requires fresh connection policy instead of inheriting historical authorization, quarantines superseded objects, and completes before SQLite opens. Chat owns discovery, shared Chat/Settings definition review, bounded credential-JSON import, OAuth handoff, and the required post-OAuth connection-policy choice. An active adapter's authentication challenge renders the shared inline sign-in surface; callback completion atomically replaces the token and resumes the exact paused call only when its definition, operation, account, and policy authority remain unchanged. The setup-complete narration is queued only after both OAuth and policy are ready. OpenAPI, OAuth2, JSON transport, cursor/event/schedule primitives, and reviewed actions are company-neutral. Refresh, durable OAuth attempts, async exports, and durable multi-human ownership remain deferred.
- Adapter OAuth completion may run one reviewed, fixed-argument identity operation and initialize the bounded, human-editable `connection_label` only when it is blank. Probe failure never blocks authorization, no identity work polls or runs at startup, `account_id` remains invocation authority, and Settings plus model service context fall back to the stable connection slug when no recognizable label exists. Active connection Settings also disclose the exact canonical definition and reviewed Luau source. Offline reviewed Gmail-profile, GitHub-user, and CSV fixtures qualify the same response-transform contract without credentials or live account data.
- `noema-capabilities-mcp` owns MCP contracts and transports. Stdio and rmcp
  Streamable HTTP are the supported transports; deprecated HTTP+SSE stays
  removed.
- Streamable HTTP setup and rediscovery may enrich a connection from a validated public `/.well-known/mcp.json` server card. Chat can initiate the same setup from an official service origin, but endpoint discovery remains server-verified and reuses the existing OAuth and policy flow. The card's bounded title/description is persisted as service-level model context, while tool descriptions remain operation-specific and ordinary invocation never performs discovery fetches.
- MCP persistence groups connections only through an explicit stable definition ID. Definitions own the immutable non-secret command/endpoint revision; connections retain their existing server IDs and independently own credential references, discovery, health/auth state, policy, overrides, and authority generation. Adding a connection requires the exact definition revision and starts with fresh credentials, discovery, and policy.
- MCP secrets use atomic, path-safe storage; transport input, cancellation,
  OAuth state, and per-server mutations remain bounded and race-safe. Anonymous
  discovery probes protected-resource metadata: advertised OAuth pauses before
  policy unless the human explicitly chooses public tools, a discovery-time
  bearer challenge remains mandatory, and OAuth always reruns discovery.
- Each concrete connection owns two independent choices: whether otherwise-safe calls
  may receive context automatically, and how unsafe calls are approved. A tool
  is risky only when it mutates and is destructive or open-world;
  every call is unsafe when either risky or covered by `review_every_call`.
  Safe calls execute directly, while unsafe calls follow only the connection's
  unsafe-action policy. Reviewer failure creates a durable approval request
  instead of executing.
- A completed reviewer assessment is composed by one global authorization/risk
  policy: explicit/substantive authorization with low/medium risk, or weak
  authorization with low risk, may execute automatically; all other pairs
  require approval. Approval is exact, revision-fenced, payload-bound, and
  consumed once. Foreground and Work
  task actions use the same authority; durable review rows retain the originating
  review route and behavior snapshot, and waiting task runs release their lease
  and resume through a pinned child run after the exact action outcome is
  recorded.
- A foreground approval request completes the proposing generation without a
  failed tool result. Resolution records safe completion metadata under the
  original provider call identity and starts an idempotent bounded continuation
  without synthesizing another human message or retrying the external action.
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
  execution; LLM- and human-reviewed reads use the same exact binding and
  one-shot authorization lifecycle as mutations.
- Web search is trusted and does not require approval. OpenAI and Codex expose
  hosted live search as the preferred model-native route while the configured
  `web.search` provider remains callable; that configured provider is primary
  for models without hosted search. Search-result URLs and fetched-page links
  enter the local `observed_urls` authority, while every fetch still reruns
  current URL, DNS, and SSRF checks. Approved replay is destination- and
  digest-bound; observations do not expire.
- Definitive authentication challenges create one provider-neutral durable
  request per exact call. Its `capability_auth_requests` row retains only bounded
  identity, revision, route, and opaque argument references; exact replay bytes live under the
  path-safe `${NOEMA_HOME}/run/capability-auth/` authority and are removed after
  idempotent origin publication.
- OAuth progress is delivered by callback and runtime subscriptions rather than polling; the
  public GraphQL start boundary is bound to the initiating human and the exact callback URL owned
  by the hosted or desktop listener, and connection/policy generations are rechecked before
  credentials publish. Restart recovery never redispatches
  an ambiguous call: it reconciles a terminal governed action or records an
  uncertain outcome. Delayed results replay as native tool outputs only when
  their matching call remains in selected model context; otherwise they become
  explicitly untrusted messages so compaction cannot create an invalid provider
  call/output sequence. One intervention projection renders task gates,
  permissions, setup, and sign-in through the shared HumanInterventionCard in
  Chat, task detail, and Work Needs You.
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
- Task detail uses one chronological transcript with inline Planner, Executor,
  and Review runs, shared response/activity lanes, and one durable rendering of
  each executor submission. Its compact task card carries title, controls,
  optional needs-input state, and validation disclosure; metadata and validation
  details remain progressively disclosed.
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
  Returning to another surface uses the persistent navigation. Memory page rows
  reuse the same control, spacing, and nesting treatment as Tasks and Settings;
  their generated icons appear only in the rail while titles remain accessible.
- The shell is a viewport-bound application surface and the browser document
  does not own product scrolling. Chat uses one nested TanStack-virtualized
  transcript; Settings and Memory scroll inside their route surfaces; Tasks
  keeps independent list, sidebar, and detail scroll regions.
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

- Evaluate native-memory recall, citation accuracy, page churn, secret-copy behavior, and root growth before adding scopes, vectors, or editing.
- Add signing, notarization, updates, and production distribution after the unsigned developer desktop build is stable.
- Continue simplifying Work, Store, Runtime, Providers, and their test fixtures under measured
  net-negative slices. Do not start another repository-wide horizontal rewrite.

## Codex Workflow

- Keep workflow phases distinct, preserve unrelated changes, work on main, commit units, and push only when asked.
- Use `docs/development/simplicity.md` for budgets, size reporting, tests, and review severity.
- Prefer one vertical implementer and one read-only review unless the work has independent slices.
- Treat raw `~/.codex/sessions` as private; read only when requested and never quote without permission.

## Validation Defaults

- Rust: `cargo fmt --all --check`, `cargo check-workspace`, `cargo gate-lint`, and `cargo gate-test`; use `cargo validate <cargo-command> [arguments]` for focused commands.
- Frontend: run `bun run gen:types`, `bun run lint`, and `bun run build` from `apps/web`; do not add frontend tests or inspect in-browser unless requested.
- Before commit or push, check status and diffs, then inspect the staged stat and name-status.
