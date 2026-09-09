# PA-052 repair nullable notice-obligation fields

Repair the active synthetic deceased-account API connector from the current
definition revision and semantic digest.

Use `adapter.definition_template` first for only `list_notices` and
`get_notice_acknowledgement`. Then propose the next definition revision.

The last exact-operation read passed, but the tax-review obligation has a JSON
`account_id: null`. The current summary transform stringifies that JSON null as
an internal userdata value. Fix both transforms so `account_id` is included
only when it is a real string, number, or boolean. Keep the joint account ID,
status, exclusion reason, tax status, due date, and description intact.

Use explicit `json.array()` and `json.object()` results. Keep the existing
bounded output schemas and all operation behavior metadata unchanged. Do not
change any write operation. This proposal changes connector definition only;
do not call the fixture or create any notice.
