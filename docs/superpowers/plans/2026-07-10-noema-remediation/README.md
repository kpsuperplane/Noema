# Noema Remediation Execution Program

> **For agentic workers:** Execute this program in sequence. Use one bounded
> implementation agent at a time, preserve unrelated work, and stop at every
> phase gate. Do not expand scope to consume another phase's budget.

## Goal

Improve Noema's security, reliability, performance, product behavior, and
architecture while reducing maintained source code from the July 10 baseline
of **93,924 lines** to **89,000 lines or fewer**. The stretch target is 86,500
lines.

This is a deletion-first remediation pass, not an attempt to complete every
item in the audit tracker. Work that does not fit the program is recorded in
`deferred-backlog.md` instead of silently extending the schedule.

## Schedule

Active agent time is aggregate: two agents working concurrently for one hour
consume two agent-hours.

| Phase | Target budget | Hard phase cap | Required net source LOC |
| --- | ---: | ---: | ---: |
| 0. Baseline and dependency decisions | 0.5 h | 0.75 h | 0 |
| 1. Web transport and authentication | 2.25 h | 3 h | -1,200 |
| 2. Integrations and provider consolidation | 2 h | 2.5 h | -1,500 |
| 3. Runtime and persistence | 2.5 h | 3 h | -800 |
| 4. GraphQL and frontend simplification | 1.5 h | 2 h | -1,000 |
| 5. Measured crate extraction | 2 h | 2.5 h | +300 maximum |
| 6. Release, CI, and closure | 1.25 h | 1.75 h | -1,000 |
| **Planned total** | **12 h** |  | **-5,200** |

The remaining four hours are a global reserve. Root may allocate the reserve in
30-minute increments only after a failed validation gate or when a nearly
complete change has demonstrated value. The complete program stops at 16
aggregate active agent-hours regardless of remaining tracker items.

## Source LOC Contract

The baseline counts tracked maintained source in Rust, TypeScript, TSX,
JavaScript, Python, Swift, GraphQL, SQL, shell, CSS, and HTML. Markdown,
dependencies, lockfiles, build output, and vendored code do not count. Generated
source checked into the normal source tree does count.

Run from the repository root:

```bash
git ls-files \
  | rg -v '\.md$|(^|/)(target|node_modules|dist|gen)/' \
  | rg '\.(rs|ts|tsx|js|jsx|py|swift|graphql|sql|sh|css|html)$' \
  | tr '\n' '\0' \
  | xargs -0 wc -l \
  | tail -1
```

Rules:

- Record the total before and after every phase.
- Do not claim savings from moving code to an uncounted extension or generated
  directory.
- Do not delete valuable tests solely to satisfy the LOC gate.
- A library adoption must delete more maintained code than it adds in the phase
  where it lands.
- A phase that misses its LOC budget may continue only if the cumulative total
  remains on track for 89,000 and root records the reason.
- Final completion requires 89,000 or fewer lines. Compilation and correctness
  take precedence over the stretch target.

## Execution Order

1. [`00-library-decisions.md`](00-library-decisions.md)
2. [`01-web-transport-and-auth.md`](01-web-transport-and-auth.md)
3. [`02-integrations-and-providers.md`](02-integrations-and-providers.md)
4. [`03-runtime-and-persistence.md`](03-runtime-and-persistence.md)
5. [`04-graphql-and-frontend.md`](04-graphql-and-frontend.md)
6. [`05-measured-crate-extraction.md`](05-measured-crate-extraction.md)
7. [`06-release-ci-and-closure.md`](06-release-ci-and-closure.md)
8. [`deferred-backlog.md`](deferred-backlog.md)

## Operating Model

- Root owns scope, live integration, validation, commits, time accounting, and
  final decisions.
- Use a Terra-medium implementation agent only when a task is large enough to
  save at least 20 minutes. Give it one bounded subsystem and no commit access.
- Use a Sol-high read-only reviewer only for transport authentication,
  persistence integrity, or final closure. Cap each review at 20 minutes.
- Do not create a separate planner for each phase. These files are the plan.
- Never run agents concurrently against the same files or shared Cargo build.
- Interrupt an agent that has produced no reviewable diff or decision within 45
  minutes. Root either narrows the task or implements it directly.
- Produce a validated commit at least every 90 minutes. A phase may contain more
  than one commit.
- Update `docs/context/current.md` only at a phase boundary and keep the update
  concise.
- Never push without explicit user approval.

## Global Guardrails

- Follow `AGENTS.md`; preserve sccache and `CARGO_BUILD_RUSTC_WRAPPER`.
- Unit tests only. Do not add Playwright, browser automation, smoke-test
  harnesses, or frontend unit tests.
- Frontend validation is generation, lint, and build only.
- No live `~/.noema` database reset is part of this program.
- Preserve provider credentials and never print secret material.
- Prefer a mature public library when it replaces protocol, authentication,
  lifecycle, schema-validation, persistence, or release machinery.
- Do not introduce a framework for one helper or when its build/maintenance cost
  exceeds the code it removes.
- No backwards-compatibility layer or migration framework is required pre-V1.
- Do not broaden the product model merely to satisfy a hypothetical threat.
  Preserve the documented multi-human direction, but implement only authority
  needed by current transports and persisted ownership.

## Phase Gate

Every phase ends with:

```bash
git diff --check
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

Run frontend generation, lint, and build when frontend or GraphQL artifacts
changed. Run Python unit tests when the sidecar changed. If the full Rust suite
cannot finish inside the phase cap, run focused tests, commit only after they
pass, and charge the final full-suite run to Phase 6.

At the gate, record:

- elapsed aggregate agent time;
- commit hashes;
- LOC before, after, and delta;
- dependencies added and custom code removed;
- focused and full validation results;
- deferred work and the reason it stopped.

## Stop Conditions

Stop the current phase and preserve its last validated commit when any of these
occurs:

- the hard phase cap is reached;
- a proposed library cannot replace the custom path without maintaining both;
- the phase requires a schema or product redesign outside its brief;
- two consecutive correction rounds fail the same gate;
- the change is LOC-positive without removing a material security or data-loss
  risk;
- the remaining work would prevent the 16-hour program cap.

Revert an unvalidated partial slice rather than carrying a broken worktree into
the next phase.

## Completion

The program is complete when:

- tracked maintained source is 89,000 lines or fewer;
- all landed changes pass the applicable validation gates;
- custom HTTP/WebSocket framing is gone;
- lifecycle and persistence changes use the selected standard utilities;
- `noema-server` and `noema-store` exist only if their measured extraction gates
  passed;
- the audit tracker reflects completed work and the deferred file honestly
  records everything not attempted;
- the repository is clean and no work has been pushed without approval.
