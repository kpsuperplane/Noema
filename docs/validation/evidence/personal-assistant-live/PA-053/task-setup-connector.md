# PA-053 connect the synthetic medical-record API

Open the API documentation at
`https://referring-ciao-transportation-employ.trycloudflare.com/docs` and
propose one API connection for that service.

This is a synthetic fixture only. Use the documented base URL. Propose the
smallest useful set of operations: profile, the combined medical-record
inventory, access status, the access request, and the post-approval fourth-provider
records read. Keep
read operations read-only. Mark only the access-request operation as an unsafe,
approval-gated, idempotent state change. Do not invent diagnosis, medication,
treatment, billing, or provider-contact operations.

Read the documentation before proposing. Include exact paths, methods,
authentication (`none`), request fields, response fields, and bounded response
schemas. The connector must preserve explicit false values and JSON nulls.

The service is synthetic and cannot diagnose, prescribe, select treatment,
contact a real provider, or expose real health data. No real account or health
record may be used.

## Success conditions

- One reviewed API connection proposal is created from the supplied
  documentation.
- The proposal contains only the documented operations and exact wire fields.
- Reads are automatic; the access request requires human approval.
- No connector call or external state change is made by this setup task.
