# PA-052 connector repair task

Repair the active `synthetic_deceased_account_notice_api` connector after the read-only acceptance task exposed `response_transform_failed` for every list operation.

Use `adapter.definition_template` first. Then propose a revision from the active semantic digest `d2b32d7ed0004fde5bde4580f36342e2ea68ada870544b4a841a18c29aa8c444` with definition revision `v2`. Keep the same unauthenticated origin and the same ten operation IDs. Do not call an operational endpoint and do not activate the proposal.

The fixture implementation is the wire-shape authority for this repair. The earlier documentation page described these concepts but did not list field names. Use these exact response shapes:

- `GET /v1/profile` returns a flat object with `case_id`, `account_label`, `current_date`, `executor_name`, `goal`, and `decision_boundary`.
- `GET /v1/executor-proof` returns `{proofs:[{proof_id, proof_type, holder, status, authority_scope, source_locator}]}`. Project the proof identifier, type, holder, status, and authority scope.
- `GET /v1/accounts` returns `{accounts:[{account_id, account_type, ownership, status, notice_required, notice_deadline, closure_eligible, exclusion_reason, institution_label, source_locator}]}`. Project the six records, ownership, status, notice requirement, deadline, closure eligibility, and exclusion reason. Do not project institutions or source locators.
- `GET /v1/deadlines` returns `{deadlines:[{deadline_id, due_on, scope, required_document_id, priority}]}`.
- `GET /v1/documents` returns `{documents:[{document_id, document_kind, status, required_for, source_locator}]}`. Project document identifier, kind, status, and a boolean `available` derived from status `available` or `verified`.
- `GET /v1/documents/{document_id}` returns a flat document object with `document_id`, `document_kind`, `status`, `required_for`, `request_count`, and nullable `receipt_id`. Project identifier, kind, status, request count, and `available`; do not require nullable receipt data.
- `GET /v1/notices` returns `{notices:[{notice_id, receipt_id, account_id, document_id, notice_kind, status, submission_count}], remaining_obligations:[{obligation_id, account_id, status, reason, due_on, description}]}`. Project bounded notice identifier, receipt, account, document, kind, status, and submission count. Convert each remaining obligation to a bounded summary string so null account IDs remain valid.
- `GET /v1/notices/{notice_id}` returns a flat acknowledgement object with `notice_id`, `receipt_id`, `account_id`, `account_type`, `deadline`, `document_id`, `notice_kind`, `status`, `acknowledged`, `submission_count`, `remaining_obligations`, and `next_step`. Project a bounded nested notice object and bounded obligation summary strings.
- `POST /v1/documents/request` accepts JSON `document_id`, `request_kind`, and `note`; map all three arguments. It returns `request_id`, `document_id`, `receipt_id`, `status`, and `request_count`; project receipt, document, status, and count. Keep it approval-gated and idempotent.
- `POST /v1/notices` accepts JSON `account_id`, `document_id`, `executor_proof_id`, `notice_kind`, and `note`; map all five arguments. It returns `notice_id`, `receipt_id`, `account_id`, `status`, and `submission_count`; project all five. Keep it one-at-a-time, approval-gated, idempotent, and excluded for joint accounts.

Use compact response schemas under the connector response-size limit. Use explicit branches for boolean values so `false` is preserved. Keep reads read-only/idempotent and writes non-read-only, destructive, idempotent, retry `never`, and `open_world=false`. Keep projections bounded and exclude source locators, institutions, real-world actions, account closure, transfer, and money movement.

Success requires a `review_required` proposal with an exact new semantic digest, no operational request, and a result that records the prior transform failures and the corrected wire mappings.
