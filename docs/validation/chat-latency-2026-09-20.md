# Chat summary and connection reuse

Base revision: `d82dfc5640efc710adacd956c761227e8b6ee607`.

## Changes

Context preparation skips optional summaries when fixed input would keep the
result above the summary threshold. Fixed input includes tool definitions and
context sections that must be restored. Input above the hard limit still needs
compaction. Tool definitions and source conversation records remain intact.

Chat retains one provider session for the active conversation. A later turn
sends new input and top-level instructions with the previous response ID.
The request also carries complete local history for existing transport recovery.

Conversation changes, model assignment changes, credential revision changes,
changed instructions, changed history, and compaction start a fresh session.
Only a successfully saved final response can retain a session. Pauses and failed
turns release their session. Chat shutdown closes the retained connection.

The existing provider recovery rules remain in force. A lost connection can
replay ordinary saved history. Provider-hosted web state still stops recovery
when the provider cannot restore it. This change does not invent missing state.

The current tool catalog is still sent with each request. Input-token totals
can still include cached context. Smaller transmitted conversation input does
not imply a smaller reported context window.

## Validation

The final focused check passed:

`CGO_ENABLED=0 scripts/with-build-limits go test ./internal/runtime ./internal/provider -run 'Test(ChatRoutesCodex|ChatSession|FixedCatalog|PrepareModelContext|OptionalCompaction|LargeNative|OpenAIResponsesWebSocket|CodexResponsesWebSocket|ResponsesWebSocket|RustProviders_MissingPrevious)'`

Runtime passed in 9.220 seconds. Provider passed in 0.026 seconds.
The extended Chat test covers a tool round, the next human turn, a failed
request, recovery from saved history, and shutdown. New tests cover repeated
optional summaries with a large catalog and session replacement after context
or authority changes. Existing tests cover hard overflow and WebSocket recovery.

An earlier focused run passed before the final shutdown and recovery assertions.
The final focused run replaces that result.

Broad commands:
- `CGO_ENABLED=0 scripts/with-build-limits go test ./cmd/... ./internal/...`
- `CGO_ENABLED=0 scripts/with-build-limits go vet ./cmd/... ./internal/...`

The first broad run exhausted `/tmp`. Runtime had one SQLite write failure.
Web and webtool could not link their test binaries. Other packages passed.
The first vet run also stopped because `/tmp` was full.

Retries use `TMPDIR=/var/tmp/noema-chat-latency-20260920/tmp` with the same build
limits and cache. The exact failed runtime test passed first. Full runtime passed in 56.786 seconds. Webtool passed in 5.716 seconds.
Broad vet passed with no diagnostics.

The web retry encountered the Unix socket path limit. It passed after changing
only `TMPDIR` to `/var/n20`. All broad packages now have passing results.
Successful results from the initial broad run cover the unchanged patch and
are reused. No production or test files changed during these retries.

Retry commands:
- `TMPDIR=/var/tmp/noema-chat-latency-20260920/tmp CGO_ENABLED=0 scripts/with-build-limits go test ./internal/runtime -run '^TestTaskExecutionRetriesThenOpensRecovery$'`
- `TMPDIR=/var/tmp/noema-chat-latency-20260920/tmp CGO_ENABLED=0 scripts/with-build-limits go test ./internal/runtime`
- `TMPDIR=/var/tmp/noema-chat-latency-20260920/tmp CGO_ENABLED=0 scripts/with-build-limits go test ./internal/web ./internal/webtool`
- `TMPDIR=/var/n20 CGO_ENABLED=0 scripts/with-build-limits go test ./internal/web`
- `TMPDIR=/var/tmp/noema-chat-latency-20260920/tmp CGO_ENABLED=0 scripts/with-build-limits go vet ./cmd/... ./internal/...`

These results cover the production and test patch on base `d82dfc56`.
The command logs are in `/var/tmp/noema-chat-latency-20260920/`.
No live user message was sent for this change. The authenticated inspection
socket still responds after the development asset watchers restarted. The watchers were restarted
to release retained build memory. The server remained available.

Inline review found a shutdown-order issue. The correction closes the session
before publishing shutdown completion. No additional review pass was required.

## Catalog proposals

1. Load full definitions on demand. Keep a small service and tool index visible.
   Let the model request exact definitions through a discovery tool. Keep each
   selected definition available across related turns. Recheck current grants
   and source schemas before execution. This avoids English keyword routing.
2. Use provider-native deferred loading where supported. OpenAI documents
   `tool_search`, `defer_loading`, and client-executed discovery. Verify support
   on the Codex endpoint before choosing this path for Codex accounts.
3. Reduce repeated descriptive text in model-facing tool definitions. Preserve
   complete source schemas for validation. This has a smaller scope but retains
   the cost of presenting every tool and risks removing useful guidance.

The first option offers the clearest provider-independent path. Discovery adds
an extra request when a needed tool has not yet been loaded. Casual replies
avoid that request and avoid the large catalog.

Sources:
- [OpenAI WebSocket mode](https://developers.openai.com/api/docs/guides/websocket-mode)
- [OpenAI tool search](https://developers.openai.com/api/docs/guides/tools-tool-search)

## Size

Production: 117 added, 4 removed; net 113 lines.
Tests: 108 added, 4 removed; net 104 lines. Two tests were added.
Generated GraphQL: unchanged. Inclusive growth: 217 lines.
Both task budgets were 220 lines.

Tracked Go totals are 96,429 production, 69,810 tests, and 78,646 generated lines.
The inclusive total is 244,885 lines. Production is 55.42% of the Rust baseline.
The inclusive ratio is 102.11%, above the existing 80% limit before this patch.
This bounded latency fix does not resolve that repository-wide size failure.
