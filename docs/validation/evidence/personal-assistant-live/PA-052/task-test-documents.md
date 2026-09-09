# PA-052 document-read mapping test

Use the active synthetic deceased-account connector to read the required
documents once, then read the missing death-certificate record once with
`document_id=death-certificate-001`.

Return the executor-proof and death-certificate records with their identifiers,
kinds, statuses, and availability. Do not request a document or call any other
operation.
