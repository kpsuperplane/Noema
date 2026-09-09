# PA-052 repair estate-notice connector operation

Repair only `submit_estate_notice` in the active synthetic deceased-account
API connector.

The active definition is revision `v9` with semantic digest
`866c7dc554b688b4239edcd591d3133b84a980c05313a7d2210026558d8c42e5`. Use
`adapter.definition_template` first, then propose revision `v10` from that
exact digest. Do not call the fixture or change any read operation.

The fixture contract is POST `/v1/notices` with a JSON body containing:

- `account_id`, required string for an individually held account
- `document_id`, required fixed enum `death-certificate-001`
- `executor_proof_id`, required string for the verified executor proof
- `notice_kind`, required fixed enum `estate_notice`
- `note`, required string with a complete notice note

Use those exact argument names and include all five in `json_body_template`
with `$argument` references. Do not use `notice_type`.

Keep the operation unauthenticated, read-only false, idempotent true,
destructive true, open-world false, retry `never`, and pagination `none`.
The description must state that this is an approval-gated synthetic notice
for individually held accounts, one at a time, and never a closure or asset
transfer.

Map the 201 response into an explicit `json.object()` with required bounded
fields `notice_id`, `receipt_id`, `account_id`, `status`, and
`submission_count`. Preserve string and integer types. Keep the existing
operation ID and path.
