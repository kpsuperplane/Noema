# PA-053 read the initial synthetic medical records

Use the active `Synthetic medical-record API` connection. Read the baseline
profile, provider list, available-provider records, and access status.

Use exactly these four read operations, once each, in this order:

1. `get_medical_profile`
2. `list_medical_sources`
3. `list_medical_records`
4. `get_medical_access_status`

Do not call `request_medical_source_access` or
`list_recovered_medical_records`. Do not call any undocumented operation.
Do not diagnose, recommend treatment, change medication, contact a provider,
or infer a condition. This is a synthetic fixture only.

Return a concise report that preserves the values from the connector. Include:

- all profile fields;
- all four providers, including the unavailable provider;
- all six available-provider records;
- the duplicate CBC records with their shared `canonical_key`;
- the superseded `allergy-001` record and current corrected `allergy-002` record;
- access status for `provider-d-001`, including `available:false` and
  `request_count:0`.

## Success conditions

- All four named operations complete successfully through the active API
  connection.
- The report contains four providers and six records.
- The report preserves the duplicate, superseded, current, unavailable, false,
  and zero values listed above.
- No state-changing endpoint is called.
- No medical diagnosis or treatment advice appears.
