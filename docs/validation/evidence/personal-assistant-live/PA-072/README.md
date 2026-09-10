# PA-072 synthetic return

Verdict: Pass after a fresh connector proposal, a boolean-transform repair,
one approval-gated synthetic submission, one status read, and direct fixture
ledger verification.

This case used the live Go Noema development instance. The return records,
purchase records, policy, receipts, shipping options, and status were
synthetic. No merchant, carrier, parcel, refund, payment, or money movement
was real.

## Case and fixture

- Case: `return-001`
- Fixture: `2026-09-09-returns-api-v1`
- Fixture source: [`run-mock-returns-api.ts`](../../../../../scripts/acceptance/run-mock-returns-api.ts)
- Origin: `https://materials-huntington-henderson-collective.trycloudflare.com/`
- Documentation: `https://materials-huntington-henderson-collective.trycloudflare.com/docs`
- Definition: `definition:synthetic_returns_api_fixed2`, revision `v5`
- Accepted semantic digest: `d151fc401c0788937e55fc12c1815724d3f2c0720936d5b0b45d54b174a41c74`
- Active connection: `7ae976b1253ad9b9a9419e79a094cfb8` (`personal-7ae976b1`)
- Connection revision: 2; policy revision: 2
- Policies: automatic reads and always-ask writes

## Connector setup

The final setup task was `task:036d90bfaea601039a00967d3c0f7d01`. Its planner
was `run:ae0e94472c69d4a0f0fd86c1df5addf3`, executor
`run:c980c19953d1df628b6e4fe905cecd9f`, and reviewer
`run:7c7e2be4c5c4b6952593b64969ec521a`. The setup opened the documentation,
called the no-argument template once, and submitted one proposal. The pending
digest was `6a657241822d4ebe2abe73103dddedc3882752e880d0216fb185994ac6c31b1`.
The operator accepted it as the canonical digest above. Setup made no `/v1`
request and no state-changing request.

The connector has exactly eight operations:

| Operation | Method and route | Behavior |
| --- | --- | --- |
| `get_return_profile` | GET `/v1/profile` | automatic read |
| `list_purchases` | GET `/v1/purchases` | automatic read |
| `get_return_policy` | GET `/v1/return-policy` | automatic read |
| `list_purchase_receipts` | GET `/v1/purchase-receipts` | automatic read |
| `list_shipping_options` | GET `/v1/shipping-options` | automatic read |
| `list_return_history` | GET `/v1/return-history` | automatic read |
| `submit_return` | POST `/v1/returns` | always-ask synthetic write |
| `get_return_refund_status` | GET `/v1/refund-status` | automatic read |

Generated transforms preserve both `true` and `false` boolean values. The
compiled GET operations use `retry: transport_safe_read`; the POST uses
`retry: never`.

Earlier setup attempts are retained in this directory. The first accepted
definition used response names absent from the fixture. A corrected proposal
used `string_array` for `allowed_reasons` although the documented field is a
string. The final proposal corrected both contracts before acceptance.

## Execution

The clean execution task was
`task:431f4416baf8f9cabf19d38a09a17163`. Its planner was
`run:05a202459474d54ea7a74db758e8bf49`; the pre-approval executor was
`run:d667795a923290bdf45b6dd26c52f4b2`; the approved continuation was
`run:a960cf936f1f03f05694a5a7bca592ff`; and the final reviewer was
`run:d108b3c5b9c3920cc2d5a641fddecb7b` after one requested-change pass.

Noema read the six sources in the required order. Its bounded Lua 5.4 check
returned `all_ok: true` and confirmed the case, dates, eligibility, policy,
receipt, amount, shipping, same-name identity separation, rejected options,
empty history, and exact write body.

Gate `gate:6002d1e11b3a6f8255396a0534ffbd47` required approval before the write.
Governed action `action:1a57c2955c6d37cb74abfa6607a604df`, revision 1, was
approved and succeeded once. The status read then succeeded once. The task
reached Done and the reviewer approved the result.

The task's generated `RESULT.md` incorrectly described a continuation replay.
The independent fixture ledger is authoritative for service calls. It records
exactly six reads, one POST, and one status read, with no replay. The ledger
and the successful action response satisfy the case's exact-call criterion.

## Evidence

The measured runs and direct service ledger are in
[`task-measured-ledger.md`](task-measured-ledger.md). The complete source
payloads are in [`task-return-evidence.md`](task-return-evidence.md). The
approval, exact body, response, and final status are in
[`task-post-write-evidence.md`](task-post-write-evidence.md).

## Acceptance

| Criterion | Result |
| --- | --- |
| Discover and review one documented API connector | Pass |
| Keep connector setup read-only | Pass; no `/v1` setup call |
| Read six sources exactly once in order | Pass; direct fixture ledger |
| Keep same-name purchases distinct | Pass; stable IDs and serials preserved |
| Apply eligibility, policy, charge, and shipping rules | Pass; bounded Lua check |
| Require approval before the write | Pass; task gate and governed action |
| Run the exact synthetic action once | Pass; submission count 1 |
| Verify one status read after the write | Pass |
| Keep the no-real-world-action boundary | Pass |

The temporary fixture and tunnel were stopped after evidence capture. The Go
server and socket relay remain running for the next case.
