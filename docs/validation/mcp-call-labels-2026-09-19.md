# MCP call labels

New MCP calls include a short label written from the model's current conversation context.
For example: `Notion · Update Quarterly plan`.
If no recognizable target is known, the model uses only the service and action.
The model uses the user's language and describes the intended action.

Noema asks for `display_label` and `arguments` in a separate call envelope.
The original MCP input schema remains inside `arguments`.
Local schema references keep their original targets.
Source fields named `display_label` or `arguments` remain intact.
Noema separates the label before argument validation, review, or execution.
Only the original arguments reach the MCP server.

Chat and Tasks save the label with the call and restore it during provider replay.
The marker uses the saved label without service-specific field matching.
Actual call and result states determine status and outcome text.
Labels do not change repeated-call detection or approval decisions.
Existing records receive no new labels.
Service setup calls keep their current input format.

## Validation

Checks cover the patch based on `d6e6539d`.

- Schema checks cover local references, source resource IDs, field-name collisions, and Unicode preservation.
- Invalid labels and malformed call envelopes are rejected.
- Chat checks verify exact remote arguments, saved labels, marker text, and replay.
- Task checks verify review arguments, saved labels, and replay after an authentication pause.
- Browser review used sample labels in read responses at 1440 and 390 pixels.
  Groups opened, labels rendered, icons loaded, and both widths had no horizontal overflow.
  No saved records changed and no remote MCP calls ran during browser review.
  The private socket lacks the favicon route, so the browser received a PNG from the earlier production-handler check.
- `CGO_ENABLED=0 scripts/with-build-limits go test ./internal/runtime -run 'TestMCPDisplay|TestPrimaryChatCallsExactMCP'`: passed after the final status correction.
- Focused runtime, store, marker, and GraphQL package checks passed before full server validation.
- `bun run check:generated`: passed.
- `bun run lint`: blocked by the existing missing `enabled` input in `TaskModelPoolsSettings.tsx:100`.
- `../../scripts/with-build-limits bunx eslint src --max-warnings=0`: passed from `apps/web`.
- `bun run build`: passed from `apps/web`.

The initial server validation stopped before completion.
A later attempt exhausted the temporary filesystem while linking tests.
A longer replacement path exceeded Unix socket limits in command-line tests.
The final run uses `GOTMPDIR=/nt`, with the same compiler and build cache.
The failed authentication and command-line checks passed after these environment corrections.
- `GOTMPDIR=/nt CGO_ENABLED=0 scripts/with-build-limits go test ./cmd/... ./internal/...`: passed.
- `GOTMPDIR=/nt CGO_ENABLED=0 scripts/with-build-limits go vet ./cmd/... ./internal/...`: passed.
- `git diff --check`: passed.

The final checks cover all source changes. Frontend source and generated inputs remained unchanged after their successful checks.
The development watcher is restored. The running server uses the rebuilt binary.
The public site serves the frontend bundle produced by this patch.
No frontend tests were added.

## Size

Go production increases by 134 lines and tests by 122 lines.
Generated GraphQL does not change. The inclusive increase is 256 lines.
Totals are 96,190 production, 69,611 test, and 78,646 generated lines.
The inclusive total is 244,447 lines.
The migration ratios are 55.28 percent production and 101.93 percent inclusive.
The inclusive ratio already exceeded the 80-percent limit before this patch.
This bounded feature does not resolve that existing repository limit.
