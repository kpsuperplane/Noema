# Engineering Simplicity

Noema optimizes for the smallest coherent system that delivers the current
product behavior. Local completeness is not success when the repository gains
parallel models, repeated tests, or infrastructure for unrequested futures.

## Implementation brief

Before nontrivial implementation, write this brief in the task commentary:

```text
Mode:
Observable outcome:
Non-goals:
Existing path to reuse, replace, or simplify:
Expected files:
Production code budget:
Test code budget:
Planned tests and the unique risk each covers:
Stop conditions:
Validation:
```

Budgets are estimates, not permission to fill the allowance. Typical starting
points are:

| Work | Production budget | Test budget | Expected tests |
| --- | ---: | ---: | ---: |
| Narrow bug | 25–150 lines | 10–80 lines | 1–2 |
| Ordinary feature | 150–600 lines | 50–180 lines | 3–8 |
| Cross-system or state-machine slice | Explicitly justified | Explicitly justified | Risk matrix |
| Refactor | Net-negative | Net-negative or unchanged | No new tests by default |

Stop and reassess when production or test code exceeds its estimate by 50% or
500 lines, whichever is smaller; when a new public abstraction becomes
necessary; or when the slice needs behavior outside its non-goals. Do not hide
growth in generated code, fixtures, or adjacent cleanup.

## Measure the patch

Use the repository reporter at each milestone and before commit:

```bash
bun run scripts/report-rust-size.ts --base HEAD
bun run scripts/report-rust-size.ts --base HEAD \
  --max-production-net 600 \
  --max-test-net 180 \
  --max-new-tests 8
bun run scripts/report-rust-size.ts --base HEAD --require-net-negative
```

Choose the base that represents the start of the unit, usually `HEAD` for an
uncommitted unit or the previous milestone commit for a longer slice. The
report is a circuit breaker: if a budget fails, stop and simplify or request a
scope decision instead of raising the limit after the fact.

## Design and abstraction

- Start with one user-visible vertical path and the current concrete data
  model. Generalize only after another production consumer exists.
- Keep one authority for each concept. Change that authority and its consumers
  together rather than adding a bridge, mirror, compatibility field, or second
  command path.
- A test seam is not enough reason for a production trait or port. Prefer a
  deterministic concrete dependency, an existing boundary, or a narrowly
  scoped test helper.
- Preserve security, privacy, data-loss, concurrency, and external protocol
  boundaries. Simplification does not mean weakening a material invariant.
- In pre-V1 code, replace obsolete schema and APIs directly. Do not preserve
  compatibility unless the task explicitly requires it.
- During refactoring, delete or consolidate before extracting helpers. Stop if
  the helper layer makes the patch net-positive.

## Testing

Every planned test must name the distinct risk it protects. Prefer the lowest
authoritative layer that can prove the behavior.

Keep tests for:

- authorization, privacy, secret handling, and path safety;
- preservation of authorized private and ordinary information whenever a
  secret-exclusion or derived-redaction boundary changes;
- transaction atomicity, data loss, idempotency, leasing, and concurrency;
- state transitions whose incorrect outcome changes user-visible behavior;
- provider wire protocols, stream termination, and hostile external input;
- demonstrated regressions that could plausibly recur.

Do not add tests merely for:

- derives, getters, constructors, or enum/string mirrors;
- pass-through resolvers and mappings with no owned policy;
- mock call choreography instead of an observable outcome;
- the same transition at every architectural layer;
- permutations that execute the same branch and consequence.

Use table-driven tests when cases share setup and differ only in inputs and
outcomes. Reuse the real in-memory SQLite store and the canonical deterministic
provider fixture where practical rather than creating a fake per crate.

More than ten new Rust tests requires a written risk matrix and a redundancy
check before code is added. The main agent, not a local implementer, approves
that expansion.

## Plans and documentation

An ordinary plan should fit in roughly 100–150 lines; a genuinely cross-system
plan should fit in roughly 300. Plans define the current user-visible slice,
constraints, non-goals, acceptance scenarios, budget, and rollback point.
Future work belongs in a short deferred section, without exact types, file
ownership packets, or exhaustive test matrices that make it look implemented.

`docs/context/current.md` contains only active direction, current constraints,
recent decisions relevant to upcoming work, open loops, and validation
defaults. Keep it below 300 lines. Put durable subsystem contracts in the
closest subsystem document and use Git history for completed execution detail.

## Subagents and reviews

Keep work inline when it is expected to touch fewer than roughly 1,000 lines or
two architectural areas. When parallel implementation is warranted, give each
agent an independently shippable vertical outcome and a share of the parent
budget. Do not split one feature horizontally across domain, store, runtime,
API, and test owners before the path exists.

Use one read-only adversarial review after implementation. The main agent ranks
findings:

- **P0/P1:** correctness, security, privacy, data loss, or a demonstrated
  regression; fix in the current unit.
- **P2:** bounded maintainability or performance issue; fix only when it is in
  scope and does not expand the budget.
- **P3:** speculative hardening, extensibility, or stylistic preference; defer
  or discard.

Run a second review only when the first identified a serious unresolved defect.
Do not review repeatedly until no possible critique remains.

## Completion

Before committing a unit:

1. Run the Rust size report against the unit base and compare actuals with the
   brief.
2. Perform a deletion pass over new helpers, fixtures, mappings, and comments.
3. Run focused validation, then the repository validation required by
   `AGENTS.md`.
4. Report production/test deltas, tests added or removed, and any remaining
   budget exception.

A coherent smaller patch is preferred over a locally perfect framework.
