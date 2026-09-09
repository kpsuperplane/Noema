# PA-056 save the complete synthetic referral record

Create exactly one local Markdown artifact with `artifact.create_local_file`.
Use only the verified results below. Do not call the referral connector, the
fixture, or any other external service. Do not create another booking,
transfer, or transport request.

Write a concise, source-backed record of the completed synthetic referral.
Preserve every listed source locator, identifier, date, boolean, number,
approval action, revision, receipt, status, and request count. Explain why the
selected provider, slot, and transport satisfy the returned constraints.
Separate returned facts from selections and follow-up. State that every action
is synthetic. Do not diagnose, recommend treatment, contact a provider, move
money, or claim a real appointment.

Verified results:

- Fixture `2026-09-09-referral-api-v1`; case `referral-001`; patient label
  `Jordan Lee (synthetic)`; current date `2026-09-09`.
- Goal: arrange a feasible synthetic specialist referral without clinical
  decisions. Boundary: no diagnosis, treatment selection, real provider
  contact, real appointment, or payment.
- Referral order `referral-order-001`; specialty `Sleep medicine (synthetic)`;
  ordered `2026-09-05`; expires `2026-10-05`; locator
  `referral://orders/referral-order-001`.
- Plan `Harbor Health PPO (synthetic)`; maximum travel `60` minutes; maximum
  transport cost `$60`; arrival buffer `30` minutes; destination scope
  `referral-order,recent-tests,medication-list`; locator
  `referral://preferences/referral-001`.
- Prerequisites: `prerequisite-001` complete, locator
  `referral://prerequisites/prerequisite-001`; `prerequisite-002` Recent sleep
  diary upload missing and booking required, due `2026-09-15`, locator
  `referral://prerequisites/prerequisite-002`; `prerequisite-003` complete,
  locator `referral://prerequisites/prerequisite-003`.
- Record bundle `record-bundle-001`, ready, destination scope as above.
  Records: `record-001` referral_order, Specialist referral order (synthetic),
  recorded `2026-09-05`, current, locator
  `referral://records/record-001`; `record-002` recent_test, Recent sleep
  diary summary (synthetic), recorded `2026-09-08`, current, locator
  `referral://records/record-002`; `record-003` medication_list, Current
  medication list (synthetic), recorded `2026-09-08`, current, locator
  `referral://records/record-003`.
- Network comparison: `provider-001` Northstar Sleep Clinic (synthetic), plan
  `Harbor Health PPO (synthetic)`, in-network `true`, accessibility
  `ground-floor entrance`, accepting new referrals `true`, address
  `100 Northstar Way (synthetic)`, locator
  `referral://network/provider-001`; `provider-002` Harbor Sleep Center
  (synthetic), in-network `false`, locator `referral://network/provider-002`;
  `provider-003` Summit Sleep Institute (synthetic), in-network `true`,
  accessibility `stairs-only`, accepting new referrals `false`, locator
  `referral://network/provider-003`.
- Selected slot `slot-003` at `provider-001`, `2026-09-18` at `14:00`
  `America/Los_Angeles`, duration `45` minutes, status `open`, in-network
  `true`, travel `45` minutes, locator `referral://slots/slot-003`.
- Selected transport `transport-002`, Harbor Ride (synthetic), travel `50`
  minutes, cost `$42`, availability `available`, arrival buffer `30` minutes,
  locator `referral://transport/transport-002`.
- Approved prerequisite action `action:007e356f61d8d5175b441431564c111c`,
  revision `1`, booking `prerequisite-booking-001`, receipt
  `prerequisite-receipt-001`, status `scheduled`.
- Approved transfer action `action:444a9dc472a20e63673b9b8ceb17226a`, revision
  `1`, transfer `record-transfer-001`, receipt `transfer-receipt-001`, status
  `submitted`.
- Approved appointment action `action:afc4927a688f50372d545feca878fcca`,
  revision `1`, booking `appointment-booking-001`, receipt
  `appointment-receipt-001`, status `booked`.
- Approved transport action `action:149343f647d03dc2bace41cae21044a6`,
  revision `1`, booking `transport-booking-001`, receipt
  `transport-receipt-001`, status `booked`.
- Final status response locator `referral://status/referral-001`; prerequisite
  status `scheduled` and receipt `prerequisite-receipt-001`; transfer status
  `submitted` and receipt `transfer-receipt-001`; appointment status `booked`
  and receipt `appointment-receipt-001`; booked slot `slot-003`; transport
  status `booked` and receipt `transport-receipt-001`; booked option
  `transport-002`; next follow-up `2026-09-26`.
- The fixture reported request count `1` for each of the four write routes.

Return the artifact ID, version ID, byte count, and content digest. If no digest
is returned, say that it was unavailable.

## Success conditions

- Exactly one local Markdown artifact is created by this task.
- The artifact preserves every full source locator, selected constraint,
  comparison fact, approval record, revision, receipt, status, identifier,
  date, boolean, number, and request count above.
- The artifact clearly states the synthetic-only boundary and contains no
  diagnosis, treatment, provider contact, payment, or real-world booking.
- No connector call or external state change occurs during creation.
