# PA-050 — Consumer dispute

Verdict: **Pass after connector projection repairs, policy setup, and one
approval-gated mock submission**

Date: 2026-09-09

Backend: live Go Noema development instance through
`/tmp/noema-codex/graphql.sock`, server build revision `3231017e`.

## Fixture

- Service: synthetic consumer-dispute API, fixture
  `2026-09-09-consumer-dispute-api-v1`.
- Documentation: `https://herald-owen-association-invisible.trycloudflare.com/docs`.
- Fixture source: [`scripts/acceptance/run-mock-consumer-dispute-api.ts`](../../../../../scripts/acceptance/run-mock-consumer-dispute-api.ts).
- Conversation: `conversation:a39407c5e9686f34225273862829bf0f`.
- Connection: `1f2238c2b8b38cd0234130482319fa51`.
- Active reviewed definition: `ae2c774d1f5dd52f186be9fe6383ae20f5913f3865af17f1eab355728b4c84bd` (revision `v3`).
- Connection revision: 4. Policy revision: 2. Data sharing is automatic and
  unsafe actions use `always_ask`.
- The fixture is synthetic. It has no real merchant, card issuer, regulator,
  complaint service, or money endpoint.

The purchase is `purchase-200-001`. It is a $200 wrong-item order. The charge
is posted. The return deadline is September 15, 2026. The complaint deadline is
September 30, 2026. Two merchant contacts failed: chat on August 20 and email
on August 27.

## Connection setup and repairs

Setup Task `task:f5ab75a214d3d92c723555f61c23b66c` read the fixture
documentation and proposed six unauthenticated operations. The first proposal
was bounded after Noema rejected an oversized list projection. The reviewed v1
definition was accepted at digest
`5bb9a0a694fb82e42ff67319867a631110bc84fa7264dfd332960a3d1a5f5e6b` and its
connection policy was configured.

The first live read exposed four projection defects. Two object transforms
failed validation. Two list transforms returned empty arrays. Repair Task
`task:690423a3b91305ebfd0e4e87238c053d` proposed v2, digest
`458b569062f8feaa38f4af3186a951ca7c7555c96092fb0ab885dae05c64a08b`. Live
retesting showed that v2 still used the wrong field names.

The main conversation then proposed the smallest field-level v3 repair. It
changed only these four operations:

| Operation | Fixture field | Output field |
| --- | --- | --- |
| `get_purchase` | `charged_status` | `charge_status` |
| `get_return_policy` | `instructions` | `return_instructions` |
| `list_merchant_contacts` | `contacted_on`, `channel` | `contacted_at`, `method` |
| `list_escalation_channels` | `label`, `eligibility` | `name`, `eligibility_rule` |

The pending v3 digest was
`55ba2f609dce33473e01af3435ad143245802d40a8d92c0aff033f6cee86e559`. It was
accepted as reviewed digest
`ae2c774d1f5dd52f186be9fe6383ae20f5913f3865af17f1eab355728b4c84bd`.

## Execution

1. Read turn `turn:27a29d3e5ab5069d87bdf92a4f9ef7b6` used the active v3
   connector. All four reads passed. They returned the posted $200 purchase,
   both failed contacts, the refund-or-replacement policy and instructions,
   and the `consumer-protection` channel eligible after two failed contacts.
2. Noema prepared the exact mock complaint in turn
   `turn:4decbff3011c7be96452d6ae9e05f631`. It included purchase
   `purchase-200-001`, channel `consumer-protection`, evidence IDs
   `contact-001` and `contact-002`, and a `$200 refund or replacement` remedy.
3. The operator approved the governed action
   `action:a67761ff08047b8910a4916aabb95040`, revision 1. The action used only
   the reviewed synthetic connection. The fixture returned complaint ID
   `complaint-consumer-001`, receipt `complaint-receipt-001`, status
   `submitted`, and submission count `1`.
4. Verification turn `turn:9fdf4806fac7e1b60b0f5ecfaf6d9036` read the complaint
   status. It returned status `resolved`, receipt
   `complaint-receipt-001`, and a $200 synthetic refund. Noema saved the final
   sourced review as artifact
   `artifact:1cdcc8e706eeb41f8acc53ffa3f32494`, version
   `artifact_version:40a2ba89182d1cedd72ae80defbd9908`, 1,943 bytes, with
   SHA-256
   `8d119f6f14fe6c3ab90c1fa92bec341561a94a8d7b1cb867bea9d2afe2d92260`.

The first verification request became stuck in the provider session. Restarting
the Go dev process recovered it as a failed turn. The retry performed one status
read and one artifact save. It did not submit another complaint.

## Independent fixture checks

Direct reads returned the documented profile, purchase, contacts, return policy,
channels, and resolved complaint status. The fixture log records documentation
discovery, connector reads, exactly one POST to `/v1/complaints`, and the later
status reads. The POST body contains the two contact evidence IDs, the correct
channel, purchase ID, chronology, and requested remedy. The fixture reported
submission count `1`.

No real merchant, card issuer, regulator, complaint service, or payment system
was contacted. No money moved.

## Acceptance

| Criterion | Result |
| --- | --- |
| Read the purchase and wrong-item facts | Pass — the connector returned purchase ID, amount 200, posted charge, and deadlines. |
| Include both failed merchant contacts | Pass — both IDs, dates, channels, and failed outcomes were returned. |
| Apply the return policy and complaint deadline | Pass — the refund-or-replacement remedy, instructions, return deadline, and complaint deadline were preserved. |
| Select the proper escalation channel | Pass — `consumer-protection` was selected because two contacts failed. |
| Ask before the state-changing mock operation | Pass — the exact governed action required approval. |
| Submit once with matching evidence | Pass — one POST returned distinct complaint and receipt IDs with count `1`. |
| Verify the later remedy | Pass — the status read returned `resolved` and a $200 synthetic refund. |
| Save a sourced review | Pass — the Markdown artifact records facts, dates, IDs, receipts, status, and source operations. |
| Keep real services and money out of scope | Pass — every connector call targeted the synthetic fixture. |
