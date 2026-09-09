# PA-062 refresh the caregiver-schedule connector origin

The first reviewed connector used a temporary tunnel that has expired. Update
that existing definition to the currently live synthetic service. Do not make
a new unlinked definition.

Call `adapter.definition_template` first with the exact base semantic digest
`8a9df7ba250cfdfd926cd5ca1c9717d81a3f1c7357a0f674337530140c93e91b` and the
operation ID `get_caregiver_schedule_profile`. Use the returned revision base
and complete operation as the structural source. Then call
`adapter.propose_definition` exactly once with:

- `base_semantic_digest` equal to that exact digest
- `revision.definition_revision` equal to `v2`
- `revision.origin` exactly `https://ca156b7dfbf630.lhr.life/`, including the
  trailing slash
- `source_reference` exactly `https://ca156b7dfbf630.lhr.life/docs`
- one complete `upsert_operations` entry for
  `get_caregiver_schedule_profile`, copied from the returned template without
  changing its method, path, response, flags, or authorization

The new revision must retain all ten operations from the base definition. Do
not remove or add operations. Do not include a `new_definition` object. Do not
include a `retry` or `retry_policy` property. Do not call any service endpoint,
send a notice, submit a schedule, reassign a shift, browse the web, or create
another artifact. The proposal must remain `review_required` until a human
reviews it.

## Success conditions

- The proposal is a revision of the exact base digest, not a new definition.
- The origin and source reference use the live tunnel above.
- The resulting manifest still has exactly ten operations and the same safety,
  response, pagination, authorization, and body-template details.
- No service endpoint or state-changing operation runs during setup.
