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
  mutation for explicit refreshes. The muted lead figure summarizes loaded memory
  themes, source observations are shown as provenance rather than as memories,
  and stubbed action, history, and recall controls should stay hidden until real
  backend operations exist. The web UI queries Noema Core GraphQL only; Noema
  Core resolves the configured Mnemosyne endpoint, fetches memories for
  `human:local`, and adapts them into grouped memory documents for the frontend.
  The browser never connects directly to Mnemosyne.
- Docker/Compose development infrastructure has been retired; local development
  uses host Rust, Bun, and web/desktop product surfaces. The old standalone
  Noema binary and local dev alias have been removed.
- First-run web onboarding is derived from backend readiness checks and blocks
  chat until an active provider account is authenticated.
- Provider credential/session material lives under
  `${NOEMA_HOME:-$HOME/.noema}/providers/<provider>/<account>/`; the structured
  store keeps only non-secret provider metadata.
- Codex provider account homes contain Noema-owned `codex_tokens.json` OAuth
  state. They are not `CODEX_HOME` directories, and Noema does not silently
  import Codex CLI `auth.json` files.
- Concrete object records are the canonical structured state; actor/principal,
  governable scope, provenance source, and transcript item are interfaces
  implemented by concrete objects rather than universal parent tables.
- Durable chat history is reconstructed from `conversation_items`; the daemon
  WebSocket and `agent_status` are live coordination state for current turns.
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
  sections: Agents; Tools with Web and MCPs; Safety with Approvals and
  Identities; and System with Providers. The Web page owns first-party
  `web.search` and `web.fetch` status plus the fetch summarizer model
  preference. Settings routes are canonical nested paths such as
  `/settings/tools/web`, `/settings/safety/approvals`, and
  `/settings/system/providers`; old flat settings paths are not supported.
  The placeholder Audit settings surface has been removed until audit event
  persistence lands. Agent management actions are not exposed yet.
- Frontend docs now distinguish currently addressable routes from target
  surfaces: TanStack Router owns `/`, `/settings`, and
  `/settings/{agents,tools/web,tools/mcps,safety/approvals,safety/identities,safety/usage,system/providers,memory}`.
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

## Open Loops

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
  OpenAI can request `reasoning.encrypted_content` once adapter tests cover it,
  while Codex must keep encrypted-reasoning include gated off until live/provider
  verification confirms the request field and replay shape.

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
- Run the local app/server.
- Capture desktop and mobile screenshots.
- Inspect overflow, spacing, safe areas, and visual regressions.

Before commit/push:

```bash
git status --short --branch
git diff --check
git diff --cached --stat
git diff --cached --name-status
```
