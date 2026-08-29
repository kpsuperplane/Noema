# Milestone 1 Personal Assistant Acceptance Package

Date: 2026-08-29

Status: all 14 main paths pass in one provider setup

Portability status: open

## Purpose

This package defines Milestone 1 behavior without requiring specific third-party services.

The live evidence uses one current user setup. Different providers can satisfy the same behavior contracts.

## Common acceptance rules

Each case must use non-empty evidence from every required source role.

Each material claim must keep a source URL or safe stable identifier.

The result must separate confirmed facts, deductions, and missing evidence.

Every time-sensitive case must record one fixed evidence cutoff.

Every source scan must state a finite bound or reach a natural continuation end.

An unavailable required behavior must stay visible and non-callable.

No Milestone 1 case needs an external write. Drafts must remain uncommitted.

Authorized private information can enter the run. It must not cross an unrelated egress boundary.

Secrets must remain outside model context, Task files, results, logs, and exports.

## Provider-neutral source contracts

| Source role | Required behavior |
| --- | --- |
| Message | Identify the account. Search with explicit bounds. Continue safely. Read exact items or complete threads. Preserve message and thread identity. |
| Event | Identify the account. Require both interval bounds. Expand recurring instances. Preserve parent IDs, all-day dates, attendees, locations, and continuation. |
| Work | List bounded Tasks or Repeat occurrences. Inspect one exact full Task. Preserve Project links, stages, documents, and timestamps. |
| Memory | Search bounded pages. Read one exact page. Preserve claim evidence, replacement text, stable page identity, and later retrieval. |
| Web | Search or open bounded sources. Preserve citations, retrieval time, and source disagreement. |
| Route | Geocode both locations. Preserve coordinates and source URLs. Return route mode, duration, distance, retrieval time, and limitations. |

Provider authentication, pagination tokens, field mapping, retries, and protocol failures belong to each adapter.

Noema owns the plan, evidence boundary, permissions, result, review, and restart behavior.

## Task packages

| Task | Required sources and state | Success and completion evidence | Maximum coverage plan | Recovery and privacy boundary |
| ---: | --- | --- | --- | --- |
| 1 | Active message and event sources, native Tasks, and Memory. Read-only. | Produce one daily brief with non-empty results from all four roles. Preserve bounds, identifiers, priorities, and missing evidence. | One bounded day interval. Three message queries with three pages each. One bounded Task list. Two Memory reads. | If one role is unavailable, identify it before synthesis. Do not expose message or Memory contents outside this private result. |
| 5 | Active event and route sources. Read-only. | Reach the seven-day event continuation end. Preserve recurrence parents, all-day dates, attendees, locations, overlaps, gaps, and one route comparison. | One seven-day interval. Continue to its natural end or 100 pages. Route only adjacent pairs with usable locations. | If route geocoding uses a proxy, label it. Never infer a location, duration, or recurrence parent. |
| 6 | Active event source, native Tasks, and Memory preferences. Read-only. | Produce one feasible daily plan. Show fixed commitments, priorities, breaks, overload, and deferred work at one fixed cutoff. | One day of events. One bounded Task list. Inspect only Tasks selected for the plan. Two Memory reads. | If work does not fit, defer it visibly. Review uses the saved cutoff and a stated tolerance. |
| 8 | Active message and event sources, native Tasks, Repeat history, and Memory. Read-only. | Compare the prior week with the next week. Preserve outcomes, open loops, deadlines, capacity, and preparation needs. | Two bounded event intervals. Three message queries with three pages each. Fifty Tasks. Fifty Repeat occurrences. | If a source remains incomplete, state the coverage limit. Do not treat an absent hit as global absence. |
| 11 | Current request evidence, Tasks, messages, events, and relationship context when available. Draft-only. | Rank conflicting requests. Explain authority, deadlines, dependencies, and relationship risks. Prepare an escalation plan without sending it. | Inspect only records linked to the conflicting requests. Stop when each claim has one direct source. | Never send or commit a draft. Keep sensitive relationship context within the intended audience. |
| 14 | One current event, related messages, Memory, and available files. Read-only. | Produce a decision-focused brief with attendee roles, history, decisions, open questions, desired outcomes, and sensitive boundaries. | One event. Three message queries with three pages each. Five related threads. Five Memory or file reads. | Exclude unrelated private history. Mark missing attachments or conflicting decisions. |
| 15 | One populated native Project and its linked Tasks. Read-only. | Produce executive, engineering, and customer updates from one fact set. Change detail and tone without changing facts. | One Project. Up to 100 linked Task rows. Inspect every material Task document once. | Keep internal risks and private details from customer copy unless authorized. |
| 17 | One exact person key across Memory, messages, and events. Read-only. | Produce one relationship brief from three source roles. Preserve identity evidence, history, commitments, upcoming contact, staleness, and missing evidence. | One Memory page. Two message pages. Eight threads. Two bounded event intervals. | Reject name-only joins. Do not infer a relationship type or disclose unrelated private information. |
| 19 | One native Project, linked Tasks, and shared artifacts. Read-only. | Produce one cited status with progress, schedule, decisions, risks, blockers, dependencies, conflicts, and confidence. | One Project. Up to 100 linked Tasks. Inspect every material Task and artifact once. | Report conflicting states. Do not convert missing updates into completed work. |
| 31 | Current web sources and Memory preferences when relevant. Read-only. | Compare current options under budget and compatibility constraints. Preserve price, warranty, disagreement, and recommendation logic. | Ten search result pages. Open at most 20 material sources. Stop when every decision field has two sources or one primary source. | Do not expose private preferences to external sites. Mark tax, shipping, and availability limits. |
| 32 | Current public web sources. Read-only. | Produce a source-grounded brief with primary evidence, citations, disagreement, deductions, uncertainty, and recommendations. | Four focused searches. Open at most 20 material sources. Stop when each major claim has direct support. | Do not present unsupported or stale claims as current facts. |
| 35 | Current public web and archives when required. Read-only. | Trace each claim to evidence. Verify dates, definitions, scope, contradictions, and unresolved uncertainty. | Four focused searches. Open at most 25 sources. Stop after every claim has a verdict and evidence state. | Preserve uncertainty. Do not use source authority as proof of an unrelated claim. |
| 75 | Active message source with actual confirmations. Read-only. | Build one itinerary from two confirmations and one later change. Reconcile duplicates, replacements, local times, references, missing segments, and risks. | Five focused queries with three pages each. Inspect up to 20 candidate threads. Use exact references for the final reconciliation. | Keep booking references private. Do not infer missing dates, transport, status, or transfer feasibility. |
| 98 | Native Memory and eligible conversation evidence. Memory consolidation may update canonical Memory. | Replace one fact, keep duplicate direct evidence, preserve the old fact as superseded, then retrieve the current fact later. | One focused Memory page. Two evidence messages. One consolidation cycle. Four retrieval queries. | Only eligible human messages and exact tool results can support claims. Secrets cannot become Memory evidence. |

## Live evidence

| Task | Result | Primary evidence | Review state |
| ---: | --- | --- | --- |
| 1 | Pass | Turn `turn:18d01ff93a8c9d2c1b6e`; Task `task:18d01ffd731ecaca1bed` | Approved and terminal success |
| 5 | Pass | Turn `turn:18d0245b5bcd9519346d`; Task `task:18d0245e3ce383b334c6` | Approved and terminal success |
| 6 | Pass | Turn `turn:18d01f103dec34a2e1`; Task `task:18d01f15daba1ae718d` | Approved and terminal success |
| 8 | Pass | Turn `turn:18d01f61f147ada9a50`; Task `task:18d01f67400f26d6ae9` | Approved and terminal success |
| 11 | Pass | Turn `turn:18cf787186722186e3e2` | Foreground pass |
| 14 | Pass | Turn `turn:18cf797a8a0ad2fb10290`; Task `task:18cf797c9fa7fd3c102da` | Approved and terminal success |
| 15 | Pass | Turn `turn:18d01c1c553998c49e3c`; Project `project:18d01c0ca430362e9c65` | Foreground pass |
| 17 | Pass | Turn `turn:18d0228af4cdf1b016f`; Task `task:18d0228d86e1722b1be` | Approved and terminal success |
| 19 | Pass | Turn `turn:18d01c2ad82524729fc4`; Project `project:18d01c0ca430362e9c65` | Foreground pass |
| 31 | Pass | Turn `turn:18cf785335a1c2dee0e2` | Foreground pass |
| 32 | Pass | Turn `turn:18cf783a8eeee0d7de6b` | Foreground pass |
| 35 | Pass | Turn `turn:18cf7866a3d65342e2da` | Foreground pass |
| 75 | Pass | Turn `turn:18d0215e75b196314335`; Task `task:18d021608fbeaba0437a` | Approved and terminal success |
| 98 | Pass | Turn `turn:18d023176f3b6f6d106d`; Task `task:18d02319bfaafb6b10b4` | Approved and terminal success |

Task 5 combines two bounded event audits with one cited route result.

The route origin remains a street-address proxy. It is not a verified building pin.

Task 17 joins one person through the exact email `corbin.mcelhanney@gmail.com`.

Task 75 uses actual connected messages. It reconciles one duplicate confirmation and one later cancellation.

Task 98 uses normal Memory consolidation. It keeps two direct evidence identifiers for the current fact.

## Adapter follow-up evidence

Turn `turn:18d024fb180b0b1245e3` proves that an empty message result now returns `messages: []`.

Turn `turn:18d025053feeecf24706` proves two message pages and stable account, message, and thread identity.

The active message connection uses reviewed semantic digest `e7fd5eb27cd7160ecfa76c22298c4c600e7d8401cbc268c624711c1d9d1a3a5c`.

The active event connection uses reviewed semantic digest `65337a4aeeeff8b7b64247d70b8e3832b5d5797b57d58d91d06bd601a3136c84`.

These values identify this setup. They are not Noema product dependencies.

## Restart follow-up evidence

The development watcher now keeps its child command in one process group.

One live restart replaced server PID `3691383` with PID `3691494`.

The old server exited before the new server bound its ports and socket.

The live runner now retries read-only socket checks during restart downtime.

Task `task:18d0267c633ac6cfab` finished with marker `RESTART-RUNNER-OK` and reviewer approval.

The runner stayed attached while the server changed from PID `3691494` to PID `3692881`.

That Task finished three seconds before the new process started. It does not prove active-run recovery.

Task `task:18d02592e7bd0d0c5665` did cross an active Executor restart.

Run `run:18d0259cf2c8bfe85766` expired safely. Run `run:18d025cde6dc688e273` retried and wrote the result.

The Task then entered an Executor continuation loop. It was cancelled before reviewer approval.

Task `task:18d0261b98922f14b61` also recovered its expired Executor lease.

The retry stopped at an unrelated Notion authentication request. It was cancelled without an external write.

Task `task:18d026bb41c93fb158a` closed the active-run recovery gate.

Server PID `3692881` stopped during Executor run `run:18d026bf54a3918d5f2`.

The replacement server started as PID `3695703`.

The expired run stopped with `lease_expired`. Run `run:18d026db6d3da46a231` retried as attempt 1.

The retry wrote `ACTIVE-RESTART-OK`. Reviewer run `run:18d026dd9f687d6d27f` approved it.

The Task reached terminal success. The live runner stayed attached throughout the restart and lease recovery.

## Authentication follow-up evidence

The Notion connection reports `authenticated` and `healthy`.

Task `task:18d026f309e1d56c4de` reopened as generation 2 after reconnection.

Executor run `run:18d02a8b18e6bb6f8e3` completed one Notion search and one exact-result fetch.

Reviewer run `run:18d02a9e8308c7dcb1f` approved the cited result.

The same Task reached terminal success at revision 9 with no pending intervention.

## Portability and lifecycle gate

| Gate | Current evidence | State |
| --- | --- | --- |
| Main path | All 14 Task families have one passing live case. | Pass |
| Delegated surface | Every required delegated case finished with reviewer approval. | Pass |
| Stable time anchor | Time-sensitive cases used fixed cutoffs. | Pass |
| Bounded source scans | Every package has a finite coverage plan. | Pass |
| Message portability | One message provider setup passed. | Waived for Milestone 1; portability remains unproven |
| Event portability | One event provider setup passed. | Waived for Milestone 1; portability remains unproven |
| Record portability | Native Project and Task paths passed. | Pass for Noema-owned records |
| Web portability | Several web providers support current research and route evidence. | Partial: repeat exact cases through another fixed route |
| Restart recovery | An active Executor lease expired, retried, and reached reviewer-approved terminal success. | Pass |
| Expired authentication | The same Task completed Notion search and fetch after reconnection and explicit reopen. | Pass |

Milestone 2 must not depend on a provider name or this setup’s connection identifiers.

Milestone 1 accepts one provider setup by explicit product decision.

This decision does not prove provider portability.
