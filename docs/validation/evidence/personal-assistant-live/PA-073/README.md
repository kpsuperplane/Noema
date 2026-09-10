# PA-073 — household service

Verdict: **Pass**.

This case ran through the live Go Noema development instance. The connected
service was a synthetic household-service API. It cannot contact a cleaner,
enter a home, receive an access code, charge a card, or move money.

## Connector setup

- Source docs: `https://attorneys-laden-sheet-residence.trycloudflare.com/docs`
- Fixture: `2026-09-10-household-service-api-v1`
- Accepted definition: `definition:synthetic_household_service_api_v1_final`
- Accepted source digest: `b5df1eaf8120e75987cf58b76c5f382cbdc70f9db14eb7929f5e9934ff132e68`
- Connection: `9028d87474d53ea996deb955824c5be8`
- Policy: `allow_automatically` for data reads and `always_ask` for unsafe actions

The first proposal attempt exposed a missing custom-response content type. The
second exposed a missing argument description. A fresh final task corrected
both rules. It opened the docs, called the no-argument template once, and
submitted one proposal. The proposal returned `review_required`, compiled
exactly nine operations, and was accepted by the operator. No `/v1` route was
called during setup.

## Execution

- Setup task: `task:2fcabb395ec6473ec90589f436e571a3`
- Execution task: `task:cb88beff5896ef1bf1c0fd36902a33fa`
- Execution runs: planner `run:4553870e7fd9e7b2f8469512b71ff885`; executor runs
  `run:e75feca1e08b616e4cb918e63e22ecb7`,
  `run:c2e43d046dc81c832342a6d7e7f6e883`, and
  `run:d040aa44ec22770808b10c28d266a315`; reviewer
  `run:5a5d7ae2196a544a83472d919c1e508c`.

The executor read the five sources in order, used bounded Lua checks, and
selected `cleaner-001` / `slot-001`. It rejected `cleaner-002` / `slot-002`
because the provider disallows pets. It kept `cleaner-003` / `slot-003` as the
eligible backup. Both writes were shown behind separate approval gates and
were approved only as synthetic actions.

The fixture returned its documented status literals
`scheduled_in_synthetic_record` and `backup_scheduled_in_synthetic_record`,
not the earlier draft literals `active` and `accepted`. The result records the
actual values and made no unsafe retry.

See [the measured ledger](task-measured-ledger.md), [the source and decision
evidence](task-household-evidence.md), and [the write evidence](task-write-evidence.md).
