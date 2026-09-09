# PA-049 — Identity-document renewal

Verdict: **Pass after response-contract repairs and policy setup**

Date: 2026-09-09

Backend: live Go Noema development instance through
`/tmp/noema-codex/graphql.sock`, server build revision `3231017e`.

## Fixture

- Service: synthetic identity-renewal API, fixture
  `2026-09-09-identity-renewal-api-v1`.
- Documentation: `https://boards-picnic-carrying-city.trycloudflare.com/docs`.
- Fixture source: [`scripts/acceptance/run-mock-identity-renewal-api.ts`](../../../../../scripts/acceptance/run-mock-identity-renewal-api.ts).
- Conversation: `conversation:a39407c5e9686f34225273862829bf0f`.
- Connection: `fd5ac8b1c5c9411d5fdf4a1883945980`.
- The fixture is synthetic. It has no government portal, real appointment,
  payment, identity-document, or fee endpoint.

The household profile is `household-001`. The trip runs from December 20 to
December 28, 2026, and documents must remain valid through June 28, 2027.
Jordan Lee's passport expires December 15, 2026. Riley Lee's passport expires
April 30, 2028.

## Connection setup and repairs

Setup task `task:ae9cf1acbc079b2684407db314465483` read the documentation and
submitted the requested seven-operation unauthenticated proposal. Its first
retry was rejected by the computed response-size limit because the document
projection could serialize 47,462 bytes. The follow-up bounded the two-record
list and tight field limits without calling a service endpoint.

The bounded proposal was accepted as reviewed v2. Its first live read showed
three mapping defects: the profile used non-existent field names, renewal
requirements were treated as strings although the endpoint returns objects,
and appointment slots used `slots` although the endpoint returns
`appointment_slots`.

Noema proposed and the operator accepted two focused revisions:

| Revision | Pending digest | Accepted digest | Change |
| --- | --- | --- | --- |
| v3 | `2a94433102a13ce395b6f1ffd12480ebdbfb32d3fd1ee33bee467c37ce5a7157` | `ffc84bcff624565e21a072d64b24b34e9eaa4bc22a28dc39be1026454d0c028c` | Correct profile field names and the appointment-slot wrapper. |
| v4 | `36c63e559c9694a44b05b64183ccdcc2e58caff3093f24e608a26c24284c2f49` | `f7a996946f1286cdcd611c1c44dc71158702b2b908b71265ba1f85be273914ac` | Project requirement objects from `/requirements`. |

The active connection is ready at connection revision 4 and policy revision
2. It has seven available tools, automatic data sharing, `always_ask` for
unsafe actions, and no credentials because the fixture is unauthenticated.

## Execution

1. The corrected read turn `turn:4207e2c918a36bf5df7dc8e3f99b5c89` fetched
   the profile, both documents, all three requirements, and all three slots.
   It identified Jordan as the only renewal risk, rejected Jordan's late slot
   because its estimated ready date was after the trip, selected
   `slot-jordan-fast`, and drafted the packet. No write occurred in this turn.
2. The assistant asked for approval before booking. The governed action
   `action:4c46df859a8372eeb65e4941f4022bcf` contained exactly
   `person-jordan`, `doc-jordan-passport`, and `slot-jordan-fast`. The operator
   approved revision 1. The synthetic service returned appointment ID
   `appointment-jordan-001`, receipt `appointment-receipt-001`, appointment
   date `2026-09-20`, 45 processing days, and estimated ready date
   `2026-11-04`.
3. The assistant asked for approval before submitting the packet. The governed
   action `action:05b26c54048588efd6ed17abbe05ef25` contained the matching
   appointment and document IDs and the note that the packet contains the
   current document, photo, and renewal form. The operator approved revision 1.
   The synthetic service returned renewal ID `renewal-jordan-001`, receipt
   `renewal-receipt-001`, status `submitted`, and submission count `1`.
4. Verification turn `turn:fe9a317dab5282a6cd4537f2baff1148` read the renewal
   status. The service returned status `approved`, the same receipt, and
   submission count `1`. Noema saved the final sourced review as artifact
   `artifact:c5359c847d5f4bf9a019d3dddebe6958`, version
   `artifact_version:5bbe989852321165cc8dde61b77b0620`, 2,002 bytes, with
   SHA-256
   `3d871f44f397aa7d37e2606e5fe4c5b2869a721f51773bff448623879986c93f`.

## Independent fixture checks

Direct reads returned the documented household profile, two documents, three
requirements, three appointment slots, and the later approved renewal. The
fixture log records documentation reads, the four read operations before the
write, exactly one POST to `/v1/appointments`, exactly one POST to
`/v1/renewals`, and the later renewal-status read. The two POST bodies used
matching Jordan, document, slot, appointment, and packet-note values. The
fixture reported submission count `1`.

No real authority was contacted. No real appointment was booked. No fee was
paid, and no real identity document was transmitted.

## Acceptance

| Criterion | Result |
| --- | --- |
| Read the household context and both family documents | Pass — the profile, trip window, validity boundary, and both document records were returned through the connector. |
| Identify the person whose document fails the validity boundary | Pass — Jordan's December 15 expiry is before the June 28, 2027 requirement; Riley remains valid. |
| Explain timing risk and choose a feasible slot | Pass — the November slot would be ready January 19, 2027; the September 20 slot is estimated ready November 4, 2026. |
| Assemble the required renewal packet | Pass — the saved review includes the current passport, recent photo, and renewal form. |
| Ask before each state-changing mock operation | Pass — both booking and submission were held for exact governed-action approval. |
| Book and submit once with matching records | Pass — one booking and one submission returned distinct receipts and submission count `1`. |
| Verify later status and preserve receipts | Pass — the later read returned `approved` with both receipt IDs and count `1`. |
| Keep real government services, documents, appointments, and fees out of scope | Pass — every call targeted the synthetic fixture, which exposes no real-world route. |
| Save a sourced final review | Pass — the Markdown artifact records dates, IDs, receipts, status, risk, and connector source operations. |
