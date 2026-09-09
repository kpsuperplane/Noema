# PA-053 read the recovered synthetic specialist records

Use the active `Synthetic medical-record API` connection after the approved
access request for `provider-d-001`.

Use exactly these two read operations, once each, in this order:

1. `get_medical_access_status`
2. `list_recovered_medical_records`

Do not call the access-request operation again. Do not call any other connector
operation. Do not diagnose, recommend treatment, change medication, contact a
provider, or infer a condition. This is a synthetic fixture only.

Return a concise report that preserves the connector values. Include:

- access status for `provider-d-001`, with `available:true`,
  `status:granted`, and `request_count:1`;
- both recovered specialist records, with every returned field and source
  locator.

## Success conditions

- Both named reads complete successfully through the active API connection.
- Access status preserves true and the integer value one.
- Exactly two recovered records are reported.
- No second access request or other connector operation occurs.
- No medical diagnosis or treatment advice appears.
