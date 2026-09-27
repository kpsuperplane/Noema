# Runtime row content

The runtime timeline omits the category, start, and status text beneath each row.
Status icons have accessible names. Failed steps use the error color.
Tool rows show the stored tool name.
Provider rows show a two-line response preview.
The selected details show the full saved text with its line breaks.

Response text comes from existing Chat and Task transcript records.
No database migration or duplicate response storage was added.
The reader matches scope, provider round, and request time bounds.
It excludes deleted Chat items and does not append the Task summary twice.
The existing profile owner check still controls GraphQL delivery.

## Checks

Base: `4ad83fa0`. These checks cover the component, schema, query, and reader changes in this commit.

- Focused runtime-profile tests passed for the store and GraphQL packages.
- Two new store tests cover Chat paragraph order, Unicode preservation, retries, deleted text, and separation between turns.
  They also cover Task output order, fallback content, duplicate-summary exclusion, and missing scopes.
- An initial GraphQL test build overlapped code generation and failed while generated definitions were absent.
  The focused run passed after generation completed.
- The web production build and ESLint passed.
- `CGO_ENABLED=0 scripts/with-build-limits go test ./cmd/... ./internal/...` passed.
- `CGO_ENABLED=0 scripts/with-build-limits go vet ./cmd/... ./internal/...` passed.
  Its first run received SIGTERM. The retry passed after the concurrent web build completed.
- Full web lint still stops at the existing missing `enabled` input in `TaskModelPoolsSettings.tsx:100`.
- Generated web types were regenerated successfully.
  The clean-artifact check initially reports the intended uncommitted schema and generated changes.

## Browser review

Read-only inspection used the authenticated development instance.
At 1440 × 1000 and 390 × 844, rows showed status icons without the old metadata line.
The latest Chat turn showed its saved response in the preview and detail column.
Selecting a continuation showed both complete saved response sections with their paragraph break.
A saved Notion turn showed three named tool calls and three separate continuation previews.
Phone tool names wrapped without horizontal overflow.
Keyboard selection and dismissal remained available.
A saved Task run returned 88 spans, 29 provider requests, 24 text-bearing responses, and 29 named tools.
Requests without saved response text keep their operation label.
No messages were sent, and no product records changed.
Screenshots remain outside Git at `/var/tmp/runtime-text-tools-*.png`.

## Size and remaining limits

Authored Go production: +58 net lines. Tests: +118 lines. Generated Go: +39 net lines.
Inclusive Go delta: +215 lines. Web production adds 18 net lines, excluding generated types.
Two tests were added. The patch stays within the task estimates.

Tracked Go totals are 96,658 production, 70,098 tests, and 78,685 generated lines; inclusive total is 245,441.
The production ratio is 55.55% of the fixed Rust baseline.
The inclusive ratio is 102.34%; it already exceeded the 80% limit before this task, at 102.25%.
This task does not repair that existing repository-wide budget failure.

The user deferred further scrolling work after reporting that the earlier rendering change did not resolve the lag.
This content change makes no claim to resolve scrolling performance.
