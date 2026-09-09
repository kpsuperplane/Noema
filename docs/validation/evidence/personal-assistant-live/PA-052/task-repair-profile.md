# PA-052 profile mapping repair

Repair only operation `get_administration_profile` in the active
`synthetic_deceased_account_notice_api` connector. The prior v1 read failed
because the transform expected `administration_goal` and `executor`, while the
fixture returns a flat object with `case_id`, `account_label`, `current_date`,
`executor_name`, `goal`, and `decision_boundary`.

Call `adapter.definition_template` first with semantic digest
`d2b32d7ed0004fde5bde4580f36342e2ea68ada870544b4a841a18c29aa8c444`.
Then call `adapter.propose_definition` with that digest as the base, revision
`v2`, the existing documentation URL as `source_reference`, and only the
changed `get_administration_profile` operation. Keep it GET `/v1/profile`,
unauthenticated, read-only, idempotent, non-destructive, closed-world, and
transport-safe retry. Use a compact Luau transform and output schema for
`case_id`, `account_label`, `current_date`, `executor_name`, `goal`, and
`decision_boundary`, bounded to 256 bytes each. Preserve false values if any
boolean is used. Do not call an operational endpoint, activate the proposal,
or change another operation.

The proposal must remain `review_required` and report its exact pending
semantic digest.
