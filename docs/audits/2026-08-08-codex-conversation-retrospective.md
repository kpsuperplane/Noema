# Recurring problems in Noema's Codex build conversations

- **Status:** Evidence-backed retrospective
- **Mode:** Explore and report only
- **Conversation window:** 2026-08-03 through 2026-08-08
- **Repository reference:** `dc9bb212`
- **Scope:** Codex conversations and delegated reviews conducted while building,
  debugging, validating, and simplifying Noema

This report identifies recurring problems in how Noema behaved and in how Codex
helped build it. It is a retrospective, not a second product or architecture
authority. Current subsystem documentation and code remain authoritative.

Raw Codex session logs were treated as private source material. This report
paraphrases evidence, omits personal content, and does not reproduce transcript
text or credential material.

## Corpus and method

The report window contained 74 session logs rooted at `/root/noema`:

- 38 top-level Codex Desktop sessions;
- 36 delegated, continuation, or guardian-agent sessions;
- 29 substantive top-level project conversations after excluding five Noema
  executor-validation sessions, three housekeeping sessions, and this report
  request.

Delegated sessions were used to corroborate findings, not counted as independent
human conversations. A recurring theme normally required evidence in at least two
distinct top-level conversations. Counts below are conversation-level lower
bounds, overlap between themes, and should not be summed.

The conversation evidence was cross-checked against:

- the [historical `.noema-dev` UX incident audit](2026-08-08-noema-dev-user-experience-history.md);
- the [current-build survivor audit](2026-08-08-current-build-ux-survivors.md);
- the [overengineering audit](2026-08-08-overengineering-audit.md) and its
  [remediation report](2026-08-08-overengineering-remediation.md);
- current project, simplicity, information-handling, and frontend contracts.

The window is short and unusually implementation-heavy. It overrepresents defects
because many conversations began only after a problem was noticed. It is strong
evidence about repeated failure shape, but not a measure of ordinary product usage
or developer productivity.

## Executive assessment

The dominant recurring problem is **fragmented authority across lifecycle seams**.
The same underlying fact—what tool exists, which contract is current, what was
approved, which connector version is active, whether a failure is retryable, or
what a user-visible action means—was repeatedly interpreted by more than one
layer. Each interpretation could be locally reasonable while the end-to-end
workflow was wrong.

The recurring shape was:

```text
setup/catalog says ready
  → invocation uses a different contract
  → approval stores only part of the action
  → restart or compaction loses exact context
  → recovery reconstructs state from stale prose or a nearby record
  → UI shows a generic error or internal identifier
  → the agent retries, invents a diagnosis, or asks the human to route around it
```

The second major problem is **insufficient vertical proof**. Focused tests often
proved that a row was written, a run was queued, a schema compiled, or a component
built. They did not prove that the original user job survived persisted history,
runtime restart, model/tool selection, approval, external execution, recovery, and
final rendering. This is why several changes were technically validated and then
failed immediately in `.noema-dev`.

The collaboration-level consequence was that the human repeatedly became both the
integration test and the architecture circuit breaker: detecting incomplete fixes,
rejecting invented diagnoses, simplifying overbuilt proposals, and identifying
when a green local check did not correspond to a completed product workflow.

## Ranked recurring problems

| Rank | Recurring problem | Distinct conversations with clear evidence | Current assessment |
| ---: | --- | ---: | --- |
| 1 | Lifecycle seams carry competing interpretations of the same state | at least 10 | Individual defects were fixed; current Work and approval-origin survivors show the class remains active. |
| 2 | Changes are declared complete before the real vertical workflow is proven | at least 11 | Guardrails improved, but this remains the most important development-process risk. |
| 3 | Failure and recovery semantics lose useful information or progress identity | at least 9 | Many envelopes and UI paths improved; deterministic repetition and missing origins remain current. |
| 4 | Historical and versioned state is omitted from the happy-path design | at least 6 | Migration guidance and several repair paths landed; legacy-state coverage is still uneven. |
| 5 | Safety, privacy, authorization, and correctness are conflated | at least 5 | The three-class information contract and remediation are strong corrections; recent recurrence makes this a standing review concern. |
| 6 | Model autonomy is judged with easier or narrower tasks than production | at least 4 | The eval suite improved materially; realistic end-to-end task design should remain mandatory. |
| 7 | UI exposes implementation state and is often not visually verified | at least 7 | Several concrete surfaces were repaired; the validation gap remains whenever browser inspection is absent. |
| 8 | Stable work is repeated on hot paths | 3 major investigations | Build and connector hotspots were fixed with measured gains; tool-context cost remains a material opportunity. |
| 9 | Repository-wide validation is chronically non-green | 18 implementation conversations mentioned blocked aggregate gates | Still open at the report cutoff and a direct drag on confidence. |

### 1. Lifecycle seams carry competing interpretations of the same state

This is the broadest and most damaging pattern.

Representative manifestations included:

- ACP tool calls and results used different correlation identities, leaving
  completed activity visually unresolved.
- A browser action existed in the model catalog but disappeared during
  approval-time revalidation because that path classified built-in tools
  differently.
- Reopening Work created a new contract while admitting a review from the
  superseded contract, then retried the deterministic invariant failure.
- An inline browser action was mistaken for a delayed approval, causing duplicate
  continuation work and an unavailable Work state.
- Adapter replacement lineage, setup cards, migrations, and UI projections each
  had separate ideas of which immutable definition was the current actionable one.
- OAuth setup accepted one scope model while callback completion enforced another.
- Compaction turned an exact provider resource identifier into prose, after which
  the model reused a Noema connection identifier as if it were the provider ID.

The common cause is not a lack of types everywhere. It is failure to carry the
existing authoritative type or identity far enough. Later layers reconstruct the
meaning from a task's latest review, a tool name, a summary, a raw identifier, or
another catalog instead of consuming the exact proposal/contract/version that
originated the action.

The strongest fixes in the corpus all followed the same direction: retain one
authority and make downstream stages consume it. The weakest fixes added another
heuristic, compatibility layer, or model instruction.

### 2. Changes are declared complete before the real vertical workflow is proven

This appeared repeatedly after substantial implementations:

- ACP passed focused tests, but the first real delegated task exposed prompt,
  schema, helper-build, fixture, and transcript-terminality problems.
- Scheduling compiled and shipped, but live use exposed migration drift, incorrect
  local-date grounding, a missed intended date, unclear occurrence language, and
  a missing manual-run path.
- Typed Calendar recovery shipped, but the same task still failed because the old
  summary remained poisoned and no calendar-enumeration operation existed.
- Browser tooling shipped, but approved actions failed during the separately
  implemented revalidation path and restart made pending approvals unusable.
- A reopen test proved only that a new run was queued; it did not admit the run
  context where the foreign-review failure occurred.
- Loading placeholders passed lint/build through multiple iterations while their
  live geometry still diverged from the transcript.

The problem is test topology more than test volume. Tests frequently stop at an
intermediate artifact instead of the first user-observable terminal state.

For stateful features, a convincing acceptance path usually needs to cross the
specific seams that can invalidate the promise:

```text
persisted precondition
  → catalog/prompt admission
  → actual invocation
  → approval or intervention when applicable
  → restart/recovery when applicable
  → terminal durable state
  → user-visible projection
```

Not every feature needs a large end-to-end harness. Each demonstrated bug does
need one regression at the lowest layer that crosses the exact seam where the old
tests stopped too early.

### 3. Failure and recovery semantics lose information or progress identity

Noema often possessed the facts needed for a useful response but discarded them
before they reached the agent or human.

Recurring symptoms included:

- generic Work, reconciliation, transform, provider, and OAuth messages where a
  precise internal failure category existed;
- approval cards that hid reviewer reasoning, the action target, or exact values
  needed for informed approval;
- raw tool names and opaque browser element references appearing when descriptive
  context was lost;
- `resource_not_found` being reinterpreted as authentication failure;
- deterministic admission failures consuming four identical attempts;
- repeated task cards and repeated model refusals despite no new progress;
- connector-authoring failures being flattened so the agent could not repair its
  transform.

This is one problem with two outputs: under-explanation to the user and
under-specification to the agent. Both come from collapsing typed failure facts
into a generic envelope, then asking a later layer to infer what happened.

The desired failure contract is small and explicit: category, retryability,
recovery action, stable failure identity, human explanation, and bounded technical
detail. It should be created by the layer that knows the failure and preserved
through storage, model context, and UI projection.

### 4. Historical and versioned state is omitted from the happy-path design

Fresh-state behavior repeatedly passed while real persisted history broke.

Examples included:

- editing a migration version after the watched dev server had already applied it;
- exact pre-migration schema validation initially preventing the forward repair;
- adapter lineage breaking when semantic digests changed across schema versions;
- old reviewed definitions resurfacing as current setup actions;
- legacy compaction checkpoints retaining an incorrect identifier after a new
  compaction policy landed;
- pending browser approvals surviving longer than their execution-owned session;
- reopened Work carrying superseded review evidence into a new contract.

These are all lifecycle-version problems. Fresh-schema convergence is necessary
but not sufficient; the production contract must say what happens to existing
state when code, schema, definition digest, contract generation, or runtime session
changes.

The forward-only migration rule added during this window is an important
correction. Equivalent version-transition tests are still needed for other
content-addressed and durable authorities.

### 5. Safety, privacy, authorization, and correctness are conflated

Several Codex proposals treated any potentially sensitive-looking value as a
secret or treated a safety reviewer as a general correctness reviewer.

This produced:

- redaction of exact values from the human who was being asked to approve them;
- concealment of ordinary identifiers, paths, URLs, and provider diagnostics;
- a browser replay vault and sanitization machinery that duplicated ordinary
  durable state;
- withholding raw transform diagnostics from the authorized model even when the
  data was needed to repair the connector;
- repeated connector validation and tamper defense on every invocation even though
  Noema was established as the only supported writer;
- string- and field-name-based secrecy heuristics that were both over-broad and
  security-incomplete.

The deeper cause was treating secrecy, privacy, trust, authorization, egress,
retention, and correctness as one conservative continuum. They are different
questions with different authorities.

The current three-class information contract—secrets, private information, and
ordinary information—plus separate authorization and egress decisions is the
right correction. Future security changes should include both a true-secret
exclusion assertion and an ordinary/private-value preservation assertion at the
same boundary.

### 6. Model autonomy is judged with easier or narrower tasks than production

Noema's model-evaluation work initially produced confident but invalid conclusions
because the test did not preserve the hard part of the real request.

The strongest example was the flight-to-calendar workflow:

- the first evaluation split discovery and writing into separate calls;
- a later case explicitly told the model to look up the facts, removing the
  inference that lower-effort models had failed in production;
- fixtures supplied a narrow tool set and much smaller context than the real
  conversation;
- an initial default-model recommendation changed, was challenged by live evidence,
  and was later corrected after restoring the ambiguous request and broader task
  shape.

The same pattern appeared in ordinary unit tests: proving queue insertion instead
of executable context, or validating setup metadata instead of the activated tool.

For primary-agent evaluation, the unit of value is an autonomous outcome under
production-like ambiguity and tool width. Micro-cases remain useful for locating a
failure, but should not independently qualify a model or feature for a broader job.

### 7. UI exposes implementation state and is often not visually verified

Repeated UI corrections were not primarily aesthetic. They reflected competing
projections and missing product semantics:

- duplicate loading stages represented the same transcript state differently;
- a container-dependent skeleton layout broke after consolidation;
- scheduling exposed cron and materialization language instead of the user's run
  and recurrence;
- immutable adapter versions appeared as parallel setup choices;
- raw canonical tool names and browser references replaced human action labels;
- task references repeated near-identically across waiting, recovery, and
  completion events;
- generic connection failure copy hid the actual next step.

Static type, lint, and build checks passed these changes. Six substantive
conversations explicitly ended without browser visual inspection, and some then
required screenshot-driven correction.

The underlying rule should be that loading, live, replay, and recovery surfaces
share the same semantic projection before they share styling. Visual inspection is
then necessary for geometry and responsive behavior, but it cannot compensate for
multiple state authorities.

### 8. Stable work is repeated on hot paths

Three major investigations found the same performance design error at different
layers: work invariant for a version or runtime was repeated for every edit,
request, invocation, or continuation.

- Incremental compilation was disabled in the development path, debug artifacts
  were oversized, unchanged GraphQL schema output retriggered frontend production
  builds, and rapid saves restarted active builds.
- A connector invocation rescanned and repeatedly compiled all installed
  definitions, constructing roughly a thousand Luau sandboxes before one provider
  request.
- Broad tool schemas were repeatedly included in model requests and continuations,
  increasing latency, cost, and attention dilution.

The build and connector-runtime cases were substantially fixed and benchmarked.
The general design test remains useful: if a value changes only on source version,
definition digest, configuration revision, or runtime startup, do that work at
that boundary rather than on every call.

### 9. Repository-wide validation is chronically non-green

Eighteen substantive implementation conversations reported that aggregate
workspace validation was blocked, usually by the same Linux/Tauri
`macos-private-api` mismatch and an unrelated MCP lint failure. Earlier setup work
also encountered missing GTK/GLib prerequisites, oversized validation artifacts,
and a nested Cargo/sccache environment bug.

Focused validation was often appropriate and usually passed, but the repeated
exception became boilerplate. That has three costs:

1. a genuinely new aggregate regression is harder to distinguish from baseline
   noise;
2. completion language sounds stronger than the evidence actually supports;
3. every session spends attention rediscovering and explaining the same blockers.

A platform-aware aggregate command that is green on this environment is more
valuable than a nominally universal gate that predictably stops before exercising
most of the workspace. Platform-specific coverage can remain an explicit separate
obligation.

## Recurring Codex collaboration problems

The product failures above were amplified by several repeated working patterns.

### Diagnosis sometimes preceded evidence

Codex occasionally formed a confident explanation before inspecting the decisive
runtime state. Examples included attributing setup cards to OAuth refresh before
finding obsolete adapter versions, treating a Calendar 404 as reconnection trouble
before checking the argument identity, and initially emphasizing concurrency
before measuring repeated connector compilation.

The durable correction is procedural: inspect the canonical state and failing
boundary first, list competing hypotheses when evidence is incomplete, and do not
turn one plausible cause into user guidance until the observed facts distinguish
it.

### The first design often optimized for theoretical completeness

The user repeatedly had to narrow proposals that introduced opaque resource
handles, broad browser secrecy machinery, a schedule-first product abstraction,
special retry suppression, or repeated connector integrity checks. These designs
were internally coherent, but they solved a larger hypothetical class than the
demonstrated user problem.

The repository's current simplicity brief directly addresses this pattern. The
important behavior is to apply it before implementation, not use the size report
only after a large design has already spread across boundaries.

### Completion claims did not consistently name their evidence boundary

Many handoffs correctly disclosed that full gates or visual inspection were
unavailable, but still led with an unqualified implementation/fix claim. The later
live failure then made the earlier statement feel unreliable.

Handoffs should distinguish:

- implementation complete;
- focused contract verified;
- exact live scenario replayed;
- restart/upgrade path verified;
- browser/native presentation visually verified;
- aggregate repository baseline green.

Only claim the levels actually demonstrated.

### Long conversations accumulated stale authority

Long-running calendar and evaluation conversations repeatedly changed plans,
assumptions, candidates, prompts, and recommendations. Old summaries and prior
assistant claims remained influential after the underlying contract changed.

Durable structured state should own execution facts; conversation summaries should
carry narrative only. For development work, a compact current brief and commit
history are safer authorities than an accreting chain of plans and corrections.

## Recommended operating changes

These are ranked by expected leverage and intentionally reuse current authorities
rather than propose a new general framework.

### P0 — Define “done” as a user-observable vertical outcome

Before implementation, name the exact terminal state and the seams the feature
must cross. After implementation, replay the original scenario when it is safe and
authorized. For stateful work, include the relevant existing-state, restart,
approval, or migration condition instead of testing only fresh state.

The final handoff should explicitly report which proof levels were and were not
performed.

### P0 — Carry typed authority through pause, resume, and recovery

For Work, connectors, and governed actions, trace one exact identity from proposal
through execution and projection. Remove fallback reads such as “latest review,”
string-based capability classification, or model reconstruction where an exact
contract/version/action identity already exists.

Add one regression for each demonstrated cross-boundary failure; do not build a
second orchestration framework.

### P0 — Preserve actionable failure semantics end to end

Make the originating layer decide failure category, retryability, stable identity,
and recovery. Preserve bounded technical detail and human explanation through the
model and UI boundaries. Deterministic failures should stop after one attempt, and
notifications should require meaningful state/progress change.

### P1 — Restore a green aggregate validation baseline

Fix or explicitly separate the Linux/Tauri feature conflict and current MCP lint
failure. Keep platform-specific native validation as a named obligation, but make
the standard command for this environment exercise the remainder of the workspace
to completion.

### P1 — Qualify integrations and models with the real job shape

An integration should not be described as ready merely because metadata compiles
and credentials exist. Exercise the operation contract, response transform, auth
scope behavior, and approval/recovery route appropriate to the capability.

Primary-model qualification should retain the ambiguous user request, production-
width tool context, multi-step continuation, exact grounding, and restraint case.
Use micro-cases for diagnosis, not broad qualification.

### P1 — Make existing/versioned state a standard risk question

Every persisted or content-addressed change should answer:

- What happens to an already-applied schema?
- What happens to an old digest or replacement lineage?
- What happens to an open approval, task, session, or summary?
- Can fresh creation and existing-version upgrade converge to the same result?

The current immutable migration rule should be enforced mechanically if a small
check can use the existing store/schema authority.

### P1 — Apply information and simplicity contracts before designing

For security changes, identify the actual secret-bearing component and the
forbidden sink; preserve authorized private and ordinary values. For architecture
changes, start with the demonstrated user job, the existing authority to change,
and the smallest general failure mode. Stop when a proposed helper becomes a new
parallel authority or when a one-consumer problem grows a general framework.

### P2 — Require visual proof for material layout changes

When browser inspection is authorized, verify loading-to-live handoff, responsive
geometry, intervention-card states, and recovery/replay variants. When it is not
authorized, state plainly that layout remains statically validated only and avoid
claiming the visual issue itself is closed.

### P2 — Budget stable work by lifecycle frequency

At review time, ask whether new work runs per source edit, process start,
definition revision, turn, continuation, or tool call. Move version-invariant work
to the narrowest existing lifecycle boundary and benchmark before/after for hot
paths.

## What improved during the window

The conversations also produced meaningful systemic corrections:

- a three-class information-handling contract replaced broad secrecy inference;
- the engineering simplicity workflow now requires budgets, stop conditions,
  existing-authority reuse, and net-negative refactors;
- migrations are explicitly immutable once the live dev server may have applied
  them;
- several Work invariant failures became non-retryable and gained seam-specific
  regression tests;
- adapter failures, OAuth callbacks, and browser approvals became more actionable
  and explainable;
- connector definitions now compile once per runtime rather than on every call;
- development rebuild latency improved substantially through measured changes;
- model evaluation now includes more realistic ambiguous, stateful workflows;
- the overengineering remediation removed parallel, dormant, and compatibility
  authorities at significant net-negative code and documentation size.

The current-build audit confirms that most recorded `.noema-dev` incidents are no
longer active: 40 of 78 were classified resolved. It also found 14 confirmed
current issues and eight partial survivors, concentrated in Work continuation and
reconciliation, repeated deterministic failures, connector/version overhead,
memory provenance, canonical terminal status, and long-tail latency. That is
consistent with this retrospective: many symptoms were repaired, while the seam
failure class remains the central risk.

## Evidence map

Short session IDs are included for auditability without embedding transcript
content.

| Session | Date | Principal evidence used here |
| --- | --- | --- |
| `019fc5b2` | Aug 3 | ACP architecture, live validation gaps, unresolved tool-call state |
| `019fc5ce` | Aug 3 | Scheduling abstraction, migration drift, local time, UX follow-ups |
| `019fc63a` | Aug 3 | Adapter write-response contract and uncertain outcomes |
| `019fc873` | Aug 3 | Platform prerequisites, build baseline, validation-cache cost |
| `019fca33` | Aug 4 | Duplicate setup cards and initial OAuth misdiagnosis |
| `019fcb35` | Aug 4 | Relative task CWD and repeated deterministic failure |
| `019fcdeb` | Aug 4 | Lossy compaction, wrong resource identity, false recovery, overbuilt proposals |
| `019fd02f` | Aug 5 | Loading/live layout drift through multiple static-green iterations |
| `019fd089` | Aug 5 | UTC versus user-local day/date authority |
| `019fd08b-1103` | Aug 5 | Composer motion |
| `019fd08b-faa6` | Aug 5 | Adapter-version presentation |
| `019fd920` | Aug 6 | Agent under-agency, conflicting prompts, omitted tool use |
| `019fda52` | Aug 7 | Browser approval continuity, explainability, and over-redaction |
| `019fda67` | Aug 7 | Evaluation realism, invalid recommendations, tool-context cost |
| `019fdac2` | Aug 7 | Development hot paths and nested Cargo/sccache boundary |
| `019fdac3` | Aug 7 | Work gate continuation and uncertain-action reconciliation |
| `019fdcb9` | Aug 7 | Repeated task notifications without meaningful progress |
| `019fdd5d` | Aug 7 | Information-class confusion and codebase-wide remediation |
| `019fdef6` | Aug 8 | Built-in browser capability disagreement during approval |
| `019fdf03` | Aug 8 | Codebase overengineering audit and net-negative remediation |
| `019fe00e` | Aug 8 | Reopen contract/review contamination and retry classification |
| `019fe01b` | Aug 8 | Connector transform diagnostics and user-driven simplification |
| `019fe234` | Aug 8 | Generic OAuth failure text and combined-scope mismatch |
| `019fe251-da26` | Aug 8 | Runtime connector hot path |
| `019fe251-31eb` | Aug 8 | Historical and current-build UX audits |

## Bottom line

Noema's recurring failures are best understood as **authority and verification
failures across seams**. The project does not mainly need more abstractions, more
retries, or more prompt rules. It needs existing structured authority to survive
the complete lifecycle, and it needs completion to be proven at the first
user-observable terminal state—including historical state, pauses, restarts, and
the real client projection when those are part of the job.
