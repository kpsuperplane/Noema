# PA-072 measured ledger

Measurements came from the live Go Noema database and the clean synthetic
fixture process. Connector calls are counted independently from planning, Lua,
task-file, and review calls.

## Connector setup

| Item | Value |
| --- | --- |
| Setup task | `task:036d90bfaea601039a00967d3c0f7d01` |
| Planner | `run:ae0e94472c69d4a0f0fd86c1df5addf3` |
| Executor | `run:c980c19953d1df628b6e4fe905cecd9f` |
| Reviewer | `run:7c7e2be4c5c4b6952593b64969ec521a` |
| Pending proposal digest | `6a657241822d4ebe2abe73103dddedc3882752e880d0216fb185994ac6c31b1` |
| Accepted definition digest | `d151fc401c0788937e55fc12c1815724d3f2c0720936d5b0b45d54b174a41c74` |
| Definition | `definition:synthetic_returns_api_fixed2` (`v5`) |
| Connection | `7ae976b1253ad9b9a9419e79a094cfb8` (`personal-7ae976b1`) |
| Connection revision / policy revision | `2 / 2` |
| Policies | `allow_automatically` reads; `always_ask` writes |

The accepted manifest contains eight operations, seven automatic reads, and
one approval-gated POST. It contains the generated `as_boolean` helper for
boolean fields. Setup called only the documentation, template, and proposal
paths. It did not call a fixture `/v1` route.

## Clean execution runs

| Run | Kind | Status | Tool calls |
| --- | --- | --- | ---: |
| `run:05a202459474d54ea7a74db758e8bf49` | planner | completed | 2 |
| `run:d667795a923290bdf45b6dd26c52f4b2` | executor, before approval | completed | 9 |
| `run:a960cf936f1f03f05694a5a7bca592ff` | executor, after approval | completed | 5 |
| `run:4650337f73e8c84c995d11b90fa8878a` | reviewer, first pass | completed | 1 |
| `run:0d7daf1c2f2d9b7e43b77f13263a0165` | executor, result correction | completed | 2 |
| `run:d108b3c5b9c3920cc2d5a641fddecb7b` | reviewer, final pass | completed | 1 |

The pre-approval executor made six connector reads, one Lua check, and one
task-file write before opening the approval gate. The approved continuation
made one POST, one status read, and two task-file writes. The final reviewer
approved the result. No connector read or write was repeated.

## Connector call sequence

The clean fixture process recorded exactly these eight acceptance calls. A
health probe appears first in the process output but is not a case operation.

| # | Operation | Method and route | Provider call ID |
| ---: | --- | --- | --- |
| 1 | `get_return_profile` | GET `/v1/profile` | `call_fG9WK5hYVWhqbssjE2qz0pM2` |
| 2 | `list_purchases` | GET `/v1/purchases` | `call_EddQDGhZcq0COz1vFf8RAT3N` |
| 3 | `get_return_policy` | GET `/v1/return-policy` | `call_hm3jdNbYPVcO6ahPN5S1f1ZX` |
| 4 | `list_purchase_receipts` | GET `/v1/purchase-receipts` | `call_gudVRaBZlIfCTvMaTfCdIkGc` |
| 5 | `list_shipping_options` | GET `/v1/shipping-options` | `call_3pw5weOn6E61ttAlen4zwfI3` |
| 6 | `list_return_history` | GET `/v1/return-history` | `call_bFMq3vAWmjawTubshu9nSOhu` |
| 7 | `submit_return` | POST `/v1/returns` | `call_fEONBFbMyC8ERT43ofT9Ou5f` |
| 8 | `get_return_refund_status` | GET `/v1/refund-status` | `call_5sO59qd2eCpeqcz50XpYASoZ` |

The POST body matched all seven expected fields by name. The fixture returned
`submission_count: 1`. The status read returned the same return ID and charge,
an expected refund of 80 USD, and `refund_status:
expected_after_synthetic_inspection`.

## Lua validation

The pre-write `code.run_luau` call was
`call_KPmdAJEF6QPtLDsXf1PO5zpq`. Its returned value was:

```json
{
  "all_ok": true,
  "checks": {
    "amount_consistent": true,
    "case_match": true,
    "current_date_match": true,
    "deadline_valid": true,
    "eligible_and_not_final_sale": true,
    "exact_approval_note": true,
    "faulty_condition": true,
    "no_prior_return": true,
    "prepaid_zero_cost": true,
    "reason_allowed": true,
    "receipt_identity_and_charge": true,
    "refund_basis_item_charge": true,
    "reject_other_identity": true,
    "reject_other_option": true,
    "reject_other_purchase": true,
    "shipping_required_and_eligible": true,
    "stable_identity_match": true,
    "target_name_match": true
  }
}
```

## Corrections

The first execution attempt used a connector whose response transform dropped
the `false` value for `final_sale`. The generated Lua expression used Lua's
`and/or` idiom, which converts a valid `false` into `nil`. The Go adapter now
uses an explicit boolean helper, and
`TestGeneratedResponsePreservesFalseBooleanValues` covers the regression.

The clean retry used the corrected manifest. The fixture's first POST check
also compared serialized JSON property order. It was corrected to compare
object fields by name before the clean retry. The final direct ledger above
was captured from the restarted, empty fixture process.
