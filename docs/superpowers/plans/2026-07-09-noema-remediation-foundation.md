# Noema Remediation Foundation Milestone

**Goal:** Establish the program control plane, proportionate green validation,
reproducibility evidence, and licensing inputs required before security and
schema implementation begins.

## Ordered Units

### F1 — Program ledger and coverage validation

- Add immutable unique IDs to every canonical checkbox in
  `docs/codebase-audit-tracker.md`, including leaf work, boundary guardrails,
  and milestone rollups.
- Create `docs/codebase-audit-ledger.md` with exactly one primary milestone,
  known consumers, automated coverage, operational verification, acceptance
  evidence, and completion commit for every ID.
- Add a lightweight repository check rejecting missing, duplicate, unknown, or
  multiply owned IDs. Deferred ownership never marks an item complete.

### F2 — Reproducibility baseline

- Record OS/architecture, Rust/Cargo, Bun, Python, Swift/Xcode, Tauri, dependency
  lock hashes, commands, and artifact hashes.
- Capture three-run median clean/no-op/representative incremental Rust timings
  using a dedicated target directory without changing the configured compiler
  wrapper or clearing shared `sccache` state.
- Record raw and gzip size for every eager/lazy frontend JS and CSS output.
- Keep timing results informational until the measured Milestone 7 gates are
  established.

### F3 — Frontend test contract

- Add `test` and bounded `test:ci` scripts.
- Correct the two stale shell-navigation expectations for Memory and Usage.
- Regenerate routes through the authoritative generator; never hand-edit the
  generated route tree.
- Require generation, tests, lint, build, and a clean generated diff twice.

### F4a — Generated state and built-asset verification

- Add a test-first repository verifier for generated GraphQL schema/types,
  route-tree cleanliness, required web entry assets, and every asset reference
  reachable from `index.html`.
- Expose focused frontend package scripts that CI and developers run without
  duplicating generation logic.

### F4b — Minimal CI

- Add least-privilege CI with bounded concurrency for Rust, frontend, Python
  validation available today, and generated-state cleanliness.
- Build frontend assets before compiling release consumers and verify that
  served `index.html` and referenced route chunks exist.
- Do not check the broad final CI or release tracker items until their complete
  later matrices pass.

### F5a — Root license

- Add the standard MIT license with `Copyright (c) 2026 Noema contributors`.

### F5b — Provisional redistribution inventory

- Record locked ecosystem counts and distribution classes plus direct runtime
  inputs for Rust/Tauri, bundled frontend assets, the current Python sidecar,
  Swift/Foundation, and repository-owned assets.
- Identify unknown, restricted, incompatible, and review-required transitive
  classes explicitly; every unresolved item remains a release blocker.
- Record the provisional notice/source obligations and root
  `THIRD_PARTY_NOTICES.md` boundary without hand-transcribing the full lockfiles.
- Defer exhaustive target-specific per-package and per-artifact notices to
  Milestone 5, when the actual Python/model/package, installer, updater,
  signing, and release bundles are frozen.

## Foundation Exit Gate

- Full Rust and frontend validation passes.
- Minimal CI and generated-state checks pass from a clean checkout.
- Every canonical tracker checkbox has one ledger owner.
- Root verifies the diff, runs the ship checklist, commissions a Sol-high
  whole-milestone review, commits, and updates `docs/context/current.md`.
