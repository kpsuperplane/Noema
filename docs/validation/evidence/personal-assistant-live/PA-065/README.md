# PA-065 asset inventory

Verdict: Pass after reviewed API setup, seven ordered source reads, one
approved synthetic repair-receipt write, final status verification, and an
independently reviewed Markdown artifact.

This case used the live Go Noema development instance. Every asset, receipt,
warranty, photo, recall, repair, and owner was synthetic. No real product,
manufacturer, warranty claim, purchase, payment, or repair was involved.

## Case and fixture

- Fixture: `2026-09-09-asset-inventory-api-v1`
- Fixture URL: `https://physically-panels-directories-coleman.trycloudflare.com`
- Documentation: `https://physically-panels-directories-coleman.trycloudflare.com/docs`
- Case: `asset-inventory-001`
- Owner: `Jordan Lee (synthetic)`
- Profile date: `2026-09-09`
- Scope: one personal Noema workspace
- Fixture implementation:
  [`run-mock-asset-inventory-api.ts`](../../../../../scripts/acceptance/run-mock-asset-inventory-api.ts)

The fixture exposed five products, five warranties, five confirmed serial
photos, five original receipts, one exact duplicate receipt, one exact recall,
one near-match recall, one existing repair, one bounded repair-receipt write,
and a final status record. It cannot repair a real product, file a recall,
contact a manufacturer, submit a warranty claim, or move money.

## Connector setup

Task `task:741e5656ede9277cdb376dc04855243b` completed with planner
`run:91f8990b8d79657e84a853b61c48bfe3`, executor
`run:738d5f8e58d43676921c4ad0648b8854`, and reviewer
`run:b0751f51d17a76808666fb0806d1c443`. The task inspected the documentation,
called `adapter.definition_template` first, and proposed exactly nine
operations. The pending digest was
`9744056fe65a5b1a6fad77388154ad62ccdbe3e2c9057c5dc66f82b464982314`; the
operator accepted that exact digest. The reviewed digest became
`9830f0d76f8dd41886dc6fb63367c09d4b619a1eacc38514efbdb5b05a283e7b`.

The active connection is `903300fb534284f245c4afc1ef251288`, exposed to the
task as `personal-903300fb`. It is active at connection revision 2 and policy
revision 2. Reads are allowed automatically. The only write,
`record_inventory_repair_receipt`, is always approval-gated.

The reviewed operations were:

| Operation | Method and path | Behavior |
| --- | --- | --- |
| `get_asset_inventory_profile` | GET `/v1/profile` | Read-only, automatic |
| `list_inventory_products` | GET `/v1/products` | Read-only, automatic |
| `list_inventory_warranties` | GET `/v1/warranties` | Read-only, automatic |
| `list_inventory_receipts` | GET `/v1/receipts` | Read-only, automatic |
| `list_inventory_serial_photos` | GET `/v1/serial-photos` | Read-only, automatic |
| `list_inventory_recalls` | GET `/v1/recalls` | Read-only, automatic |
| `list_inventory_repairs` | GET `/v1/repairs` | Read-only, automatic |
| `record_inventory_repair_receipt` | POST `/v1/repair-receipts` | Approval-gated synthetic write |
| `get_asset_inventory_status` | GET `/v1/status` | Read-only, automatic |

Setup created one proposal receipt artifact:

- Artifact: `artifact:722caad539059d582290b7c3ae5f92fa`
- Version: `artifact_version:3273b55a7c39d3c1095103958c0474f7`
- Size: 1,620 bytes
- Filename: `synthetic-asset-inventory-proposal-receipt.md`
- Content SHA-256: `8cf018484c62b31526ea51550907492834fa587ad5138bfc50b7a832eac5498c`

The setup task called no `/v1` operation. Operator health and documentation
probes are excluded from the service evidence below.

## Inventory reconciliation

Task `task:99faebb4e5c67e195bc0bedda395fe3b` completed with planner
`run:9f794063bc5522c4e0133ee0ad6873bc`, initial executor
`run:33f9cf6c2e9407a33d96b1ead56c4784`, resumed executor
`run:395aba431ef8da16d144b06c00b5c98a`, reviewer
`run:a7a0079eb093a69c964e4c59e4e6e75c`, correction executor
`run:59e1649d608bc4073706272b5b65f2a2`, and final reviewer
`run:6b1fffc60e103f744f4d9085d3e8d9f1`. The first review asked for explicit
brand and model fields. No connector calls were repeated. The corrected result
passed the final review.

The seven source reads ran once each, in the required order:

| Sequence | Operation | Result |
| ---: | --- | --- |
| 1 | `get_asset_inventory_profile` | Case, date, owner, boundary, and scope |
| 2 | `list_inventory_products` | Five stable product and serial identities |
| 3 | `list_inventory_warranties` | Five asset and serial-matched warranties |
| 4 | `list_inventory_receipts` | Five originals and duplicate `receipt-002-copy` |
| 5 | `list_inventory_serial_photos` | Five confirmed serial-photo matches |
| 6 | `list_inventory_recalls` | Exact `recall-001` and near-match `recall-002` |
| 7 | `list_inventory_repairs` | Existing `repair-000` on `asset-001` |

Noema preserved five stable assets and all product, serial, purchase, warranty,
receipt, photo, repair, recall, and `asset://` locator fields. It treated
`receipt-002-copy` as a duplicate of `receipt-002`, not a second purchase. It
marked `warranty-004` expired on 2026-08-22 relative to the profile date
2026-09-09. It flagged only `recall-001` for `asset-002`; `recall-002` names a
different model and serial prefix and remained a near match only.

The task presented gate `gate:4ae5b959123f855519f61461729378c0`. After approval,
action `action:a9f7eb0fde9d048c1ee4f218a18118fb` called the POST exactly once
with these arguments:

```text
asset_id: asset-002
serial_number: NSAP3-2024-0188
repair_date: 2026-09-09
vendor_label: Northstar Service Desk (synthetic)
repair_summary: Filter assembly replaced under synthetic warranty; no change to recall status.
warranty_id: warranty-002
```

The synthetic response was repair receipt `repair-receipt-001`, status
`recorded_in_synthetic_inventory`, submission count `1`, and locator
`asset://repairs/repair-receipt-001`.

The final status read then ran once. It returned asset count `5`, duplicate IDs
`receipt-002-copy` and `receipt-002`, exact recall `recall-001` on `asset-002`,
near match `recall-002`, repaired asset `asset-002`, repair receipt
`repair-receipt-001`, submission count `1`, and locator
`asset://status/asset-inventory-001`.

The service ledger, excluding operator probes and documentation requests, was:

1. GET `/v1/profile`
2. GET `/v1/products`
3. GET `/v1/warranties`
4. GET `/v1/receipts`
5. GET `/v1/serial-photos`
6. GET `/v1/recalls`
7. GET `/v1/repairs`
8. POST `/v1/repair-receipts`
9. GET `/v1/status`

## Sourced artifact

Task `task:48788a07a447f96af708414ecc754818` created exactly one local Markdown
artifact. Planner `run:6e358178f68e76efc45b4b8f9a349fc2`, initial executor
`run:99249d7e7470c1c7ad0bd27136bdb973`, resumed executor
`run:8d948a6144e0802fab7ea8f9e70ad5c8`, and reviewer
`run:ecc9de219da8f46417a705632cdba340` completed successfully. The artifact
task had no connector access. It first requested the prior reconciliation
because that output was not in its task directory; the operator supplied that
completed result, and the artifact then passed review.

- Artifact: `artifact:d233dd159ed4654891e38f35160b47d2`
- Version: `artifact_version:944a8b6f2d342d7086ef5eab180a8a42`
- Title: `Synthetic Asset Inventory Final Brief`
- Filename: `synthetic-asset-inventory-final.md`
- Size: 10,483 bytes
- Media type: `text/markdown`
- Preview: `MARKDOWN`
- Download: `/artifacts/versions/944a8b6f2d342d7086ef5eab180a8a42/download`
- Content SHA-256: `a7dd9afba5c117a0d628fc50a07acd2ae0b62ddbecef1ed47a4c8e98081120cb`

The artifact preview was inspected through `artifactVersionDetail`. It
preserves separate brand and model fields for all five assets, every warranty,
photo, receipt, duplicate relation, recall distinction, existing and approved
repair, final status value, amount, date, count, status, and `asset://` locator.

## Acceptance

| Criterion | Result |
| --- | --- |
| Discover the service and propose a connector | Pass; one reviewed nine-operation proposal |
| Keep setup read-only | Pass; only the documentation page and definition tools were used |
| Read all source records once in order | Pass; seven reads ran once in the specified order |
| Preserve five assets and their evidence | Pass; explicit brand, model, serial, purchase, warranty, receipt, and photo fields are present |
| Deduplicate receipts | Pass; `receipt-002-copy` points to `receipt-002` and is not counted twice |
| Match warranties and serial photos | Pass; all five matches are preserved; `warranty-004` is expired |
| Flag exact recalls only | Pass; `recall-001` is exact and `recall-002` is near-match only |
| Record the scoped repair receipt | Pass; one separately approved synthetic POST returned one receipt and count 1 |
| Verify final status | Pass; one final GET confirmed all required invariant values |
| Save one complete sourced brief | Pass; one reviewed 10,483-byte Markdown artifact with preview |
| Avoid real-world action | Pass; no product, claim, recall, manufacturer, payment, or shared-workspace action occurred |

The temporary fixture and tunnel were stopped after evidence collection. The
temporary host mapping was removed.
