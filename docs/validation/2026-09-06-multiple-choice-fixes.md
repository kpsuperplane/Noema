# Multiple-choice fixes

The fix uses `codex/go-server-migration`, as requested.
Successful question displays finish the turn without another model response.
The existing question receives usage information. No extra assistant item is saved.
Web keeps option labels within each question and invalidates the old saved cache.
Web and iOS omit successful question tool markers. Failed markers remain visible.

## Validation

Checks cover the choice patch over `c9962787`. Unrelated files changed during this task.
Successful results below remain valid for the unchanged choice patch.

- Focused Go choice runtime and store checks passed.
- The updated store check passed for question completion and saved usage.
- `CGO_ENABLED=0 go test ./cmd/... ./internal/...` completed with unrelated failures.
  Runtime, store, GraphQL, provider, and the other reported packages passed.
  Adapter OAuth audit checks failed because their OAuth service was unavailable.
  `TestLocalSocketIsPrivateAndRemoved` failed because the sandbox denied Unix socket creation.
- `CGO_ENABLED=0 go vet ./cmd/... ./internal/...` passed.
- From `apps/web`, `bun run check:generated`, `bun run lint`, and `bun run build` passed.
- A read-only review found no correctness defects.
- Controlled browser inspection used the built app at `/`, with 1440×900 and 390×900 viewports.
  Repeated option IDs kept separate labels. The selected option ID matched the current question.
  Successful tool markers were absent. Neither width had horizontal overflow.
  Requests used controlled responses and changed no live data.
- Live inspection was unavailable through both the private relay and the existing browser session.
  This host has no Swift toolchain. Native iOS rendering and compilation remain unverified.

The default Go cache was read-only. The first temporary cache exhausted `/tmp` space.
Successful Go commands used task-owned `GOCACHE` and `GOTMPDIR` directories on the workspace disk.
An attempted completion run named a nonexistent `internal/taskcard` package.
The original broad run had already completed; its results remain the validation authority.

## Size

Production changes add 35 lines and remove 6. Test changes add 30 lines and remove 7.
Two existing Go tests changed. No frontend tests were added.
Go production adds 23 lines and removes 3. Generated GraphQL has no changes.
The inclusive Go patch adds 43 net lines.
Tracked Go totals were 78,110 production, 26,741 tests, and 79,612 generated lines.
The inclusive total was 184,463 lines.
Production and inclusive migration ratios were 44.89% and 76.92%. Both remain below 80%.
