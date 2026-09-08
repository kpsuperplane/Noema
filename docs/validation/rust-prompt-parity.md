# Rust prompt text parity

Reference: Rust commit `4d29f6ba1f70a30b5959a0e460217feeeb5e8c04`, before server source removal.

## Contract

Retain the exact Rust instruction text, punctuation, whitespace, and template order.
Tests compare complete output with independent Rust reference files.
Provider tests check instruction delivery separately from ordinary system context.

Covered production prompt families:

- Primary chat and local-tool continuation, including the delegation reminder.
- Agent identity and initial name onboarding.
- Runtime environment, project catalog, and tool visibility.
- Task planner, executor, reviewer, continuation, and finalization.
- Action review and progress audit.
- Memory editing and conversation compaction.
- Task and connection notifications.
- Web summaries and MCP tool classification.

Reference files live under `internal/runtime/testdata`, `internal/webtool/testdata`, and their adjacent tests.
They come from Rust source, not from the Go builders under test.
Each family protects a separate model request path.
Additional checks cover Unicode escaping, dynamic values, and provider field placement.

## Scope of the proof

Exact instruction text does not prove identical model answers.
The model, source data, tools, and request history also affect answers.

Go sends complete context sections instead of Rust's persisted replacement/removal history.
Task source time comes from the saved source message and its time zone.
Go does not retain Rust's separate original manual-task authorization snapshot.
These context storage differences remain outside this text-copy change.
Unavailable-tool catalog rows also depend on the current Go capability surface.

## Validation

Focused runtime, web-tool, provider, MCP, and GraphQL checks cover the changed paths.
Broad validation passed for this patch:

- `CGO_ENABLED=0 go test ./cmd/... ./internal/...`
- `CGO_ENABLED=0 go vet ./cmd/... ./internal/...`
- `git diff --check`

These results cover the final production and test changes. Documentation edits do not invalidate them.

Patch size: production +796/-317 lines; tests +493/-62 lines; generated GraphQL unchanged.
Independent reference files add 290 lines.
Production additions remain within the 900-line estimate.
Test additions remain within the 350-line estimate plus its allowed 50-percent margin.

The existing inclusive size limit was already exceeded at the starting revision.
Before this change: 93,768 production, 66,400 test, and 78,072 generated lines; 238,240 total.
After this change: 94,247 production, 66,831 test, and 78,072 generated lines; 239,150 total.
The production ratio remains below 80 percent. The inclusive ratio is 99.72 percent.
The user requested preservation of the Rust tests. This patch does not remove those tests to reduce that ratio.
