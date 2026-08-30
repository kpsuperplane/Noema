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

Noema has one clear structural blocker.

- Production authentication supports only `human:local`.
- Workspace membership rows exist, and artifact reads already use them.
- Task GraphQL operations still require the local owner and the Personal workspace.
- Memory belongs only to the local human.
- No second human can sign in, inspect shared authority, approve an action, or revoke access.

Therefore, Noema cannot pass the full Milestone 5 multi-human exit gate today.

### Evidence inspected

- [Server authentication](../server-security.md) defines the single built-in human.
- [Action governance](../harness/action-governance.md) defines exact review, destinations, approvals, and execution rechecks.
- [Artifact queries](../../crates/noema-store/src/artifacts/queries.rs) already authorize Task artifacts through workspace membership.
- [Task authorization](../../crates/noema-api/src/graphql/tasks/resolvers/support.rs) still limits product operations to the local owner.
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
| 61 | Use one Project, Tasks, artifacts, events, routes, source warning rules, and reviewed external actions. | A single owner can coordinate the case, but shared caregiver authority is absent. | Test the single-owner path first. |
| 62 | Existing Project files can describe a plan, but other caregivers cannot authenticate or control their authority. | Multi-human identity, consent, inspection, and revocation are absent. | Build one shared-workspace authority slice. |
| 63 | Current tools can compare records and draft recipient-specific messages. | Noema cannot prove the care recipient authorized each recipient or let them revoke access. | Build recipient-bound authority after the shared-workspace slice. |
| 80 | A local coordinator can collect constraints and produce a feasible plan with current tools. | Participants cannot inspect shared data or approve their own disclosures inside Noema. | Test the coordinator path, then reuse multi-human authority. |
| 85 | Event, route, Task, and Project sources can produce a family schedule and backup plan. | Human assignments, custody authority, and revocation are not enforced by Noema. | Test planning first, then reuse multi-human authority. |
| 88 | Project sources and Luau can show workload evidence without a hidden fairness score. | No collaborative decision surface or authenticated participant response exists. | Test evidence quality first. Keep allocation human-owned. |
| 90 | Artifacts, Repeat checks, exact action review, and secure connector delivery cover packet mechanics. | Shared access, recipient authority, and revocation are absent. | Test packet mechanics, then reuse multi-human authority. |

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

### 4. Activate one real multi-human authority slice

This is the first justified shared production build.

The smallest useful slice is one invited human in one non-Personal workspace.
That human must be able to:

- authenticate with their own credential;
- read one shared Project, Task, and artifact;
- see why access exists;
- approve or decline their own pending action when applicable;
- leave the workspace or have membership revoked;
- lose access immediately after revocation.

Reuse the existing `humans`, `workspace_memberships`, artifact membership checks, passkey, and session authorities.
Do not add groups, relationship scopes, household types, or role inheritance in this slice.

### 5. Bind private egress to the authorized recipient

After the shared-workspace slice passes, add one recipient-bound disclosure case.

The action request already binds exact arguments and a destination.
The missing check is whether the current person or workspace authority covers that recipient and resource.

Execution must recheck that authority after approval and before sending.
Revocation must supersede a pending action.

Do not add a separate consent ledger before the shared membership path demonstrates its exact missing fields.

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
6. Build and test one second-human workspace membership path.
7. Add and test one recipient-bound disclosure with revocation.
8. Rerun Tasks 62 and 63 through the new multi-human path.

## Do not build yet

- A goal database.
- A normalized health-record database.
- Medication, referral, claim, or care-transition state machines.
- A generic urgency or clinical rules engine.
- A professional certification registry.
- Caregiver, household, family, or traveler domain models.
- A fairness score or automatic workload allocator.
- A second approval or review system.

## Audit conclusion

Milestone 5 is not primarily a health-platform build.

Most single-owner cases should first run through current general capabilities.
The audit found one certain platform gap: authenticated multi-human authority with revocation.

The next work should maximize evidence before architecture.
Run the single-owner cases first, then build the smallest shared-workspace slice that the remaining failures require.
