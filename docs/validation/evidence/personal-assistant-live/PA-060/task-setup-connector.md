# PA-060 connect the synthetic health-appeal API

Use the current documentation at
`https://reached-brush-range-romance.trycloudflare.com/docs`. Create one
reviewed API connection. Do not invoke any service endpoint during setup.

Call `adapter.definition_template` first. Then call
`adapter.propose_definition` once with a new definition. Use the generated
response recipe format exactly: `response.kind` is `flat_object` or
`object_list`; object-list responses have `source_pointer`, `output_name`,
`max_items`, and `fields`; every string field has a `max_bytes` value. Use
`pagination:{kind:"none"}` on every operation. Every argument must include a
non-empty description.

Use authentication `{kind:"none"}` for every operation. The proposal must
contain exactly these seven operations and no others:

1. `get_health_appeal_profile`: GET `/v1/profile`; no arguments; flat required
   fields `case_id` (string 64), `person_label` (string 96), `current_date`
   (string 10), `goal` (string 256), `appeal_deadline` (string 10),
   `decision_boundary` (string 256), and `source_locator` (string 128).
2. `get_coverage_denial`: GET `/v1/denial`; no arguments; flat required fields
   `denial_id` (string 64), `claim_id` (string 64), `service_label` (string
   128), `denied_on` (string 10), `reason_code` (string 64), `reason_text`
   (string 256), `appeal_deadline` (string 10), and `source_locator` (string
   128).
3. `get_appeal_policy`: GET `/v1/policy`; no arguments; flat required fields
   `policy_id` (string 64), `plan_name` (string 128), `clause_id` (string 64),
   `clause_title` (string 128), `clause_text` (string 256),
   `required_evidence` (string 128), and `source_locator` (string 128).
4. `list_appeal_evidence`: GET `/v1/evidence`; no arguments; object-list from
   `/evidence` named `evidence`, maximum 3, with required fields `evidence_id`
   (string 64), `evidence_type` (string 64), `title` (string 128), `status`
   (string 32), `summary` (string 256), and `source_locator` (string 128).
5. `get_supporting_clinician_letter`: GET
   `/v1/evidence/clinician-letter`; no arguments; flat required fields
   `evidence_id` (string 64), `letter_id` (string 64), `author_label` (string
   96), `signed_on` (string 10), `factual_text` (string 512),
   `attached_record_id` (string 64), and `source_locator` (string 128).
6. `submit_coverage_appeal`: POST `/v1/appeals`; required JSON-body arguments
   `denial_id` (string), `policy_clause_id` (string), `evidence_ids` (array of
   strings), and `factual_summary` (string). Set read-only false, idempotent
   false, destructive false, open-world true, and retry `never`. Its response
   is a flat object with required `appeal_id` (string 64), `receipt_id`
   (string 64), `denial_id` (string 64), `policy_clause_id` (string 64),
   `status` (string 32), `submission_count` (integer), `submitted_on` (string
   10), and `source_locator` (string 128). This is the only unsafe operation
   and must remain approval-gated.
7. `get_appeal_status_and_decision`: GET
   `/v1/appeal-decision/{appeal_id}`; required path argument `appeal_id`
   (string, with a non-empty description); flat required fields `appeal_id`
   (string 64), `decision_status` (string 32), `outcome` (string 64),
   `decided_on` (string 10), `note` (string 256), `receipt_id` (string 64),
   and `source_locator` (string 128).

Set read-only true, idempotent true, destructive false, and open-world false
for operations 1–5 and 7. Preserve every denial reason, policy clause,
evidence identifier, date, status, receipt, count, factual text, and
`appeal://` locator. Do not propose insurer contact, diagnosis, treatment,
legal advice, payment, or any undocumented operation.

## Success conditions

- The proposal is accepted for review and contains exactly the seven
  operations above with valid generated response recipes.
- No computed response schema exceeds 32,768 bytes.
- No service endpoint or state-changing operation is invoked during setup.
