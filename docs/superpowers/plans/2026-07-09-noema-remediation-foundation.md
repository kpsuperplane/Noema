# Noema Remediation Foundation Milestone

**Goal:** Establish the program control plane, green validation, deterministic
browser testing, live baseline, reproducibility evidence, and licensing inputs
required before security and schema implementation begins.

## Ordered Units

### F1 — Program ledger and coverage validation

- Add immutable unique IDs to every canonical checkbox in
  `docs/codebase-audit-tracker.md`, including leaf work, boundary guardrails,
  and milestone rollups.
- Create `docs/codebase-audit-ledger.md` with exactly one primary milestone,
  known consumers, automated coverage, browser scenario, acceptance evidence,
  and completion commit for every ID.
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

### F4 — Minimal CI and generated state

- Add least-privilege CI with bounded concurrency for Rust, frontend, Python
  validation available today, and generated-state cleanliness.
- Build frontend assets before compiling release consumers and verify that
  served `index.html` and referenced route chunks exist.
- Do not check the broad final CI or release tracker items until their complete
  later matrices pass.

### F5a — Injectable daemon/browser seam

- Refactor listener/runtime construction just enough for an ephemeral test
  launcher to receive a resolved random loopback address and deterministic
  provider/runtime dependencies.
- Add characterization tests first and prove test-provider code is absent from
  production binaries/configurations.

### F5b — Browser artifact sanitization

- Add failure-only screenshot/trace capture with sanitized headers, variables,
  bodies, paths, and storage state.
- Add tests proving secret canaries never enter retained artifacts.

### F5c — Deterministic browser scenarios

- Run against a temporary `NOEMA_HOME`, random port, seeded non-secret state,
  and deterministic provider.
- Cover onboarding, empty conversation, submission/streaming/replay, Memory and
  Usage routes, console errors, unhandled requests, and GraphQL errors.
- Prove database recreation preserves sentinel provider files in the temporary
  home without reading live credentials.
- Own and join daemon/browser processes and temporary state with bounded
  shutdown; require three consecutive local passes plus CI.

### F6 — Live `~/.noema` browser baseline

- Root builds the exact assets, starts the exact daemon on loopback, waits for
  readiness, and uses the in-app browser.
- Inspect shell boot, existing chat rendering, navigation, route chunks,
  Providers, Memory, MCP, Usage, artifacts, WebSocket stability, failed
  requests, and console errors while accepting valid ready/unavailable/empty
  states.
- Send one minimal Codex chat and observe streaming and a durable terminal
  state. Do not mutate provider, MCP, memory, settings, or database state.
- Store raw evidence only in ignored private artifacts and commit a sanitized
  summary without transcript, memory, account, token, request-body, cookie,
  storage, or credential content.

### F7a — Root license

- Add the standard MIT license with `Copyright (c) 2026 Noema contributors`.

### F7b — Provisional redistribution inventory

- Inventory locked transitive Rust, Bun, current Python, Swift/Foundation,
  Tauri, asset, font, icon, installer, and updater inputs by shipped artifact.
- Record package, version, license expression, source, bundled status, and
  notice/source obligations; generate `THIRD_PARTY_NOTICES.md` where required.
- Fail the release gate for unknown, incompatible, or restricted licenses. Mark
  the inventory provisional until Milestone 5 freezes Python/model/package
  contents.

## Foundation Exit Gate

- Full Rust and frontend validation passes.
- Minimal CI and generated-state checks pass from a clean checkout.
- The deterministic browser harness passes three consecutive local runs.
- The sanitized live browser baseline completes without credential exposure or
  unintended mutations.
- Every canonical tracker checkbox has one ledger owner.
- Root verifies the diff, runs the ship checklist, commissions a Sol-high
  whole-milestone review, commits, and updates `docs/context/current.md`.
