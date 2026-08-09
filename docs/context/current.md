# Current Noema Context

This is the active Codex brief. Durable contracts belong in subsystem docs and
Git history owns completed milestones.

## Active direction

Noema is an always-on, self-hosted personal agent. Chat is the primary surface;
Tasks, Memory, integrations, governance, and settings appear when backed state
and the human's current job require them.

The current foundation is one server with web and desktop shells, server-owned
SQLite state, native Markdown memory, durable conversations and tasks, governed
tools and artifacts, hosted and local model providers, and a React/Astryx web
client plus the native SwiftUI iPhone/iPad client. New work should be a small
vertical slice or a net-negative reduction.

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
- An ordinary tool result and its exact saved call finish in one store
  transaction. The result ID comes from the call item ID. The same result can
  repeat without a duplicate, and different repeat data fails.
- Each foreground action request links to its exact saved approval item. Action
  recovery reads this link directly, including when the item is not visible. It
  does not search the visible transcript for a substitute.
- Current task state is transactional. Task events provide audit and
  invalidation, not an independent replay authority.
- Task recovery uses a resolved pause only when its task generation and
  execution contract match the current task. Older pause history stays stored
  but cannot start a current run.
- Task workers keep a 120-second claim and renew it every 30 seconds. A renewal
  that starts five seconds late, or fails, records timing and the active debug
  phase. The two stored expiries had no scheduled renewal and retained an open
  provider or browser span. This evidence indicates a stopped runtime process,
  not a slow SQLite renewal. Do not increase the claim without new evidence.
- A final task-run transaction also finishes active run items and open debug
  spans. A saved final tool result decides its matching call state. Other active
  items use the run result. An interrupted run uses `failed` for child items
  because that item vocabulary has no `interrupted` value.
- A reviewer context includes all final saved items from the executor run named
  by the exact submission. It keeps saved IDs, statuses, content, and payloads.
  It fails instead of dropping evidence when the checkpoint is too large.
- A first executor run gets no prior review. A correction executor must load
  the exact saved `triggering_review_id`. It does not use the task's latest
  review pointer as a replacement.
- Every capability binding owns a source input check. The shared capability
  router runs that check after it resolves the exact binding and before it
  calls the invoker. Immediate and reviewed calls use the same check.
- Invalid task terminal input stops before invocation. It returns the existing
  invalid-terminal result so that the task can request one corrected report. A
  second invalid report ends in recovery.
- Provider schema settings describe request construction only. A strict request
  does not authorize returned input. OpenRouter keeps its current strict request
  behavior, and every returned call still passes through the source input check.
- A successful strict provider conversion returns optional null placeholders to
  omitted source fields before policy, review, or storage. Other invalid values
  stay unchanged. MCP schemas use a declared supported JSON Schema version or a
  fail-closed inferred Draft 7 or 2020-12 rule set.
- Hosted providers, local llama.cpp, Apple Foundation Models, MCP, and native
  HTTP adapters retain distinct security and transport ownership. Prefer
  consolidating duplicate readers and writers over inventing a universal layer.
- Provider accounts and exact model selections are persisted. New future
  references require a registry readiness proof held through commit.
- Adapter definitions are filesystem-canonical and written only through Noema;
  the running service executes an immutable compiled registry refreshed by
  managed definition changes. The body-free SQLite projection is disposable.
  Current manifests use strict schema version 8.
- The active reviewed Calendar definition has all 12 current operations,
  including date-only event creation. The 50-case live validation ledger has 50
  passing cases. Case 12 made no external change because the human told Noema
  to leave the date-only event uncreated.
- Browser auth is local-human WebAuthn; paired clients use independently
  revocable bearer credentials. Installed PWA behavior follows
  [../frontend/pwa.md](../frontend/pwa.md).
- The iOS client stores normalized GraphQL reads in a protected per-client
  SQLite cache. It clears the active cache during unpair and never queues
  offline writes.
- Final primary-chat replies and new Needs You interventions share one durable
  notification projection. Web Push and direct APNs have separate delivery
  queues; APNs registrations are bearer-client-bound, while the provider `.p8`
  authority lives only in the protected Noema-home credential file.
- One server-driven Tasks Live Activity represents the current active task set.
  SQLite owns its client registration, aggregate projection, and durable APNs
  start, update, alert, and end deliveries. The widget has no bearer credential.
- Frontend route and interaction truth is summarized in
  [../frontend/current-contract.md](../frontend/current-contract.md). UI changes
  follow [../frontend/product-design.md](../frontend/product-design.md).
- Optimize total system simplicity. Follow
  [../development/simplicity.md](../development/simplicity.md) before nontrivial
  architecture, refactoring, workflow, testing-policy, or harness work.

## Open loops

- The Linux workspace gates stop before code validation because the active
  Tauri allowlist rejects the enabled `macos-private-api` feature. The roadmap
  did not change or bypass this desktop configuration.
- Four unit-test failures remain outside the roadmap: two interaction-resume
  tests repeat a provider call ID, one embedded-browser test disagrees with the
  current fragment-URL rule, and one store test expects a different default
  reasoning effort. The interaction failures also occur on `origin/main`.

- Production iOS notifications require Push Notifications enabled for
  `dev.noema.app.ios`, regenerated signing profiles, and an APNs `.p8` provider
  configured from browser Settings. Simulator injection does not replace a
  signed physical-device sandbox-delivery check.
- Production Tasks Live Activities also require the widget App ID
  `dev.noema.app.ios.liveactivity` in the app's regenerated signing profiles.
- Product-scope decisions remain open for the dormant adapter scheduler, the
  long-term set of integration substrates and full-parity clients, task role
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
