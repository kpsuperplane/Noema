# One record identity for Chat tools

Base revision: `da6c933a`.

Tool calls now use their conversation record IDs for live delivery, storage,
and display. The existing deterministic record-ID function supplies the ID
before the live event and when the call is saved. Its inputs and digest are
unchanged. The runtime no longer creates a separate transient record ID.

Results use their own record IDs and reference the saved call ID. The Chat
API reads those IDs and parent references directly. It no longer reconstructs
IDs from response numbers. Existing records already contain these references,
so history needs no migration or rewrite. Old embedded display IDs are ignored.

Native calls, hosted searches, and form results use this same record identity.
Provider call IDs remain separate protocol values. Provider requests and tool
execution are unchanged. No new manager, schema field, or compatibility path
was added.

## Validation

Focused checks passed on the final code:

```sh
CGO_ENABLED=0 scripts/with-build-limits go test ./internal/graphql ./internal/runtime ./internal/store -run '(TestToolActivityRetriesKeepSeparateDisplayIDs|TestConversationActivitiesPreserveCancellationAndAuthentication|TestRustRuntime_runtime_turn_streams_tool_call_started_before_durable_response_items|TestConversationToolCallIsAtomicRepeatSafeAndRecoverable|TestConversationA2UIActionIsAtomicAndRecoverable|TestChatPersistsHostedWebFactsWithoutOrdinaryHostedReplay|TestRustRuntime_hosted_web_search_stream_events_update_one_tool_lifecycle)'
```

Existing tests cover retry separation, old history, live-to-saved replacement,
call/result pairing, repeat safety, form recovery, and provider ID preservation.
Overlapping identity and status assertions were consolidated. No new test was added.
The hosted-search test initially expected provider IDs as display IDs. Its
updated assertion verifies both identities and their pairing.

Read-only inspection passed at desktop 1440px and phone 390px widths. The existing
Notion turn shows four attempts, including separate failed and successful updates.
Progress commentary still appears only in its bubble. The API returns eight
distinct record IDs with four correctly paired call/result groups.
The first phone inspection timed out during the development rebuild; the retry passed.
No message or Notion update was sent. Private screenshots remain outside Git,
in `/var/tmp/noema-identity-20260918/`.

The broad run passed all packages except one runtime test. Its event collector
kept the first snapshot and discarded the saved update with the same ID.
The collector now replaces snapshots, like Chat. The test also selects its
expected final reply instead of assuming the last record is that reply.
The corrected focused test passed. Transient assertions now read the explicit
metadata flag instead of an ID prefix.

Initial broad processes stopped before completion. Development asset watchers
were restarted to reduce memory pressure. Interrupted runs are not counted as passes.

Final validation covers the current worktree:

- `CGO_ENABLED=0 scripts/with-build-limits go test ./cmd/... ./internal/...`: all packages passed except the corrected runtime test.
- `CGO_ENABLED=0 scripts/with-build-limits go test ./internal/runtime`: passed after the test corrections.
- `CGO_ENABLED=0 scripts/with-build-limits go vet ./cmd/... ./internal/...`: passed.
- `CGO_ENABLED=0 scripts/with-build-limits go vet ./internal/runtime`: passed after the test corrections.

Passing package results are reused from this worktree before the runtime-test-only corrections.
No production code changed after those checks. No frontend source changed.

## Review and size

Review checked record identity, result parent references, provider correlation,
and existing history. No remaining defect was found in those paths.
The patch reduces production Go by 10 lines and test Go by 7 lines.
Generated GraphQL is unchanged. The inclusive reduction is 17 lines.
Totals are 95,125 production, 69,434 test, and 79,566 generated lines: 244,125 overall.
The inherited inclusive migration size limit remains exceeded.
