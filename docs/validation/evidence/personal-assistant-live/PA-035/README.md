# PA-035 — Fact-checking

Verdict: **Pass**

Date: 2026-09-08

Backend: live Go Noema development instance through
`/tmp/noema-codex/graphql.sock`.

## Fixture

- Service: synthetic fact-check API, fixture
  `2026-09-08-fact-check-api-v1`.
- Documentation: `https://boxed-gates-buddy-sponsorship.trycloudflare.com/docs`.
- Read endpoints: `/v1/profile`, `/v1/claims`, and `/v1/sources`.
- Fixture source: [`scripts/acceptance/run-mock-fact-check-api.ts`](../../../../../scripts/acceptance/run-mock-fact-check-api.ts).
- The fixture has no regulator, utility, account, or external write. All
  claims and primary records are synthetic.
- Conversation: `conversation:a39407c5e9686f34225273862829bf0f`.

The fixture contains five claims about the Northstar 2026 heat-pump rebate
program. Its five dated primary records include current rules, a correction,
a superseded draft, an eligibility glossary, and a monitoring plan. The set
deliberately contains one supported claim, one contradiction, one outdated
claim, one imprecise definition, and one claim with no supporting evidence.

## Connection setup

Noema read the documented API guide before proposing a connection through
`web.browse.open` and `web.browse.snapshot` in setup turn
`turn:ac1a21c2eb52a705b7feae54f538f51e`. A later request for the undocumented
`/openapi.json` URL created governed action
`action:0f8448eda5eb441b0470d12a0e94ac97`; the operator declined it. Noema
continued from the documented endpoints.

The first setup request failed before reading the guide because the model
delegated unexpectedly. A direct recovery request completed the setup. Two
oversized raw projections were rejected during proposal construction. Noema
then submitted bounded JSON projections for the three documented GET
operations:

- `get_profile` — `GET /v1/profile`
- `list_claims` — `GET /v1/claims`
- `list_primary_sources` — `GET /v1/sources`

The final proposal digest was
`1bec42fddfd9db7240ebd76c2e7720168b9a0febb86bcc32f97817b13aa74d9b`.
The operator accepted that reviewed proposal. Its semantic digest was
`d3e3624d4e27c5fd752dfe3c723f64118dbaf754acb450ac548ce5fb18e53c46`.

Connection `cd40686305edafaaa6214a598f54ac5b` exposed only those three
read-only operations. It was at connection revision 2 and policy revision 2
with data policy `allow_automatically` and unsafe action policy `always_ask`.

## Live read and review

Turn `turn:128fe8c67115ae8a471abd3f43fa15f6` read the profile, all five
claims, and all five linked primary records through the accepted connection.

- Profile result: `conversation_item:2496`.
- Claims result: `conversation_item:2499`.
- Primary-source result: `conversation_item:2501`.
- Final five-row review: `conversation_item:2503`.

The review reached the required findings:

- `claim-001` was **Supported** by the current final rules.
- `claim-002` was **Contradicted** by the income-tier caps and correction.
- `claim-003` was **Contradicted** by the current 40% rate; its 30% value
  was identified as coming from a superseded draft.
- `claim-004` was kept **Imprecise** because “low-income” is not a defined
  program field; the review preserved the 80% area-median-income definition.
- `claim-005` had **No supporting evidence in the cited records**. The review
  explicitly stated that missing evidence is not proof of a false forecast.

Every row cites source IDs, publication dates, and source URLs. No service
write or external contact occurred.

## Saved artifact

Turn `turn:90857e78ca30906022eebb465cecd29b` saved the review as a local
Markdown artifact. The result was `conversation_item:2507`, followed by the
completion message in `conversation_item:2508`.

- Artifact: `artifact:495dace98ff9a270457927bef14ad906`.
- Version: `artifact_version:e0b1835b54f9c3cb8109c75abf949dfe`.
- Filename: `northstar-rebate-fact-check.md`.
- Size: 3,701 bytes.
- Media type: `text/markdown`.
- Preview kind: `MARKDOWN`.

The file was inspected through the read-only development home. It retains the
five-row table, findings, uncertainty, source IDs, dates, URLs, and the note
that absent evidence is not disproof.

## Acceptance checks

| Requirement | Result |
| --- | --- |
| Read documentation before connecting | Pass. Noema used the documented guide; an undocumented OpenAPI browse was declined. |
| Generate, review, and accept a bounded read-only connection | Pass. The accepted connection has exactly three documented GET operations. |
| Read all required records through the registered connection | Pass. Profile, five claims, and five primary records completed through connection `cd40686305edafaaa6214a598f54ac5b`. |
| Classify supported, false, outdated, ambiguous, and unsupported claims | Pass. The review used equivalent careful labels and explained the contradiction, superseded draft, imprecise term, and missing evidence. |
| Cite dated primary evidence | Pass. Each row includes source ID, publication date, and URL. |
| Preserve uncertainty and definition ambiguity | Pass. “Low-income” remains imprecise, and the demand forecast is unsupported rather than declared false. |
| Save and inspect the requested artifact | Pass. A 3.7 KB Markdown artifact was saved and its `MARKDOWN` preview was verified. |
| Avoid external writes | Pass. The fixture is read-only, and Noema made no service write or contact. |

No real regulator, utility, household, payment, or external account was used.
