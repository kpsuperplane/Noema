# Current Noema Context

This file is the durable working brief for Codex sessions. Keep it concise and update it when project direction, workflow preferences, or open loops change.

## Active Direction

Noema is an always-on, self-hosted personal agent operating system. The current build path is chat-led and object-backed: clients connect to the Noema server, the server records durable state in Postgres, and inspection/control surfaces appear when they are useful.

The current slice should stay small and concrete:

- Local Noema home and config.
- Codex-backed chat through the daemon.
- Core-hosted local React web chat as the first frontend shell.
- Postgres-backed persisted conversations, transcript items, memory records, provenance, and context packets.
- Memory review and context graph inspection from Postgres-backed repositories.
- Frontend IA that exposes memory and provenance progressively instead of starting with admin dashboards.

## Settled Decisions

- Postgres is the canonical structured store for the always-on personal server target.
- Docker Compose mounts `NOEMA_HOME` into containers and stores Postgres physical files at `${NOEMA_HOME:-$HOME/.noema}/db/postgres`.
- Concrete object rows are the canonical structured state; actor/principal,
  governable scope, provenance source, and transcript item are interfaces
  implemented by concrete objects rather than universal parent tables.
- Durable chat history is reconstructed from `conversation_items`; the daemon
  WebSocket and `agent_status` are live coordination state for current turns.
- Filesystem storage is for durable object-owned documents, attachments, and artifacts.
- `system/` state is derived and rebuildable.
- Memory is governed context, not hidden model state.
- Canonical memory rows own truth, policy, provenance, lifecycle, and audit.
- Context graph and FTS/search indexes are derived projections and must remain rebuildable.
- Graph or fuzzy retrieval can suggest candidates, but policy gates inclusion.
- Pre-stable schema changes do not need migrations or backwards compatibility unless explicitly requested.
- The initial frontend should start with chat, memory, and inspection before exposing full workspaces, tasks, agents, tools, or governance.
- The current frontend endpoint is a native Noema WebSocket served by the daemon, not an OpenAI-compatible API.
- Frontend build and lint use Bun from `crates/noema-core/web`.
- Web protocol TypeScript definitions are generated from Rust with `bun run gen:types`.

## Open Loops

- Continue removing stale SQLite/product-version wording from docs and planning notes now that runtime persistence is Postgres-only.
- Add richer web drill-ins for memory details, memory review, and context graph inspection.
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
