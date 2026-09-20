# On-demand tool definitions

Base revision: `a830cbcd`.

## Behavior

Chat and Tasks keep core tools available. Connected-service definitions load
through the existing provider request and execution paths.
Selection uses exact catalog names. No human-message keyword routing was added.

Verified Codex `gpt-5.6-terra` requests use hosted `tool_search`.
OpenAI Responses uses native loading for GPT-5.4, GPT-5.5, GPT-5.6, and GPT-6
families. Other providers and unknown model families use `tools.load`.

The fallback directory contains service rows and exact tool names.
A successful selection adds up to four complete definitions to the next request.
Saved tool results and calls retain selection across continuations.
Current catalog filtering keeps removed tools absent.

Loading does not execute a tool or grant permission.
Existing source-schema validation, role limits, action review, and
authentication recovery still control external calls.

Native loading defers schemas, while names and descriptions remain visible.
The complete catalog still travels to the provider for hosted search.
Used definitions become eager for subsequent requests and full replay.
That first change can invalidate part of the provider's cached prefix.
Compaction can discard selection history and require another load.
This change does not promise a specific end-to-end latency.

See the [capability contract](../harness/capabilities.md#on-demand-definitions)
and [OpenAI guide](https://developers.openai.com/api/docs/guides/tools-tool-search).

## Live provider check

Two synthetic requests used the existing Codex credential through its protected
store. No credential entered logs or model input.
No connected-service tool ran. No Chat or Task data changed.

| Request | Result | Input tokens | Elapsed |
| --- | --- | ---: | ---: |
| Flat deferred function with hosted search | Search, definition, then exact function call | 607 | 3.744 s |
| Full replay with the used function eager | Correct answer from synthetic result | 210 | 1.841 s |

The replay omitted hosted discovery records and the returned namespace field.
It retained the exact function name, call ID, arguments, and synthetic result.
The provider accepted this shape. This verifies the existing saved-call format.

Local synthetic request and result files are under
`/var/tmp/noema-deferred-20260920/`.
Earlier namespace checks remain in
[the initial spot test](codex-deferred-tools-2026-09-20.md).

## Validation

The provider wire test verifies deferred schemas, hosted search, call
normalization, and eager replay.
The runtime selection test verifies directory size, saved selection,
removed tools, failed loading, native context estimates, and unknown models.
Invalid selection checks cover unavailable tools, duplicate names, and bounds.

Existing Chat and Task tests now load a definition before the external call.
The Chat test verifies the exact saved call and one remote execution.
The Task test retains review and authentication recovery after loading.

Initial provider checks exposed two test setup errors.
The request needed native tool transport.
The second wire decode needed a fresh map to avoid retaining an omitted field.
Both test setup errors were corrected.

The first runtime run found an expected prompt change in the historical Rust
comparison. The test now declares that change explicitly.
It also verifies that deferred tools retain service ownership without appearing
in the immediately callable list.

Build memory pressure delayed compilation. Restarting the existing frontend
asset watchers released retained memory. The development authentication socket
still returned `{"state":"authenticated"}`.

## Size and review

The patch adds 156 production lines and 150 test lines.
The budget was 650 production lines and 260 test lines.
No generated GraphQL lines changed. No database migration was added.

The resulting Go totals are 96,585 production, 69,960 tests, and
78,646 generated GraphQL lines. The inclusive total is 245,191.
Production is 55.51% of the fixed Rust baseline.
The inclusive ratio is 102.24%, above the existing 80% migration gate.
That pre-existing repository breach remains; this patch does not resolve it.

Inline review checked current role filtering, removed-service handling,
source-schema validation, saved replay, context estimates, and call dispatch.
No new execution authority or alternate policy path was added.

## Final commands

Focused provider check passed in 0.021 seconds:

```sh
TMPDIR=/var/n20 CGO_ENABLED=0 scripts/with-build-limits go test ./internal/provider -run 'TestResponsesDeferredToolsWireAndReplay|TestDeferredToolsCapability'
```

Focused runtime check passed in 0.868 seconds:

```sh
TMPDIR=/var/n20 CGO_ENABLED=0 scripts/with-build-limits go test ./internal/runtime -run 'TestToolVisibility|TestOnDemandDefinitions|TestLoadDefinitions|TestPrimaryChatCallsExactMCPBinding|TestTaskExecutionUsesGovernedMCP'
```

Broad static checks passed with no diagnostics:

```sh
TMPDIR=/var/n20 CGO_ENABLED=0 scripts/with-build-limits go vet ./cmd/... ./internal/...
```

These checks cover the final production and test changes on base `a830cbcd`.
Later documentation changes do not invalidate these results.
Command logs are in `/var/tmp/noema-deferred-*.log`.

The full server test command passed:

```sh
TMPDIR=/var/n20 CGO_ENABLED=0 scripts/with-build-limits go test ./cmd/... ./internal/...
```

Provider tests passed in 3.707 seconds. Runtime tests passed in 49.976 seconds.
Every package completed successfully.
Frontend rebuild watchers were stopped during the remaining checks.
The existing supervisor then restarted their build process to restore normal development builds.
