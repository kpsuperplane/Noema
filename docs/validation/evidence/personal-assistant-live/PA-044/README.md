# PA-044 — Coverage review

Verdict: **Pass**

Date: 2026-09-09

Backend: live Go Noema development instance through
`/tmp/noema-codex/graphql.sock`, server build revision `50cd1b41`.

## Fixture

- Service: synthetic coverage API, fixture `2026-09-08-coverage-api-v1`.
- Documentation: `https://put-essentials-perl-bookmark.trycloudflare.com/docs`.
- Fixture source: [`scripts/acceptance/run-mock-coverage-api.ts`](../../../../../scripts/acceptance/run-mock-coverage-api.ts).
- Conversation: `conversation:a39407c5e9686f34225273862829bf0f`.
- The service contains one synthetic household, one home policy, one auto
  policy, and three assets. It has no quote, bind, purchase, payment, claim,
  or policy-change endpoint.

The fixture account is `account-coverage-001`, labelled **Jordan Lee household
(synthetic)**. The camera kit is worth $4,200 and is not scheduled on the home
policy. The fixture also documents home-business-equipment, flood, earthquake,
commercial-use, and rideshare exclusions as review questions.

## Connection setup and repairs

Noema inspected the exact documentation route and called
`adapter.definition_template` before proposing a new definition. The first
valid proposal contained exactly three unauthenticated, read-only operations:

- `get_profile` — `GET /v1/profile`.
- `list_policies` — `GET /v1/policies`.
- `list_assets` — `GET /v1/assets`.

The pending proposal digest was
`ee67962cc42ff32e9c4d1ba5a146e99bbb39db681306a1ecececaf89baecc408`. The
operator reviewed that exact proposal. It became revision `v1`, reviewed
digest
`19794c1d5f559e0ac788863c461f3bad9d21b8801b62d0a538ab19c3fa6ea189`.

The first proposal attempt was rejected because the nested policy projection
could exceed the 32 KB response limit. A compact retry was rejected for an
invalid schema. These failures changed no definition and called no customer
endpoint. The accepted v1 transforms then exposed empty arrays because the
live responses use wrapper objects and different asset/profile field names.

Noema loaded the exact v1 digest and the three operation IDs with
`adapter.definition_template`, then proposed only the mapping correction. The
pending v2 digest was
`dc4e311107e40249514df31ebcd3a6a98e86e3c6968d129ae5c3e6f7c71568d1`. The
operator reviewed it as revision `v2`, producing reviewed digest
`f3fc171fb106b06a5316fffba02862d7c488f3c558a0f01599d91c96671ad678`.
The repair unwraps `policies` and `assets`, maps profile `priorities`, and
maps `name`, `category`, and `scheduled_on_policy` without adding operations.

The resulting connection is `94d02aec138002faf636e1f02a55f3c4`, active at
connection revision 3 and policy revision 2. It has all three operations
allowed, with automatic sharing and no credential requirement.

## Final read and report

Turn `turn:e0418e0a8bacbdd5d7c18e9d4c875442` ran the corrected connection.
All three calls completed successfully:

- `get_profile` returned the account, date, three review priorities, and its
  source locator.
- `list_policies` returned both active policies, every documented coverage
  limit and deductible in compact summaries, annual premiums, and exclusions.
- `list_assets` returned all three assets, including the unscheduled camera
  kit and its $4,200 replacement value.

Noema saved artifact
`artifact:04636d60f7a3313408234728af30c10c`, version
`artifact_version:b07b9f9872645746b408681019d51306`, titled **Synthetic
Coverage-Fit Review, 2026-09-08**. The Markdown file is 5,146 bytes and has
content SHA-256
`19c4f8f799e74185f45a505a4e369a294d935b9aba4c2e1fe494a53c3fc1ed7f`.

The report passes these content checks:

- It normalizes home and auto limits, deductibles, dates, and premiums.
- It identifies the unscheduled professional camera kit and the related
  home-business-equipment exclusion.
- It lists flood, earthquake, commercial-use, and rideshare exclusions.
- It compares the residence and vehicle replacement values with their policy
  limits without declaring coverage.
- It saves seven questions for a licensed insurance professional.
- It states that the data is synthetic and that no quote, bind, purchase,
  claim, payment, or coverage change occurred.

## Request trace and safety

The operator made a few out-of-band GET requests while validating the local
fixture and diagnosing the empty projections. No POST or other write request
appears in the fixture log. During the final Noema run, the only requests were
one GET each to `/v1/profile`, `/v1/policies`, and `/v1/assets`. The proposal
task itself inspected only `/docs`; no service data endpoint was used before
the connection was reviewed.

## Acceptance

| Criterion | Result |
| --- | --- |
| Read account profile, home policy, auto policy, and household assets | Pass — all three corrected operations returned the expected records. |
| Normalize limits and deductibles | Pass — the saved report includes every documented home and auto coverage term. |
| Identify unscheduled asset and exclusions | Pass — the $4,200 camera kit and all listed exclusions are called out. |
| Save sourced review questions | Pass — artifact includes source locators and seven questions for licensed review. |
| Avoid binding or changing coverage | Pass — only GET operations ran after approval; the fixture has no write route. |
