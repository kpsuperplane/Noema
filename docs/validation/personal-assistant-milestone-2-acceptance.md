# Personal Assistant Milestone 2 Acceptance

Date: 2026-08-29

Milestone 2 remains in progress.
This record covers the first bounded slice: Task 33, topic monitoring for material changes.

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

## Remaining Milestone 2 gates

- Test a long series near the bounded history limit.
- Verify proactive trigger, source checkpoint, and stop-condition explanations.
- Run the remaining Milestone 2 acceptance cases.
- Confirm restart recovery for cases that can make external actions.
