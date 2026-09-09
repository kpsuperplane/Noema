# PA-064 second opinion

Verdict: Pass after a reviewed API proposal, six ordered source reads, one
approved synthetic estimate request, one final status read, and an independently
reviewed sourced artifact.

This case used the live Go Noema development instance. Every person, option,
evidence record, estimate, clinician, and receipt was synthetic. No real
clinician, health system, treatment, payment, or health record was involved.

## Case and fixture

- Fixture: `2026-09-09-second-opinion-api-v1`
- Fixture URL:
  `https://investment-reason-pickup-scope.trycloudflare.com`
- Documentation: `https://investment-reason-pickup-scope.trycloudflare.com/docs`
- Case: `second-opinion-001`
- Person: `Jordan Lee (synthetic)`
- Current date: `2026-09-09`
- Scope: one personal Noema workspace
- Fixture implementation:
  [`run-mock-second-opinion-api.ts`](../../../../../scripts/acceptance/run-mock-second-opinion-api.ts)

The fixture exposed two documented options, three evidence records, Jordan's
burden and uncertainty values, a missing-estimate record, one bounded estimate
request, and a final status record. It cannot diagnose, recommend or select
treatment.

## Connector setup

Task `task:def82f4289d51893eb0965070a590f13` inspected the documentation page,
called `adapter.definition_template` once, and proposed exactly eight API
operations. Planner `run:81c97a2275c9bb389380c82f7c0e82e4`, executor
`run:35b364cd4c51e253d759d942fc91dedb`, and reviewer
`run:3711a94f455463c719b9c63f93a1c860` completed successfully. The pending
proposal digest was
`834d4e53e9376873fac15f56dab4e62e184c70728f5d226c7598c18e9246a660`; the
operator accepted that exact digest. The reviewed semantic digest became
`9830da28fcc25d7a74ea9589855aa53e77364775d4a78ea5abb916a27e746067`.

The active connection is `83f33ba735b8c6d07764c8b5dc4393e`, exposed to the
task as `personal-83f33ba7`. It is active at connection revision 2 and policy
revision 2. Reads are allowed automatically. The only write,
`request_benefit_estimate`, is always approval-gated.

The eight reviewed operations were:

| Operation | Method and path | Behavior |
| --- | --- | --- |
| `get_second_opinion_profile` | GET `/v1/profile` | Read-only, automatic |
| `get_option_a` | GET `/v1/option-a` | Read-only, automatic |
| `get_option_b` | GET `/v1/option-b` | Read-only, automatic |
| `list_second_opinion_evidence` | GET `/v1/evidence` | Read-only, automatic |
| `get_second_opinion_user_values` | GET `/v1/user-values` | Read-only, automatic |
| `get_missing_benefit_estimate` | GET `/v1/benefit-estimate` | Read-only, automatic |
| `request_benefit_estimate` | POST `/v1/benefit-estimate-request` | Approval-gated synthetic write |
| `get_second_opinion_status` | GET `/v1/status` | Read-only, automatic |

Setup created one proposal receipt artifact:

- Artifact: `artifact:ba0fb5b7808479dd89ad0e547b9e59f7`
- Version: `artifact_version:ef748e88af8cf05d1f5d2f521d4c6571`
- Size: 2,092 bytes
- Filename: `synthetic-second-opinion-api-proposal-receipt.md`

The setup task called no `/v1` operation. The operator made a local preflight
read before setup; it is excluded from the service evidence below.

## Comparison execution

Task `task:53fad8b1e281631a5552a7a4b733edf7` completed with planner
`run:50adb7ae0218814d1a6c9e0d5831c393`, initial executor
`run:2737478aa9a2269a32d857ca59f85c67`, resumed executor
`run:baccb99d2376945d5d1e923d849412f9`, and reviewer
`run:b590bff4e63b07738e5ea54d81558a9f`. The reviewer approved the result.

The six source reads ran once each, in the required order:

| Sequence | Operation | Result |
| ---: | --- | --- |
| 1 | `get_second_opinion_profile` | Case, goal, question, boundary, and scope |
| 2 | `get_option_a` | Lower-burden option with documented 7 percent short-term outcome |
| 3 | `get_option_b` | Supervised weekly option with documented 12 percent short-term outcome |
| 4 | `list_second_opinion_evidence` | Three claims, limitations, and conflict groups |
| 5 | `get_second_opinion_user_values` | Lower-burden preference and no-selection boundary |
| 6 | `get_missing_benefit_estimate` | `estimate_status=missing` with explicit unavailable values |

Noema preserved the two conflicting short-term claims in
`benefit-short-term-001`. It kept Option A's lower daily burden separate from
Option B's different study result and higher weekly burden. It did not turn the
conflict into a recommendation.

The task presented gate `gate:da8808e651d24976b7228934638be139`. After approval,
action `action:371b25d10169124ecd13b8194d89300c` called the POST exactly once
with the case ID and exact bounded note. The synthetic response was:

- Request: `estimate-request-001`
- Receipt: `estimate-receipt-001`
- Status: `completed`
- Requested on: `2026-09-09`
- Submission count: `1`
- Option A estimate: `7 percent relative improvement in the synthetic six-week comparison`
- Option B estimate: `12 percent relative improvement in the synthetic six-week comparison`
- Basis: `Synthetic fixture estimate from a bounded comparison record; not a clinical prediction.`
- Locator: `second-opinion://estimates/estimate-request-001`

The final status read then ran once. It returned
`conflict_status=preserved`, `estimate_status=available`, the same request and
receipt, `estimate_submission_count=1`, both estimates, the estimate basis,
and `second-opinion://status/second-opinion-001`.

The service ledger, excluding operator probes and the documentation request,
was exactly:

1. GET `/v1/profile`
2. GET `/v1/option-a`
3. GET `/v1/option-b`
4. GET `/v1/evidence`
5. GET `/v1/user-values`
6. GET `/v1/benefit-estimate`
7. POST `/v1/benefit-estimate-request`
8. GET `/v1/status`

## Sourced artifact

Task `task:d24d6791c996d7af379cd9b2c82e71e3` created exactly one local Markdown
artifact. Planner `run:f535899ee2a2ce5f8303c5e02a520d01`, executor
`run:b2c12494e9e1a6603853b0f9c14976d3`, and reviewer
`run:2669d7e06ae1257ba99d17e7e0ebee6f` completed successfully.

- Artifact: `artifact:b69d5fa0d1f048eef5b2104f4f3fb490`
- Version: `artifact_version:fdbfbd72d5f31826ff876394a727f0b4`
- Title: `Synthetic Second-Opinion Brief — Final`
- Filename: `synthetic-second-opinion-brief-final.md`
- Size: 7,818 bytes
- Media type: `text/markdown`
- Preview: `MARKDOWN`
- Download: `/artifacts/versions/fdbfbd72d5f31826ff876394a727f0b4/download`
- Content SHA-256: `800e96f7f5f2784b97f5d5e5ed69bf519b542a386e22a8c9f0ff23523d10c83c`

The artifact preview was inspected through `artifactVersionDetail`. It
preserves every option field, evidence claim and limitation, conflict group,
user value, estimate state, request receipt, count, final status, and every
`second-opinion://` locator. It separates facts from clinician questions and
states that Noema did not select treatment.

## Acceptance

| Criterion | Result |
| --- | --- |
| Discover the service and propose a connector | Pass; one reviewed eight-operation proposal |
| Keep setup read-only | Pass; only the documentation page was inspected by the setup task |
| Read all source records once in order | Pass; six reads ran once in the specified order |
| Preserve benefits, harms, uncertainty, and conflicting evidence | Pass; both claims and limitations remain visible |
| Apply the person's burden and uncertainty values | Pass; values were carried into the comparison and questions |
| Obtain the missing fixture estimate | Pass; one separately approved POST returned one receipt and count 1 |
| Verify the final estimate status | Pass; one final GET confirmed preserved conflict and available estimate |
| Save one complete sourced brief | Pass; one reviewed 7,818-byte Markdown artifact with preview |
| Avoid treatment selection and real-world action | Pass; synthetic-only boundary was preserved |

The temporary fixture and tunnel were stopped after evidence collection.
