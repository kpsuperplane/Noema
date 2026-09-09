# PA-052 verify requested certificate availability

Use the active synthetic deceased-account API connector.

Call exactly one connector operation: `get_document` with
`document_id=death-certificate-001`.

Do not call any other connector operation. Do not request the certificate
again. Do not submit a notice.

The result must show `status=available`, `available=true`,
`request_count=1`, and `receipt_id` is not required by the bounded connector
contract. Report the returned document status and count.
