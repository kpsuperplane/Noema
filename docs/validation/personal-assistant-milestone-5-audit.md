# Milestone 5 General Capability Audit

Date: 2026-08-30

Status: architecture audit; no production changes

## Scope

This audit covers Tasks 10, 53, 54, 56, 57, 59, 60, 61, 62, 63, 80, 85, 88, and 90.

The audit asks two questions.

1. Can current Noema behavior complete a controlled provider-neutral case?
2. If not, what is the smallest shared missing authority?

The audit does not treat Calendar, messages, health portals, FHIR, maps, or file services as Noema dependencies.
They are possible sources and action destinations.

## Current evidence

Noema already has most execution mechanisms needed for single-owner Milestone 5 cases.

- Projects and `PROJECT.md` can hold current case context.
- Tasks, Task documents, Repeat history, and receipts can hold long-running execution state.
- Private artifacts support file intake, parsing, generation, preview, and reviewed upload.
- Bounded Luau supports reproducible scenario calculations.
- Reviewed adapters and browser actions support reads, writes, receipts, and uncertain outcomes.
- The Task Reviewer now checks every explicit requirement against current evidence.
- Action requests retain exact arguments, destination data, review, approval, and outcome.

Earlier acceptance provides useful evidence.

- Tasks 42 and 44 produced tax and insurance packets with professional boundaries.
- Tasks 55 and 64 preserved private clinical evidence and conflicting recommendations.
- Task 24 tracked exact reviewer responses, required roles, conflicts, and replacement versions.
- Tasks 9 and 16 used Projects and Repeat history for changing obligations and follow-up closure.
- Milestone 3 proved private packets, deterministic calculations, browser uploads, and receipt checks.

Milestone 5 now keeps one explicit product boundary.

- Production authentication supports only `human:local`.
- The local human owns each Milestone 5 Project, Task, artifact, and action request.
- Other people remain external participants represented by current source records.
- Participant input, consent, refusals, and receipts come from connected services or reviewed files.
- Shared Noema workspaces and secondary Noema human accounts are outside this milestone.

### Evidence inspected

- [Server authentication](../server-security.md) defines the single built-in human.
- [Action governance](../harness/action-governance.md) defines exact review, destinations, approvals, and execution rechecks.
- [Task authorization](../../crates/noema-api/src/graphql/tasks/resolvers/support.rs) limits current product operations to the local owner.
- [Milestone 2 acceptance](personal-assistant-milestone-2-acceptance.md) proves Project-backed changing obligations and exact reviewer response tracking.
- [Milestone 3 acceptance](personal-assistant-milestone-3-acceptance.md) proves private clinical briefs, calculations, packets, uploads, and receipts.
- [Milestone 4 browser acceptance](personal-assistant-milestone-4-browser-acceptance.md) proves reviewed portal transactions and unknown-outcome recovery.

## Task audit

| Task | Current path to test first | Demonstrated remaining gap | Audit decision |
| ---: | --- | --- | --- |
| 10 | Use one Project, linked Tasks, bounded event records, Memory priorities, and a fixed review cutoff. | No live case proves that actual time and completed work produce a useful goal adjustment. | Test first. Do not add a goal entity. |
| 53 | Parse private records, read portal or adapter records, preserve conflicts in a cited artifact, and list missing sources. | No controlled case proves multi-source coverage or conflict retention across portal and file inputs. | Test first. Do not add a health-record database. |
| 54 | Use source medication records, one Repeat, refill events, explicit source rules, and a clinician-question packet. | Refill reconciliation and safe stop behavior lack live evidence. | Test first. Do not add a medication registry. |
| 56 | Use a Project case, Tasks, event and route operations, artifacts, browser scheduling, and later result checks. | No live case spans referral, scheduling, records transfer, and follow-up. | Test first with mock services. |
| 57 | Use Repeat history, current portal or device records, source-defined thresholds, Tasks, and reviewed messages. | Connected event intake and urgent escalation have no live health case. | Test first with explicit fixture thresholds. |
| 59 | Parse plan files, fetch current network and formulary facts, and run saved Luau calculations. | No health-plan scenario proves all assumptions and outputs are reproducible. | Test first. Do not add a health calculator service. |
| 60 | Use a Project case, private artifacts, deadlines, browser upload, receipts, and later status checks. | No health appeal case proves exact evidence coverage and closure. | Test first. Reuse the packet path. |
| 61 | Use one Project, Tasks, artifacts, events, routes, source warning rules, and reviewed external actions. | No live case proves every external handoff, acknowledgment, and follow-up. | Test the local-coordinator path. |
| 62 | Use current caregiver source records, one Project, event and Task sources, and reviewed messages. | No live case proves current consent, handoff acknowledgment, and changed availability. | Test with one local coordinator. Do not add shared access. |
| 63 | Compare current records, preserve an exact authorized-recipient list, and send one reviewed private update. | No live case proves recipient, payload, purpose, consent evidence, and receipt together. | Test exact external disclosure. |
| 80 | A local coordinator can collect participant constraints and produce a feasible plan with current tools. | No live case verifies each material constraint and later participant response. | Test through external participant sources. |
| 85 | Event, route, Task, and Project sources can produce a family schedule and backup plan. | No live case proves custody constraints, assignments, cancellation, and acknowledgment together. | Test with one local coordinator. |
| 88 | Project sources and Luau can show workload evidence without a hidden fairness score. | No live case preserves participant responses and the final human-owned allocation. | Test evidence quality first. Keep allocation human-owned. |
| 90 | Artifacts, Repeat checks, exact action review, and secure external delivery cover packet mechanics. | No live case proves current consent, exact recipients, expiry, delivery, and later withdrawal. | Test external packet delivery. |

## General solution opportunities

### 1. Test the existing case path before adding domain state

The first acceptance package should use controlled records and mock portals.
It should exercise Projects, Tasks, artifacts, Luau, adapters, and the browser as one path.

This slice covers goals, medical records, refills, referrals, plan comparison, appeals, and discharge coordination.
It can expose missing general behavior without adding health-specific tables.

The case source should provide the facts that vary by domain.
These include thresholds, deadlines, professional roles, source identities, and consent evidence.

### 2. Reuse Projects as case workspaces

Long-running health and family work resembles earlier legal, financial, travel, and administrative cases.
A Project can hold the current case description.
Tasks can hold execution and follow-up work.
Artifacts can hold source files and deliverables.
Repeat history can handle later checks.

Do not add a universal case, medication, referral, claim, or care-transition registry yet.
Add stored fields only after two live paths need the same enforced query or transition.

### 3. Use explicit source rules for urgent and regulated boundaries

Noema must not invent medical thresholds or professional authority.
The controlled source should state warning rules, deadlines, and required professional decisions.

If a source does not provide a necessary threshold, the Task should request qualified guidance.
The Reviewer should reject a result that silently fills the gap.

Do not add phrase matching, a generic urgency engine, or a professional-certification registry.

### 4. Treat other people as external participants

The local human remains the only Noema principal in this milestone.
Calendars, messages, records, files, and forms supply participant constraints and responses.

Keep stable external identifiers when the source provides them.
Do not infer identity from names alone.
Do not add Noema accounts, workspace membership flows, or shared approval surfaces.

### 5. Use exact human review for private external disclosure

The action request already binds exact arguments and a destination.
Configure human review for Milestone 5 operations that disclose another person's private information.

The approval must show the exact recipient and payload.
The Task must cite current consent or proxy evidence from a current source record.
A later receipt, response, or refusal must remain linked to the Task result.

Test one withdrawal before adding consent storage or a recipient policy system.

### 6. Delay collaboration entities until a live case needs them

Shared plans can initially keep roles, availability, handoffs, and decisions in Project sources.
Noema does not yet need dedicated caregiver, family, trip-participant, workload, or transport-resource tables.

A human assignment field becomes justified when two live shared Tasks need notifications, filtering, or enforcement by assignee.
Until then, adding it would duplicate Project source meaning.

## Recommended acceptance order

1. Run Task 10 with existing Project, Task, event, and Memory sources.
2. Run one combined Tasks 53, 54, 56, and 60 private-record case.
3. Run Task 59 with saved Luau inputs and outputs.
4. Run Task 61 as a single-owner care-transition case.
5. Run Tasks 80, 85, 88, and 90 as local-coordinator cases.
6. Run Tasks 62 and 63 through external caregiver records and reviewed messages.
7. Test one consent withdrawal that makes the local human decline a pending disclosure.

## Do not build yet

- A goal database.
- A normalized health-record database.
- Medication, referral, claim, or care-transition state machines.
- A generic urgency or clinical rules engine.
- A professional certification registry.
- Caregiver, household, family, or traveler domain models.
- A fairness score or automatic workload allocator.
- A second approval or review system.
- Shared Noema workspaces for family or caregiver coordination.
- Secondary Noema human accounts or participant approval surfaces.

## Audit conclusion

Milestone 5 is not primarily a health-platform build.

Most single-owner cases should first run through current general capabilities.
Milestone 5 does not require shared Noema workspaces or multi-human application authority.

The next work should maximize evidence before architecture.
Run the local-coordinator and external-participant cases before adding new production state.
