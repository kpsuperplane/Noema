# Archived Go branch review

Reviewed main revision: `e4a9cef6`.
Archive: `.git/archives/go-migration-branches-2026-09-06.bundle`.

No missing change was found in the ten branches with unmatched patches.
None needs another merge. Keep the archive for history recovery.

## Method

Compared each unmatched commit with its integrated main commit.
Compared patch contents, rather than relying on commit titles.
Inspected the combined Kernel changes because main split them across commits.
Checked current file parsing, Task notice replay, and browser recovery paths.
This was a source review. No application files changed and no tests ran.
The result does not claim a new runtime acceptance check.

All branch names below have the prefix `codex/go-`.

| Branch | Main evidence | Result |
| --- | --- | --- |
| `browser-runtime` | `534afe79`, `bdc8566f` | Browser changes are included. Differences preserve runtime tracing and the typed unavailable-URL error. |
| `context-admission` | `c2e3e541`, `f73baffa` | Admission and checkpoint changes are included. Main also preserves newer Task replay and skipped-item support. |
| `desktop-sidecar` | `95a68ae3`; exact copied patch for `28bb82af` | Sidecar launch and arm64 packaging are included. The launch patch differs only in surrounding startup code. |
| `documents` | `e53fe916` | Spreadsheet changes are included. Differences reflect the added resolver argument and concurrent dependencies. |
| `file-parse` | `a524329c` | Parsing is included through shared Chat tool handling. Main retains Memory tools alongside parsing and Task inspection. |
| `kernel-browser` | `2224caac`, `bdc8566f`, `44502a0f` | Routing, upload receipts, recovery revisions, and uncertain outcomes are included. See the detailed check below. |
| `memory` | `86a68eb1` | Native Memory reads are included. Integration preserves the existing Artifact service in resolver construction. |
| `task-model-tools` | `7f865ccc`; exact copied patch for `60f8df84` | Task notices are included. Replay retains completed Task references after the context checkpoint. |
| `task-placement` | `e14389f2` | Scheduling is included as migration 12 because Artifact storage already used migration 11. Integration also removes a duplicate helper. |
| `web-push` | `11ff1753` | Web Push changes are included. The patch difference is dependency context. |

## Kernel comparison

Archived commit `073d07e7` has no same-title commit on main.
Its changes are included across `2224caac` and `44502a0f`.
Invalid or lost browser replies preserve an uncertain outcome after a possible action.
Kernel invalid-result coverage also remains in the current test source.

All files changed by archived final commit `84d0a990` exactly match `44502a0f`:

- `internal/webtool/browser.go`
- `internal/webtool/kernel_browser.go`
- `internal/webtool/kernel_browser_test.go`

Current browser code still preserves recovery revisions, upload receipts, and uncertain outcomes.
Current Chat code still exposes `file.parse` through the shared tool list and dispatcher.
Current Task notice replay still includes completed `task_reference` items.

## Checks

Patch comparisons covered all 15 unmatched commits across these ten branches.
Other commits on these branches were already ancestors or exact copied patches.
The working tree was clean before the review.
Documentation whitespace and local file references were checked before commit.
