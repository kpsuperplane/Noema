# OpenRouter Model Default Evaluation Maintenance Plan

**Status:** In progress — Milestones 1–3 implemented

**Goal:** Turn Noema's current model qualification runner into a repeatable
OpenRouter benchmark-to-patch workflow for reviewing and updating the defaults
in `crates/noema-providers/src/recommendations.rs`.

**Architecture:** Evaluate every candidate through OpenRouter, rank models once
per `NoemaModelUseCase`, and map qualified winners to explicit provider-specific
profile ids. Keep `recommendations.rs` as the single shipped authority. The
evaluator emits evidence and a reviewable patch but never changes product
settings or publishes a recommendation automatically.

## Design Decision

OpenRouter is the sole model-evaluation transport for this workflow. The same
model is not rerun through direct OpenAI, Codex, Apple Foundation Models, or a
local GGUF runtime to measure model quality. Capability is treated as a property
of the model and reasoning configuration rather than the route serving it.

Provider-specific recommendations remain possible through manifest mappings:

```text
Evaluated candidate: openai/example-model on OpenRouter
OpenRouter profile:  openai/example-model
OpenAI profile:      example-model
Codex profile:       example-model
```

Mappings are explicit data and are never inferred from model-name strings. A
candidate may update only providers for which it declares an exact supported
profile.

Local runtime compatibility, quantization fit, memory use, and startup behavior
remain operational concerns outside this decision. Existing local `prepare`,
`run`, and `soak` commands may remain as compatibility/resource probes, but
their results do not participate in hosted model-quality ranking or
`recommendations.rs` proposal generation.

## Current Baseline And Gaps

The repository already has:

- OpenRouter candidates executed through the production `ProviderHandle`.
- Twenty-three production-derived cases covering all nine model use cases.
- Deterministic contract grading, streaming observations, token usage, latency,
  estimated OpenRouter cost, and structured reports.
- One code-owned default authority in `recommendations.rs`.

The missing default-maintenance behavior is:

- Several specialist roles have only one scenario, so the suite proves basic
  compatibility rather than relative fitness.
- Qualified candidates have no comparative quality signal.
- Two repetitions, median latency, and no incumbent threshold provide weak
  evidence for changing a default.
- Candidates do not map evaluated identities to provider-specific profiles.
- Hosted runs can contend with the canonical Noema store.
- Incremental reports can show a recommendation before the run is complete.
- Reports neither verify shipped defaults nor produce a source patch.

## Scope

### In scope

- OpenRouter evaluation of candidate models and reasoning efforts.
- All nine `NoemaModelUseCase` values.
- Explicit mappings to OpenRouter, direct OpenAI, and Codex profiles.
- Contract gates, comparative quality, reliability, latency, and OpenRouter
  cost evidence.
- Isolated, resumable, budget-aware runs.
- Evidence-backed `recommendations.rs` proposals and verification.

### Non-goals

- Comparing providers that serve the same model.
- Direct OpenAI, Codex, Foundation, or local-model calls during a decision.
- Per-platform model-quality rankings or production traffic experiments.
- Automatic publication, account mutation, or preference mutation.
- Live external effects or network-dependent evaluation fixtures.
- A second runtime recommendation authority.

## Decision Contract

### Candidate identity and eligibility

Each candidate records:

- OpenRouter model profile and reasoning effort.
- Eligible use cases.
- Exact target-provider/profile mappings.
- Price snapshot and effective date.
- Notes needed to interpret limitations.

The evaluator ranks once per use case. Proposal generation then selects the best
qualified candidate eligible for each target provider:

- OpenRouter may select any candidate with an OpenRouter mapping.
- Direct OpenAI and Codex may select only explicitly mapped candidates.
- An unmapped provider retains its incumbent or remains without a default.

This permits a non-OpenAI model to win the OpenRouter cell while OpenAI and
Codex retain the best qualified mapped candidate from the same run.

### Run modes and completeness

- **Exploration:** Partial candidates, roles, or repetitions are allowed. No
  recommendation patch can be produced.
- **Default decision:** Every role contains its shipped incumbents and configured
  challengers, all repetitions completed, and all required graders completed.

Reports have `running`, `incomplete`, `complete`, or `failed` status. Only a
complete default decision can be proposed or verified. Incremental reports may
show provisional scores but never a final recommendation.

### Qualification

A candidate qualifies for a use case only when:

- Every required repetition completed without provider or runner error.
- Every hard contract and safety case passed.
- The returned model identity is compatible with the requested candidate.
- Role-specific timeout and reliability ceilings were met.
- Every required deterministic and comparative grader completed.

Quality, cost, and latency cannot compensate for failed safety, provenance,
grounding, or typed-terminal contracts.

### Selection policy

Each role owns a versioned policy defining hard gates, repetitions, reliability
and latency ceilings, quality weights, cost weight, and the minimum improvement
required to replace an incumbent.

Candidates sort by:

1. Qualification.
2. Role-quality score.
3. Reliability.
4. OpenRouter cost per successful case.
5. Role-appropriate latency, normally p95 for foreground work and median for
   bounded background work.
6. Stable candidate id.

A challenger that does not clear the role's replacement margin does not displace
the incumbent. Reports state the exact selection or retention reason.

## Evaluation Suite

### Hard gates

Retain deterministic grading for exact response and streaming contracts,
required tools, production payload decoding, continuation, discovery before
external action, grounding, task terminal contracts, reviewer decisions,
prompt-injection resistance, action classification, and memory provenance.

Add OpenRouter protocol cases for context preservation, required-tool behavior,
multi-round continuation, near-limit context, cached-input accounting, stream
termination, usage accounting, and returned model identity.

### Role coverage

Every role must have at least five independent critical scenarios before the
first default decision. New cases cover distinct consequences:

- Primary: synthesis, concision, tool ordering, continuation, hierarchy.
- Task tiers: bounded execution, comparison/ranking, evidence, blocking.
- Task reviewer: approval, contradiction, missing evidence, unsupported claim,
  unnecessary rejection.
- Web summary: injection, conflicts, omission pressure, length, faithfulness.
- Progress audit: complete, remaining work, failure, hostile result, ceiling.
- Action reviewer: authority, ambiguity, drift, destination, sensitive egress.
- Memory: add, update, no-op, contradiction, invented source, scope violation.

Fixtures stay synthetic and deterministic and never perform external effects.

### Comparative quality

- Use deterministic fact, constraint, and structure scores for bounded tasks.
- Use blinded pairwise grading for genuinely open-ended results through a
  separately configured, pinned OpenRouter judge candidate.
- Hide candidate labels, deterministically shuffle order, and store bounded
  rubric scores and rationale.
- Calibrate new open-ended rubrics against a small maintainer-reviewed reference
  set before they influence defaults.
- Never let a judge override a hard-gate failure.

If required comparative grading is unavailable, the decision stays incomplete.

## Evidence And Cadence

Every decision bundle records the decision id, Git state, suite/policy versions,
prompt/tool/fixture digests, candidate manifest and mappings, requested and
returned model identities, dated prices, repetition order, case outcomes, token
usage, median/p95/first-delta latency, timeout rate, cost per successful case,
selection explanations, and sanitization version.

A compact decision record and summary may be committed with a recommendation
change. Voluminous raw output remains in ignored artifacts or an explicitly
chosen archive.

Target maintainer workflow:

```bash
cargo validate run -p noema-model-evals -- defaults plan
cargo validate run -p noema-model-evals -- defaults estimate <plan>
NOEMA_HOME=/explicit/eval/home \
  cargo validate run -p noema-model-evals -- defaults run <plan>
cargo validate run -p noema-model-evals -- defaults propose <completed-run>
cargo validate run -p noema-model-evals -- defaults verify <completed-run>
```

Command names are implementation targets. The runner preflights the OpenRouter
credential, model availability, spend ceiling, and output space. It checkpoints
after every case and resumes without repeating a completed billed call unless
explicitly requested.

## Recommendation Proposal

Refactor `recommendations.rs` only as needed so provider/use-case entries exist
once in a stable code-owned table consumed by the runtime resolver and evaluator.
Do not load runtime recommendations from generated evaluation artifacts.

For a complete decision, proposal generation:

1. Reads current incumbents through the production API.
2. Resolves each role's winning evaluated candidate.
3. Filters each provider update through explicit profile mappings.
4. Applies the selection policy and incumbent threshold.
5. Emits no change for incomplete, tied, unmapped, or insufficient results.
6. Emits a deterministic unified diff and cell-by-cell rationale.
7. Never edits the worktree by default.

Verification fails when a shipped recommendation has no qualified evaluated
candidate, lacks a provider mapping, or differs from the selected mapped winner
without an explicit retained-incumbent reason.

## Implementation Milestones

### Milestone 1: Decision-correct OpenRouter reports

**Outcome:** Complete runs have validated mappings and final role winners;
partial runs cannot produce proposals.

**Expected files:** `matrix_manifest.rs`, `matrix_runner.rs`,
`matrix_report.rs`, `main.rs`, `evals/model-matrix/candidates.toml`, and the
matrix README.

- [x] Remove non-OpenRouter candidates from the decision manifest.
- [x] Add and validate target-provider mappings.
- [x] Add exploration/default-decision modes and lifecycle status.
- [x] Capture returned model identity and suite/environment fingerprints.
- [x] Suppress final winners until every selected role completes.

**Budget:** 250–400 production lines, 120–190 test lines, 4–7 tests.
Distinct risks: completeness, mapping validation, identity, sanitization.

### Milestone 2: Selection-grade suite and policy

**Outcome:** Models can be compared on representative quality and reliability.

**Expected files:** `noema-runtime/src/eval_support/`, `matrix_report.rs`,
`suite.toml`, and one role-policy manifest under `evals/model-matrix/`.

- [x] Expand every role to at least five independent critical scenarios.
- [x] Add OpenRouter protocol cases.
- [x] Separate hard gates from quality dimensions.
- [x] Add bounded blinded grading where deterministic scoring is insufficient.
- [x] Add role policies, p95/error metrics, and incumbent thresholds.
- [x] Require 3–5 decision repetitions; retain one for exploration.

**Budget:** 450–700 production/evaluation lines, 180–300 test lines, 6–10
tests. Distinct risks: gate precedence, judge bounds, tradeoff policy, churn.

### Milestone 3: Reproducible cadence runner

**Outcome:** Runs preflight, checkpoint, stop, and resume without touching the
production store or repeating successful billed work.

**Expected files:** `main.rs`, `hosted_provider.rs`, `matrix_runner.rs`, narrow
plan/checkpoint modules, and the matrix README.

- [x] Add immutable plan generation and validation.
- [x] Require an explicit evaluation home for decision mode.
- [x] Preflight credentials, availability, and spend.
- [x] Checkpoint per case and resume idempotently.
- [x] Bound spend, timeout, retry, and concurrency.
- [x] Finalize one sanitized evidence bundle.

**Budget:** 350–550 production lines, 160–250 test lines, 5–8 tests.
Distinct risks: idempotency, stale plans, spending, isolation, recovery.

### Milestone 4: Proposal and verification

**Outcome:** A completed decision emits a deterministic `recommendations.rs`
patch and verifies the shipped defaults.

**Expected files:** `recommendations.rs`, `noema-model-evals` proposal/CLI code,
the matrix README, and compact evidence for an accepted decision.

- [ ] Consolidate recommendations into one code table if needed.
- [ ] Map role winners to eligible provider profiles.
- [ ] Compare incumbents with minimum replacement margins.
- [ ] Emit deterministic diffs and human rationale without editing.
- [ ] Verify every shipped default against completed evidence.
- [ ] Run and review the first full decision.

**Budget:** 280–450 production lines, 150–240 test lines, 5–8 tests.
Distinct risks: profile mapping, stable diffs, no-change results, agreement with
the runtime resolver.

## Acceptance Scenarios

1. Partial exploration reports results but cannot produce a patch.
2. A cheap model that fails one safety gate remains unqualified.
3. A marginal challenger does not displace the incumbent.
4. A non-OpenAI winner updates OpenRouter but not OpenAI or Codex.
5. A winner with three explicit mappings produces three correct profile ids.
6. A missing Codex mapping leaves Codex unchanged and explains why.
7. An interrupted run resumes without repeating completed calls.
8. Returned model alias drift fails the candidate identity check.
9. Verification rejects a shipped default without qualified mapped evidence.

## Validation And Completion

Each milestone is a separate commit and size-budget unit. Run the size reporter
with that milestone's budgets, focused tests through `cargo validate`, then:

```bash
cargo fmt --all --check
cargo check-workspace
cargo gate-lint
cargo gate-test
git diff --check
```

Networked OpenRouter decisions never run inside unit-test gates.

The plan is complete when one command sequence can plan, estimate, run/resume,
propose, and verify an OpenRouter decision; every role has selection-grade
evidence; provider changes come only from explicit mappings; recommendation
changes retain a decision id and rationale; `recommendations.rs` remains the
single authority; and the first cadence run produces a reviewed patch or an
evidence-backed no-change decision.
