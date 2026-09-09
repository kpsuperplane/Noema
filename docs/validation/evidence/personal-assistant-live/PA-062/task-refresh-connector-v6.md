# PA-062 refresh the caregiver-schedule connector origin

The active reviewed caregiver-schedule definition is
`e3a1ff2f8aea5014b973faee48ee1417673148b44a143d7b2d0f08fb09be69b5` at v3.
Create one linked v4 revision for the live synthetic service at
`https://888e28d58ce564.lhr.life/`.

Call `adapter.definition_template` first with that exact digest and
`operation_ids:["get_caregiver_schedule_profile"]`. Then call
`adapter.propose_definition` exactly once with the exact base digest,
`revision:{"definition_revision":"v4","origin":"https://888e28d58ce564.lhr.life/"}`
and `source_reference:"https://888e28d58ce564.lhr.life/docs"`.

The proposal must contain one complete upsert for
`get_caregiver_schedule_profile` and no `new_definition`, `retry`, or
`retry_policy`. The template output may mask the authorization value as the
ordinary text `[REDACTED]`. Do not copy that text. Set the proposal's
`authorization` field to the literal JSON object `{"kind":"none"}`.

Use a generated `response` with `kind:"flat_object"` and a `fields` array.
Each array element must contain `name`, `type`, `required:true`,
`source_pointer`, and `max_bytes` for strings. Include `case_id` string 64 from
`/case_id`, `person_label` string 96 from `/person_label`, `current_date`
string 10 from `/current_date`, `schedule_window` string 64 from
`/schedule_window`, `goal` string 256 from `/goal`, `workspace_scope` string
96 from `/workspace_scope`, `decision_boundary` string 256 from
`/decision_boundary`, and `source_locator` string 128 from `/source_locator`.
Use GET `/v1/profile`, no arguments, pagination none, and read-only true,
idempotent true, destructive false, open-world false, with a non-empty
description.

The revision must retain the other nine operations from the base unchanged.
Do not call a service endpoint, browse the web, send a notice, submit a
schedule, reassign a shift, or create an artifact. Leave the proposal in
`review_required` for human review.

## Success conditions

- The proposal links to the exact base digest and retains exactly ten
  operations.
- The origin and source reference use the live replacement tunnel.
- The profile operation has a valid generated flat-object recipe and a
  literal no-auth object, not the masked text.
- No service endpoint or state-changing operation runs during setup.
