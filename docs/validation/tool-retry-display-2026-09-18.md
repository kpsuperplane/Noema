# Separate tool attempts in Chat

Base revision: `b5361f68`.

The saved Notion turn contains four calls: fetch, failed update, another fetch,
and successful update. Display IDs used the conversation turn and provider
output position. Output positions restart with each provider response. The
second fetch and update therefore reused the first attempts' IDs. The web
transcript deduplicated those activities and hid the successful retry.

The store and live runtime now include the provider response number in activity
IDs. Calls and results use the same call identity. The Chat API derives that
identity from existing saved metadata, so old history also displays correctly.
It changes neither stored history nor the action result.

A second issue repeated the assistant's progress message as the tool label.
New calls no longer copy commentary into display descriptions. The API omits
that obsolete display field from old records and supplies the tool name.
The original assistant text and saved source metadata remain intact.

## Validation

`TestToolActivityRetriesKeepSeparateDisplayIDs` covers repeated output positions,
call/result pairing, failed and successful statuses, streamed/saved identity,
source preservation, and the removal of duplicate commentary from tool labels.
The existing runtime stream test checks the real transient-to-saved transition.
Its commentary remains an assistant item before the tool activity.

Both focused checks passed on the final patch:

- `CGO_ENABLED=0 scripts/with-build-limits go test ./internal/graphql -run '^TestToolActivityRetriesKeepSeparateDisplayIDs$'`
- `CGO_ENABLED=0 scripts/with-build-limits go test ./internal/runtime -run '^TestRustRuntime_runtime_turn_streams_tool_call_started_before_durable_response_items$'`

The first ID-only API check passed. Broader checks were interrupted when the
user added the duplicate-label issue. Those interrupted runs do not establish
validation of the final patch. A later test run stopped before completion.
The complete retry passed. Final broad checks passed on this worktree:

- `CGO_ENABLED=0 scripts/with-build-limits go test ./cmd/... ./internal/...`
- `CGO_ENABLED=0 scripts/with-build-limits go vet ./cmd/... ./internal/...`

Development asset watchers were restarted to release memory during validation.
No frontend source changed, so frontend builds were not repeated.

Browser inspection passed through the read-only helper at 1440px and 390px widths.
Both views show four calls, including the failed update and successful retry.
Tool labels show their names. The progress message appears only in its bubble.
The target turn is `turn:67e819f303d0ffd64e089f5ace745dee`.
No message or Notion update was sent during this task.
Temporary screenshots and scripts are in `/var/tmp/noema-retry-display-20260918/`.
Private conversation content is excluded from committed evidence.

## Review and size

The review found no remaining defect in response identity, result pairing, live
replacement of streamed calls, or preservation of saved history.
The change adds no database migration, polling, or frontend layout code.
The budgets are 35 production lines and 70 test lines.
Production Go changes add 30 lines and remove 13, for a net increase of 17.
Test changes add 58 lines and remove one, for a net increase of 57.
Generated Go is unchanged. The inclusive increase is 74 lines.
Totals are 95,135 production, 69,441 test, and 79,566 generated lines: 244,142 overall.
The existing inclusive Go migration limit was exceeded before this patch.
