# Phase 5: Measured Crate Extraction

**Target time:** 2 hours
**Hard cap:** 2.5 aggregate agent-hours
**LOC allowance:** no more than 300 net lines added
**Prerequisite:** deletion phases have left cumulative source LOC at or below
89,500 before this phase starts

## Goal

Extract only crate boundaries that measurably improve focused and incremental
builds. The committed target for this program is `noema-server` followed by
`noema-store`. Additional proposed crates remain sequenced but are not pulled
into the 16-hour program unless both extractions finish early and the global LOC
gate is already safe.

## Intended Dependency Direction

```text
noema-store --------------------|
                                |---> noema-core ---> noema-server
                                |          |
                                |          `--------> noema-desktop
                                |
future: noema-domain -----------|
future: noema-integrations ---> noema-runtime ---> noema-core
future: noema-api -------------------------------> noema-server/noema-desktop
```

Lower-level crates never depend on `noema-core`. `noema-core` remains the
composition facade and owns `NoemaRuntimeHost`.

## Measurement Protocol

Before moving files:

- [x] Capture a clean `cargo build --workspace --timings` duration once.
- [x] Capture warm `cargo check` duration for `noema-core` and
  `noema-desktop` three times; use the median.
- [x] Capture representative warm checks after a no-op-preserving edit to a web
  route, store query, provider adapter, and runtime module, restoring the edit
  after each measurement.
- [x] Record rustc unit counts and which packages rebuild.
- [x] Do not clear sccache, change the compiler wrapper, or delete shared cache
  state.

Use wall-clock timings only as directional evidence; the primary gate is
package invalidation and focused package-check duration.

## Sequence

### Checkpoint 5A — Extract `noema-server` (60 minutes)

Create:

- `crates/noema-server/Cargo.toml`
- `crates/noema-server/build.rs` when release asset embedding still needs it
- `crates/noema-server/src/lib.rs`
- `crates/noema-server/src/web/**`
- `crates/noema-server/src/bin/noema_web.rs`
- `crates/noema-server/src/bin/noema_dev.rs`
- server-owned tests beside the moved modules

Move from `noema-core`:

- daemon HTTP/WebSocket transport composed in Phase 1;
- web route and asset serving;
- browser-facing OAuth callback transport only, not provider domain logic;
- `noema_web` and `noema_dev` binaries;
- frontend asset build/embed integration.

Requirements:

- [x] `noema-server` depends on the narrow `noema-core` composition/API surface.
- [x] `noema-core` no longer has a build script invalidated by frontend assets or
  sidecar source.
- [x] Server-only dependencies leave `noema-core` when no longer used there.
- [x] Product behavior and generated frontend paths remain unchanged.
- [x] `cargo check -p noema-core` does not compile server-only route/asset code.
- [x] Commit and measure before beginning `noema-store`.

Reject the extraction if it needs broad re-exports of internal runtime/provider
types or exceeds +200 net lines by itself.

### Checkpoint 5B — Extract `noema-store` (60 minutes)

**Gate result:** rejected for this program. The store imports conversation,
provider, MCP, path, diagnostic, and artifact vocabulary from core; extracting
it now would require a premature domain crate, an estimated 140–240 lines of
new glue, and no demonstrated build-time gain. The prerequisite cleanup is
recorded in `deferred-backlog.md`.

Create:

- `crates/noema-store/Cargo.toml`
- `crates/noema-store/src/lib.rs`
- store schema, repositories, row models, SQLite worker, errors, and tests

Move from `noema-core`:

- `crates/noema-core/src/store.rs`
- `crates/noema-core/src/store/**`
- only the persistence records and IDs truly owned by the store

Requirements:

- [ ] `noema-store` depends on SQLite/time/serde and small shared domain types,
  not GraphQL, providers, MCP transports, web parsing, or frontend assets.
- [ ] If extracting shared IDs is required to avoid a cycle, create only a tiny
  `noema-domain` crate containing those IDs and stable value types. Charge its
  LOC to the +300 allowance.
- [ ] Keep SQL and row conversion in `noema-store`; keep runtime orchestration in
  `noema-core`.
- [ ] Remove facade wrappers that add no policy or adaptation.
- [ ] Move store tests rather than duplicating them.
- [ ] `cargo test -p noema-store` runs without compiling server/provider code.
- [ ] Commit and measure the extraction independently.

Reject the extraction if it requires a temporary reverse dependency or duplicate
record models.

## Continuation Gate

The following crates remain in the longer-term sequence:

1. `noema-domain`: stable IDs, conversation/tool/provider contracts, typed
   errors; keep dependencies minimal.
2. `noema-runtime`: turns, tool loop, context planning, transcript persistence,
   supervised workers.
3. `noema-integrations`: providers, MCP, Mnemosyne, search, fetch.
4. `noema-api`: GraphQL schema, resolvers, subscriptions, API read models.
5. Reduce `noema-core` to composition and curated re-exports.

Continue into one of these during the global reserve only when all conditions
hold:

- both committed extractions passed;
- cumulative LOC is at least 1,000 lines below the 89,000 final gate;
- at least two representative warm checks improved by 20% or more;
- clean workspace build regressed by no more than 10%;
- the next boundary is already obvious and can finish in the allocated reserve.

Otherwise record these crates in `deferred-backlog.md` and stop.

## Validation

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p noema-store --no-fail-fast
cargo test -p noema-core --no-fail-fast
cargo test -p noema-server --no-fail-fast
cargo test -p noema-desktop --no-fail-fast
git diff --check
```

When server assets moved:

```bash
cd crates/noema-core/web
bun run gen:types
bun run lint
bun run build
```

Update the frontend path if Phase 5 relocates it; do not move the React
application solely to make the crate diagram symmetric.

## Exit Evidence

- [x] Net phase addition is 300 lines or fewer.
- [x] Clean and warm before/after timings recorded.
- [x] Focused server tests avoid unrelated packages; the store extraction was
  rejected before implementation.
- [x] Workspace dependency graph is acyclic.
- [x] `noema-core` has no server asset build invalidation.
- [x] Only measured, completed crate boundaries are checked off in the tracker.

## Completed Evidence

- Commit: `e3e00321`.
- Maintained source: 88,642 before, 88,571 after (`-71`).
- Baseline workspace build: 19.58s, 597 units (592 fresh, 5 dirty); generated
  assets rebuilt core, its three binaries, and desktop.
- First post-extraction workspace build: 20.10s (`+2.6%`, within the 10% gate).
- Asset-only workspace rebuild: 4.43s and only `noema-server`, down 77% from
  the baseline asset-triggered build.
- Representative core edits: about 3.4s, down from about 5.45s (38%). Warm
  no-op medians remained 0.16s for core and 0.20s for desktop.
- Full gate: 623 core, 17 server, 10 dev-supervisor, and 7 desktop tests passed;
  formatting, workspace check, strict Clippy, frontend generation/routes/lint/
  build, and diff checks passed. A first overloaded run exposed known parallel
  Foundation timeouts; targeted reruns and the subsequent full run were green.
- Two Luna-max reviews found no remaining Critical or Important issue after
  artifact authorization/filesystem/header coverage was restored.
