# PA-062 move the caregiver-schedule connector to the live tunnel

The active reviewed caregiver-schedule definition is
`8d12d454e053b7d99f6ba9152329cfcaeb4310ee9ba94c7e8adc5445d25b9d85` at
revision `v2`. Its tunnel has expired again. Create one linked replacement
revision for the live synthetic service. Do not create a new definition.

First call `adapter.definition_template` with that exact digest and operation
ID `get_caregiver_schedule_profile`. Then call `adapter.propose_definition`
exactly once with `base_semantic_digest` set to that digest,
`revision.definition_revision` set to `v3`,
`revision.origin` set exactly to
`https://f51fb252964a20.lhr.life/` (including the trailing slash), and
`source_reference` set exactly to
`https://f51fb252964a20.lhr.life/docs`.

The proposal must include one complete upsert for
`get_caregiver_schedule_profile`. Use a generated
`response.kind="flat_object"` recipe, not the compiled custom response from
the template. Use `authorization:{"kind":"none"}`, GET `/v1/profile`, no
arguments, `pagination:{"kind":"none"}`, read-only true, idempotent true,
destructive false, open-world false, and a non-empty description. Its
`response.fields` must be an array of objects with `name`, `type`,
`required:true`, `source_pointer`, and `max_bytes` for strings. Include
`case_id` string 64 from `/case_id`, `person_label` string 96 from
`/person_label`, `current_date` string 10 from `/current_date`,
`schedule_window` string 64 from `/schedule_window`, `goal` string 256 from
`/goal`, `workspace_scope` string 96 from `/workspace_scope`,
`decision_boundary` string 256 from `/decision_boundary`, and
`source_locator` string 128 from `/source_locator`.

Retain all other nine operations from the base unchanged. Do not include
`new_definition`, `retry`, or `retry_policy`. Do not call any service
endpoint, browse the web, send a notice, submit a schedule, reassign a shift,
or create an artifact. Leave the proposal in `review_required` for human
review.

## Success conditions

- The proposal is linked to the exact v2 digest and retains exactly ten
  operations.
- The active candidate uses the live f51 tunnel and source reference.
- The profile recipe is generated and structurally valid.
- No service endpoint or state-changing operation runs during setup.
