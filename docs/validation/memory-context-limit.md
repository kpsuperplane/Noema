# Memory model input limit

The development server reported an oversized conversation item during a Memory update.
The item contained an `adapter.definition_template` result.

## Rust comparison

Reference: `4d29f6ba`, `crates/noema-runtime/src/daemon/runtime/turn/finalization.rs`.

- Lines 193–200 read the selected model's context limit, with an 8,000-token fallback.
- The updater reserves 2,048 output tokens and estimates three characters per input token.
- Lines 227–237 stop when one item cannot fit. Rust does not split or truncate that item.
- Lines 501–527 retain connector results as evidence. Browser results receive separate image handling.
- Each completed batch publishes its pages and saved progress together.

Go used 8,000 tokens for every Memory model. The fix reads the selected model's limit through the existing Chat lookup.
That lookup also supplies provider-specific defaults when model metadata is absent.
An unknown provider retains the Memory fallback. This differs from Rust's universal fallback.
The automatic update trigger remains unchanged.

## Validation

The regression test covers 128,000, 8,000, and 1,024-token model limits.
The large model receives the complete connector result and advances saved progress.
The smaller models stop before generation and preserve saved progress.

- Focused check: `CGO_ENABLED=0 go test ./internal/runtime -run 'TestMemory|TestCompletedPrimaryTurnSchedulesMemory'` passed.
- `CGO_ENABLED=0 go test ./cmd/... ./internal/...` passed.
- `CGO_ENABLED=0 go vet ./cmd/... ./internal/...` passed.
- These results cover the final patch against `6b4a3a44`; later report edits do not change the tested code.
- `git diff --check` passed.
- Read-only review found no additional defect in the changed path.
- Live verification remains incomplete. The development socket reset requests after the source change.
- The database inspection view and live server returned different primary conversation identifiers.

The patch adds seven production lines and 50 test lines. Generated GraphQL is unchanged.
Tracked Go totals are 94,319 production, 66,934 tests, and 78,072 generated GraphQL lines: 239,325 inclusive.
Production is 54.21% of the fixed Rust baseline. The inclusive ratio is 99.79%.
The inclusive total already exceeded the 80% limit before this patch, at 239,268 lines.
This existing size failure remains unresolved. Unrelated code was not removed to meet the limit.

Unrelated file preserved: `scripts/acceptance/run-mock-claim-reconciliation-api.ts`.
