# PA-068 utility plan optimization

Verdict: Pass after a reviewed API setup, five ordered source reads, one
approval-gated synthetic switch, two post-write reads, and one reviewed
Markdown artifact.

This case used the live Go Noema development instance. The utility providers,
service address, account, bill, activation, and all switch results were
synthetic. No real provider was contacted, no real service changed, and no
charge, payment, or money movement occurred.

## Case and fixture

- Fixture: `2026-09-09-utility-optimization-api-v1`
- Fixture URL: `https://rapid-started-ton-sic.trycloudflare.com`
- Documentation: `https://rapid-started-ton-sic.trycloudflare.com/docs`
- Case: `utility-optimization-001`
- Owner: `Jordan Lee (synthetic)`
- Current date: `2026-09-09`
- Time zone: `America/Los_Angeles`
- Scope: one personal Noema workspace
- Fixture implementation:
  [`run-mock-utility-optimization-api.ts`](../../../../../scripts/acceptance/run-mock-utility-optimization-api.ts)

The fixture exposed five source records, one approval-gated synthetic switch,
one finalized old-provider bill, and one final status record. It cannot contact
a provider, change a real service, expose a real address, authorize a charge,
or move money.

## Connector setup

Setup task `task:5f3ae552fffa53d494a5d91e551b0076` completed with planner
`run:4267ad7e7d2bc727f4f9682f098db837`, executor
`run:68ecbeb21643a43f25c911aeb01562e7`, and reviewer
`run:15c56f02fa29bda0662f2341b138cd82`. The task first used
`adapter.definition_template`, opened the public fixture documentation through
the governed read-only action `action:c8c7eb0e7034cc57f9c6a62190cd35e1`, and
called `adapter.propose_definition` exactly once. Setup made no `/v1` calls.

The proposal returned `review_required` with pending semantic digest
`dac434a235731637d40515cf08cacb7eb3c2c7bd90fd0b5b0ccc254a22c13778`. The
operator accepted that inspected proposal. The reviewed definition has
semantic digest `59be59f03f114759774cb164198a1dc2adb28fbd95d8ddf7c7bd2fbc7885b3c2`,
definition `definition:synthetic_utility_optimization_api`, adapter
`synthetic_utility_optimization_api`, revision `v1`, exact origin
`https://rapid-started-ton-sic.trycloudflare.com/`, source reference
`https://rapid-started-ton-sic.trycloudflare.com/docs`, and no authentication.

The active connection is `156b6619f21280856fb93cd7bf9eb027`, exposed as
`personal-156b6619`. It is ready at connection revision 2 and policy revision
2. Reads are allowed automatically. The switch write is always approval-gated.

The reviewed operations were:

| Operation | Method and path | Behavior |
| --- | --- | --- |
| `get_utility_profile` | GET `/v1/profile` | Read-only, automatic |
| `list_utility_usage` | GET `/v1/usage` | Read-only, automatic |
| `list_utility_plans` | GET `/v1/plans` | Read-only, automatic |
| `get_current_utility_service` | GET `/v1/current-service` | Read-only, automatic |
| `get_utility_constraints` | GET `/v1/constraints` | Read-only, automatic |
| `switch_utility_bundle` | POST `/v1/switch` | Approval-gated synthetic write |
| `get_old_provider_final_bill` | GET `/v1/final-bill` | Read-only, automatic |
| `get_utility_status` | GET `/v1/status` | Read-only, automatic |

All eight operations use bounded JSON projections. The seven GET operations
are idempotent, non-destructive, closed-world reads. The POST is non-idempotent,
non-destructive, open-world, and uses `retry: never`. The compiled definition
stayed within the platform schema limit.

## Utility workflow execution

Execution task `task:0bdfc2cd1e9371ba87d5ff43538f2201` completed with planner
`run:1c20d43387a161df84c4bcda12f282e6`, initial executor
`run:58252957439cda85a6134440ef9c2e94`, resumed executor
`run:617c9e0402e8b2248fb637f129dd05eb`, and reviewer
`run:722a154d51d96a5a4ac978f4b459fb8c`. The task reached `Done`.

The five source reads ran once each, in the required order:

| Sequence | Operation | Result |
| ---: | --- | --- |
| 1 | `get_utility_profile` | Synthetic owner, goal, budget, reliability preference, boundary, date, and profile locator |
| 2 | `list_utility_usage` | Twelve monthly records totaling 7,210 kWh, with usage locators |
| 3 | `list_utility_plans` | Three plans with pricing, introductory periods, equipment, reliability, caps, billing notes, status, and plan locators |
| 4 | `get_current_utility_service` | Current provider, plan, monthly total, contract end, exit fee, prorated service, switch date, and current-service locator |
| 5 | `get_utility_constraints` | 99.95% uptime minimum, no cap, predictable billing, 12-month period, budget, unit, date, and constraints locator |

Noema used `code.run_luau` to sum the usage and calculate each full-year
total. It selected `plan-001` because it was the only plan meeting the hard
99.95% uptime and no-cap requirements. It rejected the cheaper `plan-002` for
99.5% uptime and a 1,000 GB cap. It rejected `plan-003` for 99.9% uptime.

The calculations were:

- Plan-001 electricity: `7,210 * 0.19 + 22 * 12 = 1,633.90` USD.
- Plan-001 internet: `45 * 3 + 70 * 9 = 765.00` USD.
- Plan-001 annual new-plan total: `2,398.90` USD.
- Plan-002 annual total including equipment: `2,047.50` USD.
- Plan-003 annual total including equipment: `2,380.70` USD.
- Old-provider final bill: `48.50 + 120.00 = 168.50` USD.
- All-in switch total: `2,398.90 + 168.50 = 2,567.40` USD.
- Budget remaining: `2,600.00 - 2,567.40 = 32.60` USD.

Noema opened task gate `gate:8458bf4b55168bc3580fcdf9edce7016` before the
write. The operator approved the synthetic-only switch. Governed action
`action:719719e26b3c441f12006d20388ec247` then ran once with these exact
arguments:

```json
{
  "case_id": "utility-optimization-001",
  "selected_plan_id": "plan-001",
  "switch_date": "2026-10-01",
  "annual_new_plan_total_usd": 2398.9,
  "old_provider_final_bill_usd": 168.5,
  "annual_switch_total_usd": 2567.4,
  "approval_note": "Synthetic switch only; no provider contact or real service change."
}
```

The governed action succeeded with activation `activation-001`, status
`activated_in_synthetic_account`, submission count 1, and locator
`utility://switches/activation-001`. No provider or real account was touched.

After the write, Noema read `get_old_provider_final_bill` exactly once. It
returned `final-bill-001`, 48.50 USD prorated service, a 120 USD early-exit
fee, a 168.50 USD final amount, status
`finalized_in_synthetic_ledger`, submission count 1, and locator
`utility://bills/final-bill-001`.

It then read `get_utility_status` exactly once. The final status returned
activation-001, plan-001, 2026-10-01, 2,398.90 USD new-plan cost, 168.50 USD
old-provider bill, 2,567.40 USD switch total, 2,600.00 USD budget, 32.60 USD
remaining, one submission, `activated_in_synthetic_account`, and locator
`utility://status/utility-optimization-001`.

The fixture service ledger, excluding health and documentation probes, contains
exactly these eight calls:

1. GET `/v1/profile`
2. GET `/v1/usage`
3. GET `/v1/plans`
4. GET `/v1/current-service`
5. GET `/v1/constraints`
6. POST `/v1/switch`
7. GET `/v1/final-bill`
8. GET `/v1/status`

The POST body matched the approved JSON exactly. It ran once, after approval.

The execution created two evidence-ledger artifacts:

| Artifact | Version | Size | Media type |
| --- | --- | ---: | --- |
| `artifact:bcc3d618fb71a0f671e37915e374af35` — source review | `artifact_version:eec090f24189c05184cebf3a46078927` | 5,717 bytes | `text/markdown` |
| `artifact:31ab03efaa44c6ba700f22be9b15749e` — switch verification | `artifact_version:d803c13731fca5fcc9e2980c6817fcdd` | 3,147 bytes | `text/markdown` |

## Final artifact

Artifact task `task:da77d4c0616077820aab2ee3a9b1cbc3` completed with planner
`run:ba3c65250a178e5cf8b4e0cad8fa45dd`, executor attempts
`run:34b6963225b37dd4937a55e048506dbc`,
`run:548b11db775025f818a0734f2d347407`, and
`run:32aa084b02bb69ff68bdd88dabd21547`, and reviewer
`run:78272771216bfd55151e51ac534b3f06`. The first executor asked for the
source record; the operator supplied the complete records and the exact ledger.
The final attempt then created and verified exactly one artifact without using
the connector.

The final artifact is:

- Artifact: `artifact:e1bd626f7a904892354590ffb99ede56`
- Version: `artifact_version:b8f6cd3aead17f8df1b85f250739fc56`
- Title: `Synthetic utility optimization final`
- Filename: `synthetic-utility-optimization-final.md`
- Size: 10,746 bytes
- Media type: `text/markdown`
- Preview: `MARKDOWN`; readable converted Markdown, untruncated
- Download: `/artifacts/versions/b8f6cd3aead17f8df1b85f250739fc56/download`
- Content SHA-256 from the artifact-version row:
  `b8a8df4189cd1b2dc6c19c86844222e976c73eb68420d8e715d2c06cb3991865`

The artifact preview contains the synthetic-only boundary, profile, all twelve
usage rows and locators, all three plans and full-year calculations, current
service, constraints, exact approved JSON, activation receipt, final bill,
final status, and the ordered eight-call ledger. The five no-argument reads are
represented as `{}` in that ledger. The task reviewer also confirmed that no
connector or external service was called by the document-only task.

## Acceptance

| Criterion | Result |
| --- | --- |
| Discover the documented service and propose one connector | Pass; documentation was opened, one exact-once proposal was reviewed, and the eight-operation definition was accepted |
| Keep setup read-only | Pass; setup made no `/v1` or state-changing call |
| Read all initial records once in order | Pass; five reads ran once in the specified order |
| Compare all plans over twelve months | Pass; all usage, pricing, introductory rates, equipment, reliability, caps, and billing notes were preserved |
| Keep the reliability preference ahead of price | Pass; plans below 99.95% uptime or with a cap were rejected |
| Require approval before the synthetic write | Pass; task gate and governed action preceded the POST |
| Run one exact switch write | Pass; one POST matched the approved arguments and returned submission count 1 |
| Verify the old-provider bill and final status | Pass; each read ran once after the write and returned the documented values |
| Save one complete sourced brief | Pass; one reviewed 10,746-byte Markdown artifact with readable preview and stored hash |
| Avoid real-world action | Pass; no real provider, service, address, charge, payment, or money movement was used |

The temporary fixture and tunnel were stopped after evidence collection. The
temporary host mapping was removed.
