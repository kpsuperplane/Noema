# PA-052 repair estate-notice operation with valid proposal shapes

Repair only `submit_estate_notice` in the active synthetic deceased-account
API connector.

Use `adapter.definition_template` with semantic digest
`866c7dc554b688b4239edcd591d3133b84a980c05313a7d2210026558d8c42e5`, then
propose definition revision `v10` from that exact digest. Do not call the
fixture or change any other operation.

Use these exact proposal values. In the operation object, `authorization` is
the object `{ kind: "none" }`, not a string. `retry` is the string `"never"`.
Every `json_body_template` value is an object such as
`{ "$argument": "account_id" }`, not a string.

Arguments and JSON body keys must be exactly:

- `account_id`: required JSON-body string
- `document_id`: required JSON-body string, enum `death-certificate-001`
- `executor_proof_id`: required JSON-body string
- `notice_kind`: required JSON-body string, enum `estate_notice`
- `note`: required JSON-body string

The operation is POST `/v1/notices`, unauthenticated, read-only false,
idempotent true, destructive true, open-world false, retry `never`, and
pagination `none`. Its description must say it is an approval-gated,
synthetic-only, one-at-a-time notice for individually held accounts, and never
closes or transfers assets.

Use an explicit `json.object()` response transform. Require exactly these
bounded response fields with their actual types: string `notice_id`, string
`receipt_id`, string `account_id`, string `status`, and integer
`submission_count`. Use no integer minimum or maximum constraints.
