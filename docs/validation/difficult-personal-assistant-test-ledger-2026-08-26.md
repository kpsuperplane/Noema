# Difficult Personal Assistant Live Test Ledger

Test date: 2026-08-26

Noema baseline: commit `6cb847c3`

The development instance also included pre-existing uncommitted worktree changes. This test did not change those files.

Environment: local Noema development instance through the authenticated Unix GraphQL socket.

Scope: the 14 rows marked `Test` in the 2026-08-25 capability assessment.

The user authorized read-only interaction with connected services as the local human.

No test requested an external write. No pending governed action or source-system write was observed.

Noema created internal delegated Tasks for several long cases. These Tasks remain part of the test evidence.

Private source contents are not copied into this ledger.

## Verdict rules

| Verdict | Meaning |
| --- | --- |
| Pass | One live case completed the main acceptance path with adequate evidence and safe behavior. |
| Partial | The case produced useful work, but missing data or capability prevented the main path from completing. |
| Fail | Noema did not reach a usable terminal result for the requested path. |

## Results

| Task | Verdict | Live evidence | What worked | Remaining gap or required improvement |
| ---: | --- | --- | --- | --- |
| 1 | Partial | Turn `turn:18cf78a655a04163e946` | Calendar, Gmail, native Tasks, and Memory reads ran. The brief separated facts, inferences, and missing evidence. | Calendar results did not provide a reliable bounded day view. Gmail second-page parsing failed. Non-empty Task and preference evidence was unavailable. Add bounded calendar reads and repair Gmail pagination. |
| 5 | Partial | Turn `turn:18cf78ba96c78e05eb77`; Task `task:18cf78c45775405eec8f` | Noema completed a fully paginated primary-calendar audit. It normalized time zones and assessed conflicts and recovery load. | Recurring instances, all-day dates, locations, and route inputs were unavailable. Add interval-native event expansion and location fields. Then test map travel time. |
| 6 | Fail | Turn `turn:18cf78d6f571d6edeebd`; Task `task:18cf78d920a75878ef0d`; Gate `gate:18cf79564ae9e19afe4b` | A cautious conditional draft used verified Calendar evidence and disclosed missing data. | The delegated worker lacked native Task access. Review repeatedly rejected a moving current-time anchor until recovery was required. Use a fixed planning anchor, allow bounded time tolerance, and expose native Tasks to delegated workers. |
| 8 | Partial | Turn `turn:18cf795ae6f7cc8ffecf`; Task `task:18cf795ccdf988d1ff0a` | Calendar and Gmail supported a useful prior-week comparison, preview, and open-loop list. | Native Tasks and Repeat history were unavailable to the delegated worker. Several secondary calendars failed response transformation. Expose those reads and repair the calendar transforms. |
| 11 | Pass | Turn `turn:18cf787186722186e3e2` | Noema separated facts and assumptions, ranked authority and relationship risks, drafted escalations, and kept every action uncommitted. | The case was synthetic. Retain this result and later add a source-backed cross-system conflict case. |
| 14 | Pass | Turn `turn:18cf797a8a0ad2fb10290`; Task `task:18cf797c9fa7fd3c102da` | Noema found a qualifying current external meeting. It joined Calendar, Gmail, and contact context into a sensitive, cited brief. | No attachment supplied useful evidence. Add a fixture with a parsed attachment and a conflicting prior decision. |
| 15 | Partial | Turn `turn:18cf79a54645207410784`; Task `task:18cf79a747cca7fa107ca` | Noema searched Project and Task evidence, detected missing relations, and correctly refused an unsupported status narrative. | No suitable Project had linked Tasks and recorded progress. The test did not exercise three fact-matched audience versions. Add a populated linked Project fixture. |
| 17 | Partial | Turn `turn:18cf79d196f8a1e110ce8`; Task `task:18cf79d3ffab445f10d2d` | Noema searched Memory, Gmail, and Calendar. It rejected unsafe name-only identity merges and applied strong privacy boundaries. | No person had unambiguous evidence across all three sources. Add stable contact identity links and expose Calendar attendees. Then retest changed facts and staleness. |
| 19 | Partial | Turn `turn:18cf79fcdad1b8e311203`; Task `task:18cf79fece95c48c11244` | Noema produced a cited status with progress, dates, blockers, conflicts, missing reports, and confidence from current project-like records. | The source lacked a native Project-to-Task relation. A Notion query limit blocked aggregate status. Test a populated native Project with linked Tasks and shared artifacts. |
| 31 | Pass | Turn `turn:18cf785335a1c2dee0e2` | Noema compared current products under budget and compatibility constraints. It normalized price, warranty, evidence disagreements, and preferences. | Add a future case with tax, shipping, and a user Memory preference. |
| 32 | Pass | Turn `turn:18cf783a8eeee0d7de6b` | Noema produced a current research brief from primary sources. It separated facts, inferences, uncertainty, and recommendations with citations. | Retain citation regression tests across provider and browser changes. |
| 35 | Pass | Turn `turn:18cf7866a3d65342e2da` | Noema traced claims to current primary sources. It handled dates, definitions, scope limits, and unresolved uncertainty. | Add an archived-source case when historical web evidence is available. |
| 75 | Partial | Turn `turn:18cf788bed8794e6e6af` | Noema normalized a non-empty multi-zone itinerary. It found a cancellation, transfer risks, missing legs, and unsupported terms. | The fixture was supplied as text. Current mail intake, parsed confirmations, change reconciliation, and live travel connections remain untested. |
| 98 | Partial | Turns `turn:18cf78968b0e7583e7a8` and `turn:18cf789fbd363e9fe895` | Memory page reads and targeted full-text searches worked. The audit checked hierarchy, citations, topic overlap, and retrieval relevance. | Memory lacks visible recency, replacement history, direct editing, and source-level duplicate evidence. Test repeated updates after those systems exist. |

## Retest results

### Native Project retests

Retest date: 2026-08-28

Noema baseline: commit `f13c8f80`

Fixture: Project `project:18d01c0ca430362e9c65` with four linked Tasks.

The retests used only native Noema records. They made no external writes.

No pending governed action or intervention remained after either case.

| Task | Verdict | Live evidence | What worked | Remaining gap or required improvement |
| ---: | --- | --- | --- | --- |
| 15 | Pass | Turn `turn:18d01c1c553998c49e3c`; Project `project:18d01c0ca430362e9c65` | Noema produced executive, engineering, and customer updates from one evidence set. It changed detail, tone, confidentiality, asks, and format without changing the facts. It cited each source and reported conflicts. | Exact full Task document reads now exist. Retain the case and add a materially different source setup later. |
| 19 | Pass | Turn `turn:18d01c2ad82524729fc4`; Project `project:18d01c0ca430362e9c65` | Noema joined the Project and four linked Tasks into one cited status. It covered progress, reliability, budget, schedule, decisions, risks, blockers, and dependencies. It separated facts, inferences, conflicts, and missing evidence. | Exact full Task document reads now exist. This synthetic native case did not test external sources. |

### Delegated Task read retests

Retest date: 2026-08-29

Noema baseline: commit `9288cec3`

These retests used native Task reads, current Memory, and connected Calendar records. They made no external writes.

No pending governed action or intervention remained after these cases.

| Task | Verdict | Live evidence | What worked | Remaining gap or required improvement |
| ---: | --- | --- | --- | --- |
| 1 | Pass | Turn `turn:18d01ff93a8c9d2c1b6e`; Task `task:18d01ffd731ecaca1bed` | The delegated brief used non-empty message, Calendar, Task, and Memory results. It preserved identifiers, bounds, deductions, and gaps. | Calendar retrieval reached its page bound and several transforms failed. An isolated follow-up migrated legacy Task roots and returned all 22 cancelled Tasks. |
| 6 | Pass | Turn `turn:18d01f103dec34a2e1`; Task `task:18d01f15daba1ae718d` | The delegated Task used a fixed cutoff. It listed bounded native Tasks and inspected full documents. It produced a reviewed plan without correction. | Calendar continuation failed. All-day dates, recurring instances, durations, and working-hour preferences remained unavailable. These limits reduced confidence but did not block the plan. |
| 8 | Pass | Turn `turn:18d01f61f147ada9a50`; Task `task:18d01f67400f26d6ae9` | The delegated Task reviewed seven Repeat occurrences. It inspected full Task documents, events, Memory, projects, deadlines, and preparation needs. | Some Calendar transforms failed. Legacy Task roots later moved into private storage. An isolated follow-up returned all 22 cancelled Tasks. |

### Remaining Milestone 1 retests

Retest date: 2026-08-29

Noema baseline: commit `627798be`, plus reviewed adapter revisions in `/var/lib/noema-dev`.

These cases used one connected provider setup. They made no external writes.

| Task | Verdict | Live evidence | What worked | Remaining gap or required improvement |
| ---: | --- | --- | --- | --- |
| 5 | Pass | Turn `turn:18d0245b5bcd9519346d`; Task `task:18d0245e3ce383b334c6` | Two bounded event audits reached natural ends. They preserved recurrence parents, all-day dates, attendees, locations, overlaps, and gaps. A cited route returned 640 seconds for a 900-second gap. | The route origin was a street-address proxy. Second-provider proof was waived on 2026-08-29. |
| 17 | Pass | Turn `turn:18d0228af4cdf1b016f`; Task `task:18d0228d86e1722b1be` | One exact email safely joined Memory, messages, and events. The result preserved identity evidence, bounded coverage, privacy limits, commitments, and staleness. | Add a contact authority only when another production path needs it. Second-provider proof was waived on 2026-08-29. |
| 75 | Pass | Turn `turn:18d0215e75b196314335`; Task `task:18d021608fbeaba0437a` | Connected messages supplied two confirmations, one duplicate, and one later cancellation. The result preserved references and reported missing dates and transport. | Attachment parsing was not needed. Second-provider proof was waived on 2026-08-29. |
| 98 | Pass | Turn `turn:18d023176f3b6f6d106d`; Task `task:18d02319bfaafb6b10b4` | Normal Memory consolidation replaced one fact, kept two direct evidence items, retained superseded values, and supported later retrieval. | Direct page editing remains a separate user-interface feature. |

### Message adapter follow-up

The active message adapter previously converted an absent `messages` field into an untyped empty table.

The reviewed revision now uses an explicit JSON array.

Turn `turn:18d024fb180b0b1245e3` returned a successful empty `messages: []` result.

Turn `turn:18d025053feeecf24706` returned two pages with stable account, message, and thread identity.

The second page preserved another continuation. No external write occurred.

### Restart follow-up

The development watcher replaced PID `3691383` with PID `3691494` without an orphan.

The live runner stayed attached through another replacement and returned Task `task:18d0267c633ac6cfab`.

That Task reached reviewer-approved terminal success with marker `RESTART-RUNNER-OK`.

The Task finished before process replacement. It does not close active-run recovery.

Task `task:18d02592e7bd0d0c5665` recovered one expired Executor lease and wrote a result.

It then entered an Executor continuation loop. The test stopped before reviewer approval.

Task `task:18d0261b98922f14b61` also recovered one expired Executor lease.

It stopped at an unrelated expired Notion authentication request. The test made no external write.

Task `task:18d026bb41c93fb158a` then closed the active-run recovery gate.

Server PID `3692881` stopped during Executor run `run:18d026bf54a3918d5f2`.

The replacement started as PID `3695703`. The expired run stopped with `lease_expired`.

Executor attempt 1 wrote `ACTIVE-RESTART-OK`. Reviewer run `run:18d026dd9f687d6d27f` approved it.

The live runner returned reviewer-approved terminal success without an external write.

### Expired-authentication follow-up

The Notion connection reports `authenticated` and `healthy`.

Read-only Task `task:18d026f309e1d56c4de` reopened as generation 2 after reconnection.

Executor run `run:18d02a8b18e6bb6f8e3` completed the Notion search and exact-result fetch.

Reviewer run `run:18d02a9e8308c7dcb1f` approved the result. The same Task reached terminal success.

## Summary

The first suite produced five passes, eight partial results, and one failure.

The accepted current results include nine retests from 2026-08-28 and 2026-08-29.

- Pass: 14
- Partial: 0
- Fail: 0

The strongest paths are research, fact-checking, conflict reasoning, and meeting preparation.

Bounded native Task reads now work inside delegated Tasks.

Earlier delegated workers lacked native Tasks, Repeat history, Calendar fields, or stable bounded reads available to the primary chat.

The Task 1, 6, and 8 retests passed with bounded Task listings and full document reads.

The Task 15 and 19 retests passed with native Project-to-Task links.

One exact email now provides a safe Memory, message, and event link for Task 17.

Long-run efficiency remains uneven. Some delegated cases used many provider rounds before reaching a bounded result.

For example, one Task 5 executor run dispatched 83 tool calls. Task 14 needed two executor runs with 97 combined tool calls.

## Priority improvements

The human waived second-provider portability testing on 2026-08-29.

The current evidence verifies one provider setup. It does not prove portability.

1. Retain Task, Repeat, Project, Memory, event, and message regression coverage.
2. Reduce repeated low-yield message searches in travel cases.
3. Retain restart and expired-authentication recovery coverage.
4. Add direct Memory page editing only when a user-facing edit path starts.
