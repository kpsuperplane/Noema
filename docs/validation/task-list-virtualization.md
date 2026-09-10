# Web task list virtualization

Date: 2026-09-10.

Active, scheduled, and historical task groups now render a bounded set of rows.
The existing task scroll area owns scrolling. Each group measures its row heights.
Resize observers update group positions when earlier content changes height.
Group boundaries and focused neighbors remain mounted for keyboard navigation.
Each list item reports its position and the loaded group size to assistive technology.
Approval forms remain mounted to preserve unfinished input.

## Browser review

The live `/tasks` route was inspected at 1440 × 900 and 390 × 900.
The private socket helper blocked mutations. No live data was changed.

- Scrolling changed the rendered row range without horizontal overflow.
- Four history page loads increased the loaded history from 10 to 50 tasks.
- The final checked position rendered 11 of those 50 history rows at both widths.
- Tab reached 25 consecutive history rows. Shift+Tab returned through the same rows.
- A history link opened task details without horizontal overflow at either width.
- Desktop and phone screenshots preserved task grouping, row spacing, and the reachable pagination control.

Empty and failed-query states were reviewed in source only.
Browser navigation emitted `Transition was skipped`; navigation and the checks completed.
Screenshots and the read-only inspection script remain outside the repository under `/var/tmp`.

## Validation

Checks cover the three changed task components on the implementation worktree.
No automated UI tests were added, as required by the repository instructions.

- `bun run check:generated`: passed; reused because no schema or operation changed afterward.
- `bunx eslint src --max-warnings=0`: passed on the final component changes.
- `bun run build`: passed after the final component changes; Vite reported large bundle warnings.
- `bun run lint`: blocked by an existing type error in `TaskModelPoolsSettings.tsx:100`.
  Its model-pool update omits the required `enabled` field. No task-list type errors were reported.
- The read-only browser inspection passed after the final row-spacing and resize-observer changes.
- `git diff --check`: passed.

The static review covered group offsets, stable row keys, keyboard continuity, and list semantics.
The correction pass added observation of neighboring groups and removed the final row's extra gap.

Production changes add 112 lines and remove 17 lines, for a net increase of 95 lines.
This is within the 180-line budget. Test code is unchanged.
