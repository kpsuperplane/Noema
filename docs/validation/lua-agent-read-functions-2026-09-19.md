# Agent Lua random and time functions

The agent can select random input values and read clocks or dates.
Each execution owns its random generator. Seeds cannot change another execution.
Date conversion preserves supplied tables. Date formatting retains memory limits.
File, network, process, environment, and module access remain unavailable.
Adapter profiles retain their previous restrictions.

The patch starts at `498fb89f` and reuses the existing sandbox profiles.
The production budget was 130 lines. The test budget was 100 lines.
Three new tests cover random bounds and seeds, adapter restrictions, and the failed page-selection pattern.
Existing tests cover blocked operations and date allocation limits.
Two legacy adapter tests now check the production adapter profile and the current agent contract.
The first broad run found their stale expectations. The corrected focused checks passed before the final broad run.

## Checks

- `GOTMPDIR=/nt CGO_ENABLED=0 scripts/with-build-limits go test ./internal/script ./internal/runtime -run 'TestAgentRandom|TestAdapterProfiles|TestLua'`: passed.
- `GOTMPDIR=/nt CGO_ENABLED=0 scripts/with-build-limits go test ./internal/adapter -run 'TestRustAdapters_(sandbox_has_no_ambient_authority_or_cross_call_state|agent_code_reads_json_and_has_no_ambient_authority)'`: passed.
- `GOTMPDIR=/nt CGO_ENABLED=0 scripts/with-build-limits go test ./cmd/... ./internal/...`: passed.
- `GOTMPDIR=/nt CGO_ENABLED=0 scripts/with-build-limits go vet ./cmd/... ./internal/...`: passed.
- `GOTMPDIR=/nt scripts/run-noema-dev-server build`: passed.
- `git diff --check`: passed.

These checks cover the final source changes. Later changes only record this report.
The development supervisor resumed. The server uses the current binary, and the authenticated socket responds.
Frontend code did not change. No frontend checks were required.

## Size

Production adds 127 net lines. Tests add 78 net lines.
Generated GraphQL does not change. The inclusive Go increase is 205 lines.
Totals are 96,316 production, 69,706 test, and 78,646 generated lines.
The inclusive total is 244,668 lines.
The migration ratios are 55.36 percent production and 102.02 percent inclusive.
The inherited inclusive ratio exceeds the 80-percent limit.
