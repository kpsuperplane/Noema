# PA-036 — Mixed inventory

Verdict: **Pass**

Date: 2026-09-08

Backend: live Go Noema development instance through
`/tmp/noema-codex/graphql.sock`.

## Fixture

- Service: synthetic mixed-record inventory API, fixture
  `2026-09-08-inventory-api-v1`.
- Documentation: `https://groundwater-mask-businesses-pine.trycloudflare.com/docs`.
- Read endpoints: `/v1/profile` and `/v1/records`.
- Fixture source: [`scripts/acceptance/run-mock-inventory-api.ts`](../../../../../scripts/acceptance/run-mock-inventory-api.ts).
- The fixture contains 20 records in PDF, image, email, and spreadsheet
  formats. It has no real account, contact, payment, or write operation.
- Conversation: `conversation:a39407c5e9686f34225273862829bf0f`.

The profile requires item, manufacturer, model, serial, purchase date, and
source locators. It defines duplicate evidence as a matching serial, or a
matching invoice, manufacturer, model, and purchase date. It forbids name-only
merges. Records `record-014` and `record-018` have unreadable serials.

## Connection setup and repair

Noema read the service guide before proposing a connection. The first setup
request unexpectedly delegated and did not persist a useful primary response.
The delegated task completed the same bounded proposal, which was accepted.

The initial accepted definition exposed two read-only GET operations:

- `get_inventory_profile` — `GET /v1/profile`
- `list_source_records` — `GET /v1/records`

The initial proposal digest was
`9e28ca7ab9b91bfe2edaf7dc56cc2ebcdf02079f90422aeec253e6cb244e7f4d`.
Acceptance produced reviewed semantic digest
`3ae40002697f3e129816590c421aa16dd8782cdd3ed9bc31d8f201dd4b9db4ac`.

The first read found that the profile contract used the wrong field types and
the records contract projected the wrong response path. Later proposals were
accepted after each live failure:

| Revision | Proposed digest | Accepted semantic digest | Change verified |
| --- | --- | --- | --- |
| v2 | `aa889a62e93c2916173b602ea10aa43ba999bace4cdaf08dde4bbd7885d21110` | `aed98945db5d870a7948dfd64d04cbf113f313a4c0da91f20c10b7cf362abe02` | Corrected response envelopes. The first retry still used wrong profile types and omitted required record fields. |
| v3 | `37e31e44d9e5b99a9133e9cffa4d4edc624ee2e94e66a937529cc83446eb7768` | `21cda76caee55ffa499e404940e1e28ebe7afb648e8c30a45320f2f0dcd27edb` | Preserved profile arrays and mapped all documented record fields. The first retry then exposed a 32-byte locator bound. |
| v4 | `b0bc52a2970592a0164bda16a9bc1cd5854c8ff968cefdb6a90db94d02910f98` | `52872ab3a91dbacb8ec2637b75200d17fb69ee6be0cb0c2300eff9cff1312f73` | Raised `source_locator` to a bounded 64-byte field. All 20 records then read, but null serials were omitted. |
| v5 | `fbd9e6b7fc3b6e24a75ef800eb525cf511858edb3048613a538cc5f871e1c931` | `1e1088c2c14778c059edc0136bb3c25faf9ad5ed03c52d44ce659e1cbbb0a44a` | Added required `serial_readable` and used a 48-byte locator bound. |

The final connection is `db7b4007e59b01284de0ad51f85b4a5c`, at connection
revision 6 and policy revision 2. It exposes only the two read-only operations.
Its data policy is `allow_automatically`; unsafe actions remain `always_ask`.

## Live read and review

The first read turn, `turn:c806822ec0282181b13659ebaa4b7899`, failed both
contract checks. A v2 retry, `turn:763c6832ba0649c843e0b356b13e2054`, also
failed both checks. The v3 retry, `turn:48c35730d8276bdc29bcf673cae1e4b7`,
read the profile but failed the record locator bound.

The v4 retry, `turn:bb91dfb789920a5f29c6b4a5bb506b37`, read the profile and all
20 records. Its durable records payload contained both null serial records,
but it did not expose an explicit flag, and the model summary misreported one
of them. The v5 retry, `turn:041a7ae216c1d2ba9d2e59172977193e`, read both
operations again and returned:

- 20 records.
- 15 distinct items after applying the documented rule.
- Five duplicate pairs with matching serial, invoice, manufacturer, model,
  and purchase date.
- All 20 record IDs, formats, and source locators.
- `record-014` and `record-018` with `serial_readable: false`.

The durable tool payload was checked directly. It contained exactly those two
unreadable records, while a normal record had `serial_readable: true` and its
serial value.

## Saved artifact

Turn `turn:fde12999c3f214341cb800388c43e783` saved a local Markdown artifact
after the successful read. The result was `conversation_item:2680`, followed
by the completion message in `conversation_item:2681`.

- Artifact: `artifact:1633d85ee11a6094338df67a3721b1d8`.
- Version: `artifact_version:bd87d55ef3f72f78f3e7d7dbb8678028`.
- Filename: `verified-inventory.md`.
- Size: 3,675 bytes.
- Media type: `text/markdown`.

The file was inspected through the read-only development home. It has exactly
15 inventory rows, all 20 source locators and formats, five duplicate pairs
with matching evidence, both unreadable serial records, and the merge rule.

## Acceptance checks

| Requirement | Result |
| --- | --- |
| Read service documentation before connecting | Pass. The agent opened the fixture guide before proposing the connection. |
| Generate, review, and accept a bounded read-only connection | Pass. The final reviewed connection has exactly two documented GET operations. |
| Read the profile and every source record through the connection | Pass. The final read returned the profile and all 20 records. |
| Produce 15 distinct items | Pass. The final response and artifact contain exactly 15 rows. |
| Merge only proven duplicates | Pass. Five pairs match the documented evidence rule; no name-only merge is used. |
| Preserve every source locator and source format | Pass. All 20 record IDs, locators, and formats are present in the payload and artifact. |
| Flag both unreadable serials without guessing | Pass. `record-014` and `record-018` are explicitly marked unreadable. |
| Save and inspect the requested artifact | Pass. The 3,675-byte Markdown file was read from the development home and checked. |
| Avoid external writes | Pass. The fixture is read-only, and Noema made no service write or contact. |

No real retailer, manufacturer, mailbox, household account, payment account,
or external service was used.
