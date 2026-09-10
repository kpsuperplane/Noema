# Task typing indicator

Task transcripts show the shared typing dots while a run is running or leased.
Progress messages do not hide the dots. Other run states hide them.
Chat retains its existing typing rules.

## Validation

Checks cover the three transcript files committed with this note.
The production change adds eight lines and removes four. No tests were added.

- `bun run check:generated` passed. Generated inputs did not change afterward.
- `bun run lint` stopped at an existing settings type error.
  `TaskModelPoolsSettings.tsx:100` omits the required `enabled` field.
- `bunx eslint src --max-warnings=0` passed with the final changes.
- `bun run build` passed with the final changes.
- `git diff --check` passed.

Browser inspection used the private socket and the existing Task detail route:
`/tasks/task%3A3ec84e45143addf4338228cb904ef415?terminal=all`.
The completed live Task had no typing dots.
Browser-only response changes supplied running, leased, approval-waiting, queued,
completed, failed, and cancelled run states. No live data changed.
At 1600×1000 and 390×1000, only running and leased states showed dots.
Existing progress messages remained visible. Neither width had horizontal overflow.
Reduced motion disabled dot animation at both widths.
Screenshots remain outside the repository in `/var/tmp/task-typing-1600.png`
and `/var/tmp/task-typing-390.png`.

A local review found no correction needed. The shared renderer keeps one typing
row. Its default preserves Chat behavior. Existing scrolling owns the new row.
