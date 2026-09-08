# PA-004 — Reply queue

Verdict: **Pass after repair and rerun**

Run date: 2026-09-08

Backend: Go development instance through `/tmp/noema-codex/graphql.sock`

Fixture: `2026-09-08-gmail-v1-notion-mcp-v8`

Public fixture route: `https://noema.kevinpei.com/__pa-replay/`

Conversation: `conversation:a39407c5e9686f34225273862829bf0f`

## Setup

The test used the reviewed synthetic Gmail API connection for account A. The
connection was re-authorized after the fixture update and remained active for
the whole case. The reviewed definition digest was
`87e848debf42f71700c77b6d2c45ce639b61ff0f923629eef74c7b96b312c3c9`.

The connection was `6665bd552f0c9ac133502f67fa9ef36b` and the synthetic grant
was `32adb7f0cca002107cd96abbf512b347`. The grant included Gmail read and send
scope. The policy allowed the adapter to run, while `send_reply` always asked
for human approval. No real mailbox was used.

## Scenario and execution

Account A started with five relevant threads and supporting inbox messages:

- an urgent client request;
- an overdue promise to Priya;
- a dinner invitation from a friend;
- a newsletter that needed no reply;
- a resolved request with a confirmation message.

Noema first read every bounded inbox page. Pages 1–7 covered message IDs
`a-msg-001` through `a-msg-022`. It then read the correction after the fixture
control action appended `a-msg-023` to the client thread.

The first synthesis ranked the urgent client request, Priya's overdue promise,
the manager reminder, and the friend's invitation. It excluded the newsletter,
the resolved request, and unrelated informational messages. It drafted replies
with the correct thread context and preserved the open questions for the human.

After the correction, Noema refreshed only the client thread. It changed the
client deadline from 17:00 on September 8 to 18:00 on September 9 and revised
the draft. It did not restore the earlier deadline.

The operator approved each synthetic send after inspecting the exact recipient,
thread, and message body. The four governed action requests and fixture
receipts were:

| Action request | Revision | Thread | Synthetic receipt |
| --- | ---: | --- | --- |
| `action:a6f64072e1198e550b8a5d74ad76ce9d` | 1 | Client | `account-a-sent-22` |
| `action:c7682cdf83b8ea379ae82270f1ce1cd4` | 1 | Priya | `account-a-sent-23` |
| `action:a56407de3ac253c86d0f9c16a6c79781` | 1 | Friend | `account-a-sent-24` |
| `action:69e2982c4ec7e0674805a354d1e0b129` | 1 | Manager | `account-a-sent-25` |

The fixture request trace records four `POST /gmail/v1/users/me/messages/send`
requests. Each returned HTTP 200 for account A. The existing threads were
used, so no duplicate conversation was created.

One automatic continuation after the first approved send stopped before it
created the next approval. The continuation had no new user message and could
not find the original human authority. The store now follows the continuation
trigger to the original user turn. A focused regression test covers that path.
The remaining sends were completed in separate ordinary turns, and all four
receipts were verified.

The final unchanged check asked whether a new reply had arrived. The bounded
`in:inbox newer_than:1d` read returned no messages. No additional action was
created.

## Acceptance checks

| Check | Result | Evidence |
| --- | --- | --- |
| Order replies by urgency and relationship | Pass | Client urgency came first, followed by the overdue promise, manager reminder, and friend invitation. |
| Draft the correct replies | Pass | Drafts used the matching sender, thread, requested outcome, and explicit times. |
| Preserve the inbound correction | Pass | `a-msg-023` changed the client deadline to September 9 at 18:00; the revised draft used that deadline. |
| Leave resolved and informational threads out | Pass | The newsletter and resolved request stayed out of the final queue. |
| Require approval before each send | Pass | Each non-read-only `send_reply` action was approved at revision 1 before execution. |
| Close only after mock send receipts | Pass | All four requested sends have fixture receipts `account-a-sent-22` through `account-a-sent-25`. |
| Use the existing threads | Pass | Every fixture send included the existing thread ID. |
| Keep an unchanged queue quiet | Pass | The final empty inbox check produced only a normal assistant message and no action. |
| Avoid real external changes | Pass | All reads and writes targeted the synthetic fixture account A. |

## Evidence and limitations

The durable conversation and action records contain the prompts, tool calls,
approvals, receipts, and final unchanged check. The fixture trace confirms the
four external POST responses. The continuation repair is covered by
`internal/store/chat_turns_test.go`; this case does not repeat a second send
solely to exercise the repaired automatic continuation.
