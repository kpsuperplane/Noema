# PA-054 repair the refill-plan response transform

Repair the active `Synthetic medication and refill API` definition after its
reviewed `get_refill_plan` operation failed to preserve the documented
`clarification_complete:false` value.

Use the exact reviewed base semantic digest
`8b54a567a98809a3c4e1589da24a54709776fe8d0b7fcc7d5c8237ce829931eb`.
Call `adapter.definition_template` first with that digest and only the
`get_refill_plan` operation. Do not call a service endpoint.

Submit exactly one revision through `adapter.propose_definition`:

- `source_reference` is
  `https://conscious-gear-transcription-member.trycloudflare.com/docs`;
- `base_semantic_digest` is the exact digest above;
- `revision.definition_revision` is `v2`;
- `upsert_operations` contains exactly one complete replacement operation with
  operation ID `get_refill_plan`;
- preserve its GET path `/v1/refill-plan`, no arguments, authentication,
  pagination, description, and behavior flags;
- replace only its response transform with a `custom` JSON response that
  returns `medication_id` (string max 64), `prescription_id` (string max 64),
  `due_on` (string max 10), `days_until_due` (integer), `refill_status`
  (string max 32), `requires_clarification` (boolean),
  `clarification_complete` (boolean), `pharmacy_label` (string max 96), and
  `source_locator` (string max 96), all required;
- the Luau transform must test each field with explicit conditionals. It must
  assign `clarification_complete` whenever the decoded value is boolean,
  including false. Do not use an `and value or nil` expression for either
  boolean;
- keep `accepted_content_types` as `application/json` and set a strict object
  output schema with no additional properties.

Use `code.run_luau` before proposing to validate the transform against this
synthetic response:

```json
{"medication_id":"med-examplestatin","prescription_id":"rx-current-001","due_on":"2026-09-14","days_until_due":5,"refill_status":"due_soon","requires_clarification":true,"clarification_complete":false,"pharmacy_label":"Harbor Pharmacy (synthetic)","source_locator":"med://pharmacy/refills/rx-current-001"}
```

The local result must contain `requires_clarification:true`,
`clarification_complete:false`, and `days_until_due:5`.
Call `adapter.propose_definition` once only. It must return `review_required`.
Do not approve the revision in this task.

## Success conditions

- The exact base digest is loaded before the revision.
- The local transform check preserves both booleans and the integer.
- One v2 revision proposal is returned for human review.
- No service route is invoked.
