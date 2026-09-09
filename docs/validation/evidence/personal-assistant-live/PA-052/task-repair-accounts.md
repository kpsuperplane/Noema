# PA-052 account mapping repair

Repair only `list_accounts` in the active
`synthetic_deceased_account_notice_api` connector. The v3 account read failed
because the transform read `deadline` instead of the fixture's
`notice_deadline`, and its schema required `deadline` and `exclusion_reason`
even though those fields are absent on individually held rows.

Call `adapter.definition_template` first with active digest
`6af754e74a7aeb97073c9843e0e4c620a2e52c5411e86a0a8f01a4c7e0c8d8f6`.
Then propose only `list_accounts` as revision `v4` with the existing source
reference. Keep the same GET path and read-only/idempotent/non-destructive,
closed-world, transport-safe behavior.

The fixture returns `{accounts:[{account_id, account_type, ownership, status,
notice_required, notice_deadline, closure_eligible, exclusion_reason,
institution_label, source_locator}]}`. Return up to six items. Require only
the fields present on every row: `account_id`, `account_type`, `ownership`,
`status`, `notice_required`, and `closure_eligible`. Include optional
`notice_deadline` and optional `exclusion_reason` only when present. Omit
institution and source locator. Preserve true and false booleans with explicit
Luau branches. Keep the response schema compact and below 32,768 bytes.

Do not call any operational endpoint or activate the proposal. Leave it
`review_required` and report the exact pending semantic digest and the prior
failure.
