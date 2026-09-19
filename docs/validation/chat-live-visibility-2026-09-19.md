# Shared Chat visibility

Hidden reasoning summaries produced empty live items.
The GraphQL contract requires each item to contain a value.
The frontend displayed the resulting `must not be null` error as a temporary notice.
Refresh removed the notices because saved history already skipped hidden items.

Saved history and live events now use `conversationItemModel`.
This existing item conversion now returns the complete shared item projection.
It applies visibility, metadata, and Task reference enrichment once for both paths.
The live subscription skips omitted items and continues with later events.
No frontend error messages are suppressed.

## Checks

The patch starts at `1a651444`.
The existing stream test now includes a hidden provider reasoning summary before a visible reply.
It verifies that the summary exists in storage, no empty item reaches the subscription, and the reply completes.
The presentation checks compare saved and live visibility and preserve visible content and metadata.

- `GOTMPDIR=/nt CGO_ENABLED=0 scripts/with-build-limits go test ./internal/graphql -run 'TestAssistantPresentationAcrossChatAndTasks|TestConversationTurnStreamsPersistsAndReplays|TestConversationActivities'`: passed.

- `GOTMPDIR=/nt CGO_ENABLED=0 scripts/with-build-limits go test ./cmd/... ./internal/...`: passed.
- `GOTMPDIR=/nt CGO_ENABLED=0 scripts/with-build-limits go vet ./cmd/... ./internal/...`: passed.
- `git diff --check`: passed.

The final checks cover the complete patch.
The development server was rebuilt through its normal launcher script.
Frontend source remains unchanged from the preceding MCP label commit.
No frontend tests or builds are needed for this backend correction.

## Size

Production changes by 24 added lines and 25 removed lines.
Tests change by 23 added lines and six removed lines.
Generated GraphQL does not change.
Production decreases by one line; the inclusive increase is 16 lines.
Totals are 96,189 production, 69,628 test, and 78,646 generated lines.
The inclusive total is 244,463 lines.
The migration ratios are 55.28 percent production and 101.93 percent inclusive.
The existing inclusive ratio remains above the 80-percent limit.
