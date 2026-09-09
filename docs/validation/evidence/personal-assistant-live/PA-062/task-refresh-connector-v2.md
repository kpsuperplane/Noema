# PA-062 refresh the caregiver-schedule connector origin

The first reviewed connector used an expired temporary tunnel. Create a
replacement revision of that exact definition so its one personal connection
uses the currently live synthetic service. Do not create an unlinked
definition.

First call `adapter.definition_template` with semantic digest
`8a9df7ba250cfdfd926cd5ca1c9717d81a3f1c7357a0f674337530140c93e91b` and
operation ID `get_caregiver_schedule_profile`. The returned base has ten
operations. Then call `adapter.propose_definition` exactly once as a revision:

- `base_semantic_digest` is exactly the digest above.
- `revision.definition_revision` is `v2`.
- `revision.origin` is exactly `https://ca156b7dfbf630.lhr.life/`, including
  the trailing slash.
- `source_reference` is exactly `https://ca156b7dfbf630.lhr.life/docs`.
- `upsert_operations` contains exactly one complete replacement for
  `get_caregiver_schedule_profile`.

Do not include `new_definition`. Do not copy the compiled custom response from
`definition_template`. The replacement operation must use the generated
response recipe shape `response.kind="flat_object"` with these required
fields and limits: `case_id` string 64, `person_label` string 96,
`current_date` string 10, `schedule_window` string 64, `goal` string 256,
`workspace_scope` string 96, `decision_boundary` string 256, and
`source_locator` string 128. Each field needs its matching source pointer.
Use GET `/v1/profile`, no arguments, authentication `{kind:"none"}`,
`pagination:{kind:"none"}`, read-only true, idempotent true, destructive
false, open-world false, and a non-empty operation description. Do not include
`retry` or `retry_policy`; Noema derives retry behavior.

The revision must retain the other nine operations, including their generated
response recipes, JSON body templates, safety flags, pagination, and
authorization. Do not call any service endpoint, send a notice, submit a
schedule, reassign a shift, browse the web, or create an artifact. Leave the
proposal in `review_required` state for human review.

## Success conditions

- The proposal replaces the exact base digest and keeps exactly ten
  operations.
- The active candidate uses the live origin and source reference above.
- The profile operation has the generated flat response fields and bounds
  above, while all other operation details remain unchanged.
- No service endpoint or state-changing operation runs during setup.
