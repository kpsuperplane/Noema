# Current Noema Context

This file is the durable working brief for Codex sessions. Keep it concise and update it when project direction, workflow preferences, or open loops change.

## Active Direction

Noema is a local-first personal agent operating system. The current build path is chat-led and object-backed: the user starts in chat, Noema records durable state behind the interaction, and inspection/control surfaces appear when they are useful.

V1 should stay small and concrete:

- Local Noema home and config.
- Codex-backed chat through the daemon.
- Persisted conversations and memory records.
- Memory review and context graph inspection.
- Frontend IA that exposes memory and provenance progressively instead of starting with admin dashboards.

## Settled Decisions

- SQLite is the canonical structured store.
- Filesystem storage is for durable object-owned documents, attachments, and artifacts.
- `system/` state is derived and rebuildable.
- Memory is governed context, not hidden model state.
- Canonical memory rows own truth, policy, provenance, lifecycle, and audit.
- Context graph and FTS/search indexes are derived projections and must remain rebuildable.
- Graph or fuzzy retrieval can suggest candidates, but policy gates inclusion.
- Pre-V1 schema changes do not need migrations or backwards compatibility unless explicitly requested.
- V1 frontend should start with chat, memory, and inspection before exposing full workspaces, tasks, agents, tools, or governance.

## Open Loops

- Persist context packets consistently for each governed run.
- Keep memory extraction from ordinary chat visible in chat as activity, similar to a tool call.
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
