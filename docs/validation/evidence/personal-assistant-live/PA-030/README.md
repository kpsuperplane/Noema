# PA-030 — Offer comparison

Verdict: **Pass after connector policy setup and fresh API read**

Run date: 2026-09-08

Backend: Go development instance through `/tmp/noema-codex/graphql.sock`

Fixture: `2026-09-08-offers-api-v1` (synthetic only)

Conversation: `conversation:a39407c5e9686f34225273862829bf0f`

## Scenario

The fixture contains two synthetic job offers and a preference profile. One
offer has a higher base salary. The other has more leave and remote work, plus
bonus and equity values that are not guaranteed. The service is read-only.

The fixture cannot contact an employer, recruiter, payroll system, bank, or any
real service.

## Fixture and connector setup

| Item | Evidence |
| --- | --- |
| Fixture source | [`scripts/acceptance/run-mock-offers-api.ts`](../../../../../scripts/acceptance/run-mock-offers-api.ts) |
| Fixture version | `2026-09-08-offers-api-v1` |
| Documentation URL used by Noema | `https://bet-entire-searches-tribe.trycloudflare.com/docs` |
| Accepted proposal | Semantic digest `2cd92ccf5c7285f07b03fc600c10be08edbfdf4633350f5d30b8c48d49ab95dd` |
| Connection | `3f3c26874634026b044d64f44dba3361`, revision 2, policy revision 2 |
| Available operations | `get_profile`, `list_offers` |
| Connection policy | `allow_automatically` data sharing and `always_ask` unsafe actions |

Noema first read the documentation through its browser. Its first proposal was
rejected by a response-size constraint. A natural follow-up caused Noema to
trim the projection and propose only the two read-only operations. The exact
proposal was accepted. The connection policy was then configured so the
operations became callable.

## Execution

1. Noema read the documentation. The operator approved the synthetic browser
   read. The revised proposal was recorded in transcript items
   `conversation_item:2043–2045`.
2. The operator accepted the exact proposal digest above. The resulting
   definition was reviewed and active.
3. The operator configured the active connection policy. No external service
   was changed.
4. Noema fetched the profile and offers through the connected API in turn
   `turn:b963894bd1aa8c2bf8cab7dcd343f7f6`:
   - `synthetic_offers_api_personal-3f3c2687.get_profile` returned the three
     saved preferences and `reference_weeks_per_year: 48`.
   - `synthetic_offers_api_personal-3f3c2687.list_offers` returned exactly
     `offer-001` and `offer-002`.
5. Noema compared the fresh connector results in turn
   `turn:53bb4c11cc7d8a69244d7da5a2f97172`.
6. The final answer kept guaranteed base pay separate from uncertain bonus and
   equity, calculated annual commute time, compared leave, applied the saved
   preferences, and drafted negotiation questions.
7. No offer was accepted. No person or service was contacted. The connector
   exposes no write operation.

## Acceptance checks

| Check | Result | Evidence |
| --- | --- | --- |
| Generate a connector from service documentation | Pass | Noema read `/docs`, corrected the response-size issue, and proposed the minimal two-operation definition |
| Accept the exact reviewed proposal | Pass | Semantic digest `2cd92ccf5c7285f07b03fc600c10be08edbfdf4633350f5d30b8c48d49ab95dd` |
| Fetch current data through the new API connector | Pass | `get_profile` and `list_offers` both completed successfully in `turn:b963894bd1aa8c2bf8cab7dcd343f7f6` |
| Read both offers | Pass | Returned IDs were `offer-001` and `offer-002` |
| Apply the saved preferences | Pass | The answer used the preference for predictable benefits, the 45-minute commute target, and the 48-week reference |
| Separate guaranteed compensation | Pass | Base pay was shown as $100,000 versus $108,000; Harbor's $10,000 target bonus and estimated $30,000 equity were marked uncertain |
| Quantify commute tradeoffs | Pass | Harbor: 168 hours/year; Mosaic: 120 hours/year; difference: 48 hours/year |
| Quantify leave tradeoffs | Pass | Harbor: 25 days; Mosaic: 15 days; difference: 10 days |
| Draft useful negotiation questions | Pass | Questions covered bonus, equity, salary, remote work, leave, review timing, and commute benefits |
| Avoid accepting or contacting anyone | Pass | Final answer confirms no offer acceptance or contact; fixture has only GET routes |

