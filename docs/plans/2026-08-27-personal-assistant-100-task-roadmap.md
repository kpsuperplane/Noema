# Roadmap to 100 Verified Personal Assistant Tasks

Date: 2026-08-27

Status: proposed product and engineering sequence

## Objective

Move all 100 tasks in the [capability assessment](../difficult-digital-personal-assistant-tasks.md) to `Verified`.

This roadmap defines dependency order and exit gates. It does not promise delivery dates.

The [live test ledger](../validation/difficult-personal-assistant-test-ledger-2026-08-26.md) supplies the current evidence.

## What 100/100 means

A task becomes `Verified` only when one current live case completes its main path.

A useful limitation report is good behavior. It does not verify a path that never ran.

Each verified case must meet these conditions:

1. Use non-empty, current evidence from every required source.
2. Preserve source links or safe record identifiers for material claims.
3. Separate confirmed facts, inferences, and missing evidence.
4. Apply the user's current preferences and authority where the task requires them.
5. Complete the main outcome instead of only describing missing capability.
6. Keep secrets outside model context and ordinary stored data.
7. Preserve authorized private information unless an egress rule forbids disclosure.
8. Require review for consequential external actions.
9. Save execution evidence, external receipts, and unknown outcomes.
10. Resume correctly after a restart, expired authentication, or a human gate when the task spans those events.

Monitoring tasks need two additional cases. One case must contain a material change. One case must contain no material change.

Write tasks need an approval, execution, receipt, and reconciliation case. Read-only success does not verify a write task.

High-stakes tasks need a domain-specific safety case and a qualified-review boundary.

## Current baseline

| Status | Count |
| --- | ---: |
| Verified | 5 |
| Test | 9 |
| Extend | 50 |
| Build | 36 |
| Total | 100 |

The current verified tasks are 11, 14, 31, 32, and 35.

The first live suite produced five passes, eight partial results, and one failure.

## Roadmap principles

- Reuse Tasks, Projects, Memory, capabilities, action requests, and object-owned files.
- Extend one current authority instead of adding a parallel registry.
- Build one end-to-end vertical slice at a time.
- Add a shared abstraction only after two different production paths need it.
- Prefer source event streams over polling.
- Use connector operations for stable APIs. Use the browser for unsupported portal steps.
- Keep external writes exact, reviewed, auditable, and recoverable.
- Treat each new adapter, field, state, and background process as a cost.
- Promote a task only after live evidence passes its exit gate.
- Keep the 100-task ledger current after every milestone.

## Critical path

The live tests exposed six shared blockers. They should be fixed in this order.

1. Delegated Tasks must receive the same approved read capabilities as primary chat.
2. Time-based work must use a fixed request anchor and bounded review tolerance.
3. Calendar and Gmail reads must support bounded, complete, and reliable retrieval.
4. Projects, Tasks, people, messages, events, and Memory need evidence-backed identity links.
5. Long runs need explicit coverage plans, evidence budgets, and stop conditions.
6. Noema needs private file intake, structured output, and deterministic calculation before many domain tasks can pass.

## Milestone 1: Make the current core reliable

Target: 14/100 Verified.

Tasks: 1, 5, 6, 8, 11, 14, 15, 17, 19, 31, 32, 35, 75, and 98.

### Build and fix

- Give delegated Tasks approved access to native Tasks, Repeat history, Memory, and current connections.
- Use the captured request time for planning. Do not review against a clock that moves during execution.
- Limit correction cycles. Open one clear human gate when the same issue remains after the allowed correction.
- Add Calendar interval reads with recurrence expansion, all-day dates, attendees, locations, and stable pagination.
- Add an upper date bound to Calendar reads.
- Repair Gmail continuation response conversion and complete thread retrieval.
- Preserve source connection and account identity through delegated work.
- Add bounded evidence plans for Calendar, Gmail, Notion, and web research.
- Add stable links between native Projects and native Tasks.
- Add explicit person links across Memory, contact records, email addresses, and Calendar attendees.
- Add Memory recency, replacement history, source dates, and direct editing.
- Ingest real travel confirmations from mail and parsed attachments.

### Required acceptance cases

- A daily brief with non-empty Calendar, Gmail, Tasks, and Memory.
- A seven-day Calendar audit with recurrence, all-day events, locations, and map travel time.
- A daily plan with overload, preferences, breaks, and a stable time anchor.
- A weekly review with Tasks, Repeat history, Calendar, and Gmail.
- Three audience updates from one populated Project and linked Task set.
- A relationship brief for one safely linked person across three sources.
- A Project status from native Projects, native Tasks, and shared artifacts.
- A live itinerary built from actual confirmations, including one changed booking.
- A Memory update case with a replaced fact, duplicate evidence, and later retrieval.

### Exit gate

Each case passes in its intended surface. Cases that delegate also pass inside delegated Tasks.

No case can remain in recovery because its time anchor became stale.

No case can require an unbounded source scan.

## Milestone 2: Close communication and work loops

Target: 32/100 Verified.

Tasks: 2, 3, 4, 7, 9, 12, 13, 16, 18, 20, 21, 22, 24, 26, 28, 33, 38, and 39.

### Build and extend

- Add source events for new mail, changed events, changed files, and Task changes.
- Save source checkpoints and material-change baselines.
- Route detected commitments into existing Tasks or Calendar records after review.
- Add Task dependencies, blockers, milestones, estimates, and explicit owners.
- Link Project decisions to source evidence and later replacements.
- Add stable contact records with interaction history and follow-up boundaries.
- Add Gmail drafts, replies, sent-message reconciliation, and thread closure evidence.
- Add Calendar free-busy, invitations, attendee responses, and negotiation support.
- Add notification thresholds and quiet conditions for proactive work.
- Add prior-result retrieval for recurring Tasks and monitored topics.
- Add learner progress and assessment evidence for adaptive plans.

### Required acceptance cases

- A promise survives source changes and closes only after completion evidence.
- A delayed event replans dependent Tasks and drafts the required notices.
- A monitored thread reports silence only after its agreed deadline.
- A meeting negotiation handles time zones, working hours, and one declined option.
- A decision log preserves the replaced decision and its source history.
- A topic monitor suppresses repeated information and reports one material change.
- A learning plan adapts after a failed assessment and a completed practice block.

### Exit gate

Each long-running case resumes after a server restart without duplicate messages or actions.

Each proactive case explains its trigger, source checkpoint, and stop condition.

## Milestone 3: Add document and decision workflows

Target: 60/100 Verified.

Tasks: 23, 25, 27, 29, 30, 34, 36, 37, 40, 42, 44, 49, 50, 55, 58, 64, 65, 67, 68, 72, 78, 79, 83, 84, 86, 89, 91, and 100.

### Build and extend

- Add private user file and attachment intake into object-owned storage.
- Add image and scanned-document OCR with preserved source images.
- Add batch parsing, field normalization, duplicate detection, and version comparison.
- Add spreadsheet generation and accessible document export.
- Add a controlled deterministic calculation service for totals, dates, and scenarios.
- Add citation-preserving report and packet generation.
- Add browser file uploads through reviewed action requests.
- Add submission receipts and source record reconciliation.
- Add map routes, commute calculations, and location normalization.
- Add current rules, fees, deadlines, and eligibility evidence from authoritative sources.
- Add accessibility profiles and output validation with affected-user testing.

### Required acceptance cases

- A mixed PDF, image, spreadsheet, and email packet produces one cited inventory.
- A calculation case reproduces every total from saved inputs.
- A generated packet preserves source versions and private disclosure boundaries.
- A portal upload returns a receipt and survives an unknown browser outcome.
- An accessible export passes automated checks and review by an affected user.

### Exit gate

No generated number depends only on model arithmetic.

Every uploaded or exported document retains its source, version, owner, and disclosure scope.

## Milestone 4: Add personal operations and transaction systems

Target: 86/100 Verified.

Tasks: 41, 43, 45, 46, 47, 48, 51, 52, 66, 69, 70, 71, 73, 74, 76, 77, 81, 82, 87, 92, 93, 94, 95, 96, 97, and 99.

### Build and extend

- Add bank, biller, merchant, insurer, loyalty, and plan-provider connections as bounded vertical paths.
- Add transaction matching and external receipt reconciliation.
- Add obligation records for bills, renewals, refunds, credits, claims, and notices.
- Add asset, vehicle, pet, subscription, vendor, account, and document records only through active production paths.
- Add inventory history, change evidence, expiry rules, and verification checks.
- Add travel inventory, disruption events, downstream dependency checks, and reviewed rebooking.
- Add return, shipping, refund, cancellation, and claim status operations.
- Add archive import, duplicate detection, backup checks, restore checks, and secure export.
- Add recovery workspaces for account compromise and identity theft.
- Add platform-specific privacy reviews, deletion requests, and later reappearance checks.
- Add digital-legacy records, trusted roles, review dates, and approved exports.

### Required acceptance cases

- A bill plan reconciles one changed amount and one failed payment.
- A cancellation verifies that billing stopped after the final permitted charge.
- A claim records missing evidence, a partial payment, and an appeal deadline.
- A travel disruption updates every downstream reservation without duplicate action.
- A backup case restores sampled files and reports missing content.
- An account recovery case protects credentials while it coordinates external steps.

### Exit gate

Every consequential financial or account write uses an exact reviewed request.

Every external change has a receipt, later verification, or an explicit unknown outcome.

## Milestone 5: Add goals, regulated work, and multi-human coordination

Target: 100/100 Verified.

Tasks: 10, 53, 54, 56, 57, 59, 60, 61, 62, 63, 80, 85, 88, and 90.

### Build and extend

- Add goal outcomes, measures, review history, and links to actual time and Tasks.
- Add authorized health-record intake and bounded FHIR or portal connections.
- Add medication, referral, test, result, authorization, appeal, and care-transition records.
- Add urgent-routing rules that never replace clinical judgment.
- Add multiple-human scopes, consent, roles, assignments, and revocation.
- Add caregiver, child, traveler, and household-specific disclosure rules.
- Add shared Calendar, transport, resource, and handoff planning.
- Add workload evidence and negotiation support without imposing a hidden fairness formula.
- Add family document packets with expiry checks and secure sharing.
- Add qualified professional review for clinical, legal, tax, and regulated financial decisions.

### Required acceptance cases

- A medical record keeps conflicting source facts and requests qualified resolution.
- A medication plan detects a refill gap without giving unsafe clinical advice.
- A care transition links discharge evidence, equipment, transport, medication, and follow-up.
- A shared caregiver plan enforces consent and recipient-specific disclosure.
- A family transport plan handles one cancellation and one unavailable driver.
- A household workload review explains evidence and lets humans decide the allocation.

### Exit gate

Every person can inspect and revoke their applicable authority.

No private information crosses a person or relationship boundary without an authorized purpose.

Noema never presents regulated analysis as a professional decision.

## Program controls

### One ledger

Extend the current 100-task assessment. Do not create a second capability score.

Each row should record status, last run, evidence identifiers, failure class, and next smallest improvement.

### One acceptance package per task

Each package should contain:

- a non-empty success case;
- one distinct failure or recovery case;
- required source and connection state;
- expected external writes and review points;
- privacy and egress boundaries;
- stable completion evidence;
- a maximum source-coverage plan;
- restart and expired-auth expectations when applicable.

### Promotion rule

Promote one row to `Verified` only after its live package passes.

Demote a row when a current regression invalidates its evidence.

### Milestone review

At each milestone, review task coverage, shared failures, run cost, and user-visible value.

Stop a milestone when a proposed system has no current production consumer.

Split work when one slice crosses two unrelated domain authorities.

## Immediate next slices

1. Fix delegated read-capability parity for Tasks, Repeat history, Memory, and connected sources.
2. Replace moving review times with captured planning anchors and bounded tolerance.
3. Deliver one complete Calendar interval read for Task 5.
4. Repair Gmail continuation conversion for Task 1.
5. Add one populated Project, Task, person, and travel acceptance fixture.
6. Rerun the nine current `Test` rows and update the shared ledger.

Do not start Milestone 2 until all Milestone 1 failures have a bounded owner and acceptance case.

## Coverage check

| Milestone | Task count | Cumulative target |
| --- | ---: | ---: |
| 1. Reliable core | 14 | 14 |
| 2. Communication and work loops | 18 | 32 |
| 3. Document and decision workflows | 28 | 60 |
| 4. Personal operations and transactions | 26 | 86 |
| 5. Goals, regulated work, and multi-human coordination | 14 | 100 |

The milestone task lists cover each task number from 1 through 100 exactly once.
