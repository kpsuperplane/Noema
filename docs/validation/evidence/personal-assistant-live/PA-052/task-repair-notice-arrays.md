# PA-052 notice-array transform repair

Repair only `list_notices` and `get_notice_acknowledgement` in the active
`synthetic_deceased_account_notice_api` connector. The v6 reads still return
`response_transform_failed` because their Luau transforms build plain Lua
tables. The runtime requires `json.array()` for arrays and `json.object()` for
objects.

Call `adapter.definition_template` first with active digest
`12ec375baa1c1af1762953104b158cd5c73e8fe9b2604a83455fe7fc696acbd6`.
Then propose only these two changed operations as revision `v7`, retaining the
source reference and every other operation.

Keep the v6 wire mappings and schemas. Rewrite the transforms with explicit
`json.object()` and `json.array()` values:

- `GET /v1/notices` reads root `notices` and `remaining_obligations`. Build a
  JSON array of notice objects with `notice_id`, `receipt_id`, `account_id`,
  `document_id`, `notice_kind`, `status`, and integer `submission_count`.
  Build a JSON array of bounded summary strings from obligation objects. The
  summary must include every present field and `account_id=` even when the
  source account ID is null.
- `GET /v1/notices/{notice_id}` reads notice fields at the response root.
  Build one JSON notice object with the v6 bounded fields, including
  `acknowledged`, and a JSON array of the same obligation summary strings.
  Preserve either boolean value with an explicit branch. Keep the existing
  path argument.

Use syntactically valid Luau with separators between declarations and
conditionals. Keep both operations unauthenticated GET reads,
idempotent, non-destructive, closed-world, and transport-safe retry. Keep
response bounds compact and below 32,768 bytes. Do not call an operational
endpoint or activate the proposal. Leave it `review_required`, report the
exact pending digest, and record the plain-table transform failure.
