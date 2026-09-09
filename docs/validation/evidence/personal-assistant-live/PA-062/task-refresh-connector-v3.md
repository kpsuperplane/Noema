# PA-062 refresh the caregiver-schedule connector origin

The first reviewed connector used an expired temporary tunnel. Create a
replacement revision of that exact definition so its personal connection uses
the currently live synthetic service. Do not create an unlinked definition.

First call `adapter.definition_template` with semantic digest
`8a9df7ba250cfdfd926cd5ca1c9717d81a3f1c7357a0f674337530140c93e91b` and
operation ID `get_caregiver_schedule_profile`. Then call
`adapter.propose_definition` exactly once as a revision with:

- `base_semantic_digest` exactly that digest
- `revision:{"definition_revision":"v2","origin":"https://ca156b7dfbf630.lhr.life/"}`
- `source_reference` exactly `https://ca156b7dfbf630.lhr.life/docs`
- one complete upsert for `get_caregiver_schedule_profile`

Do not include `new_definition`, `retry`, or `retry_policy`. The upsert must be
a generated recipe, not the compiled custom response returned by the template.
Use this exact operation contract:

- `operation_id`: `get_caregiver_schedule_profile`
- `description`: a non-empty description
- `method`: `GET`; `path`: `/v1/profile`
- `authorization`: `{"kind":"none"}`; `arguments`: `[]`
- `pagination`: `{"kind":"none"}`
- `read_only:true`, `idempotent:true`, `destructive:false`, `open_world:false`
- `response`: `{"kind":"flat_object","fields":[...]}`

The `fields` value is an array of objects. Each object has `name`, `type`,
`required:true`, `source_pointer`, and `max_bytes` for strings. Include these
eight entries: `case_id` string 64 from `/case_id`, `person_label` string 96
from `/person_label`, `current_date` string 10 from `/current_date`,
`schedule_window` string 64 from `/schedule_window`, `goal` string 256 from
`/goal`, `workspace_scope` string 96 from `/workspace_scope`,
`decision_boundary` string 256 from `/decision_boundary`, and
`source_locator` string 128 from `/source_locator`.

The revision must retain the other nine operations from the base unchanged,
including their generated response recipes, JSON body templates, safety
flags, pagination, authorization, and operation IDs. Do not call any service
endpoint, browse the web, send a notice, submit a schedule, reassign a shift,
or create an artifact. Leave the successful proposal in `review_required`
state for human review.

## Success conditions

- The proposal links to the exact base digest and has exactly ten operations.
- The active candidate uses the live origin and source reference above.
- The profile operation uses the exact generated flat-object array recipe and
  fields above. Its authorization is an object, never a redacted string.
- No service endpoint or state-changing operation runs during setup.
