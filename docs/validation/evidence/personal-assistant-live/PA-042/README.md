# PA-042 — Tax packet

Verdict: **Pass**

Date: 2026-09-08

Backend: live Go Noema development instance through
`/tmp/noema-codex/graphql.sock`.

## Fixture

- Service: synthetic tax-records API, fixture
  `2026-09-08-tax-records-api-v1`.
- Documentation: `https://partial-tail-efficient-major.trycloudflare.com/docs`.
- Endpoints: `/v1/profile`, `/v1/documents`, and `/v1/forms/1098`.
- Fixture source: [`scripts/acceptance/run-mock-tax-records-api.ts`](../../../../../scripts/acceptance/run-mock-tax-records-api.ts).
- The service is deterministic and synthetic. It has no filing, payment, or
  other write endpoint.
- Conversation: `conversation:a39407c5e9686f34225273862829bf0f`.

The fixture contains a 2025 profile, a superseded W-2 and current W-2C, a
1099-INT, one deductible medical receipt and a duplicate copy, plus a separate
1098 endpoint. The documentation requires a superseded record to yield to its
correction and a duplicate receipt to count once.

## Connection setup and repairs

Noema inspected the exact documentation URL and used
`adapter.definition_template` before proposing the connection. The first
proposal was accepted as a reviewable, unauthenticated, read-only API with
three operations:

- `get_profile` — `GET /v1/profile`.
- `list_documents` — `GET /v1/documents`.
- `get_missing_1098` — `GET /v1/forms/1098`.

The initial proposal digest was
`df1e14b8d82204346676d970ea73c14af5509f5f4104c33db741837066699c74`.
It was reviewed as revision `v1`, producing digest
`d2bd278c4804eeb8a9234dd8da419a4bfb4ab4ab29baa0e8322cdaec8445aab7` and
connection `de5a48183d4d26a18659f49e47c7f259`.

The first read found that `list_documents` returned an empty array. The
projection looked up a root key named the empty string even though the service
returns a top-level JSON array. Noema proposed and the operator reviewed a
focused revision that mapped the top-level array directly. The v2 proposal
digest was `9848c242072f46e0806b80af549f5bcaa90eb1a82d954cd91466be1e3f7c5c61`;
the reviewed digest is
`578696c71cf6157a2eec92ea6a67a807e3ddb1cb1ae14863bde190f81ff2cb9b`.

The repaired read returned four rows, but the v2 contract capped the list at
four and omitted the duplicate receipt. Noema proposed only a cap change to
five records. The v3 proposal digest was
`74a0fa4775e5d848cc014ac2686b74c3403f672ab42092d183f734a62392789c`; the
operator reviewed digest
`83ad3a2f873a502ea501ad0d2cbe369115cb4174ce6f97483096dd0001a03cc7`.

The final connection is active at connection revision 3 and policy revision 2.
It has automatic data sharing for read-only operations and no write operation.

## Final read and packet

Turn `turn:b4a8743073b41684f78e3288fbe370f9` ran the final read after the v3
review. All three API operations completed successfully. The document-list
result contained all five fixture rows, including both receipt records and
their shared receipt reference. The dedicated 1098 result contained
`form-1098-2025` with `$3,200` mortgage interest.

Noema created artifact
`artifact:a70a609eb66d2e7b7655109872ab887e`, version
`artifact_version:5542ffea61d35e36e3c7e3b9b3a98895`, titled **2025 Tax Review
Packet, Final Complete Index**. The saved Markdown packet is 3,602 bytes.

The packet passes the content checks:

- It keeps `w2-2025-original` as superseded evidence and does not use its
  `$82,000` amount as the current wage record.
- It uses current `w2-2025-corrected` and its `$83,500` amount.
- It preserves the `$425.75` 1099-INT record and its source locator.
- It preserves both medical-receipt rows and their source locators, while
  counting the `$680` expense once because the copy has
  `duplicate_of: receipt-2025-001` and the same receipt reference.
- It includes the separately retrieved 1098 and its `$3,200` interest amount.
- It preserves tax year, status, amount, source locator, correction fields,
  and duplicate fields for every returned row.
- It states that the packet is for professional review only and was not filed
  or sent outside Noema.

The final turn used only `get_profile`, `list_documents`,
`get_missing_1098`, and `artifact.create_local_file`. The fixture trace for the
final run shows one GET for each API endpoint and no POST or other write. No
tax return, payment, filing, or external submission occurred.

## Acceptance

**Pass.** Noema built a complete, sourced tax-review packet from a newly
connected API, selected the corrected record, deduplicated the receipt for
counting, retrieved the missing form, and preserved the source relationships.
The two connector defects were repaired through reviewed revisions before the
final read. No real tax data or external tax action was used.

