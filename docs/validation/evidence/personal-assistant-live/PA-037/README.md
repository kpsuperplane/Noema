# PA-037 — Personal spending analysis

Verdict: **Pass**

Date: 2026-09-08

Backend: live Go Noema development instance through
`/tmp/noema-codex/graphql.sock`.

## Fixture

- Service: synthetic spending API, fixture
  `2026-09-08-spending-api-v1`.
- Documentation: `https://sacramento-reliance-channels-greeting.trycloudflare.com/docs`.
- Read endpoints: `/v1/profile` and `/v1/transactions`.
- Fixture source: [`scripts/acceptance/run-mock-spending-api.ts`](../../../../../scripts/acceptance/run-mock-spending-api.ts).
- The fixture has no bank, card, merchant, payment, or write operation. All
  rows are synthetic.
- Conversation: `conversation:a39407c5e9686f34225273862829bf0f`.

The ledger covers 2026-06-01 through 2026-08-29. It defines two equal
45-day periods, one duplicate row, one negative refund, and one missing
category. The expected net total after deduplication is 2,400 USD.

## Connection setup and repair

Noema opened the service documentation and proposed a public, unauthenticated
connection with only these read-only operations:

- `get_spending_profile` — `GET /v1/profile`
- `list_spending_transactions` — `GET /v1/transactions`

The proposal digest was
`2cd2eee828ec8a78f0e7cadeac0663d59f6f1d2d82aeb14e8620f1dd44077bc8`.
The accepted reviewed semantic digest was
`2e7f3454cd22b069724b3df8afd5f7270a971ba1713e34d66897a34a7864d271`.

The first live read, `turn:1a2f2934e1ea2d3c7db7b66fdf5d5268`, read all 15
transactions but its profile transform expected invented field names. The
profile call failed its response-contract check. No write occurred.

Noema loaded the exact reviewed definition and proposed a profile-only repair.
The repair proposal digest was
`e6e9a9ded55cf194702e959f3674144083d5a36d550b8b533606abd11b6c20c7`.
The accepted reviewed semantic digest was
`04d30a03bd3f691d97becb22a1c14e71b49e1977c4296a1a492d03db02da56ad`.
It maps the documented profile fields directly and leaves the transaction
operation unchanged.

The final connection is `d8cbf9647e5b9e780319af7eb78a621c`, at connection
revision 3 and policy revision 2. Its data policy is
`allow_automatically`; unsafe actions remain `always_ask`. It exposes only the
two documented read-only operations.

## Live analysis

The successful read turn was `turn:aeaff3264e63bffe779e01a88500d038`.
Both tool results completed. The durable payloads contained:

- The complete profile, including both comparison periods and expected total.
- 15 ledger rows representing 14 unique transactions.
- `row-005` as an exact duplicate of `row-004`.
- `row-012` as a `-100` USD refund.
- `row-013` as a 120 USD charge with no category.

The assistant compared the equal periods and produced the correct results:

- Period A: 1,100 USD.
- Period B: 1,300 USD.
- Change: +200 USD, or 18.2%.
- Raw rows: 2,480 USD.
- Less the duplicate: 80 USD.
- Verified net: 2,400 USD.

The category comparison preserved the uncategorized charge and reduced dining
by the refund. It did not guess a category.

## Saved artifact

Turn `turn:80d85835a8c219dcf6fc89b8ef2ec594` saved a local Markdown artifact.
The result was `conversation_item:2744`, followed by the completion message in
`conversation_item:2745`.

- Artifact: `artifact:a8624a39a38dffe99932eddb87d01aab`.
- Version: `artifact_version:74b3b7444c8478ede19778ebc6878c4c`.
- Filename: `verified-spending-analysis.md`.
- Size: 2,960 bytes.
- Media type: `text/markdown`.

The file was inspected through the read-only development home. It contains the
equal-period totals, a text bar chart, category table, arithmetic check,
duplicate and refund evidence, the missing-category note, and source locators.

## Acceptance checks

| Requirement | Result |
| --- | --- |
| Read documentation before connecting | Pass. The agent opened the supplied service guide before proposing the connection. |
| Generate, review, and accept a bounded read-only connection | Pass. The final connection has exactly two documented GET operations. |
| Read the full spending profile and ledger | Pass after the profile transform repair. The final turn returned both profile and 15 transaction rows. |
| Remove the duplicate row | Pass. `row-005` was removed as an exact duplicate of `row-004`. |
| Account for the refund | Pass. `row-012` was included as -100 USD and reduced dining. |
| Preserve the missing category | Pass. `row-013` remains Uncategorized; no category was guessed. |
| Verify the 2,400 USD net total | Pass. Raw 2,480 minus the 80 USD duplicate equals 2,400 USD. |
| Compare equivalent periods and explain change | Pass. The 45-day periods show 1,100 USD versus 1,300 USD and the category changes. |
| Save and inspect a readable chart and table | Pass. The 2,960-byte Markdown artifact contains both. |
| Avoid external writes | Pass. The fixture is read-only, and Noema made no service write or payment. |

No real bank, card, merchant, payment account, or financial decision was used.
