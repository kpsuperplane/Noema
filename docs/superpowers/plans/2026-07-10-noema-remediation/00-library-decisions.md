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
| [Axum 0.8.9](https://github.com/tokio-rs/axum) | HTTP routing, extraction, responses, WebSocket upgrade | `daemon/web/http.rs`, `daemon/web/graphql_ws.rs`, routing in `daemon/web/mod.rs` | Use HTTP1/Tokio/WS only; delete raw framing with no dual server path |
| [async-graphql-axum 7.2.1](https://github.com/async-graphql/async-graphql) | GraphQL HTTP and subscription integration | Manual GraphQL request and `graphql-transport-ws` handling | Exact match for locked `async-graphql` 7.2.1 |
| [tower-http 0.6.11](https://github.com/tower-rs/tower-http) | Request limits, CORS/origin policy support, sensitive headers, response helpers | Custom request-size and common response plumbing | Reuse the already-resolved 0.6.11; enable only limit/cors/sensitive/set-header features and not `fs` |
| [tower-sessions 0.15.0](https://github.com/maxcountryman/tower-sessions) | Private in-memory daemon sessions and cookie lifecycle | Any proposed custom session registry/cookie parser | Disable defaults; enable `axum-core`, `memory-store`, and `private`; no database-backed sessions |
| [tokio-util 0.7.18](https://github.com/tokio-rs/tokio) | `CancellationToken` and `TaskTracker` | Ad hoc cancellation channels and detached task collections | Add as direct dependency with `rt`; already locked transitively |
| [tokio-rusqlite 0.7.0](https://github.com/programatik29/tokio-rusqlite) | Run SQLite work on a dedicated blocking worker | `Arc<Mutex<rusqlite::Connection>>` on async executor paths | Disable defaults; resolves to existing `rusqlite` 0.37 and one SQLite binding |
| [oauth2 5.0.0](https://github.com/ramosbugs/oauth2-rs) | PKCE, CSRF/state, token response and expiry primitives | Equivalent custom OAuth protocol primitives | Disable defaults and use core types only; enabling its reqwest feature would add reqwest 0.12 beside Noema's 0.13 |
| [secrecy 0.10.3](https://github.com/iqlusioninc/crates/tree/main/secrecy) | Redacted secret wrappers | Handwritten redacted `Debug` wrappers and accidental secret exposure | No features; adopt only at boundaries touched in Phase 2 |
| [jsonschema 0.47.0](https://github.com/Stranger6667/jsonschema) | Validate reviewed MCP arguments/results | Handwritten partial JSON-schema checks | Disable all default features and remote/file resolution; adopt only if it removes meaningful validation code |
| [atomic-write-file 0.3.0](https://github.com/andreacorbellini/rust-atomic-write-file) | Atomic private credential/config writes | Repeated temp-write/rename helpers | Conditional: explicitly create new Unix secrets as `0600`; accept new `nix`/`rand` versions only if net deletion justifies them |

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
| `axum-login 0.18.0` | Reject for this pass | No current password backend; it also requires `tower-sessions` 0.14 rather than selected 0.15 |
| `deadpool-sqlite 0.13.0` | Defer | Requires `rusqlite` 0.38; `tokio-rusqlite` matches current 0.37 and pooling is unnecessary |
| `r2d2_sqlite 0.35.0` | Defer | Requires `rusqlite` 0.40 and a synchronous pool is the wrong boundary |
| `rusqlite_migration 2.6.0` | Defer | Requires `rusqlite` 0.40; pre-V1 reset policy does not require migrations |
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

- [x] Re-run the LOC baseline and record 93,924 or explain repository drift.
- [x] Verify current stable versions and license/MSRV for adopt-now crates.
- [x] Confirm `async-graphql-axum` matches `async-graphql` 7.
- [x] Confirm `tokio-rusqlite` resolves to `rusqlite` 0.37 only.
- [x] Save the dependency matrix in the Phase 0 progress entry.
- [x] Commit only if a plan/dependency metadata change was required.

## Verified Baseline — 2026-07-10

- Maintained source LOC: `93,924`.
- Toolchain: Rust/Cargo `1.96.0` on `aarch64-apple-darwin`; sccache `0.16.0`
  through the repository `.cargo/rustc-wrapper`.
- Locked GraphQL: `async-graphql 7.2.1`; selected integration:
  `async-graphql-axum 7.2.1`.
- Locked HTTP stack: `http 1`, Hyper `1.10.1`, Tower `0.5`; selected Axum
  `0.8.9` and existing tower-http `0.6.11` do not require another major stack.
- `oauth2 5.0.0` and `tokio-util 0.7.18` are already transitive through current
  dependencies.
- `tokio-rusqlite 0.7.0` declares `rusqlite ^0.37.0` and therefore shares
  Noema's current SQLite binding.
- All selected runtime crates are compatible with the workspace Rust 1.96
  floor and use permissive MIT, Apache-2.0, MIT/Apache-2.0, or BSD-3-Clause
  licenses.
