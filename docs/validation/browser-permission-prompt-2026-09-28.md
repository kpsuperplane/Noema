# Browser permission prompt

The web approval card now names the website before asking for permission.
It shows the saved URL and Noema's stated reason without opening Review details.
The button says `Open page once`. The server's permission rules are unchanged.
Other read-only requests use the saved action summary as their question.

## Scope and review

The change reuses the shared action card and its existing URL and metadata styles.
It adds 31 production lines and removes five. No tests or server code changed.
Malformed browser arguments retain the existing generic card.
The displayed reason is attributed to Noema, rather than presented as verified evidence.
Exact arguments and the assessment remain available under Review details.

## Live inspection

The inspected request remains pending:
`action:b946f66dc0ef2f66e9c336e28eb4aabf`.
It opens `https://trimet.org/fares/` for the Portland routes Task.
No approval or external action occurred during inspection.

The real app passed visual inspection at 1600 × 1000 and 390 × 844.
The website, full address, reason, Decline, and Open page once were visible.
Neither viewport had horizontal page overflow.
The README opening image was refreshed from the desktop view.

## Checks

Checks cover the permission-card patch based on `a85f0190`.

- `bun run check:generated`: passed; generated files remain unchanged.
- `bun run lint`: blocked by an existing TypeScript error in `TaskModelPoolsSettings.tsx:100`.
  The saved model-pool input lacks the required `enabled` field. This code is unchanged from the base revision.
- `../../scripts/with-build-limits bunx eslint src --max-warnings=0`: passed from `apps/web`.
- `bun run build`: incomplete. The first run ended with SIGTERM without a compiler error.
  A separate retry slowed under shared memory pressure and was stopped.
  The build group used 3.78 GB against its 3.76 GB memory threshold.
  The development asset build completed and served the inspected change.
- `git diff --check`: passed.

No frontend tests were added, as required by the repository instructions.
