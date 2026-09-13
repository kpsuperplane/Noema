# Saved Chat context updates

Chat now restores the saved context comparison behavior from Rust `4d29f6ba`.
The Rust sources are `runtime/model_context.rs` and `runtime/model_context_ledger.rs` under `crates/noema-runtime/src/daemon/`.

The Go port rebuilt full sections before conversation history on every request.
A new clock value therefore changed the request before unchanged history.
Chat now saves full initial sections and appends only later changes.
The sections are agent identity, runtime environment, projects, and tool visibility.
A removed section gets an explicit removal update.
Compaction and context resets restore full current sections.
Initial requests, tool continuations, and finalization use the saved updates.
The existing conversation schema requires no migration.

## Behavior checks

- Later Chat requests retain the earlier request prefix exactly.
- A time zone change appends one environment replacement after the new message.
- Saved updates survive fresh database reads. Unchanged sections add no records.
- Removed project context stays intact in history and gets a removal message.
- Completed turns reject new context writes.
- Compacted requests match subsequent saved replay and retain all current sections.
- Context resets start with full sections.
- Name changes append identity replacements and preserve the existing tool catalog.
- Codex and OpenAI continuations omit unchanged project context from incremental input.

The first focused run exposed an unsafe history-length assumption during authentication continuation.
The corrected boundary retains all available history when incremental input is longer.
Two existing tests assumed the previous full-context request layout.
Their updated checks retain input delivery, saved history, and provider continuation coverage.
A synthetic continuation now uses an active turn, because updates cannot modify a completed turn.
The rejection test formerly rejected its second request, which could be a compaction request.
It now rejects the tool continuation and allows summary requests.
Its bounded input isolates provider rejection from the local context limit.
An explicit name change exercises finalization after a context update.

## Validation

Tested source: `a0c3fdebb944688f42537230cad41e6ff7011351` plus this patch.

- `CGO_ENABLED=0 scripts/with-build-limits go test ./internal/runtime -run 'TestModelContext|TestChatContextPrefix'`: passed.
- `CGO_ENABLED=0 scripts/with-build-limits go test ./internal/runtime -run 'TestChatSerializesDetachedTurnsAndPublishesOrderedEvents|TestChatRoutesCodexAssignmentThroughToolContinuation'`: passed.
- `CGO_ENABLED=0 scripts/with-build-limits go test ./internal/runtime ./internal/store`: Store passed; Runtime found the two corrected assertions.
  The Store result remains reusable because its source did not change afterward.
- `CGO_ENABLED=0 scripts/with-build-limits go test ./cmd/... ./internal/...`: passed.
- `CGO_ENABLED=0 scripts/with-build-limits go vet ./cmd/... ./internal/...`: passed.
  The final check includes the revised test code.
- Git whitespace, changed document links, and current-context length: passed.

The integrating agent reviewed the update, replay, compaction, and finalization paths.
No unresolved correctness finding remains from that review.
The development authentication status read returned `authenticated`.
No live model request was sent. The resulting live cache rate remains unmeasured.

## Size

Production: +211/-32 lines, net +179.
Tests: +273/-21 lines, net +252, including three new tests.
Generated GraphQL: unchanged. Inclusive patch: net +431 tracked Go lines.
The estimates were 400 production lines and 230 test lines.
Both patch budgets remain within their allowed limits.

The repository already exceeded the inclusive migration limit before this patch.
Its inclusive count rises from 243,325 to 243,756 lines; the limit is 191,860.
Production remains below its limit: 95,960 lines versus 139,192.
The inclusive migration limit remains exceeded.
