# Current Noema Context

This file is the durable working brief for Codex sessions. Keep it concise and update it when project direction, workflow preferences, or open loops change.

## Active Direction

Noema is an always-on, self-hosted personal agent operating system. The approved storage direction is to replace Postgres with embedded SurrealDB as the canonical structured store, opened only by the Noema server process at `NOEMA_HOME/db`. The current implementation still contains Postgres-backed persistence until that replacement lands.

The next storage slice should stay small and concrete:

- Local Noema home and config.
- Codex-backed chat through the daemon using Noema-owned OAuth tokens and direct
  Codex Responses API calls.
- Core-hosted local React web chat as the first frontend shell.
- SurrealDB-backed persisted conversations, transcript items, graph claims,
  provenance, and retrieval packets.
- Memory review and graph inspection from SurrealDB-backed repositories.
- Frontend IA that exposes memory and provenance progressively instead of starting with admin dashboards.

## Settled Decisions

- Embedded SurrealDB is the target canonical structured store for the always-on
  personal server.
- The Noema server process is the only process that opens the embedded database;
  clients, CLI, desktop, web, and future mobile use Noema APIs.
- Embedded database files live directly under `${NOEMA_HOME:-$HOME/.noema}/db`.
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
- `crates/noema-core/web/tests` has been removed; web validation should use
  `bun run lint`, `bun run build`, and local browser smoke checks.
- Agent memory reads start as an explicit `search_memory` tool-only slice:
  Noema validates arguments, builds the trusted retrieval envelope, records a
  context packet, returns approved memories plus generic omissions as a normal
  tool result, and does not inject memories automatically before turns.
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
- Local-human canonicalization is deterministic: explicit aliases are local;
  same-name Kevin is local for direct or first-person local assertions and
  non-local for named third-party evidence. Note fallback objects use opaque
  deterministic IDs plus punctuation-normalized dedupe and reinforcement, so
  note content and secrets are not embedded in entity IDs.
- The local `search_memory` tool reads graph claims and returns claim-shaped
  tool results.
- The first graph-memory inspection surface has landed. GraphQL exposes bounded
  owner/admin graph-claim inspection through `memoryClaims` and `memoryClaim`:
  lists redact non-public fact text and content-bearing display names, while
  explicit detail inspection shows full fact and evidence. CLI `noema memory
  list` and `noema memory show <id>` consume GraphQL through the daemon/web
  endpoint rather than opening SurrealDB directly. Generated web GraphQL
  schema/types are kept in sync.

## Open Loops

- Replace Postgres persistence with embedded SurrealDB-backed repositories and
  strict graph memory.
- Retire stale SQLite/Postgres storage wording from docs after the SurrealDB
  implementation lands.
- Add richer web drill-ins for memory details, predicate review, provenance,
  and graph inspection.
- Add `noema context graph` and richer graph neighborhood inspection.
- Continue aligning docs, schema, CLI inspection commands, and frontend IA.
- Decide which export formats ship first and how export preview/redaction should work.
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
