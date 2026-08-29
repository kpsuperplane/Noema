# Personal Assistant Milestone 2 Acceptance

Date: 2026-08-29

Milestone 2 remains in progress.
This record covers bounded acceptance cases for Tasks 2, 7, 12, 13, 21, 33, 38, and 39.

## Result

Task 33 passes its current live acceptance case.

Noema used existing Repeat history and Task reads to keep a baseline.
It suppressed one unchanged completion update.
It reported one controlled material change after a server restart.

No dedicated monitor table, checkpoint schema, or provider-specific adapter was necessary.

## Fixture

| Item | Value |
| --- | --- |
| Project | `project:18d0385fb8eaefc418cfc` |
| Repeat | `recurrence:18d03866f2c3ddec18dc8` |
| Source | The Project `PROJECT.md` file |
| Material rule | A `Current facts` item or value changes |
| Ignored changes | Retrieval time, Task progress, and other wording |

The baseline used source revision 1.
It recorded a stable release channel, zero incidents, and the next checkpoint date.

## Demonstrated gap

Task `task:18d038a912d9d5611956e` produced an approved `unchanged` result.
Noema still delivered a `task_completed` update.

The result could remain quiet, but the completion path could not.

## Implemented slice

`task.finish_review` now includes `notify_human`.
The Reviewer sets it to false for a verified no-change result from a changes-only Task.

The approval transaction always completes the Task.
It queues the completion update only when `notify_human` is true.

Terminal replay checks the saved notification outcome.
A conflicting replay fails instead of changing the original delivery result.

## Live evidence

### Unchanged occurrence

| Evidence | Value |
| --- | --- |
| Task | `task:18d039336dd77a6f34` |
| Reviewer run | `run:18d0394a501962112e3` |
| Outcome | `unchanged` |
| Prior Task | `task:18d03902564b26c2d6` |
| Reviewer delivery decision | `notify_human: false` |
| Completion outbox rows | 0 |

The result did not repeat the baseline facts as news.

### Material-change occurrence and restart

The controlled source changed its incident count from 0 to 1.
No other `Current facts` value changed.

| Evidence | Value |
| --- | --- |
| Task | `task:18d0395c084a5c514e2` |
| Interrupted Executor | `run:18d039622a8947335a2`, attempt 0 |
| Recovered Executor | `run:18d0397e47d06b1d19f`, attempt 1 |
| Final Reviewer | `run:18d039a2decdf1955ec` |
| Outcome | `material_change` |
| Source revision | 2 |
| Reported delta | Incident count, `0` to `1` |
| Reviewer delivery decision | `notify_human: true` |
| Completion notification | `notification:18d039a409359008619` |

The server stopped during Executor attempt 0.
Lease recovery interrupted that attempt and started attempt 1.

The Task made no external action.
It produced one final result and one completion notification.

## Validation

- `cargo validate test -p noema-store reviewer_controls_completion_notification --lib` passed.
- `cargo validate test -p noema-runtime reviewer_finish --lib` passed.
- The focused patch adds 23 production lines and 133 test lines.
- One new Rust test covers both silent and normal completion delivery.

## Reuse assessment

Task 38 reused Repeat history, prior Task reads, and no-new-information delivery.
Its bounded digest fixture passed after one Reviewer rule clarification.

Task 12 can reuse changes-only delivery.
It still needs message events, a thread checkpoint, a silence deadline, and escalation evidence.

The live Task 33 case used several earlier Task reads to recover the old value.
Before adding checkpoint storage, test a longer series against the bounded Task list.

## Task 38 acceptance

Task 38 passes its current provider-neutral live case.

### Fixture

| Item | Value |
| --- | --- |
| Project | `project:18d039fba24bccba1cf` |
| Repeat | `recurrence:18d03a04e02ce0c62d1` |
| Source | The Project `PROJECT.md` file |
| Priority input | High and medium topic preferences |
| Coverage key | Item ID and Canonical story |
| Initial budget | Two items, at most 70 words each |
| Changed budget | One item, at most 30 words |

### Baseline

Task `task:18d03a04e01a2f652d0` started from its scheduled slot.
It selected two items in priority order and preserved both exact links.

Its result stored both item IDs and Canonical story values.
It also stated the trigger, source checkpoint, and human-controlled stop condition.

Reviewer run `run:18d03a225bc4cfe7645` approved the baseline.

### Duplicate and preference change

Source revision 2 added three entries.

- One entry repeated the earlier alpha Canonical story under a new item ID.
- One beta entry repeated an earlier item.
- One gamma entry was new and matched the current high priority.

The attention budget also dropped to one item and 30 words.

Task `task:18d03a2aecb4bc0d714` selected only gamma.
It excluded the repeated alpha and beta coverage.

It preserved the gamma link exactly and carried all four item IDs in Coverage state.
Reviewer run `run:18d03a477965dc11a6c` approved the result.

### Unchanged delivery

The first unchanged run exposed a rule wording gap.
Its Reviewer treated a no-new-items digest as a normal completion.

The Reviewer rule now covers any no-new-information result when the Task forbids repeated content.

| Evidence | Value |
| --- | --- |
| Retest Task | `task:18d03a7d8d965e9e34` |
| Reviewer run | `run:18d03a972a4c75b1336` |
| Outcome | No new qualifying items |
| Reviewer delivery decision | `notify_human: false` |
| Completion outbox rows | 0 |

Task 38 required no feed table, read-state table, or provider-specific adapter for this core continuity case.
Connected source adapters remain separate intake concerns.

## Task 39 acceptance

Task 39 passes its current provider-neutral live case.

### Fixture

| Item | Value |
| --- | --- |
| Project | `project:18d03ab8235488d96df` |
| Repeat | `recurrence:18d03ac3ce2da6c0823` |
| Source | The Project `PROJECT.md` file |
| Goal | Explain and safely implement Rust async cancellation by 2026-09-30 |
| Initial capacity | Three 30-minute blocks |
| Changed capacity | Two 30-minute blocks |
| Completed evidence | `practice:block-1` |
| Failed evidence | `assessment:cancel-1`, score 40 percent |

### Initial plan

Task `task:18d03ac3ce0f765f822` created the first plan from source revision 1.
The source contained no completed learning or assessment evidence.

The result assigned three 30-minute blocks.
It recorded the trigger, source checkpoint, success condition, and human stop condition.

Reviewer run `run:18d03ae745814f69c3b` approved the initial plan.

### Evidence-driven adaptation

Source revision 2 added one completed block and one failed assessment.
The assessment named two gaps.

- Exact branch-drop timing under select-style concurrency.
- Prevention of partial state writes across an await boundary.

The source also reduced capacity to two 30-minute blocks.

Task `task:18d03af2e0919796d5e` preserved the completed block.
It did not assign that topic again.

It assigned one block to each demonstrated gap.
The second block also supplies the missing practical repository exercise.

The result retained the 40-percent assessment score.
It did not infer reassessment, mastery, or exercise completion.

Executor run `run:18d03afd84b8dda6e9b` produced the adapted plan.
Reviewer run `run:18d03b1dac0dc83e1243` approved it.

### Result correction

The first approved result copied the current Task ID with one extra character.
Noema reopened the same Task with bounded correction feedback.

Executor run `run:18d03b27623ef9841343` removed the unnecessary identifier line.
It preserved the evidence, plan, checkpoint, and stop condition.

Reviewer run `run:18d03b46fe6019e216d8` approved the correction.
The Task reached revision 7 and generation 2.

### Reuse assessment

Project files can hold explicit learner evidence for the current bounded case.
Repeat history and Task reads can compare prior plans and adapt the next plan.

No learner-progress table, course-specific connector, or provider-specific adapter was necessary.
Connected course intake remains a separate source concern.

Test a longer learning series before adding dedicated progress state or direct recurrence history.

## Task 21 acceptance

Task 21 passes its current provider-neutral live case.

### Fixture

| Item | Value |
| --- | --- |
| Project | `project:18d03b67823626aa1a60` |
| Repeat | `recurrence:18d03b730f6d042a1ba0` |
| Source | The Project `PROJECT.md` file |
| Initial decision | `decision:delivery-001` |
| Replacement decision | `decision:delivery-002` |
| Initial source | `source:operations-review-001` |
| Replacement source | `source:partner-requirement-002` |

### Baseline

Source revision 1 contained one current decision.
It included the exact statement, date, reason, Source ID, and Source statement.

Task `task:18d03b730f5b0e8e1b9f` created the baseline decision history.
It preserved every source field and added no unsupported outcome.

Executor run `run:18d03b80480a46ba1d2a` produced the result.
Reviewer run `run:18d03b929150775a1f4d` approved it.

### Replacement

Source revision 2 retained the first record and added one replacement record.
The replacement explicitly linked to `decision:delivery-001`.

Task `task:18d03b9ba99bbaa3205c` produced one current entry and two history entries.
It marked `decision:delivery-001` as superseded without changing its original source fields.

It marked `decision:delivery-002` as current.
It linked `Replaces` and `Replaced by` in both directions.

Executor run `run:18d03ba2e799c2162139` produced the result.
Reviewer run `run:18d03bc89b1227052588` approved it.

The fixture Repeat ended after acceptance.
Its final revision is 2, and it has no next run.

### Scope limit

The baseline retained its future scheduled slot as its occurrence anchor.
The later manual run had an earlier anchor and correctly excluded that baseline from prior history.

This case proves preservation through the Project source authority.
It does not prove prior-Task recovery across a normally ordered series.

Test a normally ordered longer series before adding a dedicated decision entity or direct recurrence history.

## Task 2 acceptance

Task 2 passes its current provider-neutral live case.

### Fixture

| Item | Value |
| --- | --- |
| Project | `project:18d03bdf634cc47927f6` |
| Repeat | `recurrence:18d03be6385f129f28b4` |
| Source | The Project `PROJECT.md` file |
| Promise | `promise:launch-brief-001` |
| Promise source | `source:meeting-note-001` |
| Completion source | `source:sent-receipt-002` |
| Completion receipt | `receipt:outbound-002` |

### Baseline

Source revision 1 contained one open promise and no completion evidence.

Task `task:18d03be6384e8ee328b3` started from its scheduled 09:16 UTC slot.
It kept the promise open and preserved every source field.

Executor run `run:18d03bf6e10f917c2a9e` produced the result.
Reviewer run `run:18d03c078b9d44042c9c` approved it.

### Unrelated source change

Source revision 2 changed only an unrelated priority label.
The promise record remained unchanged, and completion evidence remained absent.

Task `task:18d03c0debba7d212d3f` inspected the earlier baseline.
It carried the promise forward as open and did not present it as new information.

Executor run `run:18d03c13db5227c42dfb` produced the result.
Reviewer run `run:18d03c20969aa6812f78` approved it with `notify_human: false`.

### Explicit completion

Source revision 3 added one receipt.
The receipt named the Promise ID and an exact completion time.

Task `task:18d03c29780b23c63081` inspected the prior open result.
It closed only `promise:launch-brief-001` and retained both source records.

It did not infer acknowledgement or satisfaction.

Executor run `run:18d03c2ff87ca3b1314d` produced the result.
Reviewer run `run:18d03c41585096903353` approved it.

The fixture Repeat ended after every evidenced promise closed.
Its final revision is 2, and it has no next run.

### Reuse assessment

Project files can hold source-linked promise state for this bounded case.
Repeat history and Task reads can maintain it through changes and completion.

No promise table, message connector, or provider-specific adapter was necessary.
Connected promise capture remains a separate source concern.

Test a longer series and several concurrent promises before adding dedicated promise state.

## Task 13 acceptance

Task 13 passes its current provider-neutral live case.

### Fixture

| Item | Value |
| --- | --- |
| Project | `project:18d03c590428f7b635d3` |
| Source | The Project `PROJECT.md` file |
| Task | `task:18d03c6038c920cb369e` |
| Meeting | `meeting:launch-review-001` |
| Duration | 30 minutes |
| Participants | Los Angeles, New York, and London |
| Candidates | Five |
| Explicit decline | `option:a` from `source:rowan-reply-001` |

### Candidate evaluation

The Task converted every candidate to each participant's local start and end time.

- It rejected `option:a` because Rowan declined it.
- It rejected `option:b` because Kevin would start before working hours.
- It rejected `option:c` because Mika would start and end after working hours.
- It rejected `option:e` because Kevin would start and end before working hours.

Only `option:d` remained valid.

The result recommended September 4 from 16:00 through 16:30 UTC.
It showed 09:00 PDT, 12:00 EDT, and 17:00 BST local starts.

Mika's local end matched the 17:30 working-hours boundary.
The Task treated that exact boundary as valid.

### Drafts and boundaries

The Task drafted one group confirmation request.
It also drafted exact invitation fields and attendee identities.

It did not infer acceptance from availability.
It did not create an event or send a message.

Executor run `run:18d03c67f8f83d36378d` produced the result.
Reviewer run `run:18d03c83debc4f1e3ab1` approved it.

### Reuse assessment

Project files can hold participant constraints, availability, and responses for this bounded case.
Existing Task reasoning can negotiate and draft without a provider-specific adapter.

Connected free-busy reads, invitations, and reply handling remain separate operation concerns.

## Task 7 acceptance

Task 7 passes its current provider-neutral live case.

### Fixture

| Item | Value |
| --- | --- |
| Project | `project:18d03c980cbbadac3cc4` |
| Repeat | `recurrence:18d03c9f76aee7203d96` |
| Source | The Project `PROJECT.md` file |
| Changed event | `event:vendor-demo-001` |
| Delay source | `source:delay-notice-001` |
| Dependent work | `work:summary-002` and `work:decision-003` |
| Fixed event | `event:fixed-review-004` |

### Baseline

Source revision 1 contained the original plan and no change record.

Task `task:18d03c9f769d802a3d95` started from its scheduled 09:29 UTC slot.
It preserved every interval, dependency, due time, and fixed item.

It drafted no notices because no notice rule was active.

Executor run `run:18d03cac9eea2a163f1c` produced the result.
Reviewer run `run:18d03cbd54624c8e4111` approved it.

### Delayed event and dependent work

Source revision 2 moved the vendor demo from 14:00 to 15:00 UTC.
The source retained one fixed review from 16:30 through 17:00 UTC.

Task `task:18d03cc8df987a824248` inspected the earlier baseline.
It made these minimum changes.

- It moved the demo to 15:00 through 16:00 UTC.
- It moved the summary to 16:00 through 16:30 UTC.
- It preserved the fixed review at 16:30 through 17:00 UTC.
- It moved the decision to 17:00 through 17:30 UTC.
- It moved the decision due time from 16:00 to 17:30 UTC.

The replan preserved preparation work and every item duration.
It kept dependency order, prevented overlap, and ended at the working-hours boundary.

### Required drafts

The Task drafted the required demo-change notice for `contact:rowan-001`.
It drafted the required decision-time notice for `contact:mika-001`.

Both drafts contained exact old and new times.
No message was sent, and no calendar event changed.

Executor run `run:18d03cd0690d67d94333` produced the result.
Reviewer run `run:18d03cef00d5534046af` approved it.

The fixture Repeat ended after the approved controlled replan.

### Reuse assessment

Project files can hold bounded dependencies, fixed work, and notice rules.
Repeat history and Task reads can replan from a controlled source change.

No dependency table, calendar adapter, or message adapter was necessary for this reasoning and drafting case.
Connected updates and send receipts remain separate operation concerns.

## Task 12 acceptance

Task 12 passes its current provider-neutral live case.

### Fixture

| Item | Value |
| --- | --- |
| Project | `project:18d03d04e8a5b7b048f1` |
| Repeat | `recurrence:18d03d0cd49da19949cd` |
| Source | The Project `PROJECT.md` file |
| Thread | `thread:renewal-001` |
| Latest message | `message:progress-003` at 09:05 UTC |
| Silence deadline | 2026-08-29 at 09:42 UTC |
| Required reply | Confirmation that the renewal packet is ready |

### Baseline before the deadline

Task `task:18d03d0cd48c112649cc` started from its scheduled 09:37 UTC slot.
It inspected all message events at or before that cutoff.

It recorded `pending_before_deadline`.
It did not report silence or draft a reminder.

Executor run `run:18d03d1bfc8dd7804b8d` produced the result.
Reviewer run `run:18d03d28a61ac4dc4d0f` approved it.

### Unchanged pre-deadline occurrence

Task `task:18d03d2cb33ea6b14d6b` used a 09:38:34 UTC cutoff.
It inspected the earlier baseline and found no new event.

It kept `pending_before_deadline` and did not present the state as new information.
Reviewer run `run:18d03d3d82c4830f4f6f` approved it with `notify_human: false`.

### Post-deadline occurrence

Task `task:18d03d6880e8d13a5437` used a 09:42:51 UTC cutoff.
The same source contained no qualifying reply.

It changed the outcome to `silence_deadline_passed`.
It identified `message:progress-003` as the latest recorded message.

It drafted exactly one reminder for `contact:kevin-001`.
It did not send the reminder or change the thread.

Executor run `run:18d03d6e6e8bca7754ef` produced the result.
Reviewer run `run:18d03d7de8b3fd9f56bb` approved it.

The fixture Repeat ended after acceptance.

### Reuse assessment

Project files can hold bounded message events and one agreed deadline.
Repeat occurrence anchors provide the deadline evaluation time.

No thread table, event stream, or message adapter was necessary for this bounded polling case.
Connected event intake and reminder sends remain separate operation concerns.

## Remaining Milestone 2 gates

- Test long promise, monitoring, decision, and learning series near the bounded history limit.
- Verify shared restart recovery for the remaining long-running cases.
- Verify external action receipts for cases that can send or update data.
