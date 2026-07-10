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
- [ ] Inspect Cargo duplicate versions with `cargo tree -d`; remove avoidable
  duplicates introduced by the program.
- [ ] Remove dead feature flags, obsolete transport/provider compatibility
  modules, unused binaries, stale frontend dependencies, ignored generated
  Python artifacts, and wrapper modules made redundant by crate extraction.
- [ ] Search for the raw HTTP/WebSocket, old Responses dialect, old SQLite
  mutex, detached-task, and duplicated atomic-writer symbols removed by prior
  phases. Delete residual dead paths rather than leaving deprecated shims.
- [ ] Consolidate repeated validation/dev commands only when one existing tool
  can own them; do not add a large bespoke script.
- [ ] Re-run the LOC command. Allocate reserve only if the total exceeds 89,000
  and a safe concrete deletion remains.

### Checkpoint 6B — Standard release and compliance tooling (25 minutes)

- [ ] Add the root MIT `LICENSE`.
- [ ] Configure `cargo-deny` for licenses, advisories, sources, and duplicate
  bans that are currently actionable.
- [ ] Configure `cargo-about` to generate Rust redistribution notices instead
  of maintaining a handwritten crate inventory.
- [ ] Add a `cargo-cyclonedx` command for Rust SBOM generation; use the package
  manager's standard lockfile metadata for the frontend/Python components.
- [ ] Keep Tauri's official build/bundle/sign/updater path authoritative. Add
  only the workflow/configuration currently possible without signing secrets.
- [ ] Make release asset generation fail when the built entry asset or required
  route chunks are absent.
- [ ] Pin Python sidecar dependencies with the existing Python packaging tool;
  do not create a second lock format.
- [ ] Document clean-machine prerequisites and the exact unsigned developer
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

- [ ] Run `git diff --check` and confirm a clean worktree after commits.
- [ ] Record final LOC, total delta, and percentage change.
- [ ] Record aggregate active agent time by phase and reserve allocation.
- [ ] Update `docs/codebase-audit-tracker.md` only for evidence-backed completed
  items.
- [ ] Update `deferred-backlog.md` with uncompleted attempts and newly discovered
  work.
- [ ] Reduce `docs/context/current.md` only where current architecture or commands
  changed.
- [ ] Have one Sol-high reviewer inspect the complete program diff for Critical
  correctness/security/data-loss findings. Cap review at 20 minutes and permit
  one correction round.
- [ ] Do not push.

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
