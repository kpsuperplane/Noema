# Notion requests and the model context limit

Base revision: `d3ba3a9a`.

Three failed Chat turns logged `runtime.chat_context_failed` before the main
provider request. The error was `model context window exceeded: compacted
context does not fit the selected model`. Notion did not receive a page request.

The connected MCP catalog contains 45 tools. Descriptions total 66,883 characters.
Input schemas total 161,641 characters. The saved tool visibility section adds
about 93,000 characters. A local diagnostic combined those schemas, the saved
visibility message, and current built-in instructions and tools. The estimate
was 122,824 tokens before memory and other current context. The saved Codex
profile allows 128,000 tokens. The runtime reserves 8,192 output tokens and
128 safety tokens, leaving 119,680 input tokens.

The local Codex catalog reports `context_window: 272000` for `gpt-5.6-terra`.
The Noema catalog reader discarded this field and assigned 128,000 to every
model. The reader now preserves a positive unsigned `context_window`. It keeps
the existing default when that field is absent or invalid. It does not select
`max_context_window`, change the model, or remove tools.

## Rust comparison

At Rust revision `4d29f6ba`, `adapters/codex/responses.rs` also sets the context
limit to 128,000. `runtime/context_window.rs` uses the same three-character
estimate. Rust renders ordinary messages as text. Go serializes the complete
message records, which adds JSON escaping and empty fields. This difference
increases the Go estimate. The current investigation does not establish that
it alone caused this failure. The catalog limit was stale in both versions.

## Validation

`TestCodexCatalogPreservesModelContextWindow` failed before the fix for both
272,000 and 64,000. It passes after the fix. The test also covers missing,
zero, negative, and incorrectly typed limits. Its larger-limit case includes
`max_context_window` to verify that the normal limit remains authoritative.

Focused command:
`CGO_ENABLED=0 scripts/with-build-limits go test ./internal/provider -run 'TestCodexCatalog|TestRustProviders.*(Catalog|Context)'`.
Result: passed for the current code changes.

`CGO_ENABLED=0 scripts/with-build-limits go test ./cmd/... ./internal/...` passed.
`CGO_ENABLED=0 scripts/with-build-limits go vet ./cmd/... ./internal/...` passed
for the current code changes. Development asset watchers were restarted to
release memory during compilation.

The diagnostic did not send a provider request or change live data.
Its temporary test file was removed. Ordinary tool metadata and diagnostic
logs remain in `/var/tmp/noema-context-probe/`.

## Remaining live step

This step was superseded by the [context compaction fix](notion-context-compaction-2026-09-18.md).
The live request passed with the saved 128k limit and no new sign-in.

Go fetches the Codex model catalog during sign-in. It has no separate catalog
refresh path. Existing account metadata still contains the old limit.
Codex sign-in is required to replace that saved catalog. A successful live
Notion page request has not been verified after this change.

## Size

Production: 4 added lines. Tests: 21 added lines and one new test.
Generated GraphQL: unchanged. Inclusive growth: 25 lines.
The estimate was 15 production lines and 40 test lines.
Tracked Go totals are 95,072 production, 69,320 tests, and 79,566 generated lines.
The inclusive total is 243,958 lines. The inclusive migration limit already
failed at the base revision. See [the preceding evidence](mcp-setup-card-refresh-2026-09-18.md).
