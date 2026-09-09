# Roadmap to 100 Verified Personal Assistant Tasks

Date: 2026-08-27

Revised: 2026-09-09

Status: active product and engineering sequence

## Objective

Move all 100 tasks in the [capability assessment](../difficult-digital-personal-assistant-tasks.md) to `Verified`.

This roadmap defines dependency order and exit gates. It does not promise delivery dates.

The [live test ledger](../validation/difficult-personal-assistant-test-ledger-2026-08-26.md) supplies the current evidence.

## Portability premise

Calendar, Gmail, and Notion are evidence sources from one user setup. They are not Noema product dependencies.

Different services can supply messages, events, records, files, routes, or external actions.

Acceptance packages should state required behaviors. Noema should use reviewed tool schemas and availability at run time.

Concrete adapters own provider authentication, pagination, field mapping, and recovery.

Noema owns outcomes, permissions, required evidence links, action requests, receipts, and restart behavior.

Noema does not require a manual owner or disclosure manifest for each file.

Noema must not force every provider into one lowest-common-denominator record.

Normalize only the fields required by two current production paths. Preserve provider-specific detail through source references or bounded artifacts.

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
11. State required source behaviors without naming a provider.
12. Record whether integration portability is proved, deferred, or waived by explicit product decision.

Monitoring tasks need two additional cases. One case must contain a material change. One case must contain no material change.

Write tasks need an approval, execution, receipt, and reconciliation case. Read-only success does not verify a write task.

High-stakes tasks need a domain-specific safety case and a qualified-review boundary.

## Current baseline

| Status | Count |
| --- | ---: |
| Verified | 82 |
| Test | 0 |
| Extend | 14 |
| Build | 4 |
| Total | 100 |

Tasks 1 through 45 are verified.

Tasks 49, 50, 53 through 65, 67 through 70, 72, 75, 77 through 80, 83 through 86, 88 through 91, 95, 96, 98, and 100 are also verified.

The first live suite produced five passes, eight partial results, and one failure.

The 2026-08-28 native Project retests promoted Tasks 15 and 19.

All Milestone 1 main paths now pass in one provider setup.

All Milestone 2 main paths now pass in one provider setup.

All Milestone 3 main paths now pass in one provider setup.

All Milestone 5 main paths now pass in controlled provider-neutral cases.

The remaining 20 tasks are Milestone 4 operational paths.

The human waived second-provider portability for Milestone 1 on 2026-08-29.

The results do not prove provider portability.

## Roadmap principles

- Reuse Tasks, Projects, Memory, capabilities, action requests, and object-owned files.
- Extend one current authority instead of adding a parallel registry.
- Build one end-to-end vertical slice at a time.
- Add a shared abstraction only after two different production paths need it.
- Prefer source event streams over polling.
- Describe acceptance needs as provider-neutral behaviors. Use reviewed concrete tools at run time.
- Use adapter operations for stable APIs. Use the browser for unsupported portal steps.
- Keep provider protocol and recovery logic inside its adapter.
- Preserve source identity without copying provider fields into Noema authorities.
- Keep external writes exact, reviewed, auditable, and recoverable.
- Treat each new adapter, field, state, and background process as a cost.
- Promote a task only after live evidence passes its exit gate.
- Keep the 100-task ledger current after every milestone.

## Tool access versus tool sufficiency

Task Executors already receive role-approved read tools, Task-owned writes, and external tools from the current catalog.

They do not receive foreground control tools or unrestricted access to other Tasks. This separation enforces role authority.

Task Planners and Reviewers have narrower access because their roles must not perform the outcome.

The Executor can list bounded owner-authorized Tasks and inspect one full Task document in its workspace.

These reads now cover other Tasks and Repeat occurrences. Tasks 6 and 8 passed after this change.

Tool presence also does not prove that an operation is complete or reliable.

For example, an event tool can exist while omitting recurrence, locations, or stable pagination.

Provider-neutral behavior requirements belong in acceptance packages. They do not require a new Task schema or runtime resolver now.

## Critical path

The live tests exposed eight shared findings. Address current blockers in this order.

| Priority | Change | Why it is needed |
| ---: | --- | --- |
| 1 | Retain bounded owner-authorized Task and Repeat reads for Task Executors. | Tasks 6 and 8 passed with bounded listings and exact full-document reads. |
| 2 | Record provider-neutral operation requirements in each integration acceptance package. | A provider name does not prove that its operations supply the required fields or behavior. |
| 3 | Test current catalog operations against those requirements. | Several exposed operations failed on bounds, conversion, recurrence, or pagination during live cases. |
| 4 | Give time-sensitive outputs one explicit cutoff and review tolerance. | Task 6 entered repeated correction because review used a later moving time. |
| 5 | Make message, event, and record retrieval bounded, complete, and reliable. | Tasks 1, 5, and 8 could not prove source coverage with their available operations. |
| 6 | Add evidence-backed identity links across people, messages, events, and Memory. | Task 17 could not safely join one person across sources. Native Project-to-Task links now pass. |
| 7 | Bound long-run evidence coverage and correction cycles. | Some cases used many calls without a clear completion boundary. |
| 8 | Add private file intake, structured output, and deterministic calculation. | Many document, finance, health, and administrative tasks lack these central systems. |

## Milestone 1: Make the current core reliable

Target: 14/100 Verified.

Tasks: 1, 5, 6, 8, 11, 14, 15, 17, 19, 31, 32, 35, 75, and 98.

### Build and fix

This table records the current need, result, and remaining boundary.

| Change | Why it is needed | Current result | Remaining work |
| --- | --- | --- | --- |
| Retain bounded owner-authorized Task and Repeat reads for Task Executors. | Daily planning and weekly review need other Task records. | Implemented. Tasks 6 and 8 passed. | Keep focused regression coverage. |
| Describe required source operations in each integration acceptance package. | Provider names do not prove required behavior. | The [Milestone 1 package](../validation/personal-assistant-milestone-1-acceptance.md) defines provider-neutral contracts. | Add later tasks when their milestone starts. |
| Check required operations through the reviewed catalog. | Catalog presence proves authority, but not result completeness. | Live message, event, route, Task, Project, Memory, and web paths passed. | Deferred unless portability proof returns. |
| Show unavailable operations before dependent work. | Early notices prevent wasted calls and unsupported conclusions. | Runtime regression coverage proves unavailable rows remain non-callable and visible. | Add one live unavailable-operation case. |
| Record one explicit cutoff in time-sensitive results. | A saved cutoff keeps later review stable. | Tasks 5, 6, 17, 75, and 98 used fixed cutoffs. | Retain the result contract. |
| Limit correction cycles and open one focused human gate. | This prevents repeated disagreement from consuming every run. | Existing review bounds and recovery gates remain active. Task 6 passed without correction. | Retain policy tests. |
| Add bounded event intervals with complete required fields. | Event audits need instances and route inputs. | A reviewed adapter operation returned recurrence, dates, attendees, locations, and continuations. | Second-provider proof is waived. |
| Require both event interval bounds. | One-sided reads cannot prove coverage. | The reviewed event operation requires both bounds. | Retain adapter conformance coverage. |
| Repair message continuation conversion and complete thread retrieval. | Empty pages previously became invalid objects. | The active adapter uses an explicit JSON array. Empty and two-page cases passed live. | Second-provider proof is waived. |
| Retest connection and account identity through continuations. | Long runs must keep evidence tied to one source. | Turn `turn:18d025053feeecf24706` preserved account, message, and thread identity through page two. | Repeat after restart and expired authentication. |
| Add bounded evidence plans for connected and web sources. | Plans prevent repeated low-yield searches. | Every Milestone 1 package now defines a maximum coverage plan and stop condition. | Reduce travel-search call cost further. |
| Retain Project-to-Task reads and full Task document reads. | List previews can omit material evidence. | Implemented. Tasks 15 and 19 passed. | Keep the populated fixture. |
| Add evidence-backed person links across sources. | Name-only matching can merge people and disclose private information. | Task 17 passed with one exact email link stored in Memory. | Add a contact authority only when another path needs it. |
| Preserve Memory source dates, replacement history, and later retrieval. | Current facts need freshness and traceable replacement. | Task 98 passed through claim evidence and normal Memory consolidation. | Direct page editing remains a separate user-interface feature. |
| Ingest travel confirmations from connected messages. | Supplied text does not prove normal intake. | Task 75 used connected messages and reconciled a later cancellation. | Add attachment parsing when a live confirmation requires it. |
| Turn provider failures into adapter conformance cases. | Provider defects belong in adapters. | Event coverage and message empty-page conversion passed live. | Additional provider coverage is deferred. |
| Keep the watched server in one process group. | A restart must stop the old server before rebinding. | A live watch restart replaced one server PID without an orphan. | Retain the watcher regression check. |
| Retry read-only socket checks during restart downtime. | A disconnected test runner must not lose Task identity. | The live runner stayed attached through one server replacement. | Keep mutation submission single-shot. |

### Required acceptance cases

- A daily brief with non-empty message, event, Task, and Memory sources.
- A seven-day event audit with recurrence, all-day events, locations, and route travel time.
- A daily plan with overload, preferences, breaks, and a stable time anchor.
- A weekly review with Tasks, Repeat history, event records, and messages.
- Three audience updates from one populated Project and linked Task set.
- A relationship brief for one safely linked person across three sources.
- A Project status from native Projects, native Tasks, and shared artifacts.
- A live itinerary built from actual confirmations, including one changed booking.
- A Memory update case with a replaced fact, duplicate evidence, and later retrieval.

Tasks 1, 6, and 8 passed their delegated brief, planning, and weekly-review cases on 2026-08-29.

Tasks 15 and 19 passed their native Project acceptance cases on 2026-08-28.

Tasks 5, 17, 75, and 98 passed their live cases on 2026-08-29.

The [Milestone 1 acceptance package](../validation/personal-assistant-milestone-1-acceptance.md) records exact evidence and the portability waiver.

### Exit gate

Each case passes in its intended surface. Cases that delegate also pass inside delegated Tasks.

The human waived the second-provider setup on 2026-08-29.
The accepted cases do not prove provider portability.

No case can remain in recovery because its time anchor became stale.

No case can require an unbounded source scan.

## Milestone 2: Close communication and work loops

Target: 32/100 Verified.

Current: 32/100 Verified.

Tasks: 2, 3, 4, 7, 9, 12, 13, 16, 18, 20, 21, 22, 24, 26, 28, 33, 38, and 39.

### Build and extend

| Change | Why it is needed | Task coverage |
| --- | --- | --- |
| Add source events for changed external data. | Tasks 3, 7, and 12 passed bounded Project-source cases. Connected push intake remains open for Task 3. | 3 |
| Test existing Repeat history near its bounded limit before adding checkpoint storage. | Tasks 33, 38, and 39 passed bounded fixtures. Longer series must prove whether dedicated stored state is necessary. | 33, 38, 39 |
| Reuse Project files for source-linked promise state. | Task 2 passed a bounded continuity and completion case. Test connected capture and longer history before adding a promise entity. | 2 |
| Route detected commitments into existing Tasks or event records after review. | Tasks 3, 9, and 16 passed bounded Project-source capture, exact-ID continuity, and receipt closure. Connected intake remains separate. | 3, 9, 16 |
| Reuse Project files for bounded dependency-aware replanning. | Task 7 passed one controlled delay and fixed-item conflict. Connected Task and Calendar updates remain separate. | 7 |
| Reuse Project files and native Task documents for bounded plans and risk monitoring. | Tasks 20 and 22 passed with exact Task IDs, owners, estimates, milestones, blockers, receipts, and stop conditions. Keep dependency meaning in the Project source until another production path requires native fields. | 20, 22 |
| Reuse Project files for source-linked decision history. | Task 21 passed a bounded replacement case. Test connected capture and longer history before adding a dedicated decision entity. | 21 |
| Route captured decisions and follow-ups into the Project source. | Task 16 passed one replacement with owned follow-ups and later exact receipts. Test longer series before adding dedicated state. | 16 |
| Reuse Project files for stable contact identity, interaction history, cadence, reciprocity, and consent boundaries. | Task 18 passed a bounded changing-source series. Connected interaction intake remains separate. | 18 |
| Reuse Project files, exact thread identities, drafts, and receipts for bounded communication queues. | Tasks 4, 12, and 16 passed provider-neutral reasoning and closure cases. Connected sending and source intake remain separate. | 4, 12, 16 |
| Test connected free-busy, invitation, and reply operations. | Task 13 passed provider-neutral negotiation and drafting. Live operations still need confirmation and receipts. | 13 |
| Reuse Project files for reviewer identities, version-bound responses, quorum, and conflict resolution. | Task 24 passed a bounded approval series. Connected routing and response intake remain separate. | 24 |
| Reuse Project files for access inventories, lifecycle checks, shared ownership, and permission-safe export. | Task 26 passed a bounded receipt-driven handoff. Connected lifecycle execution remains separate. | 26 |
| Reuse Project files for listings, canonical job identity, applications, contacts, outcomes, and strategy changes. | Task 28 passed a bounded receipt-driven pipeline. Connected source intake and reviewed external actions remain separate. | 28 |
| Test additional Reviewer delivery thresholds. | Tasks 9, 12, and 33 passed overdue, silence, and no-change delivery decisions. Test longer series before adding threshold state. | 9 |
| Reuse bounded Task listing and inspection for prior Repeat results. | Tasks 33, 38, and 39 passed without another recurrence-history system. Add direct history only after a failing scale case. | 33, 38, 39 |
| Test longer adaptive learning series and connected source intake. | A bounded Project fixture passed. Add dedicated learner state only after a scale or source-boundary failure. | 39 |

### Required acceptance cases

- A promise survives source changes and closes only after completion evidence. Passed on 2026-08-29.
- A delayed event replans dependent Tasks and drafts the required notices. Passed on 2026-08-29.
- A monitored thread reports silence only after its agreed deadline. Passed on 2026-08-29.
- A meeting negotiation handles time zones, working hours, and one declined option. Passed on 2026-08-29.
- A decision log preserves the replaced decision and its source history. Passed on 2026-08-29.
- A topic monitor suppresses repeated information and reports one material change. Passed on 2026-08-29.
- A learning plan adapts after a failed assessment and a completed practice block. Passed on 2026-08-29.
- One source routes an action and date into native Tasks without duplicates or external writes. Passed on 2026-08-29.
- An obligation becomes overdue once, stays quiet when unchanged, and closes only on timely exact evidence. Passed on 2026-08-29.
- A replaced decision preserves history while every old and new follow-up closes on exact evidence. Passed on 2026-08-29.
- A multi-reviewer approval rejects stale responses, resolves one conflict, and stays quiet when unchanged. Passed on 2026-08-29.
- A relationship plan honors reciprocity, consent, cadence, exact completion, and unchanged silence. Passed on 2026-08-29.
- A handoff package prevents early revocation, preserves ownership, excludes protected data, and closes on exact receipts. Passed on 2026-08-29.
- A job pipeline deduplicates listings, requires receipts, adapts from exact outcomes, and stays quiet when unchanged. Passed on 2026-08-29.
- An ambiguous goal becomes six native Tasks with exact dependencies, milestones, authority, and stop conditions. A second run creates no duplicates. Passed on 2026-08-29.
- A risk monitor detects changed blockers, capacity, critical path, and resolved risks. It stays quiet when unchanged. Passed on 2026-08-29.
- A reply queue preserves thread continuity, exact receipt closure, priority changes, safe drafts, and unchanged silence. Passed on 2026-08-29.

### Exit gate

The 32-task provider-neutral verification target was reached on 2026-08-29.

The shared durable Task and Repeat path must pass one interrupted active run and one changed Repeat occurrence after restart.

Run another restart case only when a task adds a different durable state path or external action path.

Each proactive case explains its trigger, source checkpoint, and stop condition.

Connected intake and external execution receipts remain adapter and action-path acceptance work.
They do not require duplicate core reasoning fixtures for each provider.

## Milestone 3: Add document and decision workflows

Target: 60/100 Verified.

Current: 60/100 Verified.

Tasks: 23, 25, 27, 29, 30, 34, 36, 37, 40, 42, 44, 49, 50, 55, 58, 64, 65, 67, 68, 72, 78, 79, 83, 84, 86, 89, 91, and 100.

### Smallest tested slice

| Change | Why it is needed | Task coverage |
| --- | --- | --- |
| Add private Task artifact intake. | Public downloads cannot receive private records. Task-owned artifacts already provide the correct authority. | 23, 25, 30, 36, 42, 44, 50, 55, 64, 65, 72, 78, 79, 84, 100 |
| Add raster OCR and email parsing to the current file parser. | Mixed packets often include images and saved messages. | 25, 36, 42, 49, 65, 84, 86 |
| Let agents run bounded Luau over read-only JSON. | Calculations and transformations need a general deterministic execution path. | 25, 30, 37, 42, 44, 68, 79, 89, 91 |
| Keep exact artifact and version IDs in exports and citations. | A result needs stable evidence links without a separate provenance manifest. | 23, 25, 27, 29, 36, 42, 50, 55, 64, 89, 100 |
| Test accessibility at the requested deliverable boundary. | One hidden HTML gate cannot validate every format, renderer, or user need. | 23, 25, 29, 37, 42, 89, 100 |
| Upload exact Task artifacts through reviewed browser actions. | Portal work needs controlled file selection and disclosure. | 27, 29, 49, 50, 72, 78, 79, 89, 91 |
| Reconcile upload receipts after an unknown browser result. | A browser failure must not cause a duplicate submission. | 27, 49, 50, 72, 79, 89, 91 |

### Deferred systems

The live cases did not require dedicated case, option, application, inventory, or data-analysis systems.

They also did not require a general spreadsheet generator, map subsystem, or stored accessibility profile.

Task files, Task artifacts, current public research, bounded Luau, and reviewed browser actions cover the bounded paths.

Add a domain system only after a current path cannot continue safely without it.

### Required acceptance cases

- A mixed PDF, image, spreadsheet, and email packet produces one cited inventory.
- A calculation case reproduces every total with saved Luau input and output.
- A generated packet cites each supporting artifact with an exact artifact ID and locator.
- A portal upload returns a receipt and survives an unknown browser outcome.
- An accessible export passes checks selected for its format and target users.

### Exit gate

No accepted generated number depends only on model arithmetic.

Every cited local document has an exact artifact ID and locator.

The earlier Milestone 3 exit conditions passed on 2026-08-29.
The simplified Luau and artifact paths need focused live regression tests.

The human waived affected-user review for the controlled accessibility fixture.
This waiver does not prove usability for an affected user.

The complete evidence is in the [Milestone 3 acceptance package](../validation/personal-assistant-milestone-3-acceptance.md).

## Milestone 4: Add personal operations and transaction systems

Target: 100/100 Verified after the already completed Milestone 5 paths.

Current: 82/100 Verified.

Remaining tasks: 46, 47, 48, 51, 52, 66, 71, 73, 74, 76, 81, 82, 87, 92, 93, 94, 97, and 99.

### Browser acceptance result

The first browser transaction slice ran on 2026-08-30.

The mock bank case passed with one reviewed receipt and no duplicate commit.
The mock flight case submitted once and avoided replay after HTTP 502.

The flight case did not reconcile status until a human correction reopened the Task.
The first Reviewer approved the incomplete result.
A continuation also reopened a POST-generated page with GET and lost its state.

The August 31 rerun closed these shared gaps.

The flight Executor submitted once, observed `PROCESSING`, and reached `CONFIRMED` through read-only checks.

All six required Milestone 4 acceptance families passed.

The dated evidence is in the [remaining acceptance package](../validation/personal-assistant-remaining-acceptance-2026-08-31.md).

The complete evidence is in the [Milestone 4 browser acceptance package](../validation/personal-assistant-milestone-4-browser-acceptance.md).

The six uncertain Build cases ran on August 31.

All six Tasks reached reviewer-approved terminal success without an external action.

Task 96 passed the complete read-only monitoring contract and moved to Verified.

Tasks 46, 47, 48, 51, and 94 moved to Extend.

Their core paths work, but their remaining lifecycle or action evidence is incomplete.

The dated evidence is in the [uncertain Build acceptance package](../validation/personal-assistant-uncertain-build-acceptance-2026-08-31.md).

Use the browser as the first general transaction system.
Do not build bank, airline, or merchant-specific product systems before this slice passes.

### Immediate focus

| Change | Why it is needed | Status | Task coverage |
| --- | --- | --- | --- |
| Bind final submit actions to the current form values and destination. | A button reference does not identify the exact reviewed transaction. | Passed live acceptance. | 41, 43, 45, 48, 73, 76, 77, 81, 87 |
| Convert post-submit transport or HTTP failures into `outcome_uncertain`. | The existing replay guard needs a typed unknown outcome. | Passed live acceptance. | 41, 43, 45, 48, 73, 76, 77 |
| Return uncertain actions to the Executor for status checks. | The Executor must inspect current status before Noema requests human help. | Passed live flight acceptance. | 41, 43, 45, 48, 73, 76, 77 |
| Make the existing Task Reviewer check every explicit requirement. | The Reviewer approved a result that stated it skipped the required status check. | Passed live acceptance. | All Milestone 4 Tasks |
| Resume an active Task browser with `web.browse.snapshot` before opening a URL. | Reopening a POST-generated URL with GET loses form and session state. | Passed live acceptance. | All browser Tasks |
| Rerun bank, flight, and cancellation fixtures after these fixes. | The first generation must reconcile its own result without human correction. | Passed. | 41, 43, 73, 76, 77 |

### Build and extend

| Change | Why it is needed | Task coverage |
| --- | --- | --- |
| Add bounded bank, biller, merchant, insurer, loyalty, and plan-provider paths. | These tasks need current account state and exact provider actions. Research alone cannot complete them. | 46, 47, 48, 73, 76 |
| Add purchase matching and external receipt checks. | Payments, refunds, and claims need proof that source records and external effects agree. | 48, 73 |
| Keep benefits, retirement, debt, estate, and security facts in current Project or Task sources first. | The controlled cases did not prove a need for dedicated ledgers or registries. | 46, 47, 48, 51, 94 |
| Require Luau evidence for generated financial totals before Reviewer approval. | Two accurate results used model arithmetic without the required calculation evidence. | 47, 48 |
| Add later lifecycle and reviewed-action cases only where the outcome needs them. | Read-only reconciliation passed, but submission, transfer, dispute, export, and security changes remain unproved. | 46, 47, 48, 51, 94 |
| Add domain records only through active asset, vehicle, pet, vendor, account, or document paths. | These tasks need durable state, but speculative universal registries would add unused complexity. | 51, 66, 69, 70, 71, 73, 74, 94, 99 |
| Add household profiles for food, events, emergency needs, care, and relationships. | Planning needs preferences, restrictions, contacts, supplies, shared duties, and changing attendance. | 69, 74, 81, 87, 92 |
| Add bounded service operations for active household and lifecycle paths. | Research cannot place orders, manage vendors, coordinate care, or verify service changes. | 66, 69, 70, 71, 73, 74, 81, 82, 87, 92 |
| Add item history, change evidence, expiry rules, and verification checks. | Maintenance and renewal work must detect changed facts and confirm completion. | 66, 70, 71, 73, 74, 94 |
| Add multi-leg travel records, dependency checks, and reviewed booking. | One itinerary can contain several linked reservations and actions. | 76 |
| Add archive import, duplicate detection, backup checks, restore checks, and secure export. | An archive is trustworthy only when content can be restored and checked. | 93, 97 |
| Add move records, change routing, private document transfer, and confirmation checks. | A move affects many services, and each address or account change needs proof. | 82 |
| Add invitation, response, budget, vendor, and guest-communication state. | A personal event needs one current plan across people, payments, and changes. | 81 |
| Add childcare availability, waitlist, payment, transport, and refund operations. | Childcare coordination needs current provider state and child-specific authority. | 87 |
| Add device, backup, authenticator, migration, verification, and disposal paths. | Device replacement cannot finish through document records or browser research alone. | 97 |
| Add account metadata and password-manager or device-security status without credential values. | Security review needs account state while secrets remain outside model context. | 94, 97 |
| Add digital-legacy records, trusted roles, review dates, and approved exports. | Legacy plans need durable authority and periodic review without exposing credentials. | 51, 52, 99 |
| Add executor authority, jurisdiction rules, notices, and estate case reconciliation. | Administering a deceased person's accounts requires legal authority and proof across institutions. | 52 |

### Required acceptance cases

- A bill plan reconciles one changed amount and one failed payment.
- A cancellation verifies that billing stopped after the final permitted charge.
- A claim records missing evidence, a partial payment, and an appeal deadline.
- A travel disruption updates every downstream reservation without duplicate action.
- A backup case restores sampled files and reports missing content.
- An account recovery case protects credentials while it coordinates external steps.

All six cases passed on 2026-08-31.

These cases promoted Tasks 41, 43, 45, 77, and 95.

The backup case passed its shared gate, but Task 93 still needs its complete archive path.

### Exit gate

Every consequential financial or account write uses an exact reviewed request.

Every external change has a receipt, later verification, or an explicit unknown outcome.

## Milestone 5: Add goals, regulated work, and multi-person coordination

Target: 100/100 Verified.

Current: all 14 Milestone 5 tasks are Verified.

Tasks: 10, 53, 54, 56, 57, 59, 60, 61, 62, 63, 80, 85, 88, and 90.

### Audit result

The 2026-08-31 live audit found that most single-owner cases can use current general capabilities.

Projects, Tasks, Repeat history, artifacts, Luau, adapters, browser actions, and action requests cover their main mechanics.
Earlier milestones already proved private clinical briefs, professional boundaries, exact packets, calculations, uploads, receipts, and changing obligations.

Do not add goal, health-record, medication, referral, claim, caregiver, family, or fairness systems before those paths fail a controlled case.

Milestone 5 excludes shared Noema workspaces and secondary Noema human accounts.
The local human owns each Project, Task, artifact, and action request.
Other people participate through current external source records and reviewed external actions.

The complete evidence and per-task decisions are in the [Milestone 5 audit](../validation/personal-assistant-milestone-5-audit.md).

The first controlled result had ten passes, four partial results, and no failures.

Current August 31 follow-up records completed Tasks 57, 62, 80, and 85.

Later-state evidence closed the missing lifecycle checks for the other ten tasks.

One simulated disclosure ran under the current reviewer policy and recorded a receipt.
The withdrawal retest produced one human decision for the final send.
After the decline, Noema blocked the unchanged retry before a second action request.
The fixture recorded zero attempts and zero deliveries.

### Immediate focus

| Change | Why it is needed | Status | Task coverage |
| --- | --- | --- | --- |
| Retain the goal-review case. | The current Task path produced a useful adjustment from actual time and priorities. | Passed. | 10 |
| Retain the combined private-record case. | The current path preserved conflicts, deadlines, missing evidence, and professional boundaries. | Passed. | 53, 54, 56, 60, 61 |
| Retain the saved Luau plan comparison. | The calculation system reproduced exact expected and worst-case totals. | Passed. | 59 |
| Add one later measurement and follow-up closure. | The source warning rule passed, but long-term monitoring did not run. | Passed. | 57 |
| Add complete appointment, trip, and participant response facts. | Missing source facts prevented one feasible final plan and later acknowledgment. | Passed. | 62, 80, 85 |
| Retain transparent workload evidence and human allocation. | The current path showed capacity without a fairness score. | Passed. | 88 |
| Retain current reviewer policy for authorized writes. | The action reviewer already decides whether an action needs the human. | Passed. | 63 |
| Stop equivalent browser effects after a human decline. | New snapshots and empty optional fields previously allowed the same submit to return. | Passed live acceptance. | 90 and Task browser writes |

### Audit disposition of earlier proposals

| Earlier proposal | Audit decision | Task coverage |
| --- | --- | --- |
| Add goal outcomes and measures. | Test Project and Task sources first. Add stored fields only after a repeated enforced query needs them. | 10 |
| Add FHIR and portal connections. | Use reviewed adapters or the browser. Keep provider protocols inside each adapter. | 53, 54, 56, 57, 59, 60, 61, 63 |
| Add a normalized health record. | Do not build yet. Preserve conflicts and source facts in the current case result and artifacts. | 53, 62, 63 |
| Add health workflow records. | Do not build yet. Use one Project, Tasks, artifacts, and Repeat history in controlled cases. | 54, 56, 57, 60, 61 |
| Add health service operations. | Use current adapters and browser actions. Add only missing provider-neutral operation behavior. | 54, 56, 57, 60, 61 |
| Add a health-plan calculator. | Reuse bounded Luau and saved artifacts. | 59 |
| Add urgent-routing rules. | Use explicit source thresholds. Request qualified guidance when a required rule is absent. | 54, 57, 61, 63 |
| Add multi-human authority. | Out of scope. Use one local coordinator and external participant records. | 62, 80, 85, 88, 90 |
| Add recipient disclosure policy. | Reuse exact action requests and current review policy. Add stored policy only after another live failure proves it necessary. | 62, 63, 80, 85, 90 |
| Add shared planning state. | Keep roles and handoffs in Project sources until two live paths need enforced shared fields. | 62, 80, 85, 88 |
| Add workload balancing. | Test transparent evidence. Keep allocation decisions with humans. | 88 |
| Add family packet state. | Reuse artifacts, Repeat expiry checks, exact action review, and current delivery operations. | 90 |
| Add qualified professional review. | Reuse explicit Task boundaries and source-recorded review. Do not make Noema certify professionals. | 53, 54, 56, 57, 59, 60, 61 |

### Required acceptance cases

- A medical record keeps conflicting source facts and requests qualified resolution.
- A medication plan detects a refill gap without giving unsafe clinical advice.
- A care transition links discharge evidence, equipment, transport, medication, and follow-up.
- A shared caregiver plan enforces consent and recipient-specific disclosure.
- A family transport plan handles one cancellation and one unavailable driver.
- A household workload review explains evidence and lets humans decide the allocation.

### Exit gate

The local human owns every Noema Task and action decision.

Every material participant input, consent, refusal, and acknowledgment has a current source record.

No private information crosses a person or relationship boundary without an authorized purpose.

Each private disclosure binds its exact recipient, payload, purpose, human decision, and later receipt when available.

Noema never presents regulated analysis as a professional decision.

No shared Noema workspace or secondary Noema human account is required.

## Program controls

### Provider-neutral capability requirements

Acceptance packages describe required behaviors, not product brands.

Common behavior families include:

| Source role | Example required behaviors |
| --- | --- |
| Message | Bounded search, item and thread reads, continuation, drafts, sends, and receipts |
| Event | Bounded intervals, recurrence, attendees, locations, free-busy, writes, and receipts |
| Record | Bounded queries, relations, versions, attachments, and stable source links |
| File | List, read, download, upload, version, and export |
| Route | Geocode, route, travel time, mode, and evidence time |
| External action | Draft or perform reviewed writes, return receipts, and report unknown outcomes |

An acceptance case includes only the behaviors needed for its current path.

An adapter can support a subset. Missing behavior must remain visible and non-callable.

Noema must not infer support from a provider name, authentication state, or similar operation name.

Provider-specific fields remain in the source result. Noema stores only required normalized facts and bounded references.

The current roadmap requires acceptance metadata, not a new Task field or separate runtime resolver.

Use the existing catalog schemas, availability notices, and role policy until a demonstrated failure needs more stored state.

### Adapter conformance

Each adapter must prove its declared behavior against the same provider-neutral acceptance contract.

Conformance covers bounds, pagination, recurrence, identifiers, timestamps, failures, receipts, and unknown outcomes where applicable.

Build the first provider path concretely. Extract shared production behavior only when a second provider path needs the same contract.

A deterministic adapter fixture can test failures. It does not replace the two production consumers required for shared design.

### Portability evidence

One provider-backed pass verifies the task outcome.

The human waived a second materially different setup for the current roadmap.

Portability remains unproven. Add another setup only when the product needs that claim.

When restored, keep the Task plan, completion rule, privacy policy, and Noema-owned state unchanged.

### One ledger

Extend the current 100-task assessment. Do not create a second capability score.

Each row should record status, last run, evidence identifiers, failure class, and next smallest improvement.

Each integration row should also record provider setups and portability evidence.

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

| Priority | Slice | Why now |
| ---: | --- | --- |
| 1 | Retain the mixed-file, calculation, export, and portal regressions. | These paths support the Milestone 3 main outcomes. |
| 2 | Test one bounded Milestone 4 transaction path. | The next case should prove current account state, one reviewed action, and a receipt. |
| 3 | Test one later Milestone 5 follow-up change. | Task 57 still needs a later measurement and closure. |

Do not add a general transaction system before one bounded Milestone 4 case proves the need.

## Coverage check

| Milestone | Task count | Cumulative target |
| --- | ---: | ---: |
| 1. Reliable core | 14 | 14 |
| 2. Communication and work loops | 18 | 32 |
| 3. Document and decision workflows | 28 | 60 |
| 4. Personal operations and transactions | 26 | 86 |
| 5. Goals, regulated work, and multi-person coordination | 14 | 100 |

The milestone task lists cover each task number from 1 through 100 exactly once.
