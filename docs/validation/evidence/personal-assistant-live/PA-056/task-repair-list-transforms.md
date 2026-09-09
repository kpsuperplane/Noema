# PA-056 repair referral list response transforms

Repair the active `Synthetic referral-coordination API` definition after its
first read task rejected valid prerequisite JSON. The generated list transforms
used `and ... or nil` for boolean fields. That expression drops a legitimate
`false` value before the reviewed output contract checks it.

Use the exact active reviewed base semantic digest
`5fac7709e898144b71dd7cc52635d8fb494ef4efa67f403485fbf340712ced39`.
Call `adapter.definition_template` first with that digest and only the four
affected list operations. Do not call a service endpoint.

Submit exactly one replacement revision through `adapter.propose_definition`:

- `source_reference` is
  `https://crops-titled-gear-claims.trycloudflare.com/docs`.
- `base_semantic_digest` is the exact digest above.
- `revision.definition_revision` is `v2`.
- `upsert_operations` contains exactly these four complete replacement
  operations and no others: `list_referral_prerequisites`,
  `list_referral_network`, `list_referral_slots`, and
  `list_referral_transport_options`.
- Preserve each operation's GET path, no arguments, unauthenticated access,
  description, pagination, output name, source pointer, item limit, field
  names, types, byte bounds, required fields, and behavior flags from v1.
- Keep each operation as an `object_list` response with a strict object output
  schema and no additional properties.
- Rewrite each Luau transform with `json.object()` for each item and
  `json.array()` for the output list. Copy string, integer, and number fields
  only after checking their type and bound. Copy boolean fields with an
  explicit conditional that assigns both `true` and `false`; never use an
  `and value or nil` expression for a boolean.
- For `/v1/prerequisites`, preserve `required` and `booking_required`.
- For `/v1/network`, preserve `in_network` and
  `accepting_new_referrals`.
- For `/v1/slots`, preserve `in_network`.
- For `/v1/transport-options`, preserve all numeric and availability fields.

Use `code.run_luau` before proposing to validate at least the prerequisite
transform against this synthetic response. The local result must retain
`booking_required:false` on the first item and `booking_required:true` on the
second item, together with all required strings and dates. Also validate that
the network transform retains `in_network:false` and that the slot transform
retains a false `in_network` when present. Use explicit branches in these
checks.

Call `adapter.propose_definition` once only. It must return `review_required`.
Do not approve the revision in this task and do not invoke any connector
operation.

## Success conditions

- The exact v1 digest is loaded before the revision.
- Four and only four list operations are replaced.
- The transforms preserve explicit false booleans and all required list fields.
- The generated output schemas stay within 32,768 bytes and match the
  transforms.
- One v2 proposal is returned for human review.
- No service route or state-changing request is made.
