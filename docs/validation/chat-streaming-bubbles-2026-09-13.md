# Chat bubbles during streaming

Base revision: `e2815beb`.

Chat previously split messages only after completion. This caused a large layout change.
Chat now uses the shared Markdown splitter during streaming and completion.
Incomplete separator lines stay hidden outside code fences. Ordinary text appears before its line ends.
Saved bubble IDs remain stable. The first bubble retains complete provider text for model history.
Existing blank-line rules, citation ranges, and code fence rules remain in place.

## Validation

These checks passed against the code changes from the base revision:

- `CGO_ENABLED=0 go test ./internal/runtime ./internal/provider -run 'TestChatOutput|TestRustProviders_(BubbleBoundaries|SegmentRanges)'`
- `CGO_ENABLED=0 go test ./cmd/... ./internal/...`
- `CGO_ENABLED=0 go vet ./cmd/... ./internal/...`
- `git diff --check`

One new regression test checks each saved streaming update before completion.
It covers partial separators, CRLF, fenced separators, ordinary dash text, stable IDs, and complete model history.
Existing tests cover citation ranges and completed-only responses.
The read-only code review found no further defects.
No live browser response was inspected. No client code changed.
Successful code checks were reused after documentation changes.

## Size

Production Go adds 21 lines and removes 3 lines: net +18.
Tests add 69 lines. Generated GraphQL is unchanged. Inclusive Go grows by 87 lines.
Tracked totals are 94,861 production, 68,898 test, and 79,566 generated lines.
The inclusive total is 243,325 lines.
Production is 54.52% of the fixed Rust baseline. The inclusive ratio is 101.46%.
The inclusive total already exceeded the historical 80% limit before this task.
No database migration or public schema change was needed.
