# Roadmap to 100 Verified Personal Assistant Tasks

Date: 2026-08-27

Revised: 2026-08-28

Status: proposed product and engineering sequence

## Objective

Move all 100 tasks in the [capability assessment](../difficult-digital-personal-assistant-tasks.md) to `Verified`.

This roadmap defines dependency order and exit gates. It does not promise delivery dates.

The [live test ledger](../validation/difficult-personal-assistant-test-ledger-2026-08-26.md) supplies the current evidence.

## Portability premise

Calendar, Gmail, and Notion are evidence sources from one user setup. They are not Noema product dependencies.

Different services can supply messages, events, records, files, routes, or external actions.

Acceptance packages should state required behaviors. Noema should use reviewed tool schemas and availability at run time.

Concrete adapters own provider authentication, pagination, field mapping, and recovery.

Noema owns outcomes, permissions, provenance, source links, action requests, receipts, and restart behavior.

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
12. Pass a portability case with a second materially different provider setup when integrations are required.

Monitoring tasks need two additional cases. One case must contain a material change. One case must contain no material change.

Write tasks need an approval, execution, receipt, and reconciliation case. Read-only success does not verify a write task.

High-stakes tasks need a domain-specific safety case and a qualified-review boundary.

## Current baseline

| Status | Count |
| --- | ---: |
| Verified | 10 |
| Test | 4 |
| Extend | 50 |
| Build | 36 |
| Total | 100 |

The current verified tasks are 1, 6, 8, 11, 14, 15, 19, 31, 32, and 35.

The first live suite produced five passes, eight partial results, and one failure.

The 2026-08-28 native Project retests promoted Tasks 15 and 19.

The existing results prove behavior in one provider setup. They do not yet prove provider portability.

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

The live tests and portability requirement expose eight shared blockers. They should be fixed in this order.

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

This table separates confirmed gaps, acceptance work, live-proof needs, and retained regression coverage.

| Change | Why it is needed | Basis |
| --- | --- | --- |
| Retain bounded owner-authorized Task and Repeat reads for Task Executors. | Daily planning and weekly review need other Task records. | Implemented and passed by Tasks 6 and 8. |
| Describe required source operations in each integration acceptance package. | This makes missing fields and behavior testable without adding provider names to product logic. | Portability requirement. |
| Check those operations through the current reviewed catalog. | Catalog presence proves exposure and authority. It does not prove complete results. | Confirmed by Tasks 1, 5, and 8. |
| Verify that Task runs show unavailable operations before dependent work. | An early notice prevents wasted calls and unsupported conclusions. | Existing notice path needs acceptance coverage. |
| Record one explicit planning cutoff in time-sensitive results. | The source request time interprets the request. A saved cutoff keeps later review stable. | Confirmed by Task 6. |
| Limit correction cycles and open one focused human gate after repeated failure. | This prevents a correctable disagreement from consuming every run. | Confirmed by Task 6. |
| Add bounded event interval reads with recurrence, all-day dates, attendees, locations, and stable continuation. | Event audits need actual instances and route inputs. | Confirmed by Tasks 1 and 5. |
| Require both bounds for event interval reads. | One-sided reads cannot prove coverage and can return excessive history. | Acceptance and cost control. |
| Repair message continuation conversion and complete thread retrieval. | Task 1 could not read its second result page reliably. | Confirmed by Task 1. |
| Retest connection and account identity through Task continuations. | Evidence must remain tied to the correct source when a long run resumes. | Current binding snapshots exist; live proof remains. |
| Add bounded evidence plans for connected and web sources. | A plan defines coverage and stops repeated low-yield searches. | Task 5 used 83 tool calls. |
| Retain Project-to-Task regression coverage and expose bounded full Task document reads. | Tasks 15 and 19 passed, but list previews can omit material evidence. | Confirmed by the 2026-08-28 retests. |
| Add explicit person links across Memory, contacts, messages, and event attendees. | Name-only matching can merge different people and disclose private information. | Confirmed by Task 17. |
| Add Memory source dates, replacement history, and direct editing. | Task 98 cannot prove freshness or replace outdated facts safely. | Confirmed by Task 98. |
| Ingest travel confirmations from connected messages and parsed attachments. | Task 75 passed only with supplied text, not the normal evidence path. | Confirmed by Task 75. |
| Turn provider failures into adapter conformance cases. | Provider-specific defects belong in adapters, while core behavior remains service-neutral. | Portability requirement. |

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

### Exit gate

Each case passes in its intended surface. Cases that delegate also pass inside delegated Tasks.

Each integration case passes through two materially different provider setups.

No case can remain in recovery because its time anchor became stale.

No case can require an unbounded source scan.

## Milestone 2: Close communication and work loops

Target: 32/100 Verified.

Tasks: 2, 3, 4, 7, 9, 12, 13, 16, 18, 20, 21, 22, 24, 26, 28, 33, 38, and 39.

### Build and extend

| Change | Why it is needed | Task coverage |
| --- | --- | --- |
| Add source events for new messages, changed events, changed files, and Task changes. | Monitoring cannot react promptly or reliably without source change signals. | 3, 7, 12, 33 |
| Save source checkpoints and material-change baselines. | A monitor must distinguish new information from repeated information after restarts. | 12, 33, 38 |
| Route detected commitments into existing Tasks or event records after review. | Extracted promises need one durable owner and later completion evidence. | 2, 3, 9, 16 |
| Add Task dependencies, blockers, milestones, estimates, and explicit owners. | Project planning and replanning need relationships beyond independent Task records. | 7, 20, 22 |
| Link Project decisions to source evidence and later replacements. | A decision log must preserve history without treating an old decision as current. | 16, 21 |
| Add stable contact records with interaction history and follow-up boundaries. | Relationship work needs safe identity, recency, consent, and cadence information. | 13, 18 |
| Add message drafts, replies, sent-message checks, and thread closure evidence. | A prepared response is not a completed communication loop. | 4, 12, 16 |
| Add event free-busy, invitations, attendee responses, and negotiation support. | Multi-person scheduling needs availability and confirmed participant state. | 13 |
| Add reviewer identities, version-bound decisions, routing, quorum, and conflict handling. | One human gate and one model review cannot coordinate several independent reviewers. | 24 |
| Add access inventories, lifecycle checks, shared ownership, and permission-safe export. | Onboarding and handoff must prove access changes without losing accountable ownership. | 26 |
| Add job-source operations, application records, duplicate checks, and outcome history. | A long job search needs durable pipeline state and evidence from each application. | 28 |
| Add notification thresholds and quiet conditions for proactive Tasks. | Monitors need clear rules for when to alert and when to remain silent. | 9, 12, 33 |
| Add prior-result retrieval for recurring Tasks and monitored topics. | A repeated Task must compare with earlier results instead of starting without history. | 33, 38 |
| Add learner progress and assessment evidence for adaptive plans. | An adaptive plan needs measured outcomes before it can change pace or content. | 39 |

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

| Change | Why it is needed | Task coverage |
| --- | --- | --- |
| Add private user file and attachment intake into object-owned storage. | Public downloads cannot receive receipts, statements, forms, or other private user records. | 23, 25, 27, 29, 30, 42, 44, 49, 50, 55, 64, 65, 78, 79, 84 |
| Add image and scanned-document OCR while preserving source images. | Many receipts, forms, and identity records contain no machine-readable text. | 25, 36, 42, 49, 65, 84, 86 |
| Add batch parsing, field normalization, duplicate detection, and version comparison. | Large packets need consistent records without losing source differences or newer versions. | 23, 34, 36, 37, 42, 65, 86 |
| Add durable case, option, application, and inventory records only for active paths. | Documents alone cannot track owners, deadlines, changing status, and completion evidence. | 25, 27, 29, 40, 42, 44, 49, 50, 65, 67, 68, 72, 79, 83, 86, 89, 91 |
| Add bounded domain lookup and action operations only for active paths. | Several tasks need current networks, quotes, listings, registrations, or service actions beyond public research. | 25, 27, 34, 40, 44, 50, 55, 58, 65, 67, 68, 72, 78, 79, 83, 86, 89, 91 |
| Add spreadsheet generation and accessible document export. | Several tasks require a usable deliverable, not only chat text. | 23, 25, 29, 37, 42, 89, 100 |
| Add controlled deterministic calculation for totals, dates, and scenarios. | Financial and schedule results must be reproducible from saved inputs. | 25, 30, 37, 42, 44, 68, 79 |
| Add a controlled data-analysis path for tables and statistical checks. | Task 37 needs reproducible cleaning, analysis, and charts beyond simple totals. | 37 |
| Add report and packet generation that preserves citations. | A final document must retain evidence links, versions, and disclosure boundaries. | 23, 25, 27, 29, 42, 50, 55, 64, 89 |
| Add browser file uploads through reviewed action requests. | Portal workflows cannot finish while Noema can only read or download files. | 27, 49, 50, 72, 78, 79, 83, 89, 91 |
| Add submission receipts and source-record checks. | Noema must prove what a portal accepted and detect unknown outcomes. | 27, 49, 50, 72, 79, 89, 91 |
| Add map routes, commute calculations, and location normalization. | Travel and housing comparisons need realistic duration and consistent locations. | 58, 67, 78, 83 |
| Collect current rules, fees, deadlines, and eligibility evidence from authoritative sources. | Administrative recommendations become unsafe when rules are stale or incomplete. | 27, 40, 49, 50, 78, 89, 91 |
| Add qualified-review boundaries for medical, insurance, tax, and regulated conclusions. | These outputs must remain decision support and expose when professional review is required. | 42, 44, 55, 64 |
| Add accessibility profiles, output checks, and affected-user testing. | Accessible output must match the person's needs and preserve meaning. | 86, 100 |

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

| Change | Why it is needed | Task coverage |
| --- | --- | --- |
| Add bounded bank, biller, merchant, insurer, loyalty, and plan-provider paths. | These tasks need current account state and exact provider actions. Research alone cannot complete them. | 41, 43, 45, 46, 47, 48, 73, 76 |
| Add purchase matching and external receipt checks. | Payments, refunds, and claims need proof that source records and external effects agree. | 41, 43, 45, 48, 73 |
| Add cash-flow, debt, claim, benefit, and retirement ledgers with bounded calculations. | These tasks need forecasts, balances, eligibility, deadlines, and changing case state. | 41, 45, 46, 47, 48 |
| Add obligation records for bills, renewals, refunds, credits, claims, and notices. | Long-lived duties need dates, owners, state, evidence, and closure rules. | 41, 43, 45, 46, 47, 48 |
| Add domain records only through active asset, vehicle, pet, subscription, vendor, account, or document paths. | These tasks need durable state, but speculative universal registries would add unused complexity. | 43, 51, 66, 69, 70, 71, 73, 74, 94, 99 |
| Add household profiles for food, events, emergency needs, care, and relationships. | Planning needs preferences, restrictions, contacts, supplies, shared duties, and changing attendance. | 69, 74, 81, 87, 92 |
| Add bounded service operations for active household and lifecycle paths. | Research cannot place orders, manage vendors, coordinate care, or verify service changes. | 66, 69, 70, 71, 73, 74, 81, 82, 87, 92 |
| Add item history, change evidence, expiry rules, and verification checks. | Maintenance and renewal work must detect changed facts and confirm completion. | 66, 70, 71, 73, 74, 94 |
| Add travel records, disruption events, dependency checks, and reviewed rebooking. | A changed leg can invalidate several later reservations and actions. | 76, 77 |
| Add carrier event feeds, current passenger-rights evidence, and unknown-outcome recovery. | Disruption monitoring and rebooking need live changes, applicable rules, and safe retry boundaries. | 77 |
| Add return, shipping, refund, cancellation, and claim status operations. | Initiating a request does not prove delivery, refund, cancellation, or claim closure. | 43, 45, 72, 79 |
| Add archive import, duplicate detection, backup checks, restore checks, and secure export. | An archive is trustworthy only when content can be restored and checked. | 93, 97 |
| Add move records, change routing, private document transfer, and confirmation checks. | A move affects many services, and each address or account change needs proof. | 82 |
| Add invitation, response, budget, vendor, and guest-communication state. | A personal event needs one current plan across people, payments, and changes. | 81 |
| Add childcare availability, waitlist, payment, transport, and refund operations. | Childcare coordination needs current provider state and child-specific authority. | 87 |
| Add device, backup, authenticator, migration, verification, and disposal paths. | Device replacement cannot finish through document records or browser research alone. | 97 |
| Add account metadata and password-manager or device-security status without credential values. | Security review needs account state while secrets remain outside model context. | 94, 95, 97 |
| Add recovery workspaces for account compromise and identity theft. | Recovery spans many institutions, deadlines, evidence items, and credential-safe actions. | 94, 95 |
| Add service-specific privacy reviews, deletion requests, and reappearance checks. | A deletion request can fail or later data collection can recreate the exposure. | 96 |
| Add digital-legacy records, trusted roles, review dates, and approved exports. | Legacy plans need durable authority and periodic review without exposing credentials. | 51, 52, 99 |
| Add executor authority, jurisdiction rules, notices, and estate case reconciliation. | Administering a deceased person's accounts requires legal authority and proof across institutions. | 52 |

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

| Change | Why it is needed | Task coverage |
| --- | --- | --- |
| Add goal outcomes, measures, review history, and links to actual time and Tasks. | Goal review needs evidence of results and effort, not only a project plan. | 10 |
| Add authorized health-record intake and bounded FHIR or portal connections. | Health tasks need current private clinical records from governed sources. | 53, 54, 56, 57, 59, 60, 61, 63 |
| Add a source-preserving health record with conflict, consent, and proxy state. | Clinical sources can disagree, and each person needs explicit access authority. | 53, 62, 63 |
| Add medication, referral, test, result, authorization, appeal, and care-transition records. | These workflows need durable state across providers, deadlines, and handoffs. | 54, 56, 57, 60, 61 |
| Add pharmacy, provider, insurer, device-event, scheduling, and result operations for active health paths. | Records alone cannot refill, transfer, schedule, monitor, or close care loops. | 54, 56, 57, 60, 61 |
| Add deterministic health-plan scenario calculations from saved plan and care inputs. | Task 59 needs reproducible premiums, cost sharing, tax effects, and worst-case exposure. | 59 |
| Add urgent-routing rules that never replace clinical judgment. | Some evidence needs prompt escalation, while Noema must not make clinical decisions. | 54, 57, 61, 63 |
| Add multiple-human scopes, consent, roles, assignments, and revocation. | Shared work needs explicit authority for each person and a way to withdraw it. | 62, 80, 85, 88, 90 |
| Add recipient-specific disclosure rules for caregivers, children, travelers, and households. | Authorized access for one relationship does not authorize disclosure to every participant. | 62, 63, 80, 85, 90 |
| Add shared event, transport, resource, and handoff planning. | Multi-person coordination needs assignments and shared constraints beyond one human's Tasks. | 62, 80, 85, 88 |
| Add workload evidence and negotiation support without a hidden fairness formula. | Humans need transparent evidence and control over value-based allocation decisions. | 88 |
| Add family document packets with expiry checks and secure sharing. | Family workflows need current documents delivered only to authorized recipients. | 90 |
| Add qualified professional review for clinical, legal, tax, and regulated financial decisions. | High-stakes synthesis must stop before it becomes unauthorized professional judgment. | 53, 54, 56, 57, 59, 60, 61 |

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

Provider-specific fields remain in the source result. Noema stores only required normalized facts, provenance, and bounded references.

The current roadmap requires acceptance metadata, not a new Task field or separate runtime resolver.

Use the existing catalog schemas, availability notices, and role policy until a demonstrated failure needs more stored state.

### Adapter conformance

Each adapter must prove its declared behavior against the same provider-neutral acceptance contract.

Conformance covers bounds, pagination, recurrence, identifiers, timestamps, failures, receipts, and unknown outcomes where applicable.

Build the first provider path concretely. Extract shared production behavior only when a second provider path needs the same contract.

A deterministic adapter fixture can test failures. It does not replace the two production consumers required for shared design.

### Portability gate

One provider-backed pass verifies the task outcome. A second materially different pass verifies portability.

The second setup can use different providers for every external source.

The Task plan, completion rule, privacy policy, and Noema-owned stored-state shape must remain unchanged between setups.

Only bindings, source references, and provider-specific evidence can differ.

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
| 1 | Add bounded other-Task and Repeat reads for Task Executors. | This is the exact access gap from Tasks 6 and 8. Connected tools and Memory already follow role policy. |
| 2 | Define bounded operation requirements for Tasks 1, 5, 6, and 8 acceptance cases. | The next retests need explicit proof of bounds, fields, continuation, and source coverage. |
| 3 | Save one current planning cutoff and review with bounded tolerance. | This removes Task 6's repeated moving-time rejection without misusing the old request time. |
| 4 | Deliver one complete event interval path through the current adapter system. | Task 5 needs recurrence, all-day dates, locations, attendees, bounds, and continuation together. |
| 5 | Repair message continuation conversion at the provider-neutral result boundary. | Task 1 failed when it requested the second message page. |
| 6 | Add a second materially different provider setup for message, event, and record acceptance. | One working provider does not prove that Noema's plan remains service-neutral. |
| 7 | Retain the populated Project fixture. Add person and travel fixtures. | Project cases now pass. Tasks 17 and 75 still lack normal evidence fixtures. |
| 8 | Rerun the seven current `Test` rows and update the shared ledger. | Current verdicts must reflect the fixes and recent project changes. |

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
