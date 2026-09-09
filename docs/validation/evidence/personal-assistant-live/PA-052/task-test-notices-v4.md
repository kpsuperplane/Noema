# PA-052 notices-read nullable summary retest

Use the active synthetic deceased-account API connector.

Call exactly one connector operation: `list_notices`.

Do not call any other connector operation. Do not submit a notice or request a
document.

Return the connector result. It must contain an empty `notices` array and two
remaining-obligation summaries. The joint-account summary must include
`account_id=joint-checking-001`. The tax-review summary must include
`status=remaining`, `due_on=2026-09-30`, and its description, but must not
contain an internal userdata value for its null account ID.
