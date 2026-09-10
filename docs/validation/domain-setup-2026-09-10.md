# First-run domain setup

Base revision: `bfffa69b`.

Fresh servers now show address confirmation when no public origin or passkey exists.
Confirmation saves the current browser origin and hostname before normal services start.
The browser then shows passkey creation without a restart.
The address uses bold monospace text and the primary theme color.

## Checks

- `CGO_ENABLED=0 go test ./internal/auth ./cmd/noema -run 'TestDomainSetup|TestFreshHost|TestRustHost_' -timeout 90s` passed.
  These units cover preserved settings, rejected requests, repeated writes, saved configuration, and passkey registration with the selected domain.
- `CGO_ENABLED=0 go test ./cmd/... ./internal/...` passed.
- `CGO_ENABLED=0 go vet ./cmd/... ./internal/...` passed.
- After the final server cleanup change, `CGO_ENABLED=0 go test ./cmd/...` and `CGO_ENABLED=0 go vet ./cmd/...` passed.
  The unchanged internal-package results remain valid.
- From `apps/web`, `bun run check:generated` passed.
- `bun run lint` stopped at the existing missing `enabled` field in `TaskModelPoolsSettings.tsx:100`.
  This task leaves that file unchanged.
- `bunx eslint src/auth/AuthGate.tsx --max-warnings=0` passed.
- After the final typography change, `bun run build` passed.
- `git diff --check` passed.

The live public setup page was inspected at 1440×900 and 390×900.
Both views showed the address in JetBrains Mono, weight 700, with the primary theme color.
Neither view had horizontal overflow. The confirmation button remained visible.
Inspection did not submit the live address or create a passkey.
The server unit covers confirmation through passkey registration start.
A complete browser passkey ceremony remains untested for this change.

The private inspection socket was unavailable while initial domain setup was pending.
The existing public setup page supplied the read-only visual review.
The development session stopped during validation and was restored with `./attach`.

## Size

Three focused tests were added. Existing startup tests now specify a localhost origin.

| Go class | Base | Final | Change |
| --- | ---: | ---: | ---: |
| Authored production | 94,389 | 94,526 | +137 |
| Tests | 67,977 | 68,172 | +195 |
| Generated | 78,987 | 78,987 | 0 |
| Inclusive | 241,353 | 241,685 | +332 |

The authored-production ratio is 54.33% of the fixed Rust baseline.
The inclusive ratio is 100.78%. It already exceeded the 80% limit before this task, at 100.64%.
This task does not repair that existing repository-wide budget failure.
The change remains within its task estimate: 300 production lines and 160 test lines, with the permitted 50% stop margin.
