# Chat reasoning summary visibility

## Behavior

Main Chat omits reasoning-summary content from saved pages and live item responses.
The filter uses the existing provider metadata rule.
It also covers saved Codex summary sections without an explicit summary flag.
Stored records and model context remain intact.
Task transcripts retain reasoning-summary markers.
Progress updates, full reasoning traces, replies, and tool activity keep their existing behavior.

## Validation scope

Base: `b5a1dc5c`.
The existing presentation tests cover saved and live Chat omission, ordinary content preservation, and Task marker preservation.
The saved Codex test covers the existing section-range rule.
The initial test compile ended with signal 15 before tests ran; it is not a passing result.
A later compile was stopped to relieve shared memory pressure.
Restarting the supervised web watchers released retained memory and allowed the server rebuild to finish.
Further watcher restarts relieved the same pressure during broad checks without restarting the tests.
Build limits and cache settings remained unchanged.
The focused check then passed:

```sh
CGO_ENABLED=0 scripts/with-build-limits go test ./internal/graphql -run 'TestAssistantPresentationAcrossChatAndTasks|TestSavedCodexSummaryPresentation'
```

Read-only browser inspection passed at 1440×1000 and 390×1000.
On `/chat`, the reasoning marker was absent and the assistant reply remained visible.
On `/tasks/task:d17fae74681a284068e5e0018b9aa82d`, the Transcript view retained reasoning markers.
Screenshots remained outside the repository under `/var/tmp/noema-*-summary-*.png`.
The Chat view also displayed an unrelated `crypto.randomUUID is not a function` error.
No live turn or product mutation was used for this inspection.

The broad unit suite and static checks passed:

```sh
CGO_ENABLED=0 scripts/with-build-limits go test ./cmd/... ./internal/...
CGO_ENABLED=0 scripts/with-build-limits go vet ./cmd/... ./internal/...
```

Successful checks cover the final Go patch and are reused after documentation edits.
No frontend code or generated schema changed.
A local review checked that the shared Chat converter serves both saved pages and live item responses.
Task output uses a separate converter, which remains unchanged.

## Size

Production: +3 Go lines. Tests: +13 net Go lines. Generated GraphQL: unchanged.
Inclusive patch: +16 Go lines. No new test functions.
Tracked totals: 96,045 production, 69,463 tests, and 78,646 generated GraphQL lines.
Inclusive total: 244,154 lines.
Migration ratios: 55.20% production and 101.80% inclusive.
The existing inclusive-size exception remains unresolved.
