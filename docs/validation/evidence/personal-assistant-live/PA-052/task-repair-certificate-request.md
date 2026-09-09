# PA-052 repair certificate-request connector operation

Repair only `request_fixture_certificate` in the active synthetic
deceased-account API connector.

The active definition is revision `v8` with semantic digest
`2ddcc50eb41de54b4336bee8efe5a0e8eaafd7b7e5caded985863955a4303fd5`. Use
`adapter.definition_template` first, then propose revision `v9` from that
exact digest. Do not call the fixture and do not change read operations.

The fixture contract is POST `/v1/documents/request` with a JSON body that
requires these three fields:

- `document_id`, fixed enum `death-certificate-001`
- `request_kind`, fixed enum `obtain_fixture_certificate`
- `note`, required string with at least 12 characters

Include all three fields in `json_body_template` using `$argument` references.
Keep the operation unauthenticated, read-only false, idempotent true,
destructive true, open-world false, retry `never`, and pagination `none`.

Map the JSON response into a bounded object with required fields
`request_id`, `document_id`, `receipt_id`, `status`, and `request_count`.
Use explicit `json.object()` output and preserve string and integer types.
Keep the operation description approval-gated and synthetic-only.
