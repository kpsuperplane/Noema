# PA-023 — Deliverable assembly

Verdict: **Pass after Gmail attachment capability was added and the packet was
revised**

Run date: 2026-09-08  
Backend: Go development instance through `/tmp/noema-codex/graphql.sock`  
Fixture: `2026-09-08-gmail-v1-notion-mcp-v8`  
Conversation: `conversation:a39407c5e9686f34225273862829bf0f`

## Scenario

The bounded fixture set contained an older email version with a $1,000 budget,
an approved Notion version 2 with a $1,200 budget, a spreadsheet breakdown that
totals $1,200, and a Gmail message with a text attachment for the appendix.

The first packet was saved before the attachment operation existed. It kept an
honest open item instead of inventing attachment text. Noema then proposed and
the operator accepted a narrow Gmail v2 revision that added only
`get_attachment`. The operator completed the normal synthetic account-A
reauthorization and attached the reviewed revision to the existing connection.

| Item | Durable evidence |
| --- | --- |
| Gmail connection | `6665bd552f0c9ac133502f67fa9ef36b`, revision 4 |
| Accepted Gmail definition | `fe0701e3a4a6a92b9a132f75e6c59785e40416791a7d8bfe21cd7c930aa91de4` |
| Added operation | `get_attachment`, message `a-msg-007`, attachment `a-attachment-001` |
| Attachment result | Declared size 31 bytes; base64url data decoded to the three-line list |
| Packet artifact | `artifact:d7e40c8bc517a528ab05d6bc4287b09f` |
| Current packet version | `artifact_version:c2b9a17d5cf260ea6407eb4ecf13825b`, version 2 |

Credential values and transient OAuth URLs are not recorded.

## Execution

The bounded assembly turn was `turn:03979eaefd535198379454ac56511957`. It
created the first packet and correctly recorded that the old Gmail reader did
not expose the attachment body.

The operator submitted a minimal Gmail revision proposal. The exact pending
proposal was `16f2b07743aa228d244db7e77bd580423d8f762c59d208b2515cbb0d6ba4ad5b`.
The operator accepted it through the normal proposal command. The reviewed
revision was then reauthorized for the existing synthetic grant and attached
to the existing connection. The connection exposed all five operations,
including `get_attachment`, with read-only and idempotent behavior.

Turn `turn:3562e77ac1874b9984e0877d3acde570` then:

1. called `synthetic_gmail_personal-6665bd55.get_attachment` with the exact
   message and attachment IDs;
2. received the 31-byte base64url attachment result;
3. created version 2 of the packet with the decoded appendix text;
4. preserved the approved v2 arithmetic and the superseded v1 note; and
5. stated that the packet was not sent or published.

## Acceptance checks

| Check | Result | Evidence |
| --- | --- | --- |
| Use approved v2 | Pass | Version 2 names Notion v2 and uses its $1,200 budget. |
| Reconcile totals | Pass | The packet shows `$300 + $450 + $250 + $200 = $1,200`. |
| Mark v1 superseded | Pass | The $1,000 email version is explicitly marked superseded, with a $200 difference. |
| Obtain the missing appendix | Pass | The live tool call returned the exact attachment body from the fixture. |
| Save a revised packet | Pass | Artifact version 2 is current and has a distinct immutable version ID. |
| Preserve the requested format | Pass | The current version is Markdown with `text/markdown` media type. |
| Verify saved content | Pass | The read-only home view contains the expected budget, arithmetic, and three appendix lines. Its SHA-256 is `41581e8224912ca0f0b20d6557368d744ce503a2c2147477b4eb88e8ee6d9664`, matching the stored version metadata. |
| Verify download reference | Pass | `artifactVersionDetail` reports version 2 as current and returns `/artifacts/versions/c2b9a17d5cf260ea6407eb4ecf13825b/download`. The GraphQL-only inspection socket does not serve the HTTP download route, so the backing file and the product's download metadata were checked independently. |
| Avoid external side effects | Pass | No send, publish, or share operation occurred. |

## Recovery note

The first revision attempt exposed two real gaps: Gmail lacked an attachment
operation, and the accepted revision initially preserved the connection's old
operation selection. The operator fixed both through the product flow by
proposing the smallest reviewed operation, completing scoped reauthorization,
and attaching the revised definition to the existing grant. The final rerun
completed the requested packet without changing any external account.
