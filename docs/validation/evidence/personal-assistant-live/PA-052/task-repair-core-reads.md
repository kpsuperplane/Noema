# PA-052 core-read mapping repair

Repair only `get_executor_proof`, `list_accounts`, and `list_deadlines` in
the active `synthetic_deceased_account_notice_api` connector. The prior v1
transforms failed because they used wrong wrappers and field names.

Call `adapter.definition_template` first with the current active digest
`3ae11f3341b13dd53a8079761dacf9c1449b3b6a92cb1df1ac2bd375e8a1e5b1`.
Then call `adapter.propose_definition` with that digest as the base, revision
`v3`, the existing documentation URL as `source_reference`, and only the
three changed operations. Keep every other operation unchanged.

Use these exact fixture wire shapes:

- `GET /v1/executor-proof` returns `{proofs:[{proof_id, proof_type, holder, status, authority_scope, source_locator}]}`. Return a bounded `proofs` array with the first five fields. Do not return the source locator.
- `GET /v1/accounts` returns `{accounts:[{account_id, account_type, ownership, status, notice_required, notice_deadline, closure_eligible, exclusion_reason, institution_label, source_locator}]}`. Return up to six bounded items with account ID, type, ownership, status, notice requirement, deadline, closure eligibility, and exclusion reason. Omit institution and source locator. Preserve both true and false booleans with explicit Luau branches.
- `GET /v1/deadlines` returns `{deadlines:[{deadline_id, due_on, scope, required_document_id, priority}]}`. Return up to two bounded items with all five fields.

All three operations remain GET, unauthenticated, read-only, idempotent,
non-destructive, closed-world, and transport-safe retry. Keep response
schemas compact and below the connector response-size limit. Do not call an
operational endpoint or activate the proposal. The proposal must remain
`review_required` and report the exact pending digest, with a result that
records the prior transform failures and corrected mappings.
