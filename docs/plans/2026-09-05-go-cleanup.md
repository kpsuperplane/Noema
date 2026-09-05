# Go code cleanup

Mode: implement. The human authorized all seven review items on 2026-09-05.
An ORM is permitted. The former ORM exclusion no longer applies.

## Outcome

Normal Go and web development needs no Cargo. Model evaluations exercise Go
production code. Retained Rust owns the Tauri desktop shell.
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
Preserve browser actions and the Tauri desktop capability.

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

- Go evaluation suite 10 replaces the Rust runner's 32 model cases.
  Production prompts, schemas, replay, token counting, and adapters own execution.
  Hosted plans preserve prices, budgets, exact checkpoints, judges, and recommendation patches.
  Local runs preserve verified imports, isolated workers, resource probes, and reports.
  Interrupted billed requests require a new plan before another attempt.
  No paid model calls or weight downloads ran during implementation.

- Rust now contains only the desktop.
  Cargo metadata checked all former features, binaries, and support targets.
  The Go CDP adapter preserves browser actions, URL policy, protocol bounds, and action outcomes.
  Go owns browser sessions and process orchestration.
  Apple Foundation Models and its Swift bridge were removed at the human's request.
  Go and desktop packaging share `internal/localmodel/runtime-assets.json`.
  The Rust reduction removes 186,509 lines, including former evaluation support.

- Final validation passed: Go unit tests, vet, static analysis, native packaging tests,
  Rust format, workspace check, lint, and 18 Rust unit tests.
  The optional browser fixture compiled; it did not run.
  The vulnerability scan found no affected calls in the Go application.
  Go counts: production 78,875; tests 24,926; generated GraphQL 79,612; inclusive 183,413.
  Both migration ratios remain below 80 percent.
  All seven cleanup items are complete. No release was published or pushed.

## Upstream Obscura follow-up

The human requested direct downloads from upstream Obscura releases.
Go installs v0.2.2 archives with pinned SHA-256 digests for the five supported targets.
Each archive contains `obscura` and its companion `obscura-worker`.
The CDP adapter reuses the existing WebSocket library, process limits, and shared browser snapshot logic.
Network events distinguish main-document failures from subresource failures after an action.
The custom Rust worker and its release workflow are removed.
The estimate permits 700 added production lines and 180 added test lines, with a net code reduction.
Unit checks cover release integrity, archive paths, process protocol bounds, action outcomes, and session authority.

Go adds 198 production lines and 25 test lines after deletions. Rust loses 1,846 lines.
The source reduction is 1,623 lines. Two Go protocol tests replace nine Rust test declarations.
Go totals are 79,073 production lines, 24,951 test lines, and 79,612 generated GraphQL lines.
The inclusive total is 183,636 lines. Both migration ratios remain below 80 percent.

Validation passed: all Go unit tests and vet; browser script syntax; Rust format, workspace check, lint, and 10 desktop unit tests.
The downloaded Linux archive matched its pinned digest and contained both expected executables.
Live browser and fixture tests did not run.

## Apple model removal

The human requested removal of Apple Foundation Models and its Swift bridge.
The provider, native protocol, availability checks, recommendations, and settings note are removed.
Desktop packaging and the development watcher no longer build Swift helper code.
Migration 33 removes the provider account and its current model selections.
Other model selections and historical conversation and Task records remain intact.
Local-model evaluation reports now name `local_models` as their provider.
The estimate permits 100 added production lines and 120 test lines, with a net code reduction.
The migration test covers existing-version upgrades, fresh-schema convergence, other providers, and historical records.
Setup fills missing roles and preserves current selections from other providers.
If hosted selections lack an action reviewer, local setup explains that a hosted provider must restore that role.
The setup status test checks that incomplete selections return users to setup.

Go production changes by -885 lines. Go tests change by -145 lines.
Two Go test declarations replace five removed declarations. Four obsolete packaging tests are removed.
Go totals are 78,206 production lines, 24,926 test lines, and 79,612 generated GraphQL lines.
The inclusive total is 182,744 lines. Both migration ratios remain below 80 percent.
Validation passed: Go unit tests and vet; 18 desktop packaging tests; web generated-file checks, lint, and build.
One Memory event test failed during validation, then passed five repeat runs and the full suite.
No live provider calls or browser inspection ran.
