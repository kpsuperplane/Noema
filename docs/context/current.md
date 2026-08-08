# Current Noema Context

This is the active Codex brief. Durable contracts belong in subsystem docs and
Git history owns completed milestones.

## Active direction

Noema is an always-on, self-hosted personal agent. Chat is the primary surface;
Work, Memory, integrations, governance, and settings appear when backed state
and the human's current job require them.

The current foundation is one server with web and desktop shells, server-owned
SQLite state, native Markdown memory, durable conversations and Work, governed
tools and artifacts, hosted and local model providers, and a React/Astryx web
client. New work should be a small vertical slice or a net-negative reduction.

## Current constraints

- [Security](../harness/security.md) owns the three information classes.
  Secrets never enter model context or ordinary persistence. Authorized private
  information and ordinary technical values remain intact.
- `${NOEMA_HOME:-$HOME/.noema}` is the durable home. SQLite at
  `db/noema.sqlite3` is the only structured authority and is opened only by the
  server. Schema changes append immutable forward-only migrations.
- Native Markdown under `memory/human/` is the durable human-memory authority;
  SQLite FTS is rebuildable. Documented iconless-page reads remain supported.
- Durable chat is reconstructed from conversation items. Live daemon and
  subscription state is coordination state only.
- Work current state is transactional. Work events provide audit and
  invalidation, not an independent replay authority.
- Hosted providers, local llama.cpp, Apple Foundation Models, MCP, and native
  HTTP adapters retain distinct security and transport ownership. Prefer
  consolidating duplicate readers and writers over inventing a universal layer.
- Provider accounts and exact model selections are persisted. New future
  references require a registry readiness proof held through commit.
- Adapter definitions are filesystem-canonical; the body-free SQLite projection
  is disposable. Current manifests use strict schema version 8.
- Browser auth is local-human WebAuthn; paired clients use independently
  revocable bearer credentials. Installed PWA behavior follows
  [../frontend/pwa.md](../frontend/pwa.md).
- Frontend route and interaction truth is summarized in
  [../frontend/current-contract.md](../frontend/current-contract.md). UI changes
  follow [../frontend/product-design.md](../frontend/product-design.md).
- Optimize total system simplicity. Follow
  [../development/simplicity.md](../development/simplicity.md) before nontrivial
  architecture, refactoring, workflow, testing-policy, or harness work.

## Open loops

- Apollo iOS generation requires a macOS/Apple JavaScriptCore environment.
  Until regenerated there, iOS GraphQL source/generated divergence and removal
  of telemetry compatibility fields remain blocked.
- Product-scope decisions remain open for the dormant adapter scheduler, the
  long-term set of integration substrates and full-parity clients, Work role
  breadth, and secondary vertical systems. Do not infer retirement of a live
  capability from a local simplification task.
- Provider generation retains native-future `ModelProvider` plus object-safe
  `ProviderOperations`: the erasure boundary also owns streamed Markdown
  normalization. Collapse it only if a measured implementation is net-negative
  and preserves that single normalization authority.

## Validation defaults

- Rust: `cargo fmt --all --check`, `cargo check-workspace`, `cargo gate-lint`,
  and `cargo gate-test`; use `cargo validate` for focused commands.
- Web: `bun run lint` and `bun run build` from `apps/web`.
- Unit tests only unless smoke or fixture tests are explicitly requested.
