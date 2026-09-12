# Chat delegation response saving

The base is `267e13f8739b25475368604161e48efcf2f8f701`.

## Failure and fix

A delegation response can include provider reasoning.
`persistDelegationBatch` saves that reasoning when it starts the first tool call.
The delegation-only path then completes the same response through `finishGeneratedTurn`.
That path tried to insert the same reasoning record again.
Both inserts used the same turn, provider round, and output index.
SQLite rejected the duplicate item ID after the Tasks had already been created.

The runtime now clears reasoning from the completion input after the batch saves it.
The original saved reasoning remains intact for later provider context.
Reply text, usage, and Task creation use their existing paths.
No schema or live data changes are required.

The live failure recorded successful provider calls and a failed response save.
The existing delegation test reproduced a visible failure when provider reasoning was added.
The original test supplied text and tool calls without reasoning, so it missed this case.

## General cause of excessive delegation

The main Chat prompt tells the model to delegate work expected to exceed five tool calls.
After three tool rounds, the continuation prompt adds a private delegation reminder.
The runtime enables that reminder through `localToolContinuationPrompt(providerRound >= 3)`.
This is a model instruction, not an automatic runtime handoff.

Expected call count is a poor measure of the human request's complexity.
Pagination and resource discovery can increase that count without increasing the request's scope.
Small result pages can therefore trigger delegation for a simple lookup.
The Calendar definition uses two records per page, which amplified this general policy problem.
The scope choice to inspect every resource added more work in this incident.

This patch fixes response saving. It does not change delegation policy or connector page sizes.
A general policy correction should consider the requested outcome, interaction needs, and expected duration.
It should not classify a request as complex solely because it needs several result pages.

## Diagnostic limitation

`Chat.failTurn` shows “The provider request failed.” for errors from several stages.
Its error record omits the supplied cause.
That behavior concealed the response-saving failure. It remains a separate diagnostic limitation.

## Validation

The existing delegation test failed before the production fix when reasoning was added.
The final focused regression passes:

`CGO_ENABLED=0 go test ./internal/runtime -run '^TestRustRuntime_runtime_executes_every_homogeneous_delegation_and_uses_provider_handoff_narration$'`

It checks Task creation, preserved reply text, absence of a visible failure, and one intact saved reasoning record.

`CGO_ENABLED=0 go test ./cmd/... ./internal/...` passed all packages except `internal/runtime`.
That run failed `TestChatFinalizesRejectedTaskInspectReplay` with two provider requests instead of three.
The isolated test passed without production changes:

`CGO_ENABLED=0 go test ./internal/runtime -run '^TestChatFinalizesRejectedTaskInspectReplay$'`

`CGO_ENABLED=0 go test ./internal/runtime` passed with the final regression assertions.
The successful results from all other packages remain valid and are reused.
They cover the production patch above and the earlier version of the changed regression test.
Only runtime test assertions changed after that broad run began.

`CGO_ENABLED=0 go vet ./cmd/... ./internal/...` passed.
After the final test edits, `CGO_ENABLED=0 go vet ./internal/runtime` also passed.

One read-only review found a test assertion using a history view that excludes errors.
The correction checks visible errors and saved reasoning through their respective history views.
No production correction was required by that review.
No live provider request or live data mutation was used for verification.

## Size

The patch adds two production lines and 19 net test lines.
The inclusive patch delta is 21 lines.
No generated GraphQL files change. No tests are added as separate test functions.
The repository already exceeds the inclusive migration size limit before this patch.
This unit does not remove unrelated capabilities to reduce that existing excess.

Tracked Go totals are 94,843 production, 68,829 tests, and 79,566 generated GraphQL lines.
The inclusive total is 243,238 lines.
The production ratio is 54.51 percent of the fixed Rust baseline.
The inclusive ratio is 101.42 percent, above the 80-percent limit.
The task-start inclusive total was already 243,217 lines.
The size gate remains failed; this patch does not claim to clear it.
