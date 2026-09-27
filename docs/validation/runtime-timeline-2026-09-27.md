# Runtime timeline validation

The runtime dialog now lists steps vertically in start-time order.
Each row shows the name, duration, category, start offset, and status.
Row height does not depend on duration. Selection retains the existing details.
Chat and Task runs share this component.

## Scope and review

Base: `1626ddaa`.
Production change: 56 added lines, 296 removed lines; net reduction of 240 lines.
No tests were added. Timing collection, profile queries, and duration calculations remain unchanged.
The review checked stable ordering, selection, keyboard controls, and retained detail fields.
The chart geometry, category colors, trace key, and track packing were removed.

## Checks

Checks ran against the component changes in this commit.
Successful checks remain valid after this evidence-only addition.

- `bun run check:generated`: passed from `apps/web`.
- `bun run lint`: stopped at an existing error in `TaskModelPoolsSettings.tsx:100`.
  Its update input lacks the required `enabled` field. The same code exists at the base revision.
  No runtime-dialog type error was reported.
- `../../scripts/with-build-limits bunx eslint src --max-warnings=0`: passed from `apps/web`.
  This completed the lint stage skipped after the type error.
- `bun run build`: passed from `apps/web`, including GraphiQL assets.
  Existing large-chunk warnings remain.
- `git diff --check`: passed.

## Browser review

The authenticated development socket returned `authenticated`.
Read-only browser inspection used the existing development instance at `/`.
The latest assistant message opened the Turn runtime dialog through Debug.
The saved profile contained a 3.1-second provider request and two 2-millisecond steps.
All three rows remained readable at 1440 × 1000 and 390 × 844.
Enter selected the first step and updated its details. Escape dismissed the desktop dialog.
The phone dialog had equal client and scroll widths of 390 pixels.
A temporary browser-only long label wrapped without overflow; its region stayed 358 pixels wide.
No product records changed. No messages were sent.
Screenshots remain outside the repository in `/var/tmp/runtime-*.png`.
Running, error, empty, and Task profiles received source review only.

## Two-column follow-up

Desktop now places the timeline beside the selected span details.
The existing 900-pixel dialog gives more width to the timeline.
At 760 pixels and below, details follow the timeline in one column.
A profile without a matching focus selects its first span.
The component patch adds 25 lines and removes four lines. No tests were added.

Checks cover the follow-up component changes and remain valid after this note.
`bun run check:generated`, `bun run build`, and focused component ESLint passed.
`bun run lint` still stops at the unchanged model-settings type error described above.
Browser inspection initially saw old assets. Inspection repeated after the rebuilt assets became available.
The desktop screenshot confirms two columns at 1440 × 1000.
The phone view retains one column at 390 × 844, without horizontal overflow.
Keyboard selection, dismissal, and a browser-only long label still pass.
No product records changed.
