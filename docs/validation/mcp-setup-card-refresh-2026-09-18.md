# MCP setup card refresh

Chat saved the MCP setup result but omitted `HumanInterventionsChangedEvent`.
The web client already refreshes pending cards when this event arrives.
A page refresh loaded the saved setup and made the card appear.

The existing notification condition now includes `mcp.connect_service`.
The event follows the saved tool result. API setup keeps its existing behavior.
The change adds no polling and changes no frontend code or connection policy.

## Validation

Base revision: `1380e9c6`.

The new runtime test creates a local MCP service that requires authentication.
It runs the normal Chat tool path and verifies a saved pending setup.
It then requires the intervention event after the result event.

`CGO_ENABLED=0 scripts/with-build-limits go test ./internal/runtime -run '^TestMCPSetupPublishesInterventionAfterSavedResult$'`
failed before the fix and passed after it.

A controlled browser check used the running app through the read-only inspection
helper at 1440×1000 and 390×1000. The initial pending list was empty.
A synthetic intervention event caused a new query and displayed the setup card
without navigation or reload. The sign-in action was visible at both widths.
The card fitted both viewports. Screenshots were inspected locally.
The check changed no live data and did not start another Notion connection.
Its temporary script and screenshots remain in
`/var/tmp/noema-mcp-card-20260918/`.

The broad command was:
`CGO_ENABLED=0 scripts/with-build-limits go test ./cmd/... ./internal/...`.
All packages passed except `internal/runtime`. Two existing tests failed:
`TestChatA2UIActionPausesValidatesAndResumesExactPrivateInput` and
`TestChatExecutesDurableTaskInspectLoopWithBoundedReplay`.
Both passed together in an isolated run with the same code:
`CGO_ENABLED=0 scripts/with-build-limits go test ./internal/runtime -run '^(TestChatA2UIActionPausesValidatesAndResumesExactPrivateInput|TestChatExecutesDurableTaskInspectLoopWithBoundedReplay)$'`.
The difference is intermittent. Its cause is not established by this patch.
`CGO_ENABLED=0 scripts/with-build-limits go test ./internal/runtime` then passed
for the complete package.
`CGO_ENABLED=0 scripts/with-build-limits go vet ./cmd/... ./internal/...` passed.
No application code changed after these checks.
The other broad results remain valid for the unchanged code.
Development asset watchers required restarts to release memory during compilation.

## Review and size

A local review confirmed that notification follows successful persistence.
The existing web subscription and pending-card query consume this event.
No correction beyond the missing tool identity was needed.

Production: one line replaced, zero net growth. Tests: 64 added lines and one
new test. Generated GraphQL: unchanged. Inclusive growth: 64 lines.
The budget was 10 production lines and 80 test lines.

Tracked Go totals are 95,068 production, 69,299 tests, and 79,566 generated lines.
The inclusive total is 243,933 lines. Production remains below the migration
limit. The inclusive limit already failed at the base revision, as recorded in
[the preceding connection fix](notion-mcp-description-2026-09-18.md).
