# Action authority audit

Compared Go base `5e5e35d5` with Rust `4d29f6ba1f70a30b5959a0e460217feeeb5e8c04`.
That Rust revision was the last backend revision before its removal.
The audit covers evidence supplied for action review, its saved sources, and observed URL admission.
It does not certify parity for the entire Task scheduler or connector runtime.

## Findings and corrections

| Path | Go before this change | Correction |
| --- | --- | --- |
| Chat browser | No human request in review context | Shared action admission supplies the authenticated conversation excerpt. |
| Chat fetch, download, MCP, API | Each caller supplied its own excerpt | Shared admission now owns that evidence for every reviewed action. |
| Task browser, fetch, download, MCP, API | Only agent-written Task context | Review receives saved human authority and current-generation human replies. |
| Chat-created Task | Saved source references, but no authority snapshot | Creation saves the seven-message excerpt ending at the authenticated source item. |
| Manual Task | No separate human authority | The authenticated creation input becomes a bounded snapshot. |
| Human Task or recurrence edit | No saved authority update | Human title or document edits replace the snapshot, as in Rust. |
| Agent edit | No snapshot to preserve | Agent edits preserve the existing snapshot. |
| Task-created child | Only copied Chat references | The child inherits its parent's saved authority. |
| Recurrence and occurrence | No authority snapshot | Templates and future occurrences copy their applicable snapshot. |
| Observed download URL | Still requested review | Chat and Task downloads use the existing file and network checks without another review. |
| Reviewer prompt and decision matrix | Matches Rust | Preserved and checked. |
| Exact action, revision, run, and human decline checks | Existing enforcement | Preserved; server tests cover these boundaries. |

Rust references are `daemon/runtime/action_gateway.rs`, `work_command_tasks.rs`,
`authorization_context.rs`, `work_schedules.rs`, and `governed_actions.rs`.
The browser omission entered Go in `534afe79`, on September 5.

The reviewer still assesses authorization and risk separately.
Explicit or substantive authorization with low or medium risk permits execution.
Weak authorization permits execution only at low risk.
Other results, invalid responses, and unavailable reviewers require human approval.
Agent text cannot establish human authority.

## Saved evidence and bounds

Schema 40 adds bounded authority snapshots to Tasks and recurrence templates.
Action admission reads the snapshot after checking the exact live call and run.
The same transaction saves the resulting action request.
An action request's evidence remains fixed after creation.

Schema 41 records which run consumed each authenticated human reply.
Human replies must belong to the current Task generation and the local human.
The review context rejects more than 64 human replies rather than omitting older restrictions.
The overall context retains the existing 256 KiB limit.
Review includes replies consumed by the current run or its parent, plus replies not yet consumed.
Replies consumed by older runs remain in history but do not grant the current run permission.

The one-time upgrade recovers excerpts only from authenticated saved Chat source items.
Missing, invalid, or oversized sources retain `kind: none`.
Old manual authority cannot be inferred from mutable Task files.
A later authenticated title or document edit can establish that authority.
The upgrade does not approve, rewrite, or replay existing action requests.

## Validation

Six new unit tests cover saved source evidence, invalid sources, admission, recurrences, run ownership, and schema convergence.
Existing runtime tests now inspect actual reviewer input for Chat browsing and a Task continuation.
The observed-download test now checks that review is skipped while an existing file remains protected.
Older tests with invented source IDs now create authenticated human messages.
The manual Task API test checks that creation and editing save their human inputs.

Focused store, runtime, and API checks passed.
The exact reviewer prompt and the Rust authorization/risk matrix checks passed.
The upgrade test checks version 39 data and fresh-schema convergence.

Full server validation passed on the complete patch, including schema 41:

- `CGO_ENABLED=0 scripts/with-build-limits go test ./cmd/... ./internal/...`
- `CGO_ENABLED=0 scripts/with-build-limits go vet ./cmd/... ./internal/...`
- `git diff --check`

These results cover the current production and test changes. Later edits only record evidence.

## Live verification

The development instance restarted with the rebuilt server and applied schema 41.
SQLite integrity and foreign-key checks passed.
The Portland Task recovered its seven-message authenticated source excerpt.
Its existing approval request remains unchanged and awaits approval.

One short Chat message tested the browser path: “open https://trimet.org/fares/ in your browser”.
The resulting turn is `turn:da42f62499bbd4114fc4332c2fc7b2b1`.
Action `action:780895d8cecef1983581cc3208bdfdc8` contains the authenticated conversation excerpt.
The reviewer classified authorization as `explicit` and risk as `low`, then returned `auto_execute`.
Its explanation identifies the exact URL in the human request.
The browser action succeeded, and the Chat turn completed without an approval prompt.
This test added one Chat message and its resulting browser activity to the demo instance.
No existing approval was accepted.

## Size

The patch adds 196 net production Go lines and 356 net test lines.
It changes no generated GraphQL code. The inclusive net addition is 552 lines.
These totals are within the stated patch estimates.

The repository's inclusive migration size gate was already exceeded at the base revision.
The base contains 245,465 Go lines against the documented limit of 191,860.
The patch contains 246,017 lines. Production contains 96,855 lines against its limit of 139,192.
This audit does not remove unrelated code or tests to reduce the inherited total.
