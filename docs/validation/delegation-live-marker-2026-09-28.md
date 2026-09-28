# Delegation live marker correction

## Cause and change

Live provider calls arrive as conversation activities.
Saved calls arrive as tool calls and tool results.
The display filter hid saved delegation calls but allowed the temporary activity.
The temporary activity remained pending because its saved completion was hidden.
The same filter now checks tool activities in both forms.
Task references remain visible. Ordinary tool calls remain visible.

The affected primary conversation is `conversation:d01fcda27d72e935192a3d3ac6332700`.
Its latest delegation turn is `turn:a18cebb4fcc61101469d2f5e34de4c01`.
Read-only inspection found completed call and result records at 2026-09-28 03:01:22 UTC.

## Checks

The regression failed before the production correction and passed after it.
It covers temporary calls, saved calls, saved results, and ordinary tool visibility.

```sh
CGO_ENABLED=0 scripts/with-build-limits go test ./internal/graphql -run '^TestToolVisibilityMatchesLiveAndSavedCalls$'
```

Broad server validation passed:

```sh
CGO_ENABLED=0 scripts/with-build-limits go test ./cmd/... ./internal/...
CGO_ENABLED=0 scripts/with-build-limits go vet ./cmd/... ./internal/...
```

All checks cover base `061751af` plus the two changed Go files.
Successful results were reused after documentation edits. No source inputs changed.
Git whitespace and the evidence link passed inspection.

The development server rebuilt and restarted at 03:04:26 UTC.
Its authenticated socket responded successfully.
A read-only GraphQL request returned one Task reference and no delegation activity in the latest transcript page.
No live messages or Tasks were created during this check.
A new live delegation and browser rendering were not verified.

A local review checked the shared history and live-event conversion path.
Hidden events return no transcript item. Task references use a separate conversion path.
The correction preserves the existing hidden-tool list and ordinary tool display.

## Size

Production: +1 net line. Tests: +23 lines. Generated GraphQL: unchanged.
Inclusive change: +24 Go lines. One regression test was added.
Tracked totals: 96,659 production, 70,121 test, and 78,685 generated GraphQL lines.
Inclusive total: 245,465 lines.
Migration ratios: 55.55% production and 102.35% inclusive.
The existing inclusive limit violation remains unresolved. This fix does not expand into unrelated reductions.
