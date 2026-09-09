# Chat history preparation recovery

The failed turns at 19:52–19:54 UTC on September 9 stopped during `Prepare model context`.
The server remained reachable. The saved error contained no underlying cause.
A separate provider request successfully summarized the same completed history.
Saving a summary also passed against an isolated database copy.
These checks do not prove the exact cause of the original failures.

An optional summary failure previously stopped Chat even when its history still fit the model limit.
The runtime now retains that history and continues. Oversized history still requires successful summarization.
Cancellation still stops the request. No live history was changed during diagnosis.

The deterministic regression test covers fitting and oversized history with a failed summary request.
Focused context tests passed. Broad Go tests passed except the unrelated `TestTmpPA069Validate` temporary adapter test.
The first parallel vet run lost a compiler process. Vet passed when retried with one package at a time.

The patch adds three production Go lines and 34 test Go lines. Generated GraphQL is unchanged.
The inclusive Go addition is 37 lines. No schema migration was needed.

## Verified live recovery, September 9

The server was still using its 19:29 binary after the source fixes.
Its watcher and supervisor were absent. The server and inspection relays remained as orphaned processes.
The saved `noema-build` profile now has full local access for the privileged launcher.
The complete `./attach` launcher was restarted under that profile. It rebuilt the server and restored authenticated socket access.
The live configuration now explicitly selects Codex, matching the existing account. Previously, its absent provider selection required OpenAI credentials at startup.

Startup notification processing also held queued Chat input behind old Task notifications.
Notification processing now yields when Chat input waits. The existing retry schedule resumes notifications without advancing their saved cursor.
A deterministic test verifies that both the queued input and notification cursor remain intact.
History-preparation failures now include the underlying error in the development diagnostic log.

A test message was sent to the existing main conversation through its normal GraphQL mutation.
Its subscription received `Chat`, then `Chat is working.`, then `TurnCompletedEvent`.
The saved turn `turn:d585ff71b1496694f6c5bd13bd6f2c34` is completed.
It contains exactly one assistant message: `Chat is working.` No error notice was saved for that turn.
The conversation history was not reset.

The runtime follow-up adds four production lines. Tests add 20 lines and remove one. Generated GraphQL is unchanged.
The inclusive Go change adds 24 lines and removes one.

Focused notification-priority and Chat checks passed.
The broad Go run exposed a mock-provider deadlock in `TestChatExecutesDurableTaskInspectLoopWithBoundedReplay`.
The mock treated an empty tool list as a tool-bearing request. It now recognizes both absent and empty tool lists.
After that correction, `CGO_ENABLED=0 go test ./internal/runtime -timeout 120s` passed in 51 seconds.
All other packages passed in `CGO_ENABLED=0 go test -p 1 ./cmd/... ./internal/...`; those unchanged results were reused.
`CGO_ENABLED=0 go vet -p 1 ./cmd/... ./internal/...` passed. `git diff --check` passed.
The live subscription evidence is in `/tmp/main-chat-live-result.log`. The saved turn was independently checked through read-only SQLite.
