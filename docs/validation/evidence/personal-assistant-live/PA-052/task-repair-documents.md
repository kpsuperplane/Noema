# PA-052 document mapping repair

Repair only `list_documents` and `get_document` in the active
`synthetic_deceased_account_notice_api` connector. The prior v4 document
mapping failed because it used `document_type` and a wire-level `available`
boolean, while the fixture returns `document_kind` and a status string.

Call `adapter.definition_template` first with active digest
`d8e9be332d700acb025670a49bce790f2840356285c96353f9d564cc13a9bfea`.
Then propose only these two changed operations as revision `v5`, retaining the
existing source reference and all other operations.

Use the exact fixture shapes:

- `GET /v1/documents` returns `{documents:[{document_id, document_kind, status, required_for, source_locator}]}`. Return up to two items with `document_id`, `document_kind`, `status`, and boolean `available`. Derive `available=true` only when status is `available` or `verified`; otherwise set it explicitly to false. Omit required-for and source locator.
- `GET /v1/documents/{document_id}` returns a flat object with `document_id`, `document_kind`, `status`, `required_for`, `request_count`, and nullable `receipt_id`. Return document ID, kind, status, integer request count, and status-derived `available`. Do not require or expose the nullable receipt ID, required-for text, or source locator.

Use compact bounded schemas below 32,768 bytes. Preserve false values with
explicit Luau branches. Both operations remain unauthenticated GET reads,
idempotent, non-destructive, closed-world, and transport-safe retry.

Do not call an operational endpoint or activate the proposal. Leave it
`review_required`, and report the exact pending digest plus the prior failed
read evidence.
