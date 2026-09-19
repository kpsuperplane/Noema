# Delegation Task chip restoration

## Behavior

New Chat delegations deliver Task creation references through the existing notification path.
The frontend uses these references to show Task chips.
Delegation tool markers remain hidden, as in Rust revision `4d29f6ba`.
Previously skipped creation events are not replayed.

Go commit `4c344f3f` skipped notifications for Tasks delegated from the same Chat.
This change removes that skip.
No database, frontend, or live product data changes are included.

## Checks

Base: `0a070189`. Checks cover the two changed runtime files.
The existing delegation test now waits for both successful Task references.
It verifies that each reference belongs to its source turn.
The test failed before the correction and passed after it.

Passed focused checks:

```sh
CGO_ENABLED=0 scripts/with-build-limits go test ./internal/runtime -run '^TestRustRuntime_runtime_executes_every_homogeneous_delegation_and_uses_provider_handoff_narration$'
CGO_ENABLED=0 scripts/with-build-limits go test ./internal/store -run 'TestPrimaryNotification'
CGO_ENABLED=0 scripts/with-build-limits go test ./internal/runtime -run 'TestPrimaryNotification'
```

Broad checks also passed:

```sh
CGO_ENABLED=0 scripts/with-build-limits go test ./cmd/... ./internal/...
CGO_ENABLED=0 scripts/with-build-limits go vet ./cmd/... ./internal/...
```

Git whitespace and the changed documentation links passed inspection.

These successful checks remain valid for the committed patch.
Later documentation edits do not change their inputs.
No browser check was run for this server-only correction.

## Size and review

Production: -3 lines. Tests: +16 net lines. Generated GraphQL: unchanged.
Inclusive patch: +13 net Go lines. No new test functions.

Tracked Go totals are 96,042 production lines, 69,450 test lines, and 78,646 generated GraphQL lines.
The inclusive total is 244,138 lines.
Against the fixed migration baseline, the ratios are 55.20% production and 101.80% inclusive.
The inclusive size limit was already exceeded before this patch; that exception remains unresolved.

A local review checked delivery timing, source-turn ownership, and existing duplicate prevention.
It found no further correction needed.
