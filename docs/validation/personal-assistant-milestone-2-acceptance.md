# Personal Assistant Milestone 2 Acceptance

Date: 2026-08-29

Milestone 2 remains in progress.
This record covers bounded acceptance cases for Tasks 2, 3, 7, 9, 12, 13, 16, 18, 21, 24, 26, 28, 33, 38, and 39.

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

## Task 3 acceptance

Task 3 passes its current provider-neutral live case.

### Demonstrated failure

The first Executor lacked native `task.capture` authority.
It used a connected Notion Tasks database despite an explicit external-write prohibition.

The failed coordinator was `task:18d03dafd162c7475c22`.
It was cancelled after the two unintended writes appeared.

The connector exposed no page deletion operation.
Both pages remain marked `REMOVE — Noema acceptance test` with status `Done`.

### Implemented slice

- A Task Executor can call one scoped native `task.capture` operation.
- The runtime derives the child workspace and Project from the active Task.
- The schema excludes Project, executor, and working-directory routing fields.
- Valid parent conversation authority continues into the child source record.
- A background external mutation receives at least model review.
- Foreground behavior and external reads remain unchanged.

This slice adds no persisted field, provider rule, or Task behavior manifest.

### First successful run

| Evidence | Value |
| --- | --- |
| Project | `project:18d03da84ce02dcc5b4f` |
| Coordinator | `task:18d03ec99db2bf00bd`, generation 1 |
| Executor | `run:18d03ed489f5fec01fc` |
| Reviewer | `run:18d03ef64494c5cc5f2` |
| Inbox Task | `task:18d03eeae0857f9b496` |
| Scheduled Task | `task:18d03eeafdd268c149d` |
| Scheduled instant | `2026-09-13T16:00:00Z`, `UTC` |

Both child Tasks inherited the exact Project.
The Inbox Task preserved `action:rollback-001`, its owner, and its due instant.
The scheduled Task preserved `date:readiness-review-001` and `decision:beta-date-001`.

The Executor used two `task.capture` calls and no external tool.
The Reviewer approved the exact saved Tasks and receipts.

### Deduplication run

The coordinator reopened as generation 2.
Executor `run:18d03efd39e948dc6ac` found and inspected both saved Tasks.
Reviewer `run:18d03f0f0b00436e8c0` approved the no-write result.

No `task.capture` or external mutation occurred in the repeat run.
The Project still contains one Task for each routed source record.

### Validation

- `cargo validate test -p noema-runtime --lib daemon::task_tool::` passed 12 tests.
- `cargo validate test -p noema-runtime --lib daemon::runtime::model_tools::tests::` passed 13 tests.
- `cargo validate check -p noema-runtime` passed.
- The patch adds 163 production lines and 200 test lines.
- Three new Rust tests cover scope, scheduling, and external mutation review.

### Reuse assessment

Native Task routing and existing action review are enough for this bounded case.
No behavior declaration system is necessary yet.

Connected source intake and approved external delivery remain separate acceptance concerns.

## Task 9 acceptance

Task 9 passes its current provider-neutral live case.

### Fixture

| Item | Value |
| --- | --- |
| Project | `project:18d03f5f88fdcacd1170` |
| Repeat | `recurrence:18d03f69c923193d128a` |
| Firm obligation | `obligation:domain-renewal-001` |
| Primary source | `contract:domain-001` |
| Duplicate source | `message:renewal-reminder-002` |
| Soft target | `target:archive-review-001` |
| Exact receipt | `receipt:domain-renewal-001-complete` |

The firm obligation was due at `2026-08-29T09:00:00Z`.
The source defined warning, urgent, and overdue thresholds.

The duplicate source used the same exact Obligation ID.
The soft target had no firm date or owner and could not trigger escalation.

### Baseline and recovery

Task `task:18d03f69c82ffb5d1289` used the `10:21:00Z` occurrence cutoff.
It merged the duplicate and classified the firm obligation as overdue.

It drafted one escalation for `contact:finops-001` and did not send it.
It did not classify the soft target as overdue.

The server stopped during Executor attempt 0.
Run `run:18d03f734affa80813a2` became `INTERRUPTED`.

Run `run:18d03f8f61721e448` resumed as attempt 1.
Reviewer `run:18d03fa3119b8609256` approved one baseline result.

### Quiet unchanged occurrence

Task `task:18d03fa841df18af2c8` used the `10:24:00Z` cutoff.
It found the earlier terminal Task and kept the same overdue level.

It made no second escalation draft.
Reviewer `run:18d03fbc542b1d40523` approved with `notify_human: false`.

No completion notification exists for this Task.

### Temporal receipt boundary

Task `task:18d03fcc180caeab6ed` used the `10:25:00Z` cutoff.
The current source showed a completion receipt timestamped `10:25:30Z`.

The Task did not backdate closure across the 30-second boundary.
It kept the cutoff state overdue and produced no repeated escalation.

Reviewer `run:18d03fed981892acad1` approved with `notify_human: false`.
No completion notification exists for this Task.

### Exact-evidence closure

Task `task:18d03ff3e458fff7b8e` used the `10:28:00Z` cutoff.
The exact successful receipt now preceded the cutoff.

It changed the state from overdue to closed.
It preserved the receipt ID, Obligation ID, outcome, timestamp, and verifying source.

Reviewer `run:18d0400c86555a4ce79` approved the closure.
The Repeat ended at revision 7 with no next run.

### Delivery and action evidence

The baseline and closure produced one completion notification each.
The two unchanged cutoffs produced none.

No governed external action exists for any occurrence.
No message, calendar item, source record, or external state changed.

### Reuse assessment

Project files, Repeat history, bounded Task reads, and Reviewer delivery control were sufficient.
No obligation table, checkpoint schema, provider adapter, or threshold state was necessary.

Test a longer multi-obligation series before adding dedicated obligation storage.
Connected portal intake and approved external sends remain separate operation concerns.

## Task 16 acceptance

Task 16 passes its current provider-neutral live case.

### Fixture

| Item | Value |
| --- | --- |
| Project | `project:18d0402f5cf0b95e1241` |
| Repeat | `recurrence:18d04039104ae20c134f` |
| Meeting | `meeting:vendor-review-001` |
| Decision version 1 | `decision:vendor-pilot-001` |
| Decision version 2 | `decision:vendor-pilot-002` |
| Follow-ups | `action:security-addendum-001`, `action:pilot-seats-001`, `action:green-terms-001` |

The source also contained `discussion:future-discount-001`.
It was explicitly discussion and supplied no decision or action authority.

### Baseline

Task `task:18d040391027f73e134e` used the `10:35:00Z` cutoff.
It recorded Decision 1 as current from `transcript:vendor-review-001`.

It closed `action:pilot-seats-001` on `receipt:pilot-seats-001-complete`.
It kept `action:security-addendum-001` open and overdue.

It drafted exactly one reminder for `contact:rowan-001` and did not send it.
It did not convert the discussion record into work.

Executor `run:18d0404154f47d51144c` produced the result.
Reviewer `run:18d04058d1498a9e170b` approved it.

### Replacement and complete follow-up set

Source revision 2 preserved Decision 1 as superseded.
It recorded Decision 2 as current and explicitly linked both directions.

Task `task:18d0406a3306c12d18df` used the `10:37:56Z` cutoff.
It compared the exact earlier terminal Task.

It closed the overdue old-decision follow-up on `receipt:security-addendum-001-complete`.
It preserved the already closed pilot-seat follow-up without reporting it as new.

It recorded and closed `action:green-terms-001` for Decision 2.
The receipt completed before both its due instant and the occurrence cutoff.

It did not repeat the earlier reminder.
Reviewer `run:18d04087833003311c3f` approved the met stop condition.

### Quiet unchanged occurrence

Task `task:18d0408d103ea07c1cc0` used the `10:40:00Z` cutoff.
It found no source, decision, follow-up, receipt, or reminder change.

It preserved both decision versions and all exact source links.
It produced no reminder, Task capture, or external action.

Reviewer `run:18d040acfa9a01342070` approved with `notify_human: false`.
No completion notification exists for this Task.

The Repeat ended after revision 7 with no next run.

### Delivery and action evidence

The baseline and replacement occurrences produced one completion notification each.
The unchanged occurrence produced none.

No governed external action exists for any occurrence.
No message, Task, calendar item, or source record changed.

### Reuse assessment

Project files, Repeat history, prior Task reads, exact receipts, and Reviewer delivery control were sufficient.
No decision entity, action entity, thread table, or provider rule was necessary.

Test a longer multi-decision series before adding dedicated storage.
Connected transcript intake, approved reminders, and send receipts remain separate operation concerns.

## Task 18 acceptance

Task 18 passes its current provider-neutral live case.

### Fixture

| Item | Value |
| --- | --- |
| Project | `project:18d041d5c3d4dc6a41fe` |
| Repeat | `recurrence:18d041e3759f06d2437e` |
| Person | `person:maya-chen-001` |
| Similar excluded person | `person:maya-cheng-002` |
| Promise | `promise:send-talk-link-001` |
| Resource | `resource:talk-link-001` |

The source defined a 21-day cadence and weekday email window.
It prohibited automatic sending and limited repeated unanswered outreach.

It also supplied one explicit routine-contact boundary.
Noema could not infer private dates or turn the boundary into a due date.

### Overdue user-owned promise

Task `task:18d041e375862a64437d` used the `11:05:00Z` cutoff.
It read source revision 1 and preserved the exact Maya Chen identity.

It did not merge the similar Maya Cheng record.
It reconstructed one reciprocal inbound and outbound exchange.

The user had promised to send one talk link.
The promise was overdue and had no qualifying completion receipt.

The cutoff was Saturday at `07:05` in `America/New_York`.
The source allowed email only during weekday working hours.

The result kept the promise open and sent nothing.
It identified Monday at `09:00` Eastern as the next eligible time.

It did not draft text because the source omitted the presentation outcome.
It refused to invent that answer or ask Maya for work.

Executor `run:18d041eadd4ebaf74466` produced the result.
Reviewer `run:18d042080001e1b147b1` approved it.

### Exact completion and changed boundary

Source revision 2 added `receipt:user-send-link-001`.
The successful receipt named the exact Promise ID and Resource ID.

`interaction:maya-inbound-20260829` acknowledged the sent message.
It reset the unanswered-outreach count to zero.

Maya asked for no routine follow-up before September 4.
The source stated that this date was only a lower boundary.

Task `task:18d04229fd79194d4b56` used the `11:10:00Z` cutoff.
It closed the exact promise and produced no follow-up draft.

It calculated September 21 at `09:00` Eastern as the first eligible time.
It correctly treated that time as an eligibility floor, not scheduled outreach.

Reviewer `run:18d04258d13ac8b450a4` approved the changed result.
The exact completion produced one completion notification.

### Bounded history correction

The original Repeat instruction said to compare the prior terminal occurrence.
It did not explicitly direct a bounded Task lookup.

The changed occurrence therefore omitted the existing prior Task comparison.
Its exact source evidence still proved the completion result.

The Repeat template was corrected to require one Project-scoped Task list.
It also required inspection of the newest earlier Task from the same Repeat.

This was an instruction correction only.
No product code, schema, or new runtime state changed.

### Quiet unchanged occurrence

Task `task:18d0426fda6501dd5317` used the `11:15:00Z` cutoff.
It inspected exact prior Task `task:18d04229fd79194d4b56`.

It found no identity, interaction, promise, receipt, boundary, cadence, or draft change.
It preserved the relationship plan and produced no external action.

Reviewer `run:18d0428ec14b074a56ab` approved with `notify_human: false`.
No completion notification exists for this Task.

The Repeat ended after revision 3.
It has no next run or pending coalesced occurrence.

### Delivery and action evidence

The overdue baseline and exact completion produced one completion notification each.
The unchanged occurrence produced none.

No action request exists for any occurrence.
No message, contact, calendar item, or external source changed.

### Reuse assessment

The Project document, Repeat history, exact receipts, and Reviewer delivery control were sufficient.
No contact table, relationship entity, cadence state, or consent schema was necessary.

Test connected interaction intake and approved sending separately.
Test a longer multi-contact series before adding dedicated relationship storage.

## Task 24 acceptance

Task 24 passes its current provider-neutral live case.

### Fixture

| Item | Value |
| --- | --- |
| Project | `project:18d040e6622f2fb426c2` |
| Repeat | `recurrence:18d040f391d8b8f4283a` |
| Artifact | `artifact:release-brief-001` |
| Reviewers | `reviewer:legal-001`, `reviewer:security-001`, `reviewer:finance-001` |
| Initial version | Version 2, digest `sha256:brief-v2-9a61` |
| Approved version | Version 3, digest `sha256:brief-v3-2f04` |

The policy required two roster approvals and mandatory legal approval.
It also prohibited approval while a current-version change request remained unresolved.

Every response had to bind the exact artifact version and digest.
Superseded responses remained evidence but could not count.

### Blocked baseline

Task `task:18d040f391c4e60d2839` used the `10:48:00Z` cutoff.
It read source revision 1 and evaluated artifact version 2.

It counted `response:security-v2` as one eligible approval.
It preserved `response:finance-v1` but rejected it as stale.

It kept `response:legal-v2` as a blocking change request.
No exact resolution existed for `comment:liability-cap-001`.

The result recorded one of two approvals and no mandatory legal approval.
It correctly reported the artifact as not approved.

It drafted routes to the owner, finance reviewer, and legal reviewer.
It did not send them.

Executor `run:18d040fe43223235297d` produced the result.
Reviewer `run:18d0411f3e467dc22d4b` approved it.

### Resolved conflict and exact quorum

Source revision 2 preserved the old artifact and every old response.
It marked version 3 as current and linked it to version 2.

`resolution:liability-cap-001` resolved the exact legal comment.
It bound the resolution to version 3 and its digest.

Task `task:18d0412e8225c53c2ee9` used the `10:52:00Z` cutoff.
It compared exact prior Task `task:18d040f391c4e60d2839`.

It counted `response:legal-v3`, `response:security-v3`, and `response:finance-v3`.
Each response bound the exact current version and digest.

It reported three of two required approvals and mandatory legal approval.
It found no unresolved current-version change request.

It approved the artifact and produced no draft.
Reviewer `run:18d0414bed9898443258` approved the changed result.

### Quiet unchanged occurrences

Task `task:18d041587151554a33b2` used the `10:55:00Z` cutoff.
It found no source, response, resolution, conflict, route, or decision change.

Reviewer `run:18d04176e88d40fc3740` approved with `notify_human: false`.
No completion notification exists for this Task.

Task `task:18d041825d18ae273892` had already materialized at the `10:58:00Z` cutoff.
It also preserved the ledger and approved with `notify_human: false`.

Reviewer `run:18d041a995eb40093d19` approved the second quiet result.
No completion notification exists for this Task.

The Repeat ended after revision 2.
It has no next run or pending coalesced occurrence.

### Delivery and action evidence

The blocked and newly approved occurrences produced one completion notification each.
Both unchanged occurrences produced none.

No governed external action exists for any occurrence.
No message, artifact, reviewer record, or external source changed.

### Reuse assessment

The Project document, Repeat history, prior Task reads, and Reviewer delivery control were sufficient.
No reviewer table, approval router, quorum schema, or conflict state was necessary.

Test connected reviewer delivery and response intake separately.
Require execution receipts before claiming connected routing is complete.

## Task 26 acceptance

Task 26 passes its current provider-neutral live case.

### Fixture

| Item | Value |
| --- | --- |
| Project | `project:18d042b5efc644d75b05` |
| Repeat | `recurrence:18d042c325742f195c75` |
| Handoff | `handoff:atlas-001` |
| Departing owner | `person:avery-001` |
| Successor | `person:jordan-001` |
| Accountable manager | `person:casey-001` |

The lifecycle required grants, verification, ownership acceptance, responsibility transfer, revocation, and package acknowledgment.
Every step required exact evidence.

Departing access could not be revoked before successor access was verified.
The package audience contained only Jordan and Casey.

The source identified one unauthorized private HR record.
It also identified protected secret material without supplying any secret value.

### Invalid early attempt

Task `task:18d042c3255108cf5c74` was started before its `11:25:00Z` scheduled cutoff.
That future cutoff made the occurrence invalid for acceptance.

The Task was cancelled before it produced a result.
It is excluded from all behavioral claims below.

The Repeat then moved to normal scheduled cutoffs.
No product change was necessary.

### Incomplete baseline

Task `task:18d042df99cbe2d05fb2` used the `11:23:00Z` cutoff.
It read source revision 1 and counted zero exact receipts.

It produced one package with current state, responsibilities, access, decisions, contacts, risks, and unfinished work.
It preserved Casey as the accountable manager.

It excluded the unauthorized HR contents.
It included no credential value and prohibited credential sharing.

It drafted prerequisite access and training requests only.
It drafted no departing-owner revocation.

Executor `run:18d042e79fd99cba60a7` produced the result.
Reviewer `run:18d043191025e9c2664b` approved it.

### Reviewer rejection of unsupported completion

Source revision 2 supplied 20 successful receipts.
Task `task:18d04325727e91b467b9` used the `11:28:00Z` cutoff.

The first Executor incorrectly claimed exact completion.
It also copied resource IDs that were absent from current source revision 2.

Reviewer `run:18d04362cf86341b6ebf` rejected that result.
It identified a missing distinct grant receipt for `access:atlas-docs-001`.

Correction Executor `run:18d043665643bab76f2e` removed the unsupported resource IDs.
It kept completion false and drafted one narrow evidence request.

It preserved the recorded later revocations as source facts.
It did not treat those facts as proof of the missing predecessor grant.

Reviewer `run:18d04394e7c1e6267468` approved the corrected package.
The newly identified blocking gap produced one completion notification.

### Late receipt and exact completion

Source revision 3 added `receipt:docs-owner-grant-audit-001`.
The receipt was recorded at `11:36:20Z` and became available only in revision 3.

It confirmed that the docs owner grant completed before verification and revocation.
The source also supplied the four explicit access-to-resource mappings.

Task `task:18d043a32d9b89fb75ea` used the `11:37:00Z` cutoff.
It compared exact prior Task `task:18d04325727e91b467b9`.

It counted 21 exact successful receipts.
It verified every required receipt class and timestamp order.

It preserved the manager accountability, observability backup, and successor ownership.
It produced no new lifecycle draft or external action.

Reviewer `run:18d043cee70882407af5` approved exact completion.
The newly completed handoff produced one completion notification.

### Quiet unchanged occurrence

Task `task:18d043db0e68bfe67c29` used the `11:41:00Z` cutoff.
It inspected exact prior Task `task:18d043a32d9b89fb75ea`.

It found no source, receipt, access, responsibility, risk, exclusion, or completion change.
It preserved the complete package and produced no draft.

Reviewer `run:18d04407828ec5b98145` approved with `notify_human: false`.
No completion notification exists for this Task.

The Repeat ended after revision 3.
It has no next run or pending coalesced occurrence.

### Delivery and action evidence

The missing-evidence correction and exact completion produced one completion notification each.
The incomplete baseline and unchanged occurrence produced none.

No action request exists for any occurrence.
No access, message, credential, record, or external source changed.

### Reuse assessment

The Project document, Repeat history, Reviewer correction, exact receipts, and delivery control were sufficient.
No access table, lifecycle entity, ownership schema, or export subsystem was necessary.

Test connected grants, revocations, acknowledgments, and their execution receipts separately.
Test a larger multi-person handoff before adding dedicated lifecycle storage.

## Task 28 acceptance

Task 28 passes its current provider-neutral live case.

### Fixture

| Item | Value |
| --- | --- |
| Project | `project:18d0443165b9f57f85e5` |
| Repeat | `recurrence:18d0443ff4929039877b` |
| Baseline source | Project revision 2; source revision 1 |
| Changed source | Project revision 3; source revision 2; digest `30dad356097c8f9c52589285fcda8f36cfd37dc363b9149080050a8fcd16cb58` |
| Canonical identity | Exact employer ID plus requisition ID |

The source defined target level, work, location, compensation, industry exclusions, and three verified career claims.
It prohibited automatic applications, messages, contact, and interview scheduling.

Six baseline listings included one exact duplicate pair and one similar separate requisition.
One unsupported applied claim lacked an application ID and submission receipt.
One other application had an exact successful receipt.

### Baseline and receipt authority

Task `task:18d0443ff4825a99877a` used the `11:48:00Z` cutoff.
It preserved six listings and produced five canonical jobs.

It merged only the two ACME-77 listings.
It kept ACME-88 separate and excluded one manager role and one expired role.

It marked only Beacon applied from `receipt:beacon-submit-001`.
It preserved `event:acme-aggregate-applied-claim-001` without promoting its unsupported claim.

It ranked the eligible jobs and drafted one unsent Beacon follow-up.
Reviewer `run:18d044633053e6098b93` approved the baseline.

### First unchanged occurrence

Task `task:18d04466c37e72a08bdf` used the `11:51:00Z` cutoff.
It inspected the exact prior successful Task.

It found no pipeline, receipt, outcome, contact, draft, or ranking change.
Reviewer `run:18d0449942a31a4391a7` approved with `notify_human: false`.

### Excluded stale occurrences

Tasks `task:18d0449ea5900b1f924f` and `task:18d044ac9c2d4f7393fa` started before the source update.
Both Tasks were cancelled before acceptance results.

They are excluded from all behavioral claims below.
No product change was necessary.

### Outcomes and evidence-based adaptation

Source revision 2 added one Echo listing and retained all earlier listing evidence.
It added an exact ACME-77 application receipt and interview invitation.

It added an exact Beacon rejection after the earlier under-review event.
The rejection feedback identified insufficient customer-facing architecture examples.

It also added a receipt for the user's ACME warm-introduction request.
The source permitted one unsent ACME interview-preparation draft.

Task `task:18d044d683e72ea298bd` used the `11:59:00Z` cutoff.
It preserved seven listings and produced six canonical jobs.

It kept both the earlier Beacon state and the later exact rejection.
It closed the Beacon follow-up and prevented another ACME introduction draft.

It marked ACME-77 applied only from `receipt:acme-77-submit-001`.
It kept ACME-88 separate and did not infer an Echo application.

It adapted the interview plan with exact Beacon feedback.
It used only verified `profile:customer-migration-001` evidence.

It produced one unsent ACME interview-preparation draft.
It did not invent customer results, metrics, management experience, or scheduling authority.

Reviewer `run:18d0451a95ad82b1a075` approved the changed result.
The result used `notify_human: true` for the new role, exact outcomes, and supported strategy change.

### Final quiet occurrence

Task `task:18d0452a56320b36a222` used the `12:05:00Z` cutoff.
It inspected exact prior Task `task:18d044d683e72ea298bd`.

It found no source, receipt, outcome, contact, ranking, draft, or strategy change.
It retained the single interview-preparation draft without creating another.

Reviewer `run:18d045605bdb668ba854` approved with `notify_human: false`.
The Repeat ended at revision 2 with no next run.

### Delivery and action evidence

The baseline produced one creation notification and one completion notification.
The changed occurrence produced one completion notification.

The two approved quiet occurrences produced no completion notification.
The cancelled stale occurrences produced no completion notification.

No governed action exists for any occurrence.
No application, message, contact, interview, listing, or external source changed through Noema.

### Reuse assessment

The Project document, Repeat history, exact receipts, prior Task reads, and Reviewer delivery control were sufficient.
No job table, application schema, contact system, outcome entity, or strategy engine was necessary.

Test connected job intake, message intake, and reviewed external actions separately.
Add dedicated pipeline storage only after a larger case fails bounded Project files or Task history.

## Remaining Milestone 2 gates

- Test long promise, monitoring, decision, and learning series near the bounded history limit.
- Verify shared restart recovery for the remaining long-running cases.
- Verify external action receipts for cases that can send or update data.
