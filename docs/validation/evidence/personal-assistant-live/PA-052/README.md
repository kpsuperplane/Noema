# PA-052 — Deceased accounts

Verdict: **Pass after ten reviewed connector revisions, focused read/write
reruns, five approval-gated synthetic notices, final reconciliation, and a
sourced artifact**

Date: 2026-09-09

Backend: live Go Noema development instance through
`/tmp/noema-codex/graphql.sock`.

## Fixture and safety boundary

- Service: synthetic deceased-accounts API, fixture
  `2026-09-09-deceased-accounts-api-v1`.
- Fixture source: [`scripts/acceptance/run-mock-deceased-accounts-api.ts`](../../../../../scripts/acceptance/run-mock-deceased-accounts-api.ts).
- Documentation and API base during the run:
  `https://previous-poems-crew-tuning.trycloudflare.com`.
- Connection: `fefcbd32b738bb34c6ebe8635692834a`.
- Active reviewed definition: v10, semantic digest
  `91e177fdfc209147c9c8445a0f44dc452e2a9d9787f0808fca841032caa29031`.
- Final connection revision: 11. Policy revision: 2. Reads were automatic;
  unsafe actions used `always_ask`.
- The fixture was stopped after acceptance. It has no real bank, account,
  executor, institution, legal process, or money endpoint.

The fixture represents Taylor Morgan's synthetic estate. Alex Morgan has
verified executor proof for notices on individually held accounts. The joint
checking account is explicitly outside this run. The tax-review obligation
requires a licensed process and remains open.

## Connection setup and repairs

Setup Task `task:c5ac40eabac6e807699d6d1e1964c0e0` discovered and proposed the
initial API connector. Its reviewed active digest was
`d2b32d7ed0004fde5bde4580f36342e2ea68ada870544b4a841a18c29aa8c444`. The
connection policy was configured for automatic reads and approval for unsafe
writes.

The first read-and-plan task `task:21331900cceaab59af3cca55fc0141c1` was
cancelled after every bounded transform failed. No write occurred. The
following repairs changed only the failing operations:

1. Profile repair Task `task:88205427111935cb39de994216d038db` accepted v2 at
   digest `3ae11f3341b13dd53a8079761dacf9c1449b3b6a92cb1df1ac2bd375e8a1e5b1`.
   It mapped the fixture's flat profile fields. Test Task
   `task:b5d1597f3970da722889ea96deea6dd5` passed.
2. Core-read repair Task `task:d2c2dbf55152d54bd9f4822a2e075da4` accepted v3
   at digest `6af754e74a7aeb97073c9843e0e4c620a2e52c5411e86a0a8f01a4c7e0c8d8f6`.
   It corrected `/proofs`, `/accounts`, and `/deadlines`. The first proposal
   exceeded the computed schema limit and was retried with bounded fields.
3. Account-only repair Task `task:1e80b0d405bf240547a0f53bcb7c02c5` accepted
   v4 at digest `d8e9be332d700acb025670a49bce790f2840356285c96353f9d564cc13a9bfea`.
   It preserved explicit `false` values and optional exclusion fields. Test
   Task `task:a2b720d5f735e4334b86569200c6e6b1` passed.
4. Document repair Task `task:b7a82e01f341e5795b689f8d3482ffb9` accepted v5
   at digest `1fccce3d74734d819b80521df90cda5700e857f9a206a659920abc46fc9d2b7b`.
   Test Task `task:68b49b3bb2bdfc3884866eeaf5512fc7` passed for the verified
   executor proof and the initially missing certificate.
5. Notice-read repair Task `task:983d135829dd8e9609b360a5e8c2a98f` accepted
   v6 at digest `12ec375baa1c1af1762953104b158cd5c73e8fe9b2604a83455fe7fc696acbd6`.
   The follow-up exposed plain Lua table values, so array repair Task
   `task:cbbf86a5a7964eaeadd654abf44fbb7d` accepted v7 at digest
   `7ea823698cac766db357d8466b3e400d71bf3fe9ed2c714a1601f6e9d43811a3`.
   Null-safety repair Task `task:770ecf31305f81155a7a63323145764d` accepted
   v8 at digest `2ddcc50eb41de54b4336bee8efe5a0e8eaafd7b7e5caded985863955a4303fd5`.
   It omitted the tax obligation's JSON-null account ID instead of exposing
   an internal value. Test Task `task:0e65606f3ac3258568d5402ff15ddbf0`
   passed.
6. Certificate-request repair Task `task:4488334153cd6c0f2e9ab8d8fbea5ee1`
   produced a valid bounded proposal after rejected validator retries. The
   reviewed v9 digest was
   `866c7dc554b688b4239edcd591d3133b84a980c05313a7d2210026558d8c42e5`.
   It uses the exact JSON request body and required receipt fields.
7. Notice-write repair Task `task:10ec69707b8fe301e3f7502e355a1f96` produced
   the valid v10 proposal. The reviewed active digest is the v10 digest above.
   It uses object argument references, `{kind:"none"}` authentication, and
   exact proof, document, note, and notice-kind fields. Earlier repair tasks
   `task:2f0c439719e9d28c9496eed88db80952` and
   `task:e788fcc3171fd083e7019afa654fe60d` were cancelled after invalid
   validator retries; neither made a write.

## Execution

1. Profile Task `task:b5d1597f3970da722889ea96deea6dd5` returned the case ID,
   estate label, date, executor, goal, and decision boundary.
2. Core-read Task `task:a2b720d5f735e4334b86569200c6e6b1` returned verified
   executor proof, six accounts, and two deadlines. The joint account retained
   `notice_required=false` and `closure_eligible=false`.
3. Document Task `task:68b49b3bb2bdfc3884866eeaf5512fc7` returned the verified
   executor proof and missing certificate with request count zero.
4. Notice-read Task `task:0e65606f3ac3258568d5402ff15ddbf0` returned an empty
   notice list and two obligations. The tax obligation's null account ID was
   omitted. An earlier non-empty read (`task:e6ed66a2622a13ccc262eae8a27096c9`)
   was cancelled because its planner made an extra malformed read before the
   requested operation.
5. Certificate request Task `task:1b75b810bf29fcc21b42169a696896aa` read the
   missing document, received approval gate
   `gate:97ba619018ddb8b1765b66c044522ad5`, and created governed action
   `action:12bdd806aeb79cc83cf6c31ed29e5593`, revision 1. After approval, one
   POST returned request `certificate-request-001`, receipt
   `death-certificate-receipt-001`, status `requested`, and count 1.
6. Availability Task `task:eca47cf93b420c3810454993c4a8e42d` made one document
   read and confirmed `death-certificate-001` was available with request count
   1.
7. Five notice tasks each read the proof and certificate, waited for the task
   approval gate, made one governed write, and received one fixture receipt:

   | Account | Task | Action | Notice / receipt | Result |
   | --- | --- | --- | --- | --- |
   | checking-001 | `task:7df0c9a1f266b8a833d42581526c4e51` | `action:ee2d691d4c6b37181dfe8d035545905c` | `notice-001` / `notice-receipt-001` | submitted, count 1 |
   | savings-001 | `task:b93d0d1dc51d973b3daf65c8ac997bfe` | `action:9f9bed95cf381d19f84360742df904cb` | `notice-002` / `notice-receipt-002` | submitted, count 1 |
   | brokerage-001 | `task:5fc8b987773229cfbc886cc9e1e8080c` | `action:cf3733d30947b5cccb69140227108b24` | `notice-003` / `notice-receipt-003` | submitted, count 1 |
   | credit-card-001 | `task:bb2501829b61f170f6caa5ae7a7ca261` | `action:a7dc62c68da9994b0ad6ecb22a159204` | `notice-004` / `notice-receipt-004` | submitted, count 1 |
   | retirement-001 | `task:2e1de0ab9dbeacb07fa57e3b1279fb37` | `action:2079cd140485a9216557669c59a3ad69` | `notice-005` / `notice-receipt-005` | submitted, count 1 |

   Each action was approved at revision 1. No account closure, asset
   transfer, joint notice, real-institution contact, or money movement occurred.
8. Acknowledgement Task `task:fa54b628bf918ce8b159d4a020380e49` read
   `notice-001` once and confirmed acknowledgement, receipt, source document,
   and submission count.
9. Final reconciliation Task `task:84687c8445f4aa6a7931379320378c7a` called
   exactly `list_notices` once. It returned all five notices in account order,
   each `acknowledged` with submission count 1, plus the joint-account
   exclusion and tax-review obligation.
10. Artifact Task `task:a7308e86840862d62bd5ea2b08a1a164` created exactly one
    local Markdown artifact and passed independent review:
    `artifact:8fda84a651c2801451fe75e675e3e4fc`, version
    `artifact_version:8af570b74853e0f421a920767e4d7cdd`, 2,656 bytes. The
    artifact service did not return a content digest, so none was invented.

## Acceptance

| Criterion | Result |
| --- | --- |
| Discover and review a connector from supplied API documentation | Pass — the API connector was proposed, accepted, repaired, and policy-configured. |
| Read the profile, authority, accounts, and deadlines | Pass — all required fields, false flags, dates, and authority scope were preserved. |
| Identify the missing certificate and obtain it through approval | Pass — one approved request returned a receipt and the document became available. |
| Sequence notices by deadline and exclude the joint account | Pass — five individual accounts were submitted; the joint account remained excluded. |
| Require approval for each state-changing notice | Pass — five revision-1 governed actions were approved before their single writes. |
| Verify receipts and later acknowledgements | Pass — all five receipts are present; the final read reports five acknowledged notices. |
| Preserve unresolved work and null values | Pass — the joint exclusion remains explicit; tax review remains due 2026-09-30 with no fabricated account ID. |
| Save a sourced review artifact | Pass — one reviewed 2,656-byte Markdown artifact contains the complete case record and safety boundary. |
| Keep real-world financial and legal actions out of scope | Pass — every call targeted the synthetic fixture; no money or real institution was touched. |
