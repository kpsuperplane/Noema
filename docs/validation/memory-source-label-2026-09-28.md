# Memory source label correction

## Failure and change

The Noema dev Memory action failed for conversation `conversation:a8973e4c69cf4f7a9147897861dacff4`.
The saved runtime error named `human:item:cec2a9e79be8d61671f0c48856466101`.
The database contains the corresponding completed human message under its exact `item:` ID.
The update had 47 pending records and had not saved any progress.

The first live retry exposed the same label with the `item:` prefix omitted.
Its rejected reference was `human:55aa59b6898f76c318829a99716c67f4`.

The existing citation normalizer now removes the model's `human:` label.
It then uses the existing exact allowed-source checks.
Unknown sources remain invalid. The allowed evidence set does not change.
The final article stores the exact source ID.

## Validation

The development watcher rebuilt and restarted the existing server after the final source change.
The live API then reported no error and no pending records.
Saved progress reached sequence 73 at `2026-09-28T05:15:07.899360498Z`.
The article contains the travel preferences and a resolved `HUMAN_MESSAGE` citation.
Its exact source is `item:55aa59b6898f76c318829a99716c67f4`.
A subsequent explicit update completed with no remaining source records.

This investigation invoked the normal Memory update action twice.
The first retry failed and preserved the empty article.
By the second invocation, the article and progress were already saved.
No Chat messages, external actions, direct database writes, or manual memory edits were made.

One regression test covers both observed labels, unchanged exact IDs, and rejected sources outside the allowed set.
A read-only code review confirmed that normalization preserves the allowed-source boundary.

The focused command passed on the final code:
`CGO_ENABLED=0 scripts/with-build-limits go test ./internal/runtime -run TestMemory`.

The first overlapping full checks were stopped because they competed with development watchers for the shared build memory limit.
They are not counted as passed.
Full checks resumed sequentially on the final source.

`CGO_ENABLED=0 scripts/with-build-limits go test ./cmd/... ./internal/...` completed with one unrelated failure.
`TestChatMultipleChoiceAllowsTextAndSendsSelectionAsUserMessage` expects the final provider message to be user text.
A runtime clock-context replacement followed that text during this run.
The assertion at `internal/runtime/multiple_choice_test.go:130` rejected the system message.
All other package results passed.
This repair does not change Chat context ordering or that test.
The complete server suite is therefore not reported as passed.

`CGO_ENABLED=0 scripts/with-build-limits go vet ./cmd/... ./internal/...` passed.
These checks cover the final six-line production change and its regression test over base `91f332e6`.
Documentation-only edits followed the checks. No application checks were repeated for those edits.
Changed documentation links and Git whitespace passed.

## Size

Base revision: `91f332e6`.
The patch adds six production lines and 20 test lines.
It adds one test and changes no generated GraphQL code.
The 26-line total is within the stated estimates.

Tracked Go totals are 95,941 production, 70,497 tests, and 79,605 generated lines.
The inclusive total is 246,043 lines.
Generated files are identified by their generated-code header.
Production is 55.14% of the fixed Rust baseline.
The inclusive ratio is 102.59%, above the historical 80% limit.
That limit was already exceeded at the base, which contained 246,017 lines.
Unrelated code removal remains outside this repair.

