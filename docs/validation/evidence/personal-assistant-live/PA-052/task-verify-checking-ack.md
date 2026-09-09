# PA-052 verify checking notice acknowledgement

Use the active synthetic deceased-account API connector.

Call exactly one connector operation: `get_notice_acknowledgement` with
`notice_id=notice-001`.

Do not call any other connector operation. Do not submit or repeat a notice.

The result must show the checking notice as acknowledged, with status
`acknowledged`, account ID `checking-001`, document ID
`death-certificate-001`, notice kind `estate_notice`, and submission count 1.
Return the remaining-obligation summaries without exposing an internal value
for the null tax-review account ID.
