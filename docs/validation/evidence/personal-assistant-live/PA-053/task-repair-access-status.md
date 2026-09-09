# PA-053 repair the access-status response transform

Repair the active `Synthetic medical-record API` definition after its reviewed
`get_medical_access_status` operation failed to preserve the documented JSON
value `available:false`.

Use the exact reviewed base semantic digest
`730dfa8cd90c6c0b009a073478464adb02d1428dde65d383d5223221904b1663`.
Call `adapter.definition_template` first with that digest and only the
`get_medical_access_status` operation. Do not call a service endpoint.

Submit exactly one revision through `adapter.propose_definition`:

- `source_reference` is
  `https://referring-ciao-transportation-employ.trycloudflare.com/docs`;
- `base_semantic_digest` is the exact digest above;
- `revision.definition_revision` is `v2`;
- `upsert_operations` contains exactly one complete replacement operation with
  operation ID `get_medical_access_status`;
- preserve its GET path `/v1/access-status`, no arguments, authentication,
  pagination, description, and behavior flags;
- replace only its response transform with a `custom` JSON response that
  returns `source_id` (string max 64), `status` (string max 32),
  `request_count` (integer), and `available` (boolean), all required;
- the Luau transform must test each field with explicit conditionals. It must
  assign `available` whenever the decoded value is boolean, including false.
  Do not use an `and value or nil` expression for the boolean;
- keep `accepted_content_types` as `application/json` and set a strict object
  output schema with no additional properties.

Use `code.run_luau` before proposing to validate the transform against this
synthetic response:

```json
{"source_id":"provider-d-001","status":"unavailable","request_count":0,"available":false}
```

The local result must contain `available:false` and `request_count:0`.
Call `adapter.propose_definition` once only. It must return `review_required`.
Do not approve the revision in this task.

## Success conditions

- The exact base digest is loaded before the revision.
- The local transform check preserves false and zero.
- One v2 revision proposal is returned for human review.
- No service route is invoked.
