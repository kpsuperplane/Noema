# Durable Execution for Long Tasks

## Summary

Task files provide durable working memory for Planner, Executor, Reviewer,
compaction, and continuation runs.

`TASK.md` is the current Task content authority. Agents can create support files
when useful.

The existing Task state machine, scheduling, gates, leases, retries, and events
remain. This change replaces only content storage and role handoffs.

No Task plan, submission, review, or context content is frozen or versioned.

## Task lifecycle

### Planning

- Initialize `TASK.md` from the authenticated Task request.
- Give the Planner Task file tools.
- Let the Planner choose the plan, organization, success conditions, and support files.
- Replace `task.submit_plan` with `task.finish_planning`.
- Store only current execution complexity as workflow metadata.
- Do not copy the plan into a database contract.

For direct execution, initialize `TASK.md` from the supplied execution intent.
Then skip the Planner.

### Execution

- Require each Executor run to read the current `TASK.md`.
- Let the Executor choose how to organize and perform the work.
- Let the Executor update `TASK.md` and create support files.
- Replace `task.submit_result` with `task.finish_execution`.
- Make `task.finish_execution` advance the existing workflow to review.
- Do not pass or store a result payload.

Add `task.continue_execution` for work that needs another Executor run.

- The tool carries no Task content.
- The current run ends successfully.
- The existing workflow queues another Executor run.
- The new run reads the current Task files.
- No human action is required.

Keep `task.report_blocked` for a specific missing decision, approval, or
unavailable requirement.

### Compaction

Keep the existing generic compaction logic unchanged.

When a Task context was compacted:

1. Read the current `TASK.md`.
2. Append it as fresh developer context.
3. Run normal context admission again.
4. Continue the model call.

Wrap the document as Task data. Its contents cannot override runtime policy or
role permissions.

Do not add Task-specific compaction summaries, checkpoints, hashes, or progress
extraction.

If `TASK.md` cannot be read or admitted, use the existing recovery path. Report
a clear error.

Also fix these demonstrated runtime failures:

- Bound large tool results before context admission.
- Preserve continuation after recoverable action-storage failures.

### Review and correction

- Require the Reviewer to read current `TASK.md`.
- Let the Reviewer read linked Task and project files.
- Replace `task.submit_review` with `task.finish_review`.
- Accept a current decision and concise feedback.
- Overwrite `REVIEW.md` with that feedback.
- Do not create `REVIEW.md` before the first review.
- Do not store review-content history.

On requested changes, queue another Executor run against the same files. The
Executor reads both `TASK.md` and `REVIEW.md`.

On approval, mark the Task complete without copying file content.

Reopened Tasks continue from their existing files. Completed views and notices
always read current `TASK.md`.

## Files and working directory

### Directory placement

Each Task has one working directory.

- Project Task: `<project>/<task-slug>/`
- Projectless Task: `${NOEMA_HOME}/tasks/<task-slug>/`
- Explicit working-directory configuration remains supported.

Generate the slug once from the Task title. Add an integer suffix when the name
exists. Do not rename the directory after title changes.

Start Task processes in this directory.

For project Tasks, use the project directory as the filesystem access boundary.
Relative parent paths can access shared project resources but cannot escape the
project.

For projectless Tasks, use the Task directory as the access boundary.

Resolve the current project location when each run starts. Do not capture
project paths in content records.

### File tools

Add:

- `task.files.list`
- `task.files.read`
- `task.files.write`
- `task.files.delete`

Paths are relative to the Task working directory. Project Tasks can use relative
parent paths that remain inside the project boundary.

Permissions:

- Planner and Executor can manage files inside the Task directory.
- Planner and Reviewer can read files inside the project boundary.
- Reviewer cannot change Task or project files directly.
- `task.finish_review` owns replacement of `REVIEW.md`.
- Executor project writes use existing governed execution tools.
- ACP and provider agents receive equivalent behavior.

Safety rules:

- Reject absolute paths and paths outside the access boundary.
- Reject symbolic-link escapes.
- Use atomic whole-file writes.
- Limit model-facing reads and writes to 64 KiB of UTF-8 text.
- Apply no Task-specific limits to files created through normal execution tools.
- Do not add line paging, file records, hashes, or revisions.

Prompts must not prescribe checklists, sections, batching, or another execution
structure.

## Persistence and interfaces

Keep existing operational authorities:

- Task workflow stage and complexity.
- Run status, leasing, and continuation.
- Current gates and review decision.
- Scheduling and recurrence.
- Authorization and project membership.
- Audit events and transcript activity.

Remove Task content authority from:

- Versioned execution contracts and criteria.
- Workspace and project context captures.
- Executor submissions and criterion evidence.
- Submission citations and artifact manifests.
- Review-content histories.
- Completed-result copies.

Store only current workflow metadata needed for transitions. Resolve provider,
model, agent, project, and filesystem settings when each run starts.

Update public interfaces:

- Replace the three content-bearing terminal tools with their `finish_*` forms.
- Add `task.continue_execution`.
- Add the four Task file tools.
- Remove contract, submission, and review-history projections.
- Expose current `TASK.md` content as the Task result.
- Expose current `REVIEW.md` content when it exists.
- Continue exposing operational role runs and transcript activity.
- Do not parse Markdown into a second progress model.

The existing store command service remains responsible for atomic stage
transitions, leases, gates, events, and idempotency.

## Existing data cutover

Before removing obsolete content records:

- Resolve or allocate one working directory for each existing Task.
- Convert its current request, latest plan, latest result, and latest feedback into Task files.
- Preserve explicit working-directory configuration.
- Preserve an existing `TASK.md`.
- When `TASK.md` exists, place converted content in `legacy-task.md`.
- Write `REVIEW.md` only when existing feedback is available.
- Complete file writes before applying the destructive database migration.
- Make interrupted conversion safe to retry.
- Remove obsolete content tables after successful conversion.
- Advance the schema version with a forward-only migration.

Do not keep compatibility readers or new writes for removed content records.

## Test plan

- Confirm project and projectless Tasks receive the correct working directory.
- Confirm slug collisions receive stable integer suffixes.
- Confirm title changes do not move existing directories.
- Confirm Task file paths cannot escape their authorized boundary.
- Confirm symbolic links cannot escape that boundary.
- Confirm Planner and Executor file-write permissions.
- Confirm Reviewer read access and automatic `REVIEW.md` replacement.
- Confirm project writes still use existing authorization policy.
- Confirm provider and ACP agents receive equivalent file behavior.
- Confirm compaction injects the latest `TASK.md`.
- Confirm the second context-admission check protects provider limits.
- Confirm `task.continue_execution` queues another run without human action.
- Confirm correction runs read current Task and review files.
- Confirm approval and completed views read current `TASK.md`.
- Confirm reopened Tasks reuse their files.
- Confirm large tool results cannot break context admission.
- Confirm recoverable action-storage failures preserve continuation.
- Confirm legacy conversion preserves existing files and retries safely.
- Confirm fresh and upgraded schemas contain no removed content authorities.

## Assumptions

- `TASK.md` is the only Task result and working-content authority.
- Agents decide whether and how to use support files.
- Task files remain mutable after completion.
- Task content history is not a product requirement.
- Operational run transcripts remain available for observability.
- Scheduling, recurrence, gates, authorization, and audit behavior remain unchanged.
- Existing artifact storage remains available outside Task content authority.
- No new file browser or checklist-specific interface is included.
