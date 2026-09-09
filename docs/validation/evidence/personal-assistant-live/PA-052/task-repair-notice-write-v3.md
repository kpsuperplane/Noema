# PA-052 propose the estate-notice operation

Repair only `submit_estate_notice` in the active synthetic deceased-account
API connector. Use `adapter.definition_template` with semantic digest
`866c7dc554b688b4239edcd591d3133b84a980c05313a7d2210026558d8c42e5`, then
propose revision `v10` from that exact digest. Do not call the fixture.

Important proposal syntax:

- `authorization` must be the JSON object `{ "kind": "none" }`.
- Omit `retry`; the proposal API derives it as `never` for this POST.
- `pagination` must be `{ "kind": "none" }`.
- Each `json_body_template` value must be an object, for example
  `{ "$argument": "account_id" }`, never a string.

Use operation ID `submit_estate_notice`, method POST, and path `/v1/notices`.
Declare these five required JSON-body arguments: `account_id` (string),
`document_id` (string enum `death-certificate-001`), `executor_proof_id`
(string), `notice_kind` (string enum `estate_notice`), and `note` (string).
Bind every one in the JSON body with its matching `$argument` object. Set
`read_only=false`, `idempotent=true`, `destructive=true`, and
`open_world=false`.

Describe the operation as an approval-gated, synthetic-only, one-at-a-time
notice for individually held accounts that never closes accounts or transfers
assets.

Use a custom response with an explicit `json.object()` transform. Require
exactly five fields: bounded strings `notice_id` (128), `receipt_id` (128),
`account_id` (128), and `status` (64), plus integer `submission_count`. Do not
add integer minimum or maximum constraints. Leave every other operation
unchanged and make no live request.
