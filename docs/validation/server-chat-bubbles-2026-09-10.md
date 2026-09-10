# Server-owned Chat message boundaries

Base revision: `8d9bf9a1`.

The earlier fix split assistant text within the web component.
The user required the server to own those boundaries.
The server already had a tested Markdown splitter, but normal Chat did not call it.

Completed Chat messages now use that splitter before saving transcript items.
Blank lines and standalone three-dash separators separate messages outside code fences.
Each paragraph has a stable item ID. The first keeps the original section ID.
Provider citations use the paragraph's original UTF-16 range.
The first paragraph retains the complete provider text for model history.
Additional paragraphs do not duplicate that history.
The initial welcome also uses the shared splitter.

The web component no longer splits text.
The existing transcript layout renders each saved message.
Human text, readable reasoning, and existing saved messages remain unchanged.
Text streams in one record until the provider completes that message.

## Validation

One regression test covers streamed and final-only provider output.
It checks saved paragraph boundaries, fenced code, final-answer styling,
citation ranges, and exact provider history after turn completion.

- Focused Chat output and store checks passed.
- Existing provider splitter and initial welcome checks passed.
- `CGO_ENABLED=0 go test -p 1 ./cmd/... ./internal/...` passed except one transient startup database lock.
- The failed startup test passed alone. `CGO_ENABLED=0 go test -p 1 ./cmd/noema` then passed.
  Passing results from the other packages cover the final patch and were reused.
- `CGO_ENABLED=0 go vet -p 1 ./cmd/... ./internal/...` passed.
- `bun run check:generated` passed.
- `bun run lint` remains blocked by the existing missing `enabled` input in `TaskModelPoolsSettings.tsx:100`.
- `bun run build` passed.
- `bunx eslint src --max-warnings=0` passed.

The first focused run found an item-ID length constraint.
Paragraph IDs now use the existing stable-ID function.
The retry passed. The added final-answer phase assertion also passed.
Broad validation found an extra update for single-paragraph replies.
The phase change now applies only to split replies.
The existing event-order test and Chat output tests passed after that correction.

One broad run stopped with exit status 143 before completion.
The host had little free memory and no free swap.
The final run uses `-p 1` to limit concurrent package work.

Read-only browser inspection covered desktop 1440×1000 and phone 390×1000.
The current saved conversation remained readable at both widths.
No live Chat mutation was submitted. New server-created paragraphs were verified in unit tests.
The browser helper denied the page's automatic mutation, as expected.
The new server-created state has not been visually verified against a live response.

## Review and size

Review checked original provider text, paragraph identity, citations, and code fences.
No database migration or public GraphQL field was added.
Production Go grows by 30 lines. Tests grow by 52 lines.
Generated GraphQL is unchanged. Inclusive Go grows by 82 lines.
The web component shrinks by 33 lines.
Tracked Go totals are 94,568 production, 68,275 test, and 78,987 generated lines.
The inclusive total is 241,830 lines.
The existing inclusive total remains above the historical 80% migration gate.
