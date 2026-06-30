# Current Noema Context

This file is the durable working brief for Codex sessions. Keep it concise and update it when project direction, workflow preferences, or open loops change.

## Active Direction

Noema is an always-on, self-hosted personal agent operating system. Embedded SurrealDB is the canonical structured store, opened only by the Noema server process at `NOEMA_HOME/db`.

The next storage slice should stay small and concrete:

- Local Noema home and config.
- Codex-backed chat through the daemon using Noema-owned OAuth tokens and direct
  Codex Responses API calls.
- Core-hosted local React web chat as the first frontend shell.
- SurrealDB-backed persisted conversations, transcript items, graph claims,
  provenance, and retrieval packets.
- Memory review and graph inspection from SurrealDB-backed repositories.
- Frontend IA that exposes memory and provenance progressively instead of starting with back-office dashboards.

## Settled Decisions

- Embedded SurrealDB is the target canonical structured store for the always-on
  personal server.
- The Noema server process is the only process that opens the embedded database;
  clients, CLI, desktop, web, and future mobile use Noema APIs.
- Embedded database files live directly under `${NOEMA_HOME:-$HOME/.noema}/db`.
- The embedded store uses the stable SurrealDB v3 Rust SDK with `kv-rocksdb`;
  pre-stable local development databases created by v2 may be deleted and
  rebuilt instead of migrated.
- Docker/Compose development infrastructure has been retired after the embedded
  SurrealDB migration; local development uses host Rust, Bun, the Codex CLI, and
  the `cargo dev-daemon` alias.
- `dev-daemon` traps normal terminal/process shutdown signals and stops both
  watcher process groups so interrupted dev sessions do not leave orphaned
  daemon processes behind.
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
- Memory is governed context, not hidden model state.
- Canonical memory claims own truth, policy, provenance, lifecycle, and
  evidence.
- Memory writes should consolidate before creating or updating graph claims:
  exact fingerprints prevent duplicate claims, repeated support reinforces
  existing claims with evidence, and conflicts create reviewable disputed state
  instead of silently overwriting truth.
- The long-term future-write memory consolidation pipeline has landed: all
  explicit and provider memory writes use a shared proposal/canonicalization
  path, promoted predicates are validated before active claims, unknown
  predicates create review-gated predicate proposals, bounded match search
  supports conservative consolidation, and GraphQL/CLI expose predicate
  proposal inspection.
- The claim canonicalizer prompt must spell out the exact strict JSON contract
  consumed by the parser. Runtime canonicalization validates promoted
  `predicate_id` values against the current catalog before any graph write, so
  provider/schema mismatches surface as canonicalization failures rather than
  generic graph write failures.
- Derived search/vector indexes are rebuildable projections.
- Graph or fuzzy retrieval can suggest candidates, but policy gates inclusion.
- Pre-stable schema changes do not need migrations or backwards compatibility unless explicitly requested.
- The initial frontend should start with chat, memory, and inspection before exposing full workspaces, tasks, agents, tools, or governance.
- The first-party product API direction is GraphQL, with Apollo Client on the
  React web frontend and backend-exported schema/types feeding frontend codegen.
- GraphQL is the first-party client API for Noema web, CLI, future desktop,
  and future mobile clients. Internal Rust modules continue to use command,
  runtime, repository, policy, provenance, audit, and event interfaces directly.
- The current web UI consumes GraphQL over `/graphql` plus
  `graphql-transport-ws` subscriptions over `/graphql/ws`.
- The first macOS desktop app direction is a Tauri app in
  `crates/noema-desktop` that starts a Noema runtime host inside the app
  process, loads the existing React UI from bundled assets, and uses Tauri
  IPC/events for GraphQL instead of exposing a local HTTP/WebSocket server.
- The CLI chat path now uses GraphQL for `startPrimaryConversation` and
  `sendConversationTurn` streaming. It still uses the daemon Unix-socket
  protocol only for local lifecycle cleanup such as connection setup,
  `end_conversation`, and temporary daemon shutdown.
- The transitional product web endpoints and old frontend protocol/type export
  surfaces have been retired; GraphQL is now the only client-facing product
  API.
- The web home chat should load `human:local.primary_conversation_id`; Noema
  conversation continuity is owned by Noema structured state, not provider
  runtime state.
- Frontend build and lint use Bun from `crates/noema-core/web`.
- Web GraphQL schema and operation types are generated with `bun run gen:types`.
- The web UI uses shadcn/ui `base-rhea` components backed by Base UI, with
  Noema colors applied through local CSS tokens. Noema-owned shell and domain
  components remain responsible for chat, memory, provenance, approvals, tools,
  runs, and object detail semantics.
- Next frontend shell slice: implement the approved route-derived L0 to L1
  Settings navigation from
  `docs/superpowers/specs/2026-06-30-route-derived-shell-settings-design.md`
  and
  `docs/superpowers/plans/2026-06-30-route-derived-shell-settings.md`.
- The web shell uses a layered sidebar deck: one persistent `ShellSidebar`
  ground layer sits under the route content deck. Expanded desktop keeps the
  sidebar visible; collapsed desktop and mobile reveal navigation by moving the
  deck aside rather than rendering a separate drawer/sidebar copy.
- The web shell exposes Settings as a bottom-left sidebar cog, not as a primary
  navigation destination. `/settings` renders as a full-screen utility takeover
  after onboarding and defaults to `/settings/providers`. Settings now contains
  read-only Providers and Agents tabs: Providers shows non-secret provider
  account metadata, while Agents is backed by a dedicated GraphQL `agents` read
  model and shows safe registered-agent metadata such as agent ids. Agent
  management actions are not exposed yet.
- The first third-party MCP control-plane slice has landed. Third-party MCPs
  route through a Noema-owned Capability Gateway rather than raw model tool
  handles. MCP setup is a mandatory metadata-only calibration flow: tools get
  reviewed `read`/`write`/`export` classifications of `none`, `trusted`,
  `untrusted`, or `mixed`; ownership is resolved through deterministic
  extractors and trusted identity selectors; unresolved ownership blocks agent
  use; read results are quarantined before model-visible release; and V1
  exports always require manual approval. Web Settings can now add MCP servers
  through a guided modal setup flow that does not persist the server until
  authentication is complete and metadata discovery succeeds. Successful setup
  stores secrets under `${NOEMA_HOME}/mcp/`, verifies metadata-only
  connectivity through concrete stdio and HTTP/SSE MCP transports, fetches tool
  schemas, and automatically opens a separate tool-permissions modal while
  keeping discovered tools disabled and agent-invisible. The permissions modal
  now shows discovered schemas, owner extractor setup, agent visibility, and
  scope visibility, and blocks impossible `ready` saves before they hit the
  backend. Existing MCP rows can reopen the permissions modal or delete the
  server plus stored setup secrets through an in-app destructive confirmation.
  User-facing setup errors are sanitized while raw transport details stay out of
  the web form.
  Web Settings also exposes MCPs, Trusted Identities, Approvals, and Audit
  surfaces backed by GraphQL read models where live data exists.
- Routed web surfaces learn shell-owned deck state through
  `ShellSurfaceContext` visibility (`visible`, `hiding`, `hidden`, `showing`).
  Surfaces should run focus and other visible-only side effects only when
  visibility is `visible`; the shell owns transition settling.
- Noema-owned React product components should live one component per file.
  Pure helper/model logic belongs in `.ts` files, shared type files/types and
  nearby tests may live in component folders, and `components/ui` primitive
  wrappers may remain grouped when they mirror upstream compound APIs.
- Frontend Noema-owned product components now follow the one-component-per-file
  rule, with transcript render/model helpers split from React components.
- GraphQL, daemon web transport, daemon runtime, graph-claim store, provider
  streaming parsers, and CLI command/GraphQL client code are split into focused
  modules while preserving existing behavior.
- Memory extraction, consolidation, errors, and shared memory types now live
  under the `memory` module tree. Provider-neutral contracts, account metadata,
  and auth support live under `provider`, while concrete adapters and response
  stream helpers live under `provider::adapters`; the old top-level `providers`
  module has been retired.
- `crates/noema-core/web/tests` has been removed; web validation should use
  `bun run lint`, `bun run build`, and local browser smoke checks.
- Agent memory reads start as an explicit `search_memory` tool-only slice:
  Noema validates arguments, builds the trusted retrieval envelope, records a
  context packet, returns approved memories plus generic omissions as a normal
  tool result, and does not inject memories automatically before turns.
- The primary agent starts unnamed. Prompt construction includes an
  `onboarding_prompt` asking the model to ask the user for a name while the
  agent has no display name. The local `update_own_name` tool persists later
  naming or renaming only when the current user explicitly names or renames the
  agent. Intent is carried by the structured tool call and trusted runtime
  state rather than direct user-text matching. The runtime feeds successful
  local tool results back into the same turn plus subsequent prompts.
- The local `search_memory` tool supports validated concrete `scope_ids`.
  Empty `query` is allowed only for scoped reads, and `query` narrows within
  scope rather than broadening it. Memory write activities expose canonical
  claim outcome previews so chat markers can name the saved or reinforced fact.
- New graph memory direction: durable memories are strict graph claims over
  entities and promoted predicate records. Conversation items are direct
  provenance sources. Specialized evidence relations replace broad memory audit
  machinery for memory truth. Retrieval uses a small deterministic `use_mode`
  enum and fails closed.
- The embedded SurrealDB store is split into focused store modules. Its current
  bootstrap defines strict graph-memory tables for entities, predicates,
  predicate proposals, claims, evidence edge records, and retrieval packets,
  and seeds the built-in personal-agent predicates.
- Graph-claim retrieval now has a first deterministic store API that filters
  active/confirmed claims by simple fact/hint text matching, applies predicate
  `use_mode` plus sensitivity/context policy gates, and reports redacted
  omission counts without writing retrieval packets yet.
- Explicit `/remember` and `remember:` chat commands now create or reinforce
  SurrealDB graph claims before provider generation, using the user
  conversation item as explicit-human evidence.
- Provider-structured ordinary memory proposals now route through SurrealDB
  graph claims instead of the legacy memory persistence path. Validation and
  persistence record conversation item provenance, accept user or assistant
  evidence, track response phases and assistant items, reject split assistant
  evidence that cannot map to one item, and report created versus reinforced
  claims, partial versus full failure, and failed proposal diagnostics.
- Provider memory extraction output is treated as fallible draft data. Invalid
  extractor proposals are discarded before canonicalization; all-invalid batches
  produce no user-facing memory activity, while mixed batches persist valid
  proposals and record rejected draft counts in metadata. Assistant evidence may
  support non-human notes, but human-subject memories require direct user
  evidence.
- Local-human canonicalization is deterministic: explicit aliases are local;
  same-name Kevin is local for direct or first-person local assertions and
  non-local for named third-party evidence. Note fallback objects use opaque
  deterministic IDs plus punctuation-normalized dedupe and reinforcement, so
  note content and secrets are not embedded in entity IDs.
- The local `search_memory` tool reads graph claims and returns claim-shaped
  tool results.
- The first graph-memory inspection surface has landed. GraphQL exposes bounded
  memory-management graph-claim inspection through `memoryClaims` and `memoryClaim`:
  lists redact non-public fact text and content-bearing display names, while
  explicit detail inspection shows full fact and evidence. CLI `noema memory
  list` and `noema memory show <id>` consume GraphQL through the daemon/web
  endpoint rather than opening SurrealDB directly. Generated web GraphQL
  schema/types are kept in sync.
- The first web memory graph page has landed under `/memory/graph`, reached
  from the memory management surface. It uses a bounded `memoryGraph` GraphQL
  read model, defaults to candidate, active, and confirmed claims, caps the
  first load at 150 claims, renders entity nodes plus claim edges with React
  Flow pan/zoom, and shows selected-claim evidence/provenance in a detail
  panel rather than as canvas nodes. The local graph inspection view returns
  readable labels and facts for loaded claims, while graph search matching
  still avoids non-public fact/entity text before the bounded result is loaded.

## Open Loops

- Add richer web drill-ins for memory details, predicate review, provenance,
  and graph inspection.
- Refine the landed Memory Graph page from
  `docs/superpowers/specs/2026-06-29-memory-graph-page-design.md` with richer
  filters, neighborhoods, and detail views.
- Add richer graph neighborhood inspection for CLI, GraphQL, and web.
- Continue aligning docs, schema, CLI inspection commands, and frontend IA.
- Decide which export formats ship first and how export preview/redaction should work.
- Continue the third-party MCP control plane after the first landed slice:
  implement actual third-party server installation/auth management, approval
  decision mutations, richer audit event persistence, and any future
  auto-approval model hooks.
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
