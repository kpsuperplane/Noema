# Cruft Audit Plan

> Read-only, multi-agent audit to produce a **verified, prioritized cut-list** of removable
> code — dead paths, legacy dual-paths, speculative stubs, duplication, and needless
> abstraction — so the codebase can be shrunk to a clean, LOC-efficient state **without
> losing functionality**.

**Goal (from the user):** not a specific percentage — remove *all* cruft to reach a clean,
well-built, LOC-efficient codebase. Could be 25%, could be 75%.

**Nature:** this is an **audit only**. No files are edited during the audit. The output is a
plan; execution happens afterward in reviewable slices.

---

## Baseline (why this is worth doing)

Reconnaissance before launching the audit surfaced strong, self-documented evidence of churn
artifacts:

| Signal | Finding |
|---|---|
| Total Rust | ~58.3k LOC across 137 files (3 crates) + ~11k lines of frontend JS/TS/CSS |
| Test vs prod | ~12.8k in dedicated test files + 60 files with inline `#[cfg(test)]` modules; ~45.5k production |
| Explicit dead-code silencers | 22 `#[allow(dead_code)]` / `allow(unused)` sites |
| Legacy dual-path (the standout) | `daemon/memory_pipeline.rs` has 7+ functions kept alive with reasons like *"legacy daemon tests exercise this wrapper while runtime writes use proposal routing"* + 2 marked *"staged for the next slice"* |
| Legacy MCP transport | `mcp/http.rs` documents a *"legacy remote HTTP+SSE MCP transport"* and a *"metadata-only legacy SSE"* transport |
| Removed web routes | `daemon/web/mod.rs` guard tests: `legacy_chat_ws / status / provider_auth _is_no_longer_client_product_api` |
| `dead_code` clusters | 8 in `store/claims/rows.rs`, 4 in `store/conversations.rs`, plus `retrieval.rs`, `runtime.rs`, `runtime_host.rs` |
| Oversized files (project's own 750-line rule) | 21 files over threshold, incl. `daemon/tests.rs` (5,700), `graphql/schema.rs` (3,216) |

**Key blind spot this audit targets:** Rust's `dead_code` lint does **not** flag `pub` items in
a library crate even when nothing calls them. Post-churn, that is exactly where bloat hides —
abandoned public helpers and over-generalized layers with a single caller. Clippy (`-D warnings`)
is already enforced, so trivial dead code is gone; the remaining cruft is the kind only a
reader can find.

---

## Architecture of the audit

Three phases, run as a background workflow. Auditing and verification are **pipelined** — each
subsystem's candidates are verified as soon as its audit finishes, rather than waiting for all
audits to complete.

```
Phase 1: AUDIT          Phase 2: VERIFY              Phase 3: SYNTHESIZE
16 auditors (parallel)  1 adversarial verifier per   1 synthesizer folds all
one per subsystem  ───▶  subsystem, checks each   ───▶ confirmed findings into an
deep-read + flag         candidate against the        ordered cut-list report
cruft candidates         WHOLE repo
```

### Phase 1 — Audit (16 parallel auditors)

Each auditor owns one module area, reads **every** file in scope, and flags cruft candidates.
Ownership is disjoint so there is no overlap or shared state.

| # | Subsystem | Scope |
|---|---|---|
| 1 | `store-general` | `store.rs` + `store/*.rs` (excl. `claims/`, tests) |
| 2 | `store-claims` | `store/claims/*.rs` |
| 3 | `graphql` | `graphql.rs` + `graphql/*.rs` |
| 4 | `daemon-core` | `daemon.rs` + top-level `daemon/*.rs` (excl. runtime/web/pipeline/tests) |
| 5 | `daemon-runtime` | `daemon/runtime.rs` + `daemon/runtime/*.rs` |
| 6 | `daemon-web` | `daemon/web/*.rs` (Rust only) |
| 7 | `memory-pipeline` | `daemon/memory_pipeline.rs` (**hotspot**) |
| 8 | `memory` | `memory.rs` + `memory/**` |
| 9 | `memory-persistence` | `memory_persistence.rs` + `memory_persistence/*.rs` |
| 10 | `mcp` | `mcp.rs` + `mcp/**` |
| 11 | `provider-core` | `provider.rs` + `provider/*.rs` (excl. adapters) |
| 12 | `provider-adapters` | `provider/adapters/*.rs` |
| 13 | `core-misc` | `lib.rs`, `runtime_host.rs`, `config/**`, `capability/**`, unused-dep check on `Cargo.toml` |
| 14 | `cli` | `noema-cli/src/**` + its `Cargo.toml` |
| 15 | `desktop` | `noema-desktop/src/**` + its `Cargo.toml` |
| 16 | `frontend` | `crates/noema-core/web/**` + `daemon/web/assets/**` |
| 17 | `cross-cutting-duplication` | Whole `crates/` tree — *cross-module* duplication & needless abstraction that single-subsystem auditors structurally cannot see |

**Cruft categories each auditor hunts:**

- `dead_code` — `pub`/private items referenced nowhere (compiler won't flag `pub` in a lib).
- `legacy_dual_path` — old path kept alive after the runtime moved to a new one.
- `speculative` — code written ahead of any caller ("staged for next slice").
- `dead_test` — tests that only exercise dead/legacy code, or duplicate other tests.
- `duplication` — logic duplicated within the subsystem.
- `over_abstraction` — traits/generics with one implementor, forwarding-only wrappers, needless indirection.
- `oversized_refactor` — >750-line files carrying removable bloat (the removable part, not the whole file).
- `unused_dep` — declared dependency never used.

Each finding is emitted as structured data: `id, file, lines, loc (est. removable), category,
symbol, summary, evidence, references_to_check, confidence, risk`. Crucially, auditors do **not**
assert repo-wide deadness — for anything suspected dead beyond its own module, the auditor records
the exact symbol names in `references_to_check` and defers the judgment to Phase 2.

### Phase 2 — Verify (adversarial, repo-wide)

One verifier per subsystem re-checks **every** candidate against the **entire repository**: all
three crates, all tests, the frontend, the GraphQL schema, and ts-rs generated bindings. This is
the safety net that prevents cutting something that only *looks* dead.

For each candidate the verifier greps every symbol in `references_to_check` and classifies each
reference as:

- **LIVE** — used from production code, or from a test guarding a live feature.
- **NOT LIVE** — only the definition itself, dead/legacy tests, or other already-dead code.

**Verdict rules (principled, not reflexively conservative):**

| Verdict | Condition |
|---|---|
| `keep` | Any live reference exists (reported with location) |
| `confirmed_cruft` | Zero live references (and for legacy paths, the runtime provably uses a different path) |
| `needs_human` | Usage genuinely undeterminable statically — dynamic dispatch that can't be traced, macro-generated names, or serde/GraphQL/ts-rs name resolution consumed by the frontend |

Special caution flags baked into the verifier: **GraphQL types, ts-rs exports, serde-derived
structs, trait impls used via dynamic dispatch, and macro-reachable names** all look unused but
may not be — these route to `needs_human` rather than `confirmed_cruft` when unresolvable.

### Phase 3 — Synthesize

One synthesizer folds all `confirmed_cruft` and `needs_human` findings into a prioritized
Markdown cut-list:

1. **Executive summary** — total confirmed removable LOC, item count, the 3–5 biggest wins.
2. **Removal slices** — confirmed items grouped into an *ordered* sequence of independent,
   safely-shippable slices (safest + highest-ROI first), each with files/symbols touched, LOC
   removed, ordering notes, and the validation to run.
3. **Consolidation / refactor opportunities** — duplication & over-abstraction that shrink code
   but need care (not pure deletions).
4. **Needs-your-decision** — `needs_human` items, each as a crisp yes/no question.
5. **Totals** — removable LOC by subsystem and by category.

---

## What happens after the audit

1. Review the cut-list; answer the `needs_human` questions.
2. Execute **slice by slice**, each independently validated and committed:
   - `cargo fmt --all --check`
   - `cargo check --workspace`
   - `cargo clippy --workspace --all-targets -- -D warnings`
   - `cargo test --workspace --no-fail-fast`
3. Safest / highest-ROI slices first. Expected Slice 1: **remove the legacy memory-pipeline
   dual path + the daemon tests that exist only to exercise it.**

## Guardrails

- **Read-only audit.** No edits until slices are approved.
- **Preserve unrelated dirty worktree changes** (there are in-flight edits to web assets & theme CSS).
- **No compatibility layers / migrations** — the project is pre-V1 and explicitly forbids them
  unless requested, which makes aggressive deletion safe.
- **Never touch the `sccache` / `CARGO_BUILD_RUSTC_WRAPPER` build cache** during validation.
- A candidate is only deleted once verification proves zero live references; ambiguous cases
  become user decisions, never silent deletions.
