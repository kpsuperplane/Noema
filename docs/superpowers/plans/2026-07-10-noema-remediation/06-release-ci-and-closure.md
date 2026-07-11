# Phase 6: Release, CI, and Closure

**Target time:** 1 hour 15 minutes
**Hard cap:** 1 hour 45 minutes before global reserve
**LOC target:** at least 1,000 net lines deleted, or whatever deletion is needed
to finish at 89,000 lines or fewer
**Prerequisite:** all earlier landed phases are committed

## Goal

Remove the last obsolete dependencies and compatibility code, replace custom
license/SBOM/release machinery with established tooling, run full validation,
and leave an honest tracker and release contract.

## Sequence

### Checkpoint 6A — Deletion and dependency sweep (30 minutes)

- [ ] Run `cargo-shear` and confirm every finding with `rg` before removal.
  The tool was unavailable; direct dependencies were audited with `rg` instead.
- [x] Inspect Cargo duplicate versions with `cargo tree -d`; remove avoidable
  duplicates introduced by the program.
- [x] Remove confirmed obsolete transport/provider helpers, stale frontend
  dependencies, generated Python artifacts, and wrapper types made redundant
  by the remediation; no unverified feature or binary was removed.
- [x] Search for the raw HTTP/WebSocket, old Responses dialect, old SQLite
  mutex, detached-task, and duplicated atomic-writer symbols removed by prior
  phases. Delete residual dead paths rather than leaving deprecated shims.
- [x] Consolidate repeated validation/dev commands only when one existing tool
  can own them; do not add a large bespoke script.
- [x] Re-run the LOC command. Allocate reserve only if the total exceeds 89,000
  and a safe concrete deletion remains.

### Checkpoint 6B — Standard release and compliance tooling (25 minutes)

- [x] Add the root MIT `LICENSE`.
- [ ] Configure `cargo-deny` for licenses, advisories, sources, and duplicate
  bans that are currently actionable.
- [ ] Configure `cargo-about` to generate Rust redistribution notices instead
  of maintaining a handwritten crate inventory.
- [ ] Add a `cargo-cyclonedx` command for Rust SBOM generation; use the package
  manager's standard lockfile metadata for the frontend/Python components.
- [x] Keep Tauri's official build/bundle/sign/updater path authoritative. Add
  only the workflow/configuration currently possible without signing secrets.
- [x] Make release asset generation fail when the built entry asset or required
  route chunks are absent.
- [ ] Pin Python sidecar dependencies with the existing Python packaging tool;
  do not create a second lock format.
- [x] Document clean-machine prerequisites and the exact unsigned developer
  bundle command. Signing, notarization, and updater publication remain
  deferred until credentials exist.

### Checkpoint 6C — Full validation and accounting (20 minutes plus command time)

Run:

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

When relevant:

```bash
cd crates/noema-core/web
bun run gen:types
bun run gen:routes
bun run lint
bun run build
```

Use the post-Phase-5 frontend path if it changed. Run Python unit tests for the
managed sidecar. Do not run smoke, fixture, frontend unit, browser, or installed
application tests.

Then:

- [x] Run `git diff --check` and confirm only the preserved user `AGENTS.md`
  change remains after commits.
- [x] Record final LOC, total delta, and percentage change.
- [x] Record aggregate active agent time by phase and reserve allocation.
- [x] Update `docs/codebase-audit-tracker.md` only for evidence-backed completed
  items.
- [x] Update `deferred-backlog.md` with uncompleted attempts and newly discovered
  work.
- [x] Reduce `docs/context/current.md` only where current architecture or commands
  changed.
- [x] Have one Luna-max reviewer inspect the complete program diff for Critical
  correctness/security/data-loss findings. Cap review at 20 minutes and permit
  one correction round.
- [x] Do not push.

## Completed Evidence

- Commits: `7d6f1386`, `f813d0b1`, `943a1efa`, and final security correction
  `98a8d777`.
- Maintained source: 88,571 before, 88,807 after (`+236`); program total is
  `-5,117` lines (`-5.45%`) from the 93,924 baseline. Phase 6 exceeded its
  deletion target only because the final review required 255 lines of focused
  MCP fail-closed implementation and coverage; the program remains under its
  89,000-line hard gate.
- Phase 6A centralized three persistence conversions, collapsed duplicate web
  provider records, removed a one-use server wrapper and unused direct
  dependency, and eliminated the duplicate scraper 0.27 parser stack (`-85`).
- Phase 6B added the MIT license, Python artifact hygiene, generated-source
  cleanliness, honest developer-bundle documentation, and a Vite-manifest
  release gate. Positive release builds passed; missing, nested, traversal, and
  absolute manifest asset cases failed closed (`+66`).
- CI now runs Rust on macOS, frontend generation/lint/build plus release asset
  embedding, and Python sidecar unit tests. `cargo-deny` remains explicitly
  partial/deferred because no tool or validated policy exists.
- Final review found and closed one Critical gap: write/export MCP tools are now
  excluded from enabled store projection and model exposure and revalidated at
  dispatch, while current Ready read-only tools remain available.
- Final local gate: formatting, workspace check, strict Clippy, 629 core + 17
  server + 10 dev-supervisor + 7 desktop tests, frontend frozen install/
  generated/lint/build, Python 5/5, release server build, LOC, and diff checks
  passed.
- Estimated aggregate active agent time: approximately 12 agent-hours, below
  the 16-hour cap: Phase 0 `0.2h`, Phase 1 `3.0h`, Phase 2 `1.5h`, Phase 3
  `1.4h`, Phase 4 `1.0h`, deletion reserve `1.0h`, Phase 5 `1.7h`, and Phase 6
  `2.2h`. The reserve funded the two deletion slices that secured the LOC gate;
  the final security correction stayed within the review reserve.

## CI Minimum

Add or update a small CI workflow that runs existing authoritative commands:

- Rust formatting, check, clippy, and unit tests;
- frontend generation/lint/build;
- Python unit tests;
- generated-artifact cleanliness;
- `cargo-deny` policy;
- release asset presence on the release job.

Do not add browser matrices, screenshot artifacts, platform packaging matrices,
or installed-app tests in this 16-hour program. A CI job is accepted only if it
runs an existing local command rather than embedding another implementation.

## Completion Gate

The program may be called complete only when:

- maintained source LOC is **89,000 or fewer**;
- all full validation commands applicable on the current platform pass;
- no Critical final-review finding remains;
- every public dependency added has a direct owner and replaces custom code;
- crate extraction evidence meets Phase 5's gate or the incomplete extraction
  was reverted;
- tracker and deferred backlog match actual evidence;
- aggregate active agent time is under 16 hours.

The 86,500-line stretch target is desirable but never justifies deleting useful
coverage or rushing a correctness fix.

## Final Report

Report:

- outcome and final LOC first;
- phase-by-phase time and LOC table;
- custom systems deleted and public utilities adopted;
- validation results;
- commits created;
- deferred risks ordered by practical severity;
- remaining dirty/untracked files;
- explicit statement that nothing was pushed.
