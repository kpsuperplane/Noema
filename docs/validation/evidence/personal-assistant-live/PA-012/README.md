# PA-012 — Conversation monitoring

Verdict: **Pass after resolved and overdue branches**

Run date: 2026-09-08

Backend: Go development instance through `/tmp/noema-codex/graphql.sock`

Fixture: `2026-09-08-gmail-v1-notion-mcp-v8`

Conversation: `conversation:a39407c5e9686f34225273862829bf0f`

## Execution

The first request asked Noema to watch the synthetic client launch-support
thread until 15:00 Pacific, ignore unrelated mail, alert once if needed, and
stop after a substantive reply. The delegated monitor Task was
`task:20ab46f9f4cc2332b730fba260bd81ae`. Its executor inspected only
`a-thread-replies-client`, found substantive inbound message `a-msg-017` at
08:15 UTC, and stopped silently before the deadline. The Task finished with a
result and review and made no provider or native-record writes.

The chat turn that delegated this Task returned a provider-stream error after
the delegation, but the durable Task continued and completed. The saved Task
result is the authoritative outcome.

For the overdue branch, Noema checked the synthetic launch-report thread as of
08:00 UTC in turn `turn:9e9b403cff214ccb5a0c0748649741e7`. It found no new
substantive inbound reply by that checkpoint, ignored the unrelated inbox
items, and issued one alert without sending or editing anything. The unchanged
follow-up in turn `turn:ad086b1f49e4d7ab6b35f979a9b355dd` returned “No additional
alert issued.”

## Acceptance checks

| Check | Result | Evidence |
| --- | --- | --- |
| Watch only the requested conversation | Pass | The monitor Task names and inspects only `a-thread-replies-client`; the alert branch targets the launch-report thread. |
| Ignore unrelated messages | Pass | Both branches explicitly ignored unrelated mailbox items. |
| Stop after a substantive reply | Pass | Task result records inbound `a-msg-017` before the deadline and silent completion. |
| Alert once after a missed deadline | Pass | Turn `turn:9e9b403cff214ccb5a0c0748649741e7` issued one bounded alert. |
| Keep an unchanged check quiet | Pass | Turn `turn:ad086b1f49e4d7ab6b35f979a9b355dd` issued no additional alert. |
| Avoid prohibited writes | Pass | The Task guardrails and both turns record no sends or edits. |
| Preserve durable delegated outcome | Pass | `tasks get task:20ab46f9f4cc2332b730fba260bd81ae` shows terminal success with result and review documents. |

## Limitation

The public fixture already contained the substantive client reply, so the
resolved branch did not require a live inbound-message injection. The
overdue branch used a historical UTC checkpoint to test the alert path without
waiting for wall-clock time.
