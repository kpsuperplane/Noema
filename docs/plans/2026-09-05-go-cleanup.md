# Go code cleanup

Mode: implement. The human authorized all seven review items on 2026-09-05.
An ORM is permitted. The former ORM exclusion no longer applies.

## Outcome

Normal Go and web development needs no Cargo. Model evaluations exercise Go
production code. Retained Rust owns only current native and browser targets.
Libraries replace repeated SQL mapping, ACP protocol, and XLSX parsing code.
One migration list owns fresh creation and upgrades. CI checks generated files.

## Scope and budgets

Each phase has one implementation pass, one review pass, and one correction pass.
Stop a phase when its additions exceed the estimate by 50% or 500 lines.
Preserve current capabilities, database contents, exact action checks, and clients.
Keep the total authored change net negative across languages.
Keep Go production and inclusive lines below the migration's 80-percent limits.

| Phase | Expected files | Added production/tooling budget | Added test budget |
| --- | --- | ---: | ---: |
| Migration registry | `internal/store`, `docs/sqlite.md` | 40 | 80 |
| Development and CI | scripts, web package, Cargo launcher, CI, README | 250 | 100 |
| Bun store trial and adoption | `internal/store`, module files | 400 | 180 |
| ACP SDK | `internal/acp`, module files | 250 | 150 |
| XLSX parser | `internal/documents`, module files | 100 | 100 |
| Go evaluations | `cmd`, `internal`, `evals`, docs | 5,000 | 400 |
| Rust support reduction | Cargo workspace, browser worker, Swift bridge, docs | 400 | unchanged |

The storage, ACP, parser, and tooling phases must remove more implementation
code than they add. Go evaluation additions replace the retained Rust suite.
Do not remove evaluations until their Go replacement preserves their features.
Do not remove the browser worker or Tauri desktop capability.

## Validation

- Migration tests preserve upgrade data and match fresh schema structure.
- Supervisor tests check shutdown and development process boundaries.
- Storage tests check nullable values, millisecond timestamps, rollback,
  dynamic filters, zero-valued updates, and current-run checks.
- ACP tests preserve cancellation, message bounds, permissions, and uncertain outcomes.
- Parser tests preserve supported spreadsheet content and resource bounds.
- Evaluation tests check scoring, selection, budgets, and production execution.
- Run Go unit tests and vet after each Go phase. Run required retained Rust
  checks when Rust changes. Preserve the compiler wrapper and build cache.
- Run web build and static checks for tooling changes. Do not inspect browsers.
- Check generated files, code size, staged paths, and whitespace before commits.
- Run no paid evaluation or live fixture without explicit authorization.

## Progress

- Baseline: `5715c916`; clean `codex/go-server-migration` worktree.
- Main is open in another worktree. Continue on the current migration branch.
- Go CI now exists. Extend its generated-code checks rather than duplicate it.
- Full permissions now allow dependency downloads and socket tests.
- Migration registry complete. All Go unit tests and vet passed.
- Use `GOTMPDIR=$PWD/target/go-tmp` while `/tmp` has limited free space.

- XLSX replacement complete: Excelize v2.11.0; 205 fewer implementation lines.
  Existing tests now use valid XLSX namespaces and package relationships.
  Truncated XML and archive limits remain checked. All Go unit tests and vet passed.

- Development and CI complete. Go supervises pinned Air and native web watchers.
  The Rust launcher is removed. Generated Go and web checks now run in CI.
  The focused Rust wrapper is `scripts/validate-rust`; compiler cache settings remain unchanged.
  Linux document workers now limit writable data mappings; address reservations caused intermittent failures.
  Go tests and vet, web lint/build, and all retained Rust checks passed.
  The new supervisor adds 136 production lines and 60 test lines; Rust loses 993 lines.

- ACP SDK replacement complete: coder/acp-go-sdk v0.13.5 owns JSON-RPC dispatch.
  Local limits, secret-free diagnostics, exact permission handling, and process cleanup remain.
  Existing ACP unit tests and race tests passed; 99 implementation lines removed.
  Full Go validation found unrelated active public-page edits with two stale text assertions.

- Bun v1.2.18 now owns Task, Task-run, Task-event, and Agent row mapping.
  The existing SQLite driver, schema version, and transaction checks remain.
  Production store code changed by -100 lines. Two mapping tests add 89 lines.
  Go unit tests and vet passed. The Memory test now accepts merged invalidations.
