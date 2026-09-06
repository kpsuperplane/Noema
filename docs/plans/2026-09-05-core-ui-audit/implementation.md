# Core UI gallery implementation

The human approved implementation on September 6, 2026.
The changes use the current Go branch and preserve the existing shell, fonts, and floating task controls.

## Applied changes

- Tasks retain one compact title, close control, and Workspace/Transcript header through execution.
- Title and instruction fields share capture controls and save on blur.
- Saving reads the current editor text, including pending rich-text changes.
- Failed saves preserve the draft. Closing and route changes wait for saving.
- Stale drafts retain revision and document checks, acknowledgement, retry, and discard controls.
- Starting first saves pending changes, then uses the current task revision.
- Project and Task agent choices appear in the Workspace body. Working folder and repeated detail metadata are omitted.
- Inbox and recurring details use the gallery's centered 720-pixel column, larger title, and green timing panel.
- Timing appears with Schedule or Reschedule. Execution controls remain in the floating bar.
- Markdown source remains available while editing. Resting instructions have no extra source control.
- New task keeps a full-height editor. Add to Inbox and Run now remain visible together.
- Capture Advanced controls appear above the action row.
- Recurring tasks use shared instruction editing and existing task cards for run history.
- Recurrence policies remain visible before history. Lifecycle actions save pending edits first.
- Matching schedules reopen with their preset. Unmatched cron expressions remain unchanged.
- Scheduling shows searchable time zones and server-calculated dates.
- Task dialogs identify their task with the existing card. Cancellation has no reason field.
- Dialog footers keep equal-width actions reachable, with the primary action on the right.
- Provider details use a compact header and connection facts. Technical records remain available.
- Device notifications appear before server delivery setup. Installation guidance wraps on phones.
- Model choices remain visible. Agent and task models appear before external agent configuration.
- Task-model enablement is removed from web, iOS, GraphQL, and storage.
- Migration 36 preserves configured models while removing the obsolete enable column.

## Validation

No live task or settings changes were submitted during browser inspection.
The private development socket served the actual app at desktop and phone widths.
Populated Inbox tasks were available. Recurring tasks used local browser responses because the server had no active recurrences.
Local responses also covered saving and execution transitions without changing stored tasks.
Live screenshots remain outside the repository.
The final Inbox comparison uses the gallery sample text in local browser responses.

| Check | Result and scope |
| --- | --- |
| Frontend lint | `bun run lint` passed from `apps/web` with the final layout corrections. |
| Frontend build | `bun run build` passed from `apps/web` with the same changes. |
| Go unit tests | `CGO_ENABLED=0 go test ./cmd/... ./internal/...` passed with the complete model-removal patch. |
| Go analysis | `CGO_ENABLED=0 go vet ./cmd/... ./internal/...` passed with the same Go changes. |
| Migration | Existing-version upgrade and fresh creation passed. Configured model choices survive removal of the enable column. |
| Native operations | Authored iOS settings operations validate against the changed schema. |
| Browser layout | Tasks, capture, model settings, providers, notifications, scheduling, and cancellation inspected at 1440 and 390 pixels. |
| Task editing | Current text saves before Start. One click queues once. New external text updates the mounted editor. |
| Draft recovery | A failed save keeps the detail open and preserves the exact draft. Retry succeeds with local responses. |
| Source mode | Switching modes immediately after typing preserves the final character. |
| Recurrence | Policies precede history. History uses task cards. Weekday schedules reopen without a cron field. |
| Dialogs | Mobile footers remain visible. Cancellation has no text field. |

The Go results remain valid because later corrections affect only frontend code and documentation.
The normal asset watcher exceeded its default Node memory limit during compilation.
The production build uses an 8 GB Node heap for this environment.
Existing large-bundle advisories remain.

The Linux host cannot run the native iOS build or Apollo's macOS generation workflow.
Native generated changes remove only the retired field. A macOS build remains unverified.
Apple push configuration reads are unavailable through the inspection socket. The current device section and access error were inspected.

## Scope and size

The authored production patch adds 737 lines and removes 820 lines.
This stays below the 2,500-line change budget. No UI tests were added.
Go tests add 55 lines and remove 13 lines, including one 47-line migration regression check.

Go production adds 15 lines and removes 18 lines.
Generated Go adds one line and removes 57 lines.
Including tests and generated files, Go adds 71 lines and removes 88 lines.
Both authored production and inclusive Go size decrease.

The review used one read-only adversarial pass and one correction pass.
Corrections cover external editor updates, closing with a draft, stale-choice retry, capture flushing, and starting after editing.

The implementation does not add recurrence project or agent fields unsupported by the existing command interface.
The standalone Start mock remains a copy reference. The existing direct Start action remains direct.

Unrelated work remains outside this commit: `scripts/run-live-noema-case.ts`, `.go-cache/`, `.go-mod-cache/`, and `.runtime-rerun.log`.
