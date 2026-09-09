# PA-056 — Referral coordination

Verdict: **Pass after a response-contract repair, a policy-configured v2
connector, four approval-gated synthetic actions, final status verification,
and an independently inspected artifact.**

This case used the live Go Noema development instance through
`/tmp/noema-codex/graphql.sock`. Every service route was synthetic. No real
patient, clinician, provider, insurer, transport service, appointment, or
payment was involved.

## Fixture and safety boundary

- Fixture: `2026-09-09-referral-api-v1`
- Fixture URL during the run:
  `https://crops-titled-gear-claims.trycloudflare.com`
- Case: `referral-001`
- Patient label: `Jordan Lee (synthetic)`
- Goal: arrange a feasible synthetic specialist referral without clinical
  decisions.
- Boundary: no diagnosis, treatment selection, real provider contact, real
  appointment, or payment.
- Mock implementation:
  [`run-mock-referral-api.ts`](../../../../../scripts/acceptance/run-mock-referral-api.ts)

## Connector setup and repair

The first setup task, `task:eaa7ee8af7eafc4a9755967ec66ca497`, proposed an
invalid operation that mixed flat bundle fields with a nested records list.
No connector was created from that proposal. The corrected setup task,
`task:30cc0996bfb6477db6134155d5c6d75a`, split those response shapes and
submitted exactly twelve operations. Its reviewed proposal digest was
`cb5c72e06fa4a5380c181238e353238b66e4977c9388c53e6b8315af3e33cf2b`.

The accepted v1 definition uses semantic digest
`5fac7709e898144b71dd7cc52635d8fb494ef4efa67f403485fbf340712ced39`.
Connection `3a5cb3f9fa4ec8514c2b111a32d7ec54` was configured for automatic
reads and `always_ask` for unsafe actions. It exposed exactly twelve tools.

The first coordination attempt, `task:130c7f2e2b0a0dd6ca5a58359b3a0f18`,
showed a real v1 defect. The generated list transform used a Lua
`and ... or nil` expression for booleans, so `booking_required:false` was
dropped and the required response contract failed. The task was cancelled
before any write.

Repair task `task:51765ac0c24ccaae5bd9f5e3c41759c0` loaded the exact v1 digest,
validated explicit true and false branches with bounded Luau, and proposed
only four v2 replacements: prerequisites, network, slots, and transport
options. The reviewed v2 digest was
`6069db93655aec544eceea39b30ca5a4005f6e0cb5ccd3e17d082b1700b1c4c3`; the
accepted active v2 digest is
`99868bd0735ea702f6f3c9cb991d0685c5d93cbb8f9566a1f2d3c4ba004a3056`.
The repair evidence measured schemas of 598, 687, 832, and 666 bytes. All
were strict and below the 32,768-byte limit. The repair task's reviewer
remediation repeated the idempotent proposal call, but made no connector or
fixture call; this is recorded in its task history and did not alter the
accepted definition.

## Ordered reads and selection

The clean rerun was `task:d1423c966ce7af844a0a1bbf8ed9f623`, executor run
`run:2ca76426e62cdaf896b7db01afde5112`. Its persisted tool ledger and fixture
log show exactly twelve connector calls in this order, with no retry:

1. `get_referral_profile`
2. `list_referral_prerequisites`
3. `get_referral_record_bundle`
4. `list_referral_records`
5. `list_referral_network`
6. `list_referral_slots`
7. `list_referral_transport_options`
8. `schedule_referral_prerequisite`
9. `transfer_referral_records`
10. `book_referral_appointment`
11. `book_referral_transport`
12. `get_referral_follow_up_status`

The reads returned the missing booking-required prerequisite
`prerequisite-002` due `2026-09-15`; ready bundle `record-bundle-001` with
scope `referral-order,recent-tests,medication-list`; and three providers,
three slots, and three transport options. The selected path was:

- `provider-001`, Northstar Sleep Clinic (synthetic): in-network, accepting
  referrals, ground-floor entrance.
- `slot-003`: `2026-09-18` at `14:00` in `America/Los_Angeles`, 45 minutes
  of travel, within the 60-minute limit.
- `transport-002`, Harbor Ride (synthetic): available, 50 minutes of travel,
  `$42`, and a 30-minute arrival buffer within the `$60` limit.

The v2 transforms retained explicit false values for rejected alternatives and
all required identifiers, dates, numbers, and source locators.

## Approval-gated actions

Each unsafe operation created one revision-1 governed action. The operator
inspected and approved it through the normal action interface before its one
synthetic write. SQLite action events show `human:local` approval followed by
gateway execution and success for each action.

| Operation | Action | Synthetic result |
| --- | --- | --- |
| `schedule_referral_prerequisite` | `action:007e356f61d8d5175b441431564c111c` | `prerequisite-booking-001`, `prerequisite-receipt-001`, `scheduled`, request count 1 |
| `transfer_referral_records` | `action:444a9dc472a20e63673b9b8ceb17226a` | `record-transfer-001`, `transfer-receipt-001`, `submitted`, request count 1 |
| `book_referral_appointment` | `action:afc4927a688f50372d545feca878fcca` | `appointment-booking-001`, `appointment-receipt-001`, `booked`, request count 1 |
| `book_referral_transport` | `action:149343f647d03dc2bace41cae21044a6` | `transport-booking-001`, `transport-receipt-001`, `booked`, request count 1 |

The final status read returned the four statuses and receipts, booked slot
`slot-003`, booked transport option `transport-002`, source locator
`referral://status/referral-001`, and next follow-up date `2026-09-26`.

## Evidence artifact

Artifact task `task:710c0447e2545c8e9223421279f92ae2` created exactly one local
Markdown artifact and passed independent review:

- Artifact: `artifact:ea33749d18b584af1dbb2144643554f5`
- Version: `artifact_version:8e14067ebfe421662cfa77061cb6fd69`
- Size: 7,574 bytes
- Content digest: unavailable from the artifact service (the read-only
  database records SHA-256
  `64bd97ec550a5a0143ac9f6edb7b86f05be5581e11e8aa1b8ef7f84f74648706`).

The artifact read-back preserves all full `referral://` locators, comparison
facts, constraints, selections, action IDs and revisions, receipts, statuses,
identifiers, dates, booleans, numbers, request counts, and the synthetic-only
boundary.

## Reviewer discrepancy and independent audit

The built-in reviewer for the coordination task produced an unmet-requirements
report that claimed completed connector calls were re-invoked. The persisted
executor ledger contains exactly the twelve calls listed above, and the
fixture log contains one successful read sequence, one call per write route,
and one final status read. After the reviewer continuation began, it made only
`task.files.write` and artifact calls; it made no connector call. The
continuation was cancelled before further service activity. The verdict above
therefore follows the directly inspectable call ledger, action events, fixture
state, and artifact, rather than the contradictory model-generated narrative.

## Acceptance

| Criterion | Result |
| --- | --- |
| Discover and review a connector from documentation | Pass — corrected twelve-operation proposal, reviewed v1, policy, and accepted v2 repair. |
| Preserve explicit false values in list responses | Pass — v2 branches retain false prerequisite, network, and slot values. |
| Read constraints and choose a feasible path | Pass — provider, slot, and transport satisfy returned network, accessibility, travel, arrival, and cost constraints. |
| Sequence prerequisite, scoped transfer, appointment, and transport | Pass — returned identifiers and documented scope were used in order. |
| Require human approval for every unsafe operation | Pass — four revision-1 actions were approved before one write each. |
| Verify receipts, statuses, and follow-up | Pass — final read preserves all four receipts, statuses, selected IDs, and `2026-09-26`. |
| Prevent duplicate synthetic writes | Pass — fixture request count is 1 for all four write routes. |
| Save a complete sourced artifact | Pass — independently reviewed 7,574-byte Markdown artifact. |
| Keep real-world and clinical activity out of scope | Pass — synthetic fixture only; no diagnosis, treatment, provider contact, appointment, or payment. |

The fixture and Cloudflare tunnel were stopped after artifact inspection.
