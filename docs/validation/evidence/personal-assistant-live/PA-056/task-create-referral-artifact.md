# PA-056 save the synthetic referral plan

Create exactly one local Markdown artifact with `artifact.create_local_file`.
Use only the verified synthetic facts below. Do not call the referral
connector, make another external request, or create another booking or
transfer.

The artifact must be a concise referral-coordination plan. Separate returned
facts, selected constraints, approval records, receipts, and follow-up work.
Preserve every source locator and every identifier. State that all activity is
synthetic. Do not diagnose, recommend treatment, contact a provider, move
money, or claim a real appointment.

Verified facts:

- Fixture: `2026-09-09-referral-api-v1`.
- Case: `referral-001`; patient label `Jordan Lee (synthetic)`; current date
  `2026-09-09`.
- Goal: arrange a feasible synthetic specialist referral without clinical
  decisions.
- Decision boundary: no diagnosis, treatment selection, real provider
  contact, real appointment, or payment.
- Referral order: `referral-order-001`, Sleep medicine (synthetic), ordered
  `2026-09-05`, expires `2026-10-05`, source
  `referral://orders/referral-order-001`.
- Plan: Harbor Health PPO (synthetic); maximum travel 60 minutes; maximum
  transport cost $60; arrival buffer 30 minutes; destination scope
  `referral-order,recent-tests,medication-list`; source
  `referral://preferences/referral-001`.
- Prerequisites: `prerequisite-001` complete; `prerequisite-002` Recent sleep
  diary upload missing and booking required, due `2026-09-15`; `prerequisite-003`
  complete. Preserve each `referral://prerequisites/...` locator.
- Record bundle: `record-bundle-001`, ready, with the three documented records
  and their `referral://records/...` locators.
- Selected provider: `provider-001`, Northstar Sleep Clinic (synthetic),
  in-network, accepting referrals, ground-floor entrance, source
  `referral://network/provider-001`.
- Selected slot: `slot-003`, `2026-09-18` at `14:00` in
  `America/Los_Angeles`, 45-minute travel, source
  `referral://slots/slot-003`.
- Selected transport: `transport-002`, Harbor Ride (synthetic), available,
  50-minute travel, $42 cost, 30-minute arrival buffer, source
  `referral://transport/transport-002`.
- Approved prerequisite action: `action:007e356f61d8d5175b441431564c111c`,
  revision 1, booking `prerequisite-booking-001`, receipt
  `prerequisite-receipt-001`, status `scheduled`.
- Approved transfer action: `action:444a9dc472a20e63673b9b8ceb17226a`,
  revision 1, transfer `record-transfer-001`, receipt
  `transfer-receipt-001`, status `submitted`.
- Approved appointment action: `action:afc4927a688f50372d545feca878fcca`,
  revision 1, booking `appointment-booking-001`, receipt
  `appointment-receipt-001`, status `booked`.
- Approved transport action: `action:149343f647d03dc2bace41cae21044a6`,
  revision 1, booking `transport-booking-001`, receipt
  `transport-receipt-001`, status `booked`.
- Final status locator: `referral://status/referral-001`; next follow-up date
  `2026-09-26`; final statuses are scheduled, submitted, booked, and booked.
- Fixture request counts are one for each of the four write routes. The final
  follow-up read returned all four receipts and selected IDs.

Return the artifact ID, version ID, byte count, and content digest. If the
artifact service does not return a digest, state that it was unavailable.

## Success conditions

- Exactly one local Markdown artifact is created.
- The artifact includes all verified selections, constraints, approval IDs,
  revision numbers, receipts, statuses, request counts, identifiers, dates,
  and source locators above.
- The artifact clearly marks the synthetic-only boundary and contains no
  diagnosis, treatment, provider contact, payment, or real-world booking.
- No connector call or external state change occurs during artifact creation.
