# PA-052 notices-read exact-operation retest

Use the active synthetic deceased-account API connector.

Call exactly one connector operation: `list_notices`.

Do not call `get_notice_acknowledgement`, even with an empty identifier. Do not
call any other connector operation. Do not submit a notice or request a
document.

Return the connector result. It must contain an empty `notices` array and two
remaining-obligation summaries. Identify the joint account exclusion and the
tax review due on 2026-09-30.
