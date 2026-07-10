# Phase 1: Web Transport and Authentication

**Target time:** 2 hours 15 minutes
**Hard cap:** 3 aggregate agent-hours
**LOC target:** at least 1,200 net lines deleted
**Risk:** high; one 20-minute read-only security review is allowed
**Prerequisite:** Phase 0 decisions are current

## Goal

Replace Noema's handwritten HTTP parser, HTTP response writer, WebSocket frame
implementation, and GraphQL subscription protocol with Axum, Hyper/Tower, and
the official async-graphql Axum integration. Add a standard private in-memory
session boundary without inventing a general identity platform.

## Files

Primary source:

- Delete: `crates/noema-core/src/daemon/web/http.rs`
- Delete: `crates/noema-core/src/daemon/web/graphql_ws.rs`
- Modify or delete: `crates/noema-core/src/daemon/web/origin.rs`
- Split/trim: `crates/noema-core/src/daemon/web/mod.rs`
- Modify: `crates/noema-core/src/daemon/web/assets.rs`
- Modify: `crates/noema-core/src/daemon/web_server.rs`
- Modify: `crates/noema-core/src/daemon/mod.rs`
- Modify: `crates/noema-core/src/bin/noema_web.rs`
- Modify: workspace and `noema-core` Cargo manifests

Expected focused modules:

- `crates/noema-core/src/daemon/web/router.rs`: route composition only
- `crates/noema-core/src/daemon/web/session.rs`: tower-session bootstrap and
  authenticated-session extractor
- `crates/noema-core/src/daemon/web/authority.rs`: exact listener Host/Origin
  policy, kept under 150 lines
- `crates/noema-core/src/daemon/web/download.rs`: authorized artifact response
- existing provider callback code remains beside its domain service

Do not create compatibility adapters for the raw server.

## Threat Boundary

Defend against:

- remote access caused by a non-loopback bind;
- a hostile webpage reaching loopback APIs through DNS rebinding or cross-origin
  requests;
- another unprivileged local process guessing an application session;
- unauthenticated artifact downloads;
- client-supplied GraphQL or WebSocket identity;
- a compromised secondary Tauri window invoking desktop commands.

Do not model password login, JWTs, organizations, roles, or remote account
recovery in this phase. The current web daemon has one local authenticated
session subject; persisted object ownership remains explicit for future
multi-human work.

## Sequence

### Checkpoint 1A — Axum composition (45 minutes)

- [ ] Add Axum `0.8.9`, `async-graphql-axum 7.2.1`, selected existing
  `tower-http 0.6.11` features,
  and `tower-sessions` with private in-memory sessions.
- [ ] Build one GraphQL schema in `WebState` during server startup.
- [ ] Replace the accept/connection loop with `axum::serve` on a numeric
  loopback `TcpListener`.
- [ ] Add typed routes for GraphQL POST, GraphQL WS, OAuth callback, artifact
  download, assets, and SPA fallback.
- [ ] Preserve current response bodies and attachment filename normalization
  where they are product behavior rather than parser behavior.
- [ ] Run `cargo check -p noema-core` and commit a compiling route skeleton.

Stop this checkpoint if Axum requires a second incompatible Hyper/Tokio stack.
Do not leave both servers callable.

### Checkpoint 1B — Session and authority boundary (45 minutes)

- [ ] Derive canonical authority from the numeric bound loopback socket.
- [ ] Add one short Axum middleware for exact Host validation on every request
  and exact Origin validation on every GraphQL POST and WebSocket upgrade.
- [ ] Use the framework's parsed `HeaderMap`; do not parse raw HTTP or cookie
  strings.
- [ ] Generate a cryptographically random, redacted, one-shot bootstrap
  capability at daemon startup. Consuming it creates a private `HttpOnly`,
  `SameSite=Strict`, path `/` in-memory session and redirects to `/`.
- [ ] Keep OAuth callbacks outside the application cookie requirement; they
  authenticate through their stored expiring state/PKCE attempt.
- [ ] Inject a small server-derived request context into GraphQL HTTP and WS.
  Client payloads cannot supply or replace it.
- [ ] Require the configured main Tauri window for desktop GraphQL commands and
  rely on Tauri capability scoping rather than a second desktop auth system.
- [ ] Add focused Rust unit tests for missing/wrong Host, wrong/missing Origin,
  missing session, bootstrap replay, and wrong Tauri window.

Use framework request construction and temporary listeners. Do not recreate a
raw-protocol adversarial test suite.

### Checkpoint 1C — Protected routes and deletion (45 minutes)

- [ ] Require the authenticated request context for GraphQL application APIs,
  subscriptions, provider/MCP administration, and artifact downloads.
- [ ] Authorize artifact metadata before opening bytes; wrong-owner and missing
  artifacts share the same public response.
- [ ] Keep static assets public after Host validation; they contain no user
  data.
- [ ] Restrict GraphiQL/schema endpoints to debug builds or authenticated
  sessions.
- [ ] Delete raw request parsing, response serialization, WebSocket handshake,
  mask/frame parsing, protocol dispatch, and their parser-specific tests.
- [ ] Remove dependencies used only by the deleted implementation.
- [ ] Run focused `daemon::web`, GraphQL, and desktop tests.

## Library Acceptance

The adoption passes only if:

- `http.rs` and `graphql_ws.rs` are deleted;
- no custom WebSocket framing or SHA-1 accept-key code remains;
- one router/schema instance serves all web operations;
- the session implementation delegates cookie/session lifecycle to
  `tower-sessions` rather than a custom registry;
- the phase deletes at least 1,200 net maintained source lines;
- `cargo tree -d` shows no avoidable duplicate Axum/Hyper/Tower major versions.

If `tower-sessions` does not support the required loopback/private-cookie model
inside 20 minutes, use `tower-cookies` private cookies plus a single in-memory
opaque session value. The fallback is capped at 100 production lines and must
still use library cookie parsing/signing.

## Validation

```bash
cargo fmt --all --check
cargo check -p noema-core -p noema-desktop
cargo clippy -p noema-core --all-targets -- -D warnings
cargo test -p noema-core daemon::web -- --test-threads=1
cargo test -p noema-core graphql:: -- --test-threads=1
cargo test -p noema-desktop
git diff --check
```

If GraphQL schema/types changed:

```bash
cd crates/noema-core/web
bun run gen:types
bun run lint
bun run build
```

No browser or frontend unit tests.

## Exit Evidence

- [ ] Before/after LOC and at least -1,200 net.
- [ ] Dependency tree delta.
- [ ] Exact focused test counts.
- [ ] Security review has no unresolved Critical finding.
- [ ] Tracker transport/auth/download rows updated.
- [ ] Final worktree clean after one or more bounded commits.

## Deferred from This Phase

- Password login and remote browser access.
- General multi-human role administration.
- Full authorization policy language.
- OAuth attempt redesign beyond what is necessary for trusted callbacks.
- Automated browser acceptance.
