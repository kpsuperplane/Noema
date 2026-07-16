# Current Noema Context

This file is the durable working brief for Codex sessions. Keep it concise and update it when project direction, workflow preferences, or open loops change.

## Active Direction

Noema is an always-on, self-hosted personal agent operating system. SQLite is
Noema's canonical structured store at
`${NOEMA_HOME:-$HOME/.noema}/db/noema.sqlite3`. Local Mnemosyne owns durable memory
truth, extraction, updates, and memory search indexes.

The next storage slice should stay small and concrete:

- Local Noema home and config.
- Codex-backed chat through the daemon using Noema-owned OAuth tokens and direct
  Codex Responses API calls.
- First-party local GGUF chat through a pinned, packaged llama.cpp runtime, with
  local setup recommended before cloud-provider setup.
- Core-hosted local React web chat as the first frontend shell.
- SQLite-backed persisted conversations, transcript items, provider accounts,
  MCP setup, approvals, and memory service configuration.
- Local Mnemosyne-backed memory search through explicit `search_memory`.
- Frontend IA that keeps memory configuration in Settings > Memory and exposes
  human memories through the top-level `/memory` page.

## Settled Decisions

- SQLite is the target canonical structured store for the always-on personal
  server. The Noema server process is the only process that opens the SQLite
  database; desktop, web, and future mobile clients use Noema APIs.
- The clean pre-V1 storage reset has no SurrealDB migration path.
- SQLite database files live under
  `${NOEMA_HOME:-$HOME/.noema}/db/noema.sqlite3`.
- Local Mnemosyne state lives under `${NOEMA_HOME:-$HOME/.noema}/mnemosyne/data`; runtime
  sidecar state lives under `${NOEMA_HOME:-$HOME/.noema}/mnemosyne/run`.
- Persisted user messages enqueue Mnemosyne memory observations immediately after
  the user item is durably stored. The Mnemosyne add payload uses the current
  user text as the authoritative `role: user` source observation and may include
  bounded prior `role: assistant` context from assistant messages since the
  previous user message, so short replies like "cars" can be interpreted from
  conversational context without treating assistant text as a memory source. The
  request uses `user_id: human:local`, `agent_id: agent:local`, `run_id` set to
  the Noema conversation id, and Noema provenance metadata
  (`noemaConversationId`, `turnId`, `userItemId`, `sourceKind`,
  `sourceObservation`). Assistant responses, tool results, reasoning, notices,
  and empty user text are not independently submitted. Mnemosyne extraction never
  blocks normal provider response generation.
  Noema does not keep a SQLite memory ingest job/outbox table yet; submit
  failures are best-effort diagnostics until a real retry surface exists.
- Managed local Mnemosyne runs as a private Noema sidecar. The sidecar embeds Mnemosyne
  OSS behind a Noema-owned FastAPI contract, stores memory and indexes in
  Mnemosyne's local SQLite/sqlite-vec database, uses FastEmbed local embeddings,
  enables Mnemosyne's enhanced/polyphonic recall paths, and binds a
  runtime-selected loopback port. `cargo dev` prepares a repo-local Python 3.10+
  virtualenv under
  `crates/noema-core/target/mnemosyne-sidecar-venv` and passes
  `NOEMA_MNEMOSYNE_SIDECAR_COMMAND` to the watched server process. The endpoint is
  kept in memory and is not stored in SQLite or shown in Settings.
- Managed Mnemosyne receives model access through a Noema-hosted private loopback
  OpenAI-compatible `/v1/chat/completions` proxy. Noema injects
  `NOEMA_MEMORY_OPENAI_BASE_URL`, `NOEMA_MEMORY_OPENAI_API_KEY`, and
  `NOEMA_MEMORY_MODEL` into the child process at startup; the proxy routes
  generation through the provider/model selected in Settings > Memory, falling
  back to the daemon default only when no Memory model preference is saved.
- The top-level `/memory` page presents Mnemosyne-backed human memories as a
  Wikipedia-like personal memory article: Noema Core lazily asks the configured
  runtime model to write Markdown from Mnemosyne facts when the page is visited,
  caches that Markdown in SQLite by `human:local` fact fingerprint, automatically
  refreshes changed facts at most every 4 hours, and exposes a manual regenerate
  mutation for explicit refreshes. Generated prose carries stable fact citation
  keys; the web article renders them as numbered inline footnotes, orders the
  References list by first use, and keeps source messages, provenance, and
  derived memories in an accessible citation popover. Legacy uncited cache rows
  are regenerated, and invalid model citations fall back to deterministic cited
  prose. The muted lead figure summarizes loaded memory themes, while stubbed
  action, history, and recall controls should stay hidden until real backend
  operations exist. The web UI queries Noema Core GraphQL only; Noema Core
  resolves the configured Mnemosyne endpoint, fetches memories for `human:local`,
  and adapts them into grouped memory documents for the frontend. The browser
  never connects directly to Mnemosyne.
- Docker/Compose development infrastructure has been retired; local development
  uses host Rust, Bun, and web/desktop product surfaces. The old standalone
  Noema binary and local dev alias have been removed.
- First-run onboarding leads with a first-party local-model path and treats a
  ready local model or an authenticated cloud provider as alternative ways to
  enter chat.
- First-party GGUF inference uses the built-in `local_models` provider over a
  supervised loopback-only llama.cpp server. The bundled catalog lives in
  `crates/noema-core/resources/local-models/catalog.toml`; it contains only
  curated model/build data and generic RAM/VRAM/backend thresholds. Selection
  orders fitting models by priority and catalog order, then chooses the best
  available backend build for that model. Gemma 4 E4B IT is the only model to
  pass the current 12-case qualification suite (priority 100) and is recommended
  on Metal machines with at least 16 GB of unified memory. Smaller or non-Metal
  machines receive no curated recommendation until another model passes the
  same qualification bar.
- Local-model installation state, provenance, progress, errors, events, and
  default references live in SQLite. Verified content-addressed GGUF files live
  under `${NOEMA_HOME}/models/blobs/`, resumable transfers under
  `${NOEMA_HOME}/models/downloads/`, and advanced public-Hugging-Face imports
  require an immutable commit plus SHA-256. Initial activation atomically assigns
  the installed model to the primary agent, task executors/reviewer, all task
  tiers, memory, progress audits, and web summarization. Adding cloud accounts
  later does not rewrite those explicit selections.
- Local-model onboarding keeps the recommended installation visible while it is
  queued, downloading, or verifying, serializes each installation/import worker,
  and reports ready only after the active llama.cpp supervisor is healthy.
  Existing content-addressed blobs are rehashed before reuse, worker state
  transitions cannot regress cancelled or installed records, and advanced local
  imports must contain a GGUF header in addition to using a `.gguf` filename.
  Settings model selectors derive local profiles from these installation rows:
  the active GGUF is assignable, while other installed GGUFs remain visible but
  disabled until activated in Settings > Local models.
- The local provider constrains llama.cpp Chat Completions with Noema's shared
  strict response schema and request-specific tool payload schemas. Built-in
  and MCP tools now share one typed request catalog and one transport mode;
  memory is an ordinary catalog entry rather than a separate fallback prompt
  path. The local response grammar is derived from the exact request catalog,
  and consecutive kernel/developer context is coalesced into one leading system
  message before llama.cpp template rendering so later identity and tool updates
  are not discarded. Task executors and reviewers must finish through their
  role-specific terminal contract; local task model snapshots are valid
  first-class selections. Local chat-template thinking is disabled until Noema
  exposes an explicit reasoning policy, so reasoning-first GGUFs cannot consume
  the complete visible output budget internally. The adapter requests prompt
  reuse and the supervisor gives llama.cpp a bounded checkpoint cache equal to
  one thirty-second of detected system RAM, capped at 2 GiB; that is 512 MiB on
  16 GB machines and 1 GiB on 32 GB machines, with caching disabled only when
  hardware detection fails. One priority-aware generation arbiter still keeps a
  single active request, but queued primary-conversation work runs before queued
  memory, task, audit, background-compaction, and task-originated web-summary
  work. A turn's memory observation also waits until that foreground turn ends,
  avoiding deterministic cache eviction and latency contention. The llama.cpp
  schema lowering removes large string length bounds that otherwise expand task
  payload grammars beyond llama-server's parser; canonical runtime handlers
  still validate those tool payloads.
- The opt-in `noema-model-evals` runner qualifies local GGUF candidates against
  production model-sensitive contracts without changing the user's installed
  models or preferences. Its pinned candidate and suite inputs live under
  `evals/local-models/`; downloads are resumable, checksum-verified, and shared
  under `target/noema-model-evals/cache`, while every model runs sequentially in
  an isolated worker and emits incremental JSON/Markdown reports. The suite
  deterministically checks strict chat, streaming, multiple choice, memory tool
  selection and continuation, executor/reviewer/blocked task terminals,
  progress audit JSON, web-summary injection resistance, and compaction. Runtime
  compatibility, correctness, and latency remain separate results. A separate
  `soak` mode keeps those 11 correctness gates unchanged, then runs a calibrated
  near-context request and 20 distinct turns while recording resident-set
  stability. Bonsai 27B
  experiments must use the official `Q2_g64` artifact with stock b10015; its
  smaller g128 `Q2_0` artifact requires Prism's fork and fails the pinned
  upstream runtime.
- The first 2026-07-15 Apple M5/32 GB qualification snapshot remains at
  `evals/local-models/results/m5-air-2026-07-15.md`; the bounded-cache 16 GB tier
  follow-up is at
  `evals/local-models/results/m5-air-16gb-tier-2026-07-15.md`. NVIDIA Nemotron 3
  Nano 4B Q4_K_M is the measured 16 GB default candidate: after a generic
  reviewer-prompt restructure it passed all 11 contracts in three isolated
  workers and again during a resource soak, with a 3.37 GiB peak server RSS,
  0.61-0.62-second load, and 7.14-8.67-second qualification medians. The soak
  processed 6,517 input tokens and 20 follow-up turns with flat post-turn RSS.
  Gemma 4 12B also passed 11/11 but peaked at 9.08 GiB and belongs in an advanced
  tier. These are projected 16 GB fits measured on the 32 GB Air; a physical
  16 GB acceptance run and Nemotron license review remain catalog-promotion
  gates, so the snapshot does not automatically replace Ternary Bonsai.
- The post-unification 32 GB follow-up is at
  `evals/local-models/results/m5-air-32gb-unified-tools-2026-07-15.md`. Gemma 4
  E4B IT Q4_K_M passed all 11 contracts in three isolated workers and again in a
  resource soak. The soak processed 6,513 input tokens, completed 20 follow-up
  turns, and held post-turn RSS within 16 KiB; peak server RSS was 5.51 GiB.
  Gemma E4B is the measured best-qualified 16 GB and 32 GB default and is ranked
  first in the bundled catalog. A final installed-blob rerun in real `~/.noema`
  kept E4B at 11/11, while Gemma 26B and 12B scored 10/11 and Ternary Bonsai 8B
  scored 9/11. Those three are excluded together with Nemotron and Qwen, whose
  historical 11/11 reports copied the hostile web instruction that the corrected
  grader rejects. The final installation/runtime validation is at
  `evals/local-models/results/m5-air-live-noema-2026-07-15.md`.
- The qualification suite now includes agent onboarding/name persistence as a
  twelfth critical case. The local adapter strips regex `pattern` constraints
  when lowering canonical tool schemas for llama.cpp because b10015 rejects
  valid expressions such as `\S` while initializing its grammar; runtime tool
  handlers still validate the canonical schema. E4B passed the expanded suite
  12/12 and emitted `update_own_name` with the requested `Momo` payload.
- Runtime-host startup now eagerly starts an active installed local model before
  serving clients. A failed llama-server launch leaves Noema available, exposes
  the existing failed/retry state, and records a `local_model_runtime_unavailable`
  system error instead of presenting every healthy restart as inactive.
- The daemon shares one local-model supervisor between provider dispatch and
  runtime status. Activating a local model hot-swaps the managed Mnemosyne model
  proxy route without restarting the sidecar, and memory requests use a
  generation-sized timeout rather than the old three-second health-check bound.
- The standalone debug web shell passes the prepared
  `crates/noema-desktop/binaries/runtime` resource root into the shared runtime
  host, matching the desktop shell instead of looking beside the dev server
  executable for `llama-server`.
- Desktop builds package pinned llama.cpp release `b10015` at commit
  `12127defda4f41b7679cb2477a4b0d65ee6a0c8f`. The build preparation script
  downloads the platform archives, verifies the bundled manifest hashes, keeps
  `llama-server` with its required adjacent libraries, and bundles Metal for
  macOS, CUDA/Vulkan/CPU for Windows, and Vulkan/CPU for Linux. Runtime lookup
  never trusts ambient `PATH`; debug builds may use the explicit
  `NOEMA_LLAMA_SERVER_PATH` override.
- Provider credential/session material lives under
  `${NOEMA_HOME:-$HOME/.noema}/providers/<provider>/<account>/`; the structured
  store keeps only non-secret provider metadata.
- Codex provider account homes contain Noema-owned `codex_tokens.json` OAuth
  state. They are not `CODEX_HOME` directories, and Noema does not silently
  import Codex CLI `auth.json` files.
- Codex model/profile catalogs are fetched from the provider `/models` endpoint
  after a six-hour TTL. The `client_version` query value is resolved from the
  official `@openai/codex` npm `latest` metadata endpoint, with the last cached
  value or `0.144.0` as an offline fallback. Successful
  catalogs persist the resolved client version and refresh timestamp; failed
  refreshes preserve the previous catalog, while the metadata schema version
  still forces refreshes after parser changes. Codex Responses calls resolve
  that same current client version once per runtime and send the Codex client
  identity headers plus the selected ChatGPT workspace extracted from the OAuth
  access token; model availability on the subscription backend is scoped by
  those request fields.
- Concrete object records are the canonical structured state; actor/principal,
  governable scope, provenance source, and transcript item are interfaces
  implemented by concrete objects rather than universal parent tables.
- Durable chat history is reconstructed from `conversation_items`; the daemon
  WebSocket and `agent_status` are live coordination state for current turns.
- Browser GraphQL connections and terminally failed subscription operations
  retry indefinitely with capped backoff. A disconnected chat becomes read-only
  while reconnecting; once a fresh subscription is acknowledged, the web client
  refetches active queries and backfills the latest durable transcript page so
  server restarts do not require a reload.
- Filesystem storage is for durable object-owned documents, attachments, and artifacts.
- `system/` state is derived and rebuildable.
- Governed artifacts are durable, versioned outputs owned by the same concrete
  contexts as memory governance. SQLite owns artifact metadata and immutable
  version records; local bytes live under the owner filesystem area and
  external resources are represented as URL versions. The first product slice
  creates conversation-owned artifacts, exposes artifact reads through GraphQL,
  serves local file versions through read-only download URLs, and renders typed
  artifact references in the transcript. The canonical store validates external
  artifact URLs as HTTP(S), and local artifact file writes/downloads reject
  symlinked artifact paths.
- The primary agent can create conversation-owned local file artifacts through
  the first-party `artifact.create_local_file` tool. The tool accepts complete
  text versions from the model, writes local bytes through Noema's governed
  artifact helpers, appends immutable version metadata, returns version
  download URLs in the tool result, and persists a transcript artifact reference
  for the current version. GraphQL may verify/read the resulting artifact, but
  agent-created local artifacts should go through this runtime tool path rather
  than direct artifact mutations.
- Local transcript artifact cards open a chat-owned generic detail rail with
  `type=artifact&version=<artifact_version_id>` state held inside the chat root.
  Markdown and plain-text local file versions render inline through GraphQL
  `artifactVersionDetail`; download is a secondary action in the rail, and there
  is intentionally no artifact URL route in this slice.
- The first one-off background task slice is durable and supervised. The primary
  agent's `task.delegate` tool validates one of the three global
  simple/medium/difficult executor model settings, persists the request and immutable validation
  criteria, and queues an executor run. The host worker supervises up to eight
  independently leased task runs concurrently while the store prevents active
  run overlap within one task. Each run executes role-gated native tools,
  records canonical agent/tool transcripts and
  cumulative usage, creates a typed submission, queues an adversarial reviewer,
  and either requests revision, pauses for human input, or approves the task.
  Task status is delivered idempotently as a structured conversation event, and
  terminal outcomes also enter a durable completion outbox. The primary runtime
  serializes each completion report behind any active foreground turn, invokes
  the primary agent with tools disabled, and persists one event-derived
  `assistant_text` item with a deterministic id; provider failures fall back to
  a deterministic report so approved work is still visible after restart.
  Reviewer configuration is stored through the normal agent model preference surface for
  `agent:task-reviewer`; executor selection is snapshotted per task.
  Task executors share exactly three global model settings: one each for simple,
  medium, and the highest-complexity work. The configured provider supplies the
  initial values, while Settings edits persist as user overrides. Codex defaults to
  GPT-5.6-Luna/medium for simple work, GPT-5.6-Luna/xhigh (shown as max) for
  medium work, and GPT-5.6-Sol/high for the highest tier. Only enabled entries
  backed by an authenticated provider account are advertised to the primary.
  Settings nests these three model controls inside the Task Executor agent card;
  the executor identity exists for run ownership and audit but has no independent
  agent model preference. Complexity selects only the model/reasoning tier. All
  task runs use the same provider-independent execution policy, currently 80
  provider continuations, 400 tool calls, 120 active minutes, and a progress
  audit every 20 steps; each run snapshots that policy. Delegation, revision,
  and continuation revalidate authenticated provider accounts and reject exact
  model profiles that are absent from an available provider catalog before a
  run is queued.
- Task references are first-class transcript items. The web chat renders a
  marker and opens a GraphQL-backed detail rail showing the request, criteria,
  revisions, executor/reviewer runs, model snapshots, submissions, reviews, and
  final result. The detail rail subscribes to cursor-bearing GraphQL task and
  run-item events rather than polling, upserts streaming items, and pages older
  transcript rows directly from SQLite. Background roles reuse the primary
  runtime's prompt, provider continuation, progress audit, repetition detection,
  tool dispatch, and finalization primitives while replacing interactive user
  input with typed terminal contracts: `task.submit_result`,
  `task.submit_review`, and `task.report_blocked`. Response-capable providers
  continue foreground and background tool rounds with `previous_response_id`
  plus only the latest tool outputs, while Noema retains the complete ordered
  context locally for audit, compaction, and automatic stateless replay when a
  provider does not support chaining or rejects a retained response id.
  Revision executors receive the exact prior submission and reviewer verdict,
  criterion evidence, and feedback, while reviewer inspection targets the
  executor run linked to the submission instead of the reviewer run itself.
  Lease heartbeats and cancellation interrupt active provider/tool futures;
  expired leases have bounded automatic recovery.
  Human-blocked and failed tasks resume only as linked child runs with reconstructed
  lineage and an optional answer—there is no fresh retry path. A continuation
  after a committed reviewer verdict starts a new executor revision linked to
  that review; only a reviewer blocked before committing a verdict resumes as a
  reviewer. The worker also rejects any execution path that returns while its
  run remains active, so terminal persistence bugs fail immediately instead of
  idling until lease recovery. Agents can inspect, resume, and cancel owner-scoped
  tasks through canonical task tools, and the UI offers the same
  continue/answer/cancel controls. Terminal task events form a
  durable delivery outbox, and deterministic conversation item ids make crash
  recovery idempotent. Transcript persistence redacts secret-shaped fields and
  all opaque MCP arguments/results while retaining full payloads only in the
  live provider continuation. The task runtime exposes read-only search, fetch,
  memory, task inspection, and calibrated MCP tools to background roles.
  Executors can also create multiple task-owned local artifacts through the
  governed artifact tool and attach their ids to `task.submit_result`; submission
  rows snapshot linked versions and reject missing, duplicate, or foreign
  artifacts. Reviewers receive the manifest and can read only linked bounded
  UTF-8 content through `task.read_artifact`. Approved artifacts appear in task
  detail and are delivered as artifact-reference cards beside the primary
  agent's completion update in the originating conversation. Reviewer prompts
  also reconstruct question-and-answer guidance from the task's causal run
  history through the submitted executor and treat human answers as authoritative
  clarifications that may refine the original request or validation criteria. Richer
  workspace/project orchestration remains a later milestone.
- Task run transcripts reuse the shared chat `Transcript` renderer and scroller.
  The task adapter maps run items into the common assistant/activity entry model,
  uses an embedded density without mounting a composer, so task conversations
  inherit the main chat's Markdown, tool markers, live-arrival behavior,
  pagination, and accessibility infrastructure. Persisted tool calls and results
  pair into the normal chat tool marker. Provider request payloads, reconstructed
  continuation context, and semantic compaction checkpoints remain runtime
  implementation details rather than run transcript rows, matching main chat's
  canonical event persistence. All transcript text bubbles cap
  their inline preview at roughly six lines; overflowing content fades out and
  opens the complete rendered message in a dialog when activated.
- Memory is governed context, not hidden model state. Durable memory truth now
  belongs to local Mnemosyne, while Noema owns service lifecycle, configuration,
  live readiness proxying, provenance, UI, model routing, ingest diagnostics,
  and explicit `search_memory` tool calls.
- Chat provider responses use an explicit object contract:
  `response_status`, `responses[]`, and `tool_calls[]`. The legacy
  `memory_proposals` field is rejected at the provider parser boundary.
  `responses[]` may be empty only for `response_status: "needs_tools"` with
  one or more tool calls, so models can run routine single or multiple tools
  without filler commentary.
- Multiple-choice agent responses are first-class transcript primitives,
  separate from A2UI cards. Providers can emit `multiple_choice` final response
  items with `pick_one` or `pick_many` modes, Noema persists assistant
  `multiple_choice_prompt` items and human `multiple_choice_selection` items,
  and the web transcript submits typed selections through GraphQL instead of
  converting option picks into plain user text.
- The primary agent's ordinary chat voice should default to informal
  human-texting brevity, including lowercase short replies when context allows.
  In casual mode, lowercase sentence starts should stay consistent across
  split response items; capitalization is reserved for names, acronyms, code,
  commands, headings, quoted text, dates, paths, tool names, high-stakes topics,
  polished deliverables, and clarity/respect. Casual bubbles should skip final
  periods while keeping question marks or exclamation points when useful.
  Casual chat should add light cheer often, including casual exclamation marks,
  occasional emoji, and word elongation, without forcing any of them; small wins
  can start with a tiny cheer, and playful metaphors are okay when they do not
  replace the answer. Casual option-picking and recommendations should lead
  with the pick in short bubbles rather than drifting into review-column or
  consultant prose.
- Casual assistant replies may use multiple `responses[]` text items as
  separate chat bubbles. Provider streaming, GraphQL, and the web transcript use
  per-response-item stream ids so split replies stream and finalize without
  collapsing into one bubble. Prompt guidance now explicitly keeps split chat
  bubbles inside one `responses[]` array in a single JSON envelope, and the
  required-response parser tolerates exact duplicate envelopes as idempotent
  stream/provider duplication while still rejecting conflicting multiple
  envelopes. The live structured-response delta extractor also stops emitting
  visible assistant deltas after the first completed top-level envelope so the
  web UI does not temporarily show duplicate streamed bubbles before replay.
  The web transcript also carries assistant `responseIndex` and uses
  `(turnId,responseIndex)` as a fallback replacement key so finalized assistant
  items remove their ephemeral stream bubbles even when stream metadata is
  missing or mismatched. Finalized assistant messages that came from a stream
  keep the stream id as their render identity so React updates the streamed
  bubble in place instead of remounting it and replaying the text animation.
- The provider-neutral native tool plane has landed from
  `docs/superpowers/specs/2026-07-04-native-tool-plane-design.md`: Noema now
  builds canonical local/MCP tool specs, advertises them through provider-native
  channels for OpenAI/Codex Responses providers, keeps Foundation Local on the
  audited builtin-only JSON fallback, parses native `function_call` items into
  canonical tool calls, and feeds local tool results back through stateless
  native `function_call`/`function_call_output` continuation input with
  provider call-id correlation preserved. OpenAI/Codex native call parsing
  rejects malformed/non-object arguments, provider-safe name collisions, mixed
  native plus legacy JSON tool calls, missing native `call_id`s, and
  `final_answer` text in tool-waiting responses.
- Durable model context now includes typed tool call and tool result history
  after the active context checkpoint. OpenAI/Codex adapters replay that history
  as native Responses `function_call`/`function_call_output` input items, while
  Foundation Local receives a conservative assistant-text replay fallback.
- The first web-search slice is a first-party `web.search` native tool, not an
  MCP: the only initial provider is a Rust-only best-effort DuckDuckGo public
  adapter, model-proposed queries are visible in normal chat markers, and
  results return as normalized search metadata without fetching pages. Routine
  first-party read-only web tools such as `web.search` and public `web.fetch`
  should not be permission-gated when the user asks for current information or
  the task clearly needs them; private, write/export, expensive, irreversible,
  and major actions still require confirmation or policy approval.
- The first web-fetch slice is a first-party `web.fetch` native tool, not an
  MCP: the initial provider directly fetches public HTTP(S) pages with strict
  URL/redirect/private-address policy checks, extracts readable article content
  through `readabilityrs` markdown output, summarizes large pages through a
  configurable auxiliary model preference, and shows compact normal chat
  markers. The summarizer defaults to the Codex/OpenAI tool-classification
  default model (`gpt-5.4-mini`) when no web UI preference is saved. End-to-end
  verification covered the running local agent at `:3737` calling `web.fetch`
  against `https://www.rust-lang.org/`.
- Providers should be understood broadly as service integrations that can
  supply one or more Noema capabilities, not only as model vendors. A provider
  account may supply `model.generate`, `model.classify`, `web.search`,
  `web.fetch`, or future capabilities such as `web.crawl`; stable model-visible
  tools such as `web.search` and `web.fetch` stay provider-neutral, with
  Noema-owned bindings selecting the backend capability. Exa is implemented as
  a user-created `secret_input` provider account that supplies `web.search` and
  `web.fetch`; API keys are stored only under the provider account home and are
  never read from environment variables or returned through GraphQL. OpenAI
  hosted web search is a future `web.search` provider option, while Codex
  product-native web search should not be treated as a backend until there is a
  documented callable provider API. Firecrawl is only an illustrative future
  provider example, not planned implementation in the current slice.
- The provider capabilities implementation plan lives at
  `docs/superpowers/plans/2026-07-07-provider-capabilities.md`; Task 10
  surfaces provider fallback metadata in transcript displays for first-party
  web tools without exposing raw provider payload content.
- Provider tool continuations now use a progress-audited continuation policy:
  the runtime allows longer same-turn tool chains, builds a bounded progress
  digest instead of sending raw tool history to the audit model, surfaces
  visible progress-check activity markers every 20 continuation steps, and
  gives the agent one no-tools finalization attempt when the hard ceiling or
  audit execution fails. The auxiliary audit model preference is configurable;
  Codex/OpenAI default to `gpt-5.4-mini`, while Foundation Local must use a
  provider-native profile. The model picker lives in Settings > Safety > Usage
  at `/settings/safety/usage`.
- Derived search/vector indexes are rebuildable projections.
- Graph or fuzzy retrieval can suggest candidates, but policy gates inclusion.
- Pre-stable schema changes do not need migrations or backwards compatibility unless explicitly requested.
- The initial frontend should start with chat, memory, and inspection before exposing full workspaces, tasks, agents, tools, or governance.
- The first-party product API direction is GraphQL, with Apollo Client on the
  React web frontend and backend-exported schema/types feeding frontend codegen.
- GraphQL is the first-party client API for Noema web, desktop, and future
  mobile clients. Internal Rust modules continue to use command,
  runtime, repository, policy, provenance, audit, and event interfaces directly.
- The current web UI consumes GraphQL over `/graphql` plus
  `graphql-transport-ws` subscriptions over `/graphql/ws`.
- The standalone web daemon binds only a numeric loopback address and derives
  its exact authority from the bound listener. Axum owns HTTP and WebSocket
  transport; every request must present the exact Host, GraphQL POST/WS also
  require the exact Origin, and the handwritten parser/framer no longer exists.
- Web startup prints one random one-shot bootstrap URL. Consuming it creates a
  private, HttpOnly, SameSite=Strict in-memory `tower-sessions` session. GraphQL,
  subscriptions, schema/GraphiQL, and artifact downloads require that session;
  assets and the state/PKCE-authenticated MCP OAuth callback remain public after
  Host validation. OAuth callback URLs come from listener authority, never Host.
  The `cargo dev` supervisor explicitly enables the debug-only `dev-no-auth`
  feature and binds to `0.0.0.0` for trusted-LAN development; it relaxes Host
  and Origin checks alongside the unauthenticated session mode. Direct daemon
  and release builds remain loopback-only and retain this session boundary.
- `cargo dev` builds its Rust watcher under `target/noema-dev`, separate from
  the validation target, so live reload does not contend with checks or tests.
  The supervisor reuses the editable Mnemosyne install until its `pyproject.toml`
  changes. `cargo fast`, the focused lint/test pair, and the gate lint/test pair
  provide package-scoped, unit-level, and full-workspace validation tiers respectively.
- Authenticated web and desktop GraphQL operations receive the server-derived
  `human:local` request principal. Desktop commands additionally require the
  Tauri `main` window. Artifact downloads authorize the version's live
  conversation ownership to that human before opening local bytes.
- Each runtime host builds one GraphQL schema and reuses it across operations.
- The first macOS desktop app direction is a Tauri app in
  `crates/noema-desktop` that starts a Noema runtime host inside the app
  process, loads the existing React UI from bundled assets, and uses Tauri
  IPC/events for GraphQL instead of exposing a local HTTP/WebSocket server.
- The first Apple Foundation Models provider direction is `foundation_local`:
  a macOS-only Swift bridge owned by the Rust daemon process, with live
  stateful `LanguageModelSession`s while the daemon runs and restart resume by
  replaying Noema-owned persisted transcript state. Linux and Windows builds
  must continue to compile and deploy without Foundation Models support, showing
  the provider as unavailable rather than making Apple tooling a global
  dependency. Provider Settings should show backend/account availability, while
  the web Settings Agents surface should own per-agent provider and
  model/profile selection. The portable Rust backend now includes
  `foundation_local` config/provider metadata, default provider-account seeding,
  agent runtime preferences, GraphQL read/write APIs, provider bridge protocol
  and lifecycle stubs, and runtime model/profile preference resolution; the
  daemon now resolves the saved agent provider preference at conversation/turn
  time instead of requiring a matching startup provider or daemon restart. The
  Swift bridge and web controls remain separately owned implementation lanes.
- The old Noema CLI and raw daemon Unix-socket protocol have been removed.
  Chat product traffic remains on GraphQL mutations/subscriptions for web and
  desktop surfaces.
- The transitional product web endpoints and old frontend protocol/type export
  surfaces have been retired; GraphQL is now the only client-facing product
  API.
- The web home chat should load `human:local.primary_conversation_id`; Noema
  conversation continuity is owned by Noema structured state, not provider
  runtime state.
- The web chat transcript loads durable history through paged GraphQL replay:
  `primaryConversation` reads identity, `ensurePrimaryConversation` creates the
  rare missing primary conversation, `conversationTranscriptPage` returns
  cursor-based visible item windows, and the frontend renders loaded transcript
  rows through TanStack Virtual while keeping live updates on
  `conversationEvents`.
- Frontend build and lint use Bun from `crates/noema-core/web`.
- Web GraphQL schema and operation types are generated with `bun run gen:types`.
  TanStack Router file routes are generated with `bun run gen:routes`, and the
  normal dev/build/lint scripts run both generators before Vite or TypeScript.
- The web UI uses Astryx as its component foundation, with a Noema-owned
  Neutral-derived theme and StyleX for Noema-specific layout and state styling.
  Noema-owned shell and domain components remain responsible for chat, memory,
  provenance, approvals, tools, runs, settings, and object detail semantics.
  shadcn/Base UI/Tailwind are no longer part of the frontend foundation.
- Production frontend changes should translate relevant brand intent into the
  active Astryx/StyleX patterns in `crates/noema-core/web` rather than copying
  design-kit demo components or CDN assumptions. (The former `design/` prototype
  kit and `mocks/` static IA prototype have been removed as dead scaffolding.)
- The route-derived L0 to L1 Settings navigation has landed from
  `docs/superpowers/specs/2026-06-30-route-derived-shell-settings-design.md`
  and
  `docs/superpowers/plans/2026-06-30-route-derived-shell-settings.md`.
- The web shell uses a layered sidebar deck: one persistent `ShellSidebar`
  ground layer sits under the route content deck. Expanded desktop keeps the
  sidebar visible; collapsed desktop and mobile reveal navigation by moving the
  deck aside rather than rendering a separate drawer/sidebar copy.
- The web shell exposes Settings as a bottom-anchored L0 sidebar item that
  opens a route-derived L1 Settings submenu inside the same shell. `/settings`
  and `/settings/agents` default to Agents. Settings now uses grouped live
  sections: Agents; Memory; Tools with Web and MCPs; Safety with Usage; and
  System with Local Models and Providers. The Local Models page remains at
  `/settings/models`. The Web page owns first-party
  `web.search` and `web.fetch` status plus the fetch summarizer model
  preference. Settings routes are canonical nested paths such as
  `/settings/tools/web`, `/settings/safety/usage`, and
  `/settings/system/providers`; old flat settings paths are not supported.
  The placeholder Audit settings surface has been removed until audit event
  persistence lands. Agent management actions are not exposed yet.
- Frontend docs now distinguish currently addressable routes from target
  surfaces: TanStack Router owns `/`, `/settings`, and
  `/settings/{agents,models,memory,tools/web,tools/mcps,safety/usage,system/providers}`.
  `/memory` redirects to `/settings/memory`; `/memory/graph` is not a current
  route.
  `routes.ts` remains a shell route mapping helper for breadcrumbs, sidebar
  selection, and canonical path generation. Older `/setup`, `/chat/:id`, memory
  detail/review, and `/inspect` paths are target routes until product routing
  implements them.
- The web UI uses TanStack Router file-based routes with Vite automatic route
  code splitting. Chat and the shell stay in the initial bundle; settings
  surfaces load through route chunks. The daemon web asset resolver
  serves emitted `/assets/*.js`, `.css`, `.svg`, and `.html` files from
  `target/web-assets` in debug and embeds the generated asset table for release
  builds, so route chunks work in the Rust-served web UI as well as the Tauri
  asset bundle.
- The first third-party MCP control-plane slice has landed. Third-party MCPs
  route through a Noema-owned Capability Gateway rather than raw model tool
  handles. MCP setup is a mandatory metadata-only calibration flow: tools get
  reviewed `read`/`write`/`export` classifications of `none`, `trusted`,
  `untrusted`, or `mixed`; ready tools must expose at least one non-`none`
  axis; and `mixed` ready tools require owner extractors. Runtime enforcement
  is currently limited to server enabled/health/authentication state, reviewed
  metadata fingerprint freshness, and ready calibration status. Read-result
  quarantine, export approvals, and deeper owner-trust enforcement remain the
  next MCP gateway policy slice rather than shipped behavior. Web Settings can
  now add MCP servers
  through a guided modal setup flow that does not persist the server until
  authentication is complete and metadata discovery succeeds. Successful setup
  stores secrets under `${NOEMA_HOME}/mcp/`, verifies metadata-only
  connectivity through concrete stdio, Streamable HTTP, and legacy SSE MCP
  transports, fetches tool schemas, and automatically opens a separate
  tool-permissions modal while keeping discovered tools disabled until
  calibrated. The permissions modal
  now shows discovered schemas and owner extractor setup, and blocks impossible
  `ready` saves before they hit the backend. MCP tool calibrations are exposed
  to all agents and scopes until a richer visibility model lands. Existing MCP
  rows can reopen the permissions modal or delete the
  server plus stored setup secrets through an in-app destructive confirmation.
  Authentication-required setup results move to their own modal screen with a
  Back affordance so the initial server-detail form and credential retry form do
  not stack. HTTP auth-required setup can now offer OAuth client-secret
  credentials; the backend exchanges them through the MCP Rust SDK OAuth flow,
  injects a bearer token for metadata discovery, and stores the OAuth credential
  material only under the MCP secret directory. Hosted MCP OAuth without client
  credentials uses browser authorization through an in-memory setup attempt; the
  web daemon handles same-origin callbacks, while the Tauri desktop app exposes
  a runtime-owned localhost callback URL so redirects do not land on the Vite
  asset server. MCP tool calibration can now request LLM-generated Autofill
  suggestions from persisted tool metadata; suggestions populate frontend draft
  state only and require explicit Save before backend calibration records are
  written, with batch calibration saves, stale-value glimmers while suggestions
  are pending, compact TSV-style tool prompt rows, bounded description hints,
  compact model-facing classification codes, a configurable provider-default
  tool classification model (`gpt-5.4-mini` for Codex/OpenAI), and deterministic
  shallow owner extractor discovery from argument and structured-output schemas.
  Calibrated MCP tools are now advertised to the model only when their server is
  enabled, healthy, authenticated, and their reviewed metadata fingerprint still
  matches the discovered tool metadata; the Capability Gateway re-checks those
  conditions at execution time and calls calibrated tools through stdio,
  Streamable HTTP, or SSE transports. Saving at least one ready calibration
  enables the server for agent use. MCP `tools/call` failures now persist as
  failed tool results that are still fed back to the provider for same-turn
  recovery or explanation, while raw transport/tool diagnostics are written to
  `errors.log` under `mcp_tool_call_failure`. User-facing setup errors are
  sanitized while raw transport details stay out of the web form. Runtime MCP
  call failures now mark the server unhealthy in persisted setup state; auth
  shaped failures also mark the server as needing authentication. MCP Settings
  exposes a per-server reauthentication dialog that retries persisted setup
  discovery through the existing continue-setup path with replacement secrets or
  OAuth client credentials. Servers originally authenticated through hosted
  browser OAuth, such as Dex, are detected from persisted OAuth credential refs
  and restart the browser authorization flow for reauthentication instead of
  prompting for OAuth client ID/secret material. Browser OAuth credentials now
  persist token receipt time, refresh near expiry before tool calls, preserve
  refresh tokens when providers omit unchanged refresh material, and write
  refreshed credentials back to the MCP secret file.
  Web Settings also exposes MCPs, Trusted Identities, and Approvals surfaces
  backed by GraphQL read models where live data exists.
- Routed web surfaces learn shell-owned deck state through
  `ShellSurfaceContext` visibility (`visible`, `hiding`, `hidden`, `showing`).
  Surfaces should run focus and other visible-only side effects only when
  visibility is `visible`; the shell owns transition settling.
- Noema-owned React product components should live one component per file.
  Pure helper/model logic belongs in `.ts` files, shared type files/types and
  nearby tests may live in component folders. Shared local UI components should
  express Noema domain semantics and use Astryx/StyleX rather than generic
  compatibility wrappers for the retired shadcn/Base UI foundation.
- Frontend Noema-owned product components now follow the one-component-per-file
  rule, with transcript render/model helpers split from React components.
- GraphQL, daemon web transport, daemon runtime, SQLite store, and provider
  streaming parsers are split into focused modules while preserving existing
  product behavior.
- Core Rust source organization favors focused module trees over broad flat
  files: store MCP persistence, daemon memory work, config loading/resolution,
  conversation domain types, and MCP trusted identity helpers are split into
  nearby submodules with root files acting as stable facades.
- Memory service configuration lives in SQLite store modules, while live
  service status is queried through Noema core as a proxy to Mnemosyne and local
  Mnemosyne owns durable memory behavior. Provider-neutral contracts,
  account metadata, and auth support live under `provider`, while concrete
  adapters and response stream helpers live under `provider::adapters`; the old
  top-level `providers` module has been retired.
- `crates/noema-core/web/tests` has been removed; web validation should use
  `bun run lint`, `bun run build`, and local browser smoke checks.
- Agent memory reads are explicit `search_memory` tool-only calls. Noema
  validates arguments, maps trusted active scopes to Mnemosyne user/run filters
  where available, returns Mnemosyne search results as a normal tool result, and
  does not inject memories automatically before turns.
- The primary agent starts unnamed. Prompt construction includes an
  `onboarding_prompt` asking the model to ask the user for a name while the
  agent has no display name. The local `update_own_name` tool persists later
  naming or renaming only when the current user explicitly names or renames the
  agent. Intent is carried by the structured tool call and trusted runtime
  state rather than direct user-text matching. The agent identity prompt now
  also carries a priority-ordered onboarding agenda: name first, then learn what
  the user wants help with, which tools/connectors they want to use, useful
  user/project context, and preferred collaboration/proactivity style. The
  runtime feeds successful local tool results, and failed MCP gateway tool
  results, back into the same turn plus subsequent prompts.
- The local `search_memory` tool supports validated concrete `scope_ids`.
  Empty `query` is allowed only for scoped reads, and `query` narrows within
  scope rather than broadening it.
- Owner-facing memory visibility is available through the top-level `/memory`
  page as a native Mnemosyne-backed personal memory article. Transcript memory
  markers and `/remember` are removed for now.
- Provider tool continuations follow a bounded same-turn loop inspired by the
  OpenAI Codex turn runner: local tool results are fed back to the provider, a
  continuation may request another model-visible local or calibrated MCP tool,
  and the loop stops only when no tool result requires another provider
  continuation or the runtime limit is hit. Continuations hide the one-shot
  `update_own_name` tool while preserving normal `search_memory` and calibrated
  MCP tool use.
- Assistant text and runtime tool execution are distinct transcript concepts.
  Provider `assistant_text` items may carry `commentary` or `final_answer`
  phase metadata, while visible tool markers are emitted from Noema runtime
  execution start/result timing rather than from provider output array order.
  This keeps GPT 5.4 and GPT 5.5 provider ordering differences from changing
  whether a tool appears to have run before the assistant's pre-tool message.
  Provider-stream tool start signals are surfaced as transient activity rows
  and replaced by durable `noema_local` tool execution rows when the runtime
  actually starts the side effect.
- Memory reset implementation state: Noema no longer has local graph-claim
  tables, predicate proposal APIs, Noema-owned memory extraction, contradiction
  resolution, transcript memory markers, `/remember`, or `/memory/graph` in the
  current slice. Provider responses no longer include memory proposals.
  `/remember` is ordinary user text.
- Required Noema provider responses should be enforced as close to the provider
  boundary as the transport allows. The OpenAI and Codex Responses adapters send
  a `text.format` JSON schema for the `noema_response` envelope, while the
  parser still recovers from common non-strict text shapes: prose-wrapped single
  envelopes and plain assistant-text fallbacks.
  Duplicate structured envelopes remain malformed because they indicate stream
  assembly corruption.
- Local-human canonicalization is deterministic: explicit aliases are local;
  same-name Kevin is local for direct or first-person local assertions and
  non-local for named third-party evidence. Note fallback objects use opaque
  deterministic IDs plus punctuation-normalized dedupe and reinforcement, so
  note content and secrets are not embedded in entity IDs.
- GraphQL exposes `memorySettings`, `saveMemoryServiceSettings`,
  `checkMemoryService`, and `memoryGraph` for the current memory surface.
  Generated web GraphQL schema/types are kept in sync. Settings > Memory at
  `/settings/memory` owns Mnemosyne mode, external base URL, live status, and
  extraction model preference.
- Apple Foundation Models local-provider implementation is in active bridge
  integration shape: the Swift bridge lives under
  `crates/noema-core/apple-foundation-bridge`, Rust provider account/runtime
  plumbing records `foundation_local` as a provider kind, the web dashboard
  exposes per-agent provider/model selection, and the daemon resolves saved
  agent provider preference at conversation/turn time without restart. The
  Rust provider now resolves a daemon-owned default bridge path, can
  materialize the source-tree Swift bridge with `swift build` on macOS debug
  builds, launches the bridge over stdio, performs handshake/health checks,
  creates a FoundationModels `LanguageModelSession`, forwards generation to the
  Swift bridge, and uses a longer generation response timeout than control
  messages. `cargo dev` is the active local web supervisor and runs the web
  asset watcher, `noema_web` watcher, and macOS Swift bridge watcher when the
  bridge package is present.
  Unsupported Windows/Linux builds can still ship without the Swift bridge;
  Foundation Local remains unavailable there rather than blocking the rest of
  Noema. Primary chat continuity is owned by Noema's human primary conversation
  and no longer forks when the agent switches providers. A live GraphQL probe
  against the dev daemon on July 1, 2026 returned an assistant response through
  `foundation_local`. The context-window slice from
  `docs/superpowers/specs/2026-07-01-context-compaction-design.md` and
  `docs/superpowers/plans/2026-07-01-context-compaction.md` is implemented:
  Noema now persists durable `conversation_context_summaries`, advertises
  provider context metadata, counts or estimates prompt tokens, assembles prompts
  from the latest active summary plus all post-checkpoint text items, performs
  foreground compaction before over-limit turns, schedules background compaction
  after large turns, and reports foreground compaction failures through the
  existing chat `ErrorNotice` path. Normal chat provider requests now send the
  durable post-checkpoint transcript as role-tagged provider input messages,
  request provider prompt-cache retention where supported, and budget that same
  message-shaped input for compaction decisions. Foundation Local forwards max
  output token limits to the Swift bridge and parses required Noema response
  envelopes for normal chat turns. Backend review follow-ups that remain intentionally separate:
  The remaining bridge follow-ups have landed: GraphQL Providers and Agents
  opportunistically refresh Foundation Local availability in real runtime-host
  state before rendering model options, keep unknown/unavailable Foundation
  accounts visible but disabled in the dashboard, and reject saving a
  Foundation model preference until the account is actually available. The Rust
  provider contract now carries an optional Noema conversation id, normal chat
  turns pass it through, and `foundation_local` keeps a daemon-owned bridge
  process with live sessions reused by conversation/model/instructions while
  one-shot compaction requests stay transient. The bridge protocol now
  exposes Rust replay/cancel request methods, and the Swift bridge decodes
  replay/cancel, replays prior turns by rebuilding the session transcript when
  supported, and returns an explicit unsupported-cancellation error for now.
- Normal chat now keeps its instruction kernel immutable and persists mutable
  model context as hidden append-only `model_context_update` items. Stable keys
  currently cover `agent.identity`, `runtime.environment`, and
  `tools.visibility`; each developer message replaces only its named section,
  and compaction checkpoints are followed by a full keyed snapshot. Prompt
  planning reconciles against any summary activated concurrently so a turn
  cannot lose identity or tool authority. OpenAI GPT-5.6 profiles attach
  explicit cache options and up to four developer-message breakpoints. Native
  tool requests keep a stable catalog of calibrated, read-only, prompt-safe MCP
  definitions across transient health/auth changes and use provider-enforced
  `allowed_tools` for the currently callable subset; calibration, schema, and
  installation changes remain intentional catalog invalidations. Foundation
  Local keys sessions by the exact immutable instructions, appends keyed
  application-context suffixes, and recreates a session whenever durable
  user/assistant replay diverges from its tracked transcript.

## Open Loops

- The approved `noema-core` crate decomposition is in implementation under
  `docs/superpowers/plans/2026-07-15-noema-core-crate-decomposition.md`. Phase 0
  landed as tooling commit `41dafcd52` and committed-baseline/CI commit
  `c18d4cce4`: dependency and feature ownership, all-target focused trees,
  generated frontend identity, GraphQL/local-model/SQLite preservation, and
  normalized Rust unit-test ownership are now executable gates. The settled
  target has no generic common/domain/objects crate; MCP is a capability child,
  local models remain an internal provider implementation, and host concrete
  composition is shell-enabled so focused API builds stay backend-free. Phase 1
  extracted `noema-home`: it owns filesystem layout, safe path components,
  initialization, and the generic JSONL diagnostic sink; configuration bytes
  and diagnostic categories remain with their semantic owners, and all
  consumers use direct dependencies instead of `noema-core` forwarding exports.
  Phase 2 extracted `noema-conversations` with the schema-exact conversation
  owner vocabulary and fallible actor/owner references. The unused generic
  typed IDs, object registry/table mappings, and `MemoryPersistenceError`
  wrapper were deleted; store, runtime, and GraphQL now consume the conversation
  leaf directly, and store owns the error conversion at its boundary. Phase 3A
  extracted the always-compiled `noema-artifacts` domain, path, metadata-port,
  and consumer-operations contracts without a core forwarding export. Artifact
  paths and filename validation left `noema-home`, server consumes download
  slugs directly, and store maps artifact-domain failures at its boundary.
  Phase 3B completed the artifact extraction. `noema-artifacts` now owns the
  root-bound capability filesystem service, CSPRNG operation-private staging,
  hard-link no-clobber publication, verified reads, bounded stale cleanup, and
  cancellation-safe rollback. `NoemaStore` implements the narrow metadata port
  with no-yield SQLite transactions, expected-index CAS, and bounded async
  retries for cross-connection `SQLITE_BUSY`; runtime and GraphQL receive
  artifact filesystem authority only through `ArtifactOperationsHandle`, while
  the host remains the production composition root. The old core writers and
  forwarding exports are deleted. Independent store handles now exercise
  barrier-controlled conversation and task append races, proving one
  readable/hash-valid winner, one typed conflict, and exact cleanup of the
  loser's operation-private staging/object directories. Shared staging,
  version, and `objects` directories remain as permanent coordination
  scaffolding so one service can never invalidate another service's verified
  handle. Artifact download diagnostics remain redacted through a separately
  injected narrow reporter rather than resolver-owned path/logger construction.
  Phase 4 extracted `noema-capabilities` at commit `0883c45e1`. Provider-visible
  `ToolSpec` values now contain no execution authority; runtime retains one
  immutable request-local `CapabilityBinding` snapshot and continuation policy
  can only shrink. Foreground and background dispatch reject unadvertised or
  role-denied names without parsing authority from model output. MCP bindings
  carry opaque captured authority with generation-safe server identity,
  canonical SHA-256 metadata fingerprints, exact calibration revalidation, and
  binding-owned persistence omission. Web search/fetch schemas, parsers,
  redaction, and pure URL policy live in the leaf crate, while provider dialect
  lowering and concrete network backends remain outside it. The old gateway
  parser/fallback facades and provider tool aliases are deleted. The Phase-4
  workspace, inventory, dependency, boundary, and preservation gates are green.
  Phase 5's provider-vocabulary unit landed at `74e75be75`.
  `noema-providers` now owns generation contracts, transport-neutral errors,
  resolved provider/OAuth configuration, account/auth/capability-assignment
  models, typed model-profile metadata, durable selection provenance, and
  public local-model persistence codecs. Core, server, desktop, and model evals
  consume those types directly with no core forwarding facade. Credential- and
  path-reachable `Debug` implementations are redacted, credential-bearing
  endpoint URLs are rejected, stable test ownership is preserved, and the
  GraphQL/frontend/local-model/SQLite decomposition baselines are unchanged.
  Until Phase 10C, exact provider instance keys remain optional in snapshots and
  legacy durable task/run writes reject `Some(key)` instead of discarding it.
  Checkpoint 5A completed at `c20bcbe2f`. Provider-owned boxed-future
  persistence ports now cover accounts, capability assignments, model catalogs,
  and local-model installation/activation state, while SQLite remains behind
  `NoemaStore`. Account and catalog mutations commit related status and metadata
  atomically, local-model lifecycle writes return their committed projections,
  and corrupt durable rows map to typed invariants rather than repository
  outages. Runtime, GraphQL, provider auth, and web-tool consumers use the new
  ports. Three adversarial reviewers found no remaining blocking issue, and the
  full workspace, dependency, inventory, boundary, and preservation-baseline
  gates are green. Checkpoint 5B is complete. `noema-providers` owns the
  object-safe provider call surface, erased handle, generation-safe registry,
  retirement leases, strict selection resolver, and typed route failures.
  Core's temporary `LegacyProviderRoutes` bridge preserves complete selection
  provenance while attaching process-local identity only in memory. Foreground
  turns, background task continuations/finalization, task completion, progress
  audits, web summarization, memory generation, and one-shot API calls retain
  one exact route lease for each execution chain. Provider replacement keeps
  old generations alive through their final in-flight call, MCP classification
  reads the replacement provider's current model instead of a startup cache,
  and managed-memory proxy shutdown drains active requests before the local
  supervisor stops. Local-model activation starts the replacement first, then
  gates the SQLite preference commit, registry publication, and supervisor
  swap so readers cannot observe the middle state. Exact durable instance keys
  and multi-instance local-model retirement remain Phase 10C work rather than
  being implied by this legacy bridge. Checkpoint 5C is next: move hosted
  adapters, provider-account orchestration, concrete web backends, and the
  Foundation Swift bridge into `noema-providers`. The `local-models` provider
  feature remains transitional shared response support until the concrete
  implementation moves.

- The bounded remediation program completed Phase 2 at `92,157` maintained
  source lines (`-1,767` from its baseline). Axum/tower-sessions now own the
  local web boundary; OpenAI and Codex share a Responses dialect; MCP supports
  stdio and `rmcp` Streamable HTTP only; the deprecated HTTP+SSE transport and
  unreachable OpenAI-hosted search path are gone. The full workspace gate is
  green. Phase 3 landed owned runtime cancellation/draining and atomic durable
  shutdown recovery at `92,387` lines, but rejected a partial `tokio-rusqlite`
  conversion and LOC-positive atomic-writer adoption. Phase 4 removed the
  unenforced MCP ownership/trusted-identity surfaces and English schema-field
  inference, leaving Mixed calibrations fail-closed, and ended at `90,541`
  lines. Two reserve deletions then consolidated Responses request profiles and
  removed the inert MCP approval placeholder, reaching `88,642`. Phase 5
  extracted the loopback HTTP/session/asset boundary and web/dev
  binaries into `noema-server` at `88,571` lines. Asset-only workspace rebuilds
  now invalidate only the server (4.43s versus 19.58s baseline), and
  representative core edits check in about 3.4s versus 5.45s. The store split
  was rejected pending a true leaf-domain cleanup; do not force it through
  reverse dependencies or an accidental store-owned domain layer. Phase 6 is
  complete at `88,807` maintained lines (`-5,117`, or 5.45%, from baseline).
  It removed duplicate persistence/provider helpers and the second scraper
  stack, added root licensing and Python artifact hygiene, made release web
  assets fail closed through Vite's manifest, documented the unsigned
  developer-bundle limitation, and added focused Rust/frontend/Python CI.
  Cargo-deny/about/CycloneDX, Python locking/provisioning, signing, and
  clean-machine packaging remain explicit follow-ups rather than unvalidated
  release claims. Write/export MCP tools now remain hidden and fail closed at
  dispatch until exact one-shot approval is implemented; current-fingerprint
  Ready read-only tools remain available.

- Decide whether richer Mnemosyne memory browsing should include entity, provenance,
  and history drill-ins.
- Decide how Mnemosyne extraction/ingest visibility should surface without bringing
  back transcript memory markers prematurely.
- Persist audit-only `search_memory` diagnostics if needed, without mirroring
  Mnemosyne memory truth in SQLite.
- Continue aligning docs, schema, and frontend IA.
- Decide which export formats ship first and how export preview/redaction should work.
- Continue the third-party MCP control plane after the first landed slice:
  implement actual third-party server installation/auth management, approval
  decision mutations, richer audit event persistence, and any future
  auto-approval model hooks.
- Implement the approved system errors log design in
  `docs/superpowers/specs/2026-07-02-system-errors-log-design.md`: append
  developer-diagnostic JSONL events to `${NOEMA_HOME:-$HOME/.noema}/errors.log`
  for system-level errors that likely require Noema code, prompt, schema,
  protocol, parser, or adapter changes. The log is intentionally uncapped and
  unredacted.
- Add signing, notarization, update, and production distribution for the Tauri
  macOS app after the unsigned developer build is stable.
- Revisit migrations only when the project needs persisted user data compatibility.

## Codex Preferences

- Be direct and implementation-oriented when the user asks to build.
- Ask before broad product direction changes when the request is ambiguous.
- Use task modes to avoid mixing exploration, implementation, review, and shipping.
- Preserve user edits and unrelated dirty worktree changes.
- Prefer adversarial review for memory, security, retrieval, governance, and large refactors.
- For UI work, optimize for restrained, polished, information-dense interfaces rather than decorative complexity.
- Treat raw `~/.codex/sessions` as private memory source material. Summarize, do not quote, unless asked.
- The user is a big fan of trains; train or rail references are welcome when they fit the context.
- Stable prompt-cache work treats OpenAI and Codex Responses as separate dialects:
  OpenAI supports the explicit cache controls and provider-enforced tool subsets
  described above, while Codex keeps unsupported cache fields and encrypted
  reasoning gated off until live/provider verification confirms their request
  and replay shapes.

## Task Modes

Use these labels in prompts and status updates:

- `explore only`: gather context, compare options, no edits.
- `plan only`: produce an implementation plan, no edits.
- `implement`: make scoped changes and verify them.
- `adversarial review`: find correctness, security, architecture, and test gaps; do not edit unless asked.
- `ship`: validate, stage, commit, and push only the approved scope.

## Validation Defaults

For Rust work:

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

For frontend or UI work:

- Run `bun run gen:types`, `bun run lint`, and `bun run build` in `crates/noema-core/web`.
- Do not add frontend unit tests or browser automation unless explicitly requested.

Before commit/push:

```bash
git status --short --branch
git diff --check
git diff --cached --stat
git diff --cached --name-status
```
