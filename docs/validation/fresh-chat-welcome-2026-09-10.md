# Fresh Chat welcome request

Base revision: `87b5ee3f`.

The live server reported complete onboarding and an existing Codex conversation.
That conversation had five failed welcome turns and no transcript items.
The failures occurred between 20:49 and 20:53 UTC.
The stored turns did not retain the provider error.

The welcome path called the provider directly, bypassing its normal generation session.
It requested `store:true` for Codex. The documented Codex contract requires `store:false`.
The session already applies that setting and owns WebSocket and HTTP fallback behavior.
The welcome path now opens and closes that same session.
This fixes the demonstrated request defect without changing saved accounts or conversations.
A live welcome response remains unverified until the browser retries setup.

## Validation

The existing welcome test now verifies session use and closure, saved assistant text,
and absence of fabricated human messages. It failed before the production change.
It passed after the two-line fix.

- `CGO_ENABLED=0 go test ./internal/runtime -run '^TestRustRuntime_(start_primary_conversation_generates_initial_name_onboarding_message|failed_initial_name_onboarding_logs_runtime_invariant)$'` passed.
- `CGO_ENABLED=0 go test ./internal/provider -run '^TestCodexResponsesWebSocketFallbackUsesFullReplay$'` passed.
  This existing test checks complete replay and `store:false` during HTTP fallback.
- `CGO_ENABLED=0 go test ./cmd/... ./internal/...` passed.
- `CGO_ENABLED=0 go vet ./cmd/... ./internal/...` passed.

The read-only inspection browser initially reported a missing `crypto.randomUUID` function.
Its HTTP inspection origin lacks that browser function. This was an inspection limitation, not evidence of the public HTTPS failure.
No live welcome request was submitted during inspection.

Parallel broad checks exhausted host memory and stopped the frontend watcher at 20:58 UTC.
The development supervisor then stopped its session. Both broad checks completed successfully. The launcher was restarted after they finished.

## Size and review

The patch adds two production Go lines and five net test Go lines.
Generated GraphQL is unchanged. Inclusive Go growth is seven lines.
The existing inclusive repository size already exceeds its historical 80% gate.
This bounded fix does not change that prior budget failure.

The review checked provider-session closure on success and failure.
Existing providers without sessions retain the current direct-call behavior.
The change adds no schema, configuration, retry policy, or live-data rewrite.
