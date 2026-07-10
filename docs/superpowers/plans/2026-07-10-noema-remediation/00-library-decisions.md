# Phase 0: Baseline and Library Decisions

**Timebox:** 30 minutes target, 45 minutes hard cap
**LOC budget:** zero
**Outcome:** Pin a small adopt/defer/reject matrix before implementation starts.

## Research Standard

Use the current crate documentation and upstream repository as primary sources.
Check MSRV, license, most recent stable release, compatibility with Noema's
locked dependency graph, default features, and whether the candidate eliminates
an existing custom path.

Do not spend more than ten minutes researching one candidate. If compatibility
or value remains unclear, defer it.

## Adopt

| Utility | Intended use | Existing code it must replace | Decision gate |
| --- | --- | --- | --- |
| [Axum 0.8](https://github.com/tokio-rs/axum) | HTTP routing, extraction, responses, WebSocket upgrade | `daemon/web/http.rs`, `daemon/web/graphql_ws.rs`, routing in `daemon/web/mod.rs` | Delete raw HTTP and WebSocket framing; no dual server path |
| [async-graphql-axum 7](https://github.com/async-graphql/async-graphql) | GraphQL HTTP and subscription integration | Manual GraphQL request and `graphql-transport-ws` handling | Version must match `async-graphql` 7 |
| [tower-http](https://github.com/tower-rs/tower-http) | Request limits, CORS/origin policy support, sensitive headers, static response helpers | Custom request-size and common response plumbing | Enable only used features |
| [tower-sessions 0.15](https://github.com/maxcountryman/tower-sessions) | Private in-memory daemon sessions and cookie lifecycle | Any proposed custom session registry/cookie parser | No database-backed session store; exact-origin/Host checks remain small middleware |
| [tokio-util](https://github.com/tokio-rs/tokio) | `CancellationToken` and `TaskTracker` | Ad hoc cancellation channels and detached task collections | Add as direct dependency using `rt` only; it is already transitive |
| [tokio-rusqlite 0.7](https://github.com/programatik29/tokio-rusqlite) | Run SQLite work on a dedicated blocking worker | `Arc<Mutex<rusqlite::Connection>>` on async executor paths | It must continue using `rusqlite` 0.37 and avoid a second SQLite build |
| [oauth2 5](https://github.com/ramosbugs/oauth2-rs) | PKCE, CSRF/state, token response and expiry primitives | Equivalent custom OAuth protocol primitives | Already transitive through `rmcp`; do not add a parallel HTTP stack |
| [secrecy](https://github.com/iqlusioninc/crates/tree/main/secrecy) | Redacted secret wrappers | Handwritten redacted `Debug` wrappers and accidental secret exposure | Adopt only at boundaries touched in Phase 2 |
| [jsonschema](https://github.com/Stranger6667/jsonschema) | Validate reviewed MCP arguments/results | Handwritten partial JSON-schema checks | Disable network/file resolution features; compile reviewed local schemas only |
| [atomic-write-file](https://github.com/andreacorbellini/rust-atomic-write-file) | Atomic private credential/config writes | Repeated temp-write/rename helpers | Verify permission and durability semantics on macOS/Linux before adoption |

## Use as External Tooling

These are CI/release tools, not runtime dependencies:

- [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) for license,
  advisory, ban, and source policy.
- [cargo-about](https://github.com/EmbarkStudios/cargo-about) for Rust notices.
- [cargo-cyclonedx](https://github.com/CycloneDX/cyclonedx-rust-cargo) for a
  Rust SBOM.
- Tauri's official bundler, signing, and updater facilities for desktop
  releases; do not build parallel packaging machinery.
- `cargo-shear` as a local/CI dependency audit if it runs reliably on this
  workspace.

## Defer or Reject

| Candidate | Decision | Reason |
| --- | --- | --- |
| `axum-login` | Reject for this pass | Noema has no current password/login backend; transport sessions are enough |
| `deadpool-sqlite` | Defer | Current release uses a different `rusqlite` version; `tokio-rusqlite` matches exactly |
| `r2d2_sqlite` | Defer | Different `rusqlite` version and a pool is unnecessary for one-writer SQLite |
| `rusqlite_migration` | Defer | Current release uses a different `rusqlite`; pre-V1 reset policy does not require migrations |
| `cargo-dist` | Defer | Tauri already owns installed desktop bundles; adopt only if it demonstrably replaces release workflow code |
| New frontend state/UI framework | Reject by default | Existing React, Apollo, TanStack Router, Astryx, and StyleX are sufficient; adoption must be net-LOC-negative |
| General DI/framework crates | Reject | Composition is explicit and does not justify another abstraction layer |
| Password hashing/JWT crates | Reject | No password or bearer-JWT product surface exists in this pass |

## Dependency Rules

- Pin compatible major/minor versions in workspace dependencies.
- Disable unused default features.
- Run `cargo tree -d` after each adoption and reject avoidable duplicate protocol,
  TLS, SQLite, or HTTP stacks.
- A dependency with no direct use after its phase is removed immediately.
- Record gross deleted LOC, gross added LOC, and net delta for every adoption.

## Exit Checklist

- [ ] Re-run the LOC baseline and record 93,924 or explain repository drift.
- [ ] Verify current stable versions and license/MSRV for adopt-now crates.
- [ ] Confirm `async-graphql-axum` matches `async-graphql` 7.
- [ ] Confirm `tokio-rusqlite` resolves to `rusqlite` 0.37 only.
- [ ] Save the dependency matrix in the Phase 0 progress entry.
- [ ] Commit only if a plan/dependency metadata change was required.
