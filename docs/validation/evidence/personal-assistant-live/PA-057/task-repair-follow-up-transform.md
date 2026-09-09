# PA-057 repair the care-plan follow-up response transform

Repair the active `Synthetic care-plan API` definition after its first read
rejected valid JSON. The generated list transform used `and ... or nil` for
boolean fields. That expression drops a legitimate `false` value before the
reviewed output contract checks it.

Use the exact active reviewed base semantic digest
`92593be727432f139c8d70971bd1c078d16ceb1744fc144a3b8d96353793388c`. Call
`adapter.definition_template` first with that digest and the affected
operation. Do not call a service endpoint.

Submit exactly one replacement revision through `adapter.propose_definition`:

- `source_reference` is
  `https://dental-printing-throughout-con.trycloudflare.com/docs`.
- `base_semantic_digest` is the exact digest above.
- `revision.definition_revision` is `v2`.
- `upsert_operations` contains exactly one complete replacement operation,
  `list_care_follow_ups`, and no other operation.
- Preserve its GET path `/v1/follow-ups`, no arguments, unauthenticated
  access, description, pagination, output name `follow_ups`, source pointer
  `/follow_ups`, maximum of 3, field names, primitive types, byte bounds,
  required fields, and behavior flags from v1.
- Keep an `object_list` response with a strict object output schema and no
  additional properties.
- Rewrite the Luau transform with `json.object()` for each item and
  `json.array()` for the output list. Copy each string field only after its
  type and byte bound check. Copy `required` and `booking_required` with an
  explicit conditional that assigns both `true` and `false`; never use an
  `and value or nil` expression for either boolean.

Use `code.run_luau` before proposing to validate the replacement transform
against this synthetic response:

```json
{"follow_ups":[{"follow_up_id":"F-811","name":"Required blood test (synthetic)","required":true,"status":"missing","due_on":"2026-09-12","booking_required":true,"source_locator":"care://follow-ups/F-811"},{"follow_up_id":"F-812","name":"Resting heart-rate check","required":true,"status":"complete","due_on":"2026-09-09","booking_required":false,"source_locator":"care://follow-ups/F-812"},{"follow_up_id":"F-813","name":"Medication list review (synthetic)","required":false,"status":"pending","due_on":"2026-09-20","booking_required":false,"source_locator":"care://follow-ups/F-813"}]}
```

The local result must retain `booking_required:false` on F-812 and
`required:false` on F-813, together with every required string and date. Call
`adapter.propose_definition` once only. It must return `review_required`. Do
not approve the revision in this task and do not invoke any connector
operation.

## Success conditions

- The exact v1 digest is loaded before the revision.
- One and only one list operation is replaced.
- The transform preserves explicit false booleans and every required list
  field.
- The generated output schema stays within 32,768 bytes and matches the
  transform.
- One v2 proposal is returned for human review.
- No service route or state-changing request is made.
