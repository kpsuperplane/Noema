# PA-074–PA-100 set up the provider-neutral case workflow API

Create one reviewed API connector for the synthetic provider-neutral workflow
service. This connector is the shared test foundation for PA-074 through
PA-100. It cannot contact a travel provider, school, household, account,
recipient, device, or other real service. It cannot move money or release
credentials.

## Source and order

- adapter ID: `synthetic_case_workflow_api_v1`
- definition ID: `definition:synthetic_case_workflow_api_v1`
- display name: `Synthetic case workflow API`
- definition revision: `v1`
- origin: `https://hood-marker-tests-firewall.trycloudflare.com/`
- source reference: `https://hood-marker-tests-firewall.trycloudflare.com/docs`
- authentication: `{kind: "none"}`
- fixture version: `2026-09-10-case-workflow-api-v1`

Open the exact documentation URL first. Then call
`adapter.definition_template` once with no arguments. Then call
`adapter.propose_definition` once for this definition. Do not retry either
call. Keep the successful proposal pending for operator review. Do not call a
`/v1` route during setup. Do not switch web providers.

Every argument object must include a concise non-empty `description`, its
name, type, and required flag. Use `pagination: {kind: "none"}` everywhere.
Use generated `flat_object` responses, so do not add a custom transform or
`accepted_content_types` field.

Set the definition authentication to the exact object `{"kind":"none"}`.
Set each operation authorization to the exact object `{"kind":"none"}`.
Do not copy masked markers such as `[REDACTED]` into the proposal.
Do not include `fixture_version` in `new_definition`; it is test metadata for
the task and documentation, not a supported manifest field.
Do not add `max_bytes` to request arguments. Argument schemas do not support
that field. Keep request payloads and approval notes concise instead.

## Exact operations

Propose exactly these three operations and no others:

| Operation | Request and response |
| --- | --- |
| `get_case_context` | GET `/v1/cases/{case_id}/context`; required path argument `case_id` (string); flat fields `case_id`, `case_title`, `facts`, `constraints`, `expected_outcome`, `source_locator`. |
| `record_case_action` | POST `/v1/cases/{case_id}/actions`; required path argument `case_id` and required JSON-body arguments `action`, `payload`, `approval_note`, each bound to its same-named `$argument`; flat fields `action_id`, `case_id`, `action`, `status`, `submission_count` (integer), `source_locator`. |
| `get_case_status` | GET `/v1/cases/{case_id}/status`; required path argument `case_id` (string); flat fields `case_id`, `action_count` (integer), `last_action`, `status`, `source_locator`. |

Response string fields need a `max_bytes` bound. Use 96 bytes for IDs and
source locators, 128 bytes for titles, action names, statuses, and last-action
values, and 1024 bytes for facts, constraints, and expected outcomes. These
bounds keep each generated response below Noema's 32 KiB model-result limit
while preserving the complete synthetic case records. Mark every listed
response field as required and keep all output schemas closed. Request
payloads and approval notes are concise strings without `max_bytes` fields.

The two GETs are read-only, idempotent, non-destructive, and closed-world.
The POST is non-read-only, exact-body-idempotent, non-destructive, open-world,
and approval-gated. The compiled GET retry must be
`transport_safe_read`. The compiled POST retry must be `never`.

## Acceptance

- The docs page is opened before the one template call.
- The template is called once with no arguments.
- One proposal is called once and returns `review_required`.
- The proposal contains exactly the three operation IDs and paths above.
- Both GETs compile as automatic reads. The POST compiles with `retry: never`
  and remains approval-gated.
- Setup makes no `/v1` request, no state-changing request, and no provider
  switch. Record the pending semantic digest for operator acceptance.

## Current accepted revision

The original setup notes above are retained as the contract. The first
temporary host was retired before the clean acceptance run. On 2026-09-10,
the agent corrected the pending proposal to the current fixture host and the
operator accepted that exact proposal.

- Current fixture: `2026-09-10-case-workflow-api-v3`.
- Current origin: `https://dispatched-conscious-minimum-fat.trycloudflare.com/`.
- Current source reference:
  `https://dispatched-conscious-minimum-fat.trycloudflare.com/docs`.
- Corrected pending digest:
  `8f0f1ff998ac4a7a31b4f181db5f7bd1ee5256670ca2b9471714b25d8bf09ff9`.
- Accepted reviewed digest:
  `8c49054349ae4ab55fee5213578d69ce8e3a3d41b60296c9ee0c6c97210117b0`.
- Connection: `2d79d72190362a19d0a25ab4af1c607b`.
- Accepted revision: `v3`.

The accepted manifest contains only `get_case_context`,
`get_case_status`, and `record_case_action`. The two reads use
`transport_safe_read`. The action uses `retry: never` and remains behind the
normal human approval card. The clean direct rechecks are recorded in
[PA-078](../PA-078/), [PA-085](../PA-085/), and [PA-097](../PA-097/).
