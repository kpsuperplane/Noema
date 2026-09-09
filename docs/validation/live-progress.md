# Live progress in Chat and Tasks

## Delivered behavior

Chat and Tasks save readable provider output while generation continues. Each message and reasoning section retains its identity and order.
Normal snapshots have a 100 millisecond minimum interval. Section completion and tool boundaries force a save.
Completion updates existing items. Interrupted output remains visible. Current-run checks reject stale writes.
Response usage is recorded at completion. Reconnects read saved snapshots.

OpenAI and Codex phases survive storage and replay. Unsupported providers retain an absent phase.
Encrypted reasoning stays in provider replay data. Only explicit readable fields reach displayed reasoning.
Hosted searches and native calls retain their provider order during replay.
Per-message citations use offsets within that message.

The shared progress instruction extends Chat and every Task role. Restored Rust prompt constants remain unchanged.
Model wording and update timing remain model-dependent. No extra model call creates progress text.
Existing history remains readable. Previously lost message boundaries cannot be recovered.
No database migration was added.

## Reviewable units

- Provider preservation: `ba3f52c7`, with ordered replay correction in `a2fb8681`.
- Durable Chat and Task delivery, including progress instructions: `394d5f46`.
- Shared bubble display and validation: the commit containing this document.

## Validation

Focused deterministic tests cover section order, phase replay, readable reasoning, completion reconciliation, snapshots, stale writes, and prompt delivery.
Existing Chat and Task tests also cover usage, cancellation, recovery, and tool execution.
The adversarial review found reasoning replay, search ordering, interrupted display, and citation defects. All were corrected.
A narrow follow-up review confirmed the serious replay defect and citation correction.

Commands and results for the delivered worktree:

- `CGO_ENABLED=0 go test ./internal/provider ./internal/localmodel`: passed.
- Focused runtime, store, and GraphQL tests: passed.
- `bun test apps/web/src/components/transcript/renderModel.test.ts`: eight tests passed.
- `CGO_ENABLED=0 go test ./cmd/... ./internal/...`: all packages passed except an unrelated temporary adapter test.
- `CGO_ENABLED=0 go vet ./cmd/... ./internal/...`: passed.
- `bun run build` in `apps/web`: passed, with the existing large-chunk warning.
- Focused ESLint for the six changed production frontend files: passed.
- `bun run lint` in `apps/web`: blocked by the unrelated missing `enabled` field in `TaskModelPoolsSettings.tsx:100`.
- `git diff --check`: passed.

The broad Go failure was `TestTmpPA069Validate` in `internal/adapter/tmp_pa069_validate_test.go:17`.
Its message was `shape adapter proposal is invalid`. This untracked test belongs to concurrent work and was preserved.
The two initial runtime failures expected old combined output. Updated expectations passed in the final broad run.
Go vet preceded those expectation-only edits. Its production inputs were unchanged, so its result was reused.
The final frontend build and focused ESLint include the output-order correction.

Controlled desktop and phone previews used the actual transcript components at 1440 and 390 pixels.
They verified separate sections, grouped corners, tighter spacing, paragraph wrapping, normal reasoning weight, and interrupted output.
Neither width had horizontal overflow. Reload retained the controlled section text.
The live Task page returned a loading error. Full live saved-history browser verification remains unavailable.
Store tests verify saved snapshots independently of that page failure.

Local check logs are `/tmp/progress-go-test-final.log`, `/tmp/progress-go-vet.log`, `/tmp/progress-build-final.log`, and `/tmp/progress-lint.log`.
Preview images are `/tmp/progress-desktop.png` and `/tmp/progress-phone.png`.

## Change budget

The Go changes add 841 production lines and remove 147. Tests add 386 lines and remove 24.
Generated GraphQL adds no lines. These counts stay within the stated 1,000 production and 450 test line budgets.
