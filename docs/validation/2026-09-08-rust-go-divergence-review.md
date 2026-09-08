# Rust and Go behavior review

Baseline: Rust commit `a007a4fa984f0d2eaeb2c101337dbbe7881d9379`.
Test run: Go commit `eeb20d79`, with existing unrelated fixture and documentation edits.
Command: `CGO_ENABLED=0 go test ./... -count=1 -timeout 5m`.
Result: 15 packages passed, 10 failed, and 7 contained no tests.
There were 197 reported top-level failures and one ACP timeout.
The ACP timeout prevented later tests in that package from completing.
These counts describe the September 7 run, before the changes discussed below.

## Decisions supplied by the user

- Rust behavior is the default target. Review each divergence before changing it.
- Onboarding creates no model-provider account until the human configures one. Built-in browser and web service records remain.
- Rust database compatibility is not required. Migration numbers and identical SQL tables are not acceptance requirements.
- Preserve exact Rust response contracts, prompts, tool names, ordering, and visibility.
- Tests must use deterministic mocks.
- Remove ACP support for now.
- Review MCP, task lifecycle, concurrency, and filesystem differences in detail.

Recommendation for GraphQL tests: create authenticated test sessions. Preserve production authentication checks.
The user confirmed this onboarding distinction after reviewing the meaning of credential-free accounts.
Credential-free browser and web entries are internal service records, despite using the provider-account table.
They contain no login credentials. Their `authenticated` status represents availability and is misleading terminology.
Retaining those entries preserves existing browser and web routes.
Commit `57b1d9a4` implements model-account creation on explicit configuration.
Both reported host startup tests and mocked Codex login tests pass.
Commit `eafca44c` also creates accounts during explicit OpenAI setup and local model activation.
Focused environment, account protection, local model, and configured-state API checks pass.

## Limits of the earlier audit

The earlier claim that every mapped test faithfully reproduces Rust was incorrect.
A matching source comment and distinct Go function establish traceability, not equivalent assertions or execution.
Direct inspection found these counterexamples:

- Three authentication tests repeat action-request setup and never exercise their named authentication scenarios.
  See `internal/store/rust_store_parity_test.go`: shared adapter authentication, MCP authentication, and connection deletion tests.
  Their Rust originals create authentication requests and check grouping, repeated requests, revisions, and deletion outcomes.
- The empty-result test calls `Store.FinishTaskExecution` directly.
  The production runtime already rejects missing or blank `RESULT.md` before this call.
  See `internal/runtime/task_execution.go`, the `taskFinishExecution` branch.
- The stale-document test bypasses document preparation and calls `Store.UpdateInboxTask` directly.
  `internal/graphql/task_lifecycle.go` calls `home.PrepareTaskDocumentReplace`, which checks the expected digest.
- The interrupted-run test changes SQL status directly. It cannot prove production interruption cleanup.
  The same test explicitly finishes its debug span, so it cannot prove automatic span completion.
- Several scheduling and project tests stop at unavailable model-profile setup.
  Those failures do not establish broken scheduling or project behavior.

Correct these tests before assigning their failures to product decisions.
Do not add duplicate production checks merely to satisfy a test at the wrong boundary.

## 7. MCP differences

| Area | Observed Go result | Rust expectation or limit of evidence |
| --- | --- | --- |
| Arguments | `CallExact` does not reject array arguments locally. | Reject non-object arguments before invocation. The failure alone does not prove a remote side effect occurred. |
| Schema limits | A 128 KiB description fails the 64 KiB string limit. | The mapped scale test expects acceptance under the 256 KiB schema budget. Other tests impose different string bounds; verify the Rust contexts before choosing limits. |
| Error category | Oversized schemas return an invalid-schema error. | Distinguish unsupported size from malformed schema and identify the bound. |
| Metadata identity | Revisions contain a bare SHA-256 digest. | Prefix the digest with `mcp-tool-metadata:v2:`. Reordered object keys already produce the same digest. |
| Credential replacement | A rejected replacement remains in the active credential file. | Restore the previous credential and leave the connection unavailable pending authentication. |
| OAuth URLs | HTTPS URLs with fragments are accepted. | Reject fragments in MCP endpoint and OAuth authorization URLs. |
| Debug output | Formatting OAuth structures exposes synthetic access and refresh tokens. | Exclude actual secrets from debug output. This test uses synthetic credentials, not discovered live credentials. |
| Secret classification | Rust also hides a public OAuth client ID in one assertion. | Current information policy treats client IDs as ordinary information. Do not restore this historical over-redaction. |
| Persisted results | Structured values are redacted, but an SDK-generated text copy retains their contents. | The original Rust test checks structured output only. The Go test adds a text copy and therefore tests a different boundary. |
| Tool failure | A fake handler error becomes an MCP tool-error result; the server remains healthy. | The test expects a transport failure and unavailable server. Confirm equivalent failure injection before changing health behavior. |
| Shutdown | A closed service can still return a catalog; setup reports unsupported transport. | Reject new work with shutdown state. The mapped timeout test does not actually force a shutdown timeout. |
| Deletion | The credential directory remains after deletion. | Remove the directory. This failure alone does not establish that credential files remain. |
| OAuth timeout | A one-millisecond test gets an unavailable error instead of deadline exceeded. | The test contacts an external hostname. Replace this with a controlled stalled mock before classifying the result. |

Review choices: schema budgets, exact metadata format, shutdown catalog behavior, and the secret-store directory lifecycle.
Credential rollback and actual secret exclusion are correctness requirements under the current contract.
Public client IDs and authorized ordinary text must remain intact.

## 8. Task lifecycle differences

| Area | Observed Go result | Meaning and next check |
| --- | --- | --- |
| Delegation | A batch containing only delegation calls creates zero tasks and reports an unsupported sequence. | The Rust scenario expects every task and the provider's handoff narration. Reproduce with equivalent tool names and schemas. |
| Mixed tool batch | Go rejects the batch with a generic unsupported-sequence notice. | The mapped test expects Rust's specific rejection. The output does not prove partial execution occurred. |
| Prompts | Role prompts omit original-request, document, repair, research, and delegation text checked by Rust. | Exact Rust text is already approved as the target. |
| Completion items | An assistant item remains `running` after a terminal store operation. | The UI can show an unfinished item in a finished run. Verify the full runtime terminal path before assigning the defect. |
| Repeated completion | Repeated execution and review commands return a stale-run error. | Rust expects the committed result to be returned again. |
| Runtime continuation | One test creates several child runs instead of one. | Check resume processing and mocked provider responses; the failure is concrete, but the cause is unresolved. |
| Restart and retries | Several tests leave planner or reviewer runs queued beyond their wait limit. | These are unresolved execution failures. A timeout is not a deliberate alternative behavior. |
| Notifications | The expected notification ledger table is absent. | Different SQL storage is allowed. Test delivery and duplicate suppression through the public path. |
| Results and stale documents | Direct store tests accept operations that higher layers reject. | These are misplaced tests, not established user-visible regressions. |

Review choice: preserve Rust's repeated-command result and exact delegation semantics.
Prompt fidelity is settled. Repair blocked tests before claiming lost task behavior.
ACP-specific task expectations are retired by the user's removal decision.

## 9. Concurrency and version checks

| Area | Observed Go result | Meaning and next check |
| --- | --- | --- |
| Artifact append race | The losing append returns an invalid-version-target error. | Rust expects the version-conflict category. The test stops before its later cleanup assertions. |
| Task generation | A mismatched generation returns stale revision. | Rust uses a distinct stale-generation error. The operation is rejected in both cases. |
| Notification checkpoint | Reusing a checkpoint returns a changed-cursor error. | Rust expects the repeated checkpoint to succeed without another change. |
| Notification suppression | A nearby repeated waiting event creates another task reference. | Rust suppresses the repeated reference. |
| Event reads | Pagination accepts a malformed stored event payload. | Rust rejects malformed persisted events in either direction. |
| Old action approval | A reconciliation scenario leaves a run queued where Rust expects cancellation. | Verify the actual action and current task gate before classifying an authorization defect. |
| Cancellation cleanup | A replacement run can start before the cancelled provider finishes cleanup. | The focused test reproduces this before ACP removal: 5 failures in 20 runs at `8751da56`. This is an existing race. |
| Document updates | The direct store test accepts a digest that the public path rejects. | Repair the test boundary; no lost-update conclusion is established. |

Review choice: preserve exact conflict categories, repeated-checkpoint success, event validation, and repeated-notification suppression.
The existing request to match Rust response contracts already covers error categories.

## 10. Filesystem and diagnostics

| Area | Go | Rust expectation or limit of evidence |
| --- | --- | --- |
| Database path | `NOEMA_HOME/noema.sqlite3` | `NOEMA_HOME/db/noema.sqlite3`. Current project documentation specifies the Go path. |
| Conversation directory | Direct store creation retains an empty working directory. | The Rust store test expects an allocated absolute directory. Verify the product creation path before changing ownership. |
| Artifact root replacement | Artifact creation succeeds after the original root is renamed and replaced. | Rust rejects the operation when root identity changes. Success does not by itself prove a path escape. |
| Diagnostics structure | Records use `event`, nested string fields, and `time`. | Rust expects fields such as `category`, `severity`, and structured context. |
| Diagnostics rotation | A test configured for 512 bytes retains a 4,029-byte file. | Rust enforces the configured limit and one bounded backup. |
| Large diagnostics | Oversized raw details cause an error; another oversized event is accepted. | Rust substitutes bounded raw details and rejects an event larger than the file budget before writing. |
| Unwritable home | Test setup attempts to write under `/usr/local/bin` and fails. | This is an environment-dependent fixture. It has not tested preservation of existing content. |

Review choices: database location, automatic conversation directories, root-replacement rejection, and exact diagnostic format and limits.
No Rust database upgrade support is needed. This does not authorize resetting an existing Go database.
Deterministic fixture repair is already approved.

## Implemented ACP removal

Commits `9637b1a5` and `2889e5a3` remove the ACP API, client controls, execution code, and SDK dependency.
Migration 38 preserves history, stops unfinished ACP work, and pauses its recurrences.
Historic placement markers prevent automatic fallback to a provider executor.
The [retirement record](../development/acp-retirement.md) lists 11 removed Rust mappings and the replacement preservation check.
Go static checks and the web build pass for this change.
Frontend type checking still fails on an existing missing `enabled` input in `TaskModelPoolsSettings.tsx`.
iOS compilation and rendered inspection were unavailable in this environment.
The combined Go suite still fails; this change does not establish full parity.

## Combined validation after corrections

Tested `eafca44c` plus the retained-operation authentication count correction.
Command: `CGO_ENABLED=0 go test ./cmd/... ./internal/... -timeout 2m`.
Result: 16 packages passed, 8 failed, and 6 contained no tests.
There were 192 top-level failures and no package timeouts.
Failing packages: adapter, artifact, diagnostics, GraphQL, home, MCP, runtime, and store.
The main server command and provider packages pass.
`CGO_ENABLED=0 go vet ./cmd/... ./internal/...` passes for the same changes.
Unchanged successful package results were reused by Go's test cache.
The only newly named failure against September 7 is the independently reproduced pre-existing cancellation race.
The earlier progress-finalization timeout did not recur in this run.
