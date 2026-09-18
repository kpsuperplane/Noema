# Notion page fetch with a 128k context limit

Base revision: `125cf285`.

The user authorized sending `fetch a random page` in Noema dev as their account.
The request succeeded without changing the saved 128,000-token Codex limit.

## Changes

Native tool definitions remain the authority for descriptions and schemas.
The Chat tool visibility section keeps callable names, service ownership, and
instructions. It no longer repeats the complete descriptions.

Text counting no longer adds a JSON escape layer or empty message fields.
Structured tool history and reasoning remain included in the estimate.
The existing provider tokenizer remains available when supported.

Compaction compares current sections with retained history before restoring
missing or changed sections. The same comparison controls saved updates.
Sections in the retained checkpoint also participate in later comparisons.
The runtime preserves saved message order and does not rewrite source history.

Rust revision `4d29f6ba` counted ordinary messages as text. Its context ledger
compared current sections with retained updates after the summary boundary.
Those behaviors support the counting and restoration corrections here.
Removing duplicate descriptions is an additional reduction in fixed input size.
All callable definitions remain intact.

## Unit validation

Two new tests cover text counting and a large native catalog at 128k.
The catalog test uses 45 large tool definitions and enough history to require
compaction. It verifies that the resulting request fits and keeps tool metadata.

The existing saved-context test now changes a large tool section in the active
turn. It requires one copy after compaction and exact agreement with saved replay.
The previous implementation appended the full section again.

An existing compaction test initially stopped triggering compaction because the
corrected count was smaller. Its history grew from 1,000 to 1,100 characters
per message. The test then passed and still checks one summary with tools disabled.

Focused runtime command:
`CGO_ENABLED=0 scripts/with-build-limits go test ./internal/runtime -run 'Test(ContextCounts|LargeNative|ModelContext|ChatContextPrefix|ToolVisibility|PrepareModelContext)'`.
Result: passed for this patch after the test input correction.

Existing wire tests now require native descriptions and schemas for Codex,
OpenAI, OpenRouter, and local models. Each focused check passed. The existing
Codex disabled-transport rejection also passed. Tool visibility still advertises
no tools when native transport is disabled.

Provider commands:
`CGO_ENABLED=0 scripts/with-build-limits go test ./internal/provider ./internal/localmodel -run 'Test(CodexGeneration|OpenAIGeneratorPreservesResponsesWireAndProtectedSetup|OpenRouterRequestLowersToolsAndReplayHistory|RustProviders_DisabledTransportRejectsAnAdvertisedToolCatalog|LocalGenerationPreservesToolsReplayAndTokenization)'`.
The Codex generator name did not match that expression, so its check ran separately:
`CGO_ENABLED=0 scripts/with-build-limits go test ./internal/provider -run '^TestCodexGeneratorPreservesResponsesWireAndOutput$'`.

`CGO_ENABLED=0 scripts/with-build-limits go test ./cmd/... ./internal/...` passed.
`CGO_ENABLED=0 scripts/with-build-limits go vet ./cmd/... ./internal/...` passed
for this patch. Development asset watchers were restarted to release memory
during compilation. No build limits changed.

## Live validation

The message was sent once through the private development GraphQL socket.
A WebSocket subscription was established before sending the mutation.
No browser authentication or public access settings changed.

Conversation: `conversation:d01fcda27d72e935192a3d3ac6332700`.
Turn: `turn:6f12b2a9a5c71198f2a7cd061428cc1b`.

The saved model profile still reported 128,000 tokens before the request.
The turn saved one compaction checkpoint. It then called
`mcp.mcp:notion.notion-list-recent-pages` and `mcp.mcp:notion.notion-fetch`.
Both saved results have `success: true`. The assistant named the selected page
and supplied a summary. The turn completed after 30.131 seconds with no error notice.

The updated tool visibility message contains 13,499 characters, compared with
about 93,000 before this patch. Complete descriptions remain in native definitions.
The private page contents remain in their governed source and Chat history.
The subscription log and sending script are in `/var/tmp/noema-context-fix-20260918/`.

## Review and size

An inline review checked retained-history order, removal updates, checkpoint
reconstruction, request/replay agreement, and each provider's native tool path.
No further code correction was required.

Production: 77 added and 31 removed lines; net 46.
Tests: 79 added and 15 removed lines; net 64, with two new tests.
Generated GraphQL: unchanged. Inclusive Go growth: 110 lines.
The budgets were 120 production lines and 120 test lines.
The existing inclusive migration size limit remains exceeded; this patch does
not resolve that separate repository-wide constraint.

Tracked Go totals: 95,118 production, 69,384 tests, and 79,566 generated lines.
The inclusive total is 244,068 lines.
