# PA-052 notice-read mapping repair

Repair only `list_notices` and `get_notice_acknowledgement` in the active
`synthetic_deceased_account_notice_api` connector. The v5 notices read failed
because its transform returned raw obligation objects while its reviewed
contract requires strings. The acknowledgement endpoint also returns notice
fields at the response root, not under a `notice` object.

Call `adapter.definition_template` first with active digest
`1fccce3d74734d819b80521df90cda5700e857f9a206a659920abc46fc9d2b7b`.
Then propose only these two changed operations as revision `v6`, keeping the
existing source reference and all other operations unchanged.

Exact fixture shapes:

- `GET /v1/notices` returns `{notices:[{notice_id, receipt_id, account_id, document_id, notice_kind, status, submission_count}], remaining_obligations:[{obligation_id, account_id, status, reason, due_on, description}]}`. Return up to five notice items with all seven bounded fields. Return up to eight obligation summary strings. Build each summary from every present field, using an empty value for null `account_id`, so both the joint-account exclusion and tax-review obligation remain distinguishable.
- `GET /v1/notices/{notice_id}` returns root fields `notice_id`, `receipt_id`, `account_id`, `account_type`, `deadline`, `document_id`, `notice_kind`, `status`, `acknowledged`, `submission_count`, `remaining_obligations`, and `next_step`. Build a bounded nested `notice` object from the root fields and bounded obligation summary strings from the root `remaining_obligations` array. Preserve `acknowledged=false` or true with an explicit branch. Use the existing path argument `notice_id`.

Use compact custom response schemas below 32,768 bytes. Limit IDs to 64
bytes, statuses and kinds to 32, notes/next-step text to 256, and summary
strings to 256. Keep both operations unauthenticated GET reads,
idempotent, non-destructive, closed-world, and transport-safe retry. Do not
call an operational endpoint or activate the proposal. Leave it
`review_required` and report the exact pending digest plus the prior failure.
