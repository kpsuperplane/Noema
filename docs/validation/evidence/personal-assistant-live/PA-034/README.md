# PA-034 — Literature review

Verdict: **Pass**

Date: 2026-09-08

Backend: live Go Noema development instance through
`/tmp/noema-codex/graphql.sock`.

## Fixture

- Service: synthetic literature-index API, fixture
  `2026-09-08-literature-api-v1`.
- Documentation: `https://newfoundland-rolling-statute-granted.trycloudflare.com/docs`.
- Read endpoints: `/v1/profile`, `/v1/index-a/search`, and
  `/v1/index-b/search`.
- Fixture source: [`scripts/acceptance/run-mock-literature-api.ts`](../../../../../scripts/acceptance/run-mock-literature-api.ts).
- The fixture has no publisher, scholarly index, author, or account. All
  profile and paper records are synthetic. Every endpoint is read-only.
- Conversation: `conversation:a39407c5e9686f34225273862829bf0f`.

The two indexes returned six records each. Three works appeared in both
indexes under different source record IDs and URLs. One work was marked
retracted. The fixture therefore contained 12 records and nine unique works.

## Connection setup

Noema fetched the documentation through the browser before proposing a
connection. The documentation review actions were
`action:d22b2af7ff3e5ca68bd76e6bf09ce632`,
`action:a264e5d8fae2d98cf4ee960359e7cab7`,
`action:270b92b1743582be2d17418b9d0ee22b`, and
`action:9cfb40631559d76e07e5131d80c46595`.

The first proposal digest was
`58429f4e6a351cb3a655f8309a282babcb125f8375959a9bdcce0f0bda29a1f7`.
Inspection found that its search projection omitted source record IDs,
titles, and authors. Noema proposed a focused revision with those fields and
the required evidence fields. The revision reached review with digest
`6dcb42825a0695f77f3d7d890060422afa775ba6dfa6e8fc196c702f164a8790`.

The operator accepted that exact revision. The reviewed definition returned
semantic digest
`1021f98bc91dc7378859c1f2963e1584bcf54162ce84c7b76df8b700076be243`.
Connection `061250644be8e8f2ea30db41e0b845a3` exposed only the three
documented GET operations. It was at connection revision 2 and policy
revision 2 after setup. The data policy was `allow_automatically`, and the
unsafe action policy was `always_ask`.

## Live read and synthesis

Turn `turn:94168139f97e902e38b5040c08ba36ba` read the profile and both index
searches through the accepted connection. The profile result preserved the
question and scope. Both searches used the query
`four-day workweek wellbeing service output` with `limit=6`.

- Profile result: `conversation_item:2447`.
- Northstar result: `conversation_item:2449`.
- Harbor result: `conversation_item:2451`.

The first synthesis continuation was canceled after a provider failure. It
had already completed all three reads, and no duplicate read was submitted.
Turn `turn:d953d8f3de6fd7b80ae75d23f826c703` then produced the concise review
from those stored results. Its final item was `conversation_item:2454`.

The review identified these duplicate pairs:

- `work-002`: `northstar-002` and `harbor-001`.
- `work-004`: `northstar-004` and `harbor-002`.
- `work-006`: `northstar-006` and `harbor-003`.

It compared randomized trials, a cohort, qualitative interviews, a
meta-analysis, a difference-in-differences study, a survey, and a protocol.
It preserved mixed output findings, identified staffing and implementation as
important conditions, and excluded retracted `work-006` from substantive
support. It also stated gaps in long-term retention, patient outcomes,
service quality, and coverage reliability.

## Saved artifact

Turn `turn:c511100ea822f6e0c42be9f525cedcb5` saved the review as a local
Markdown artifact. The result was `conversation_item:2458`, followed by the
completion message in `conversation_item:2459`.

- Artifact: `artifact:0fed6c2f5073b58a6c0f73b3d0c36991`.
- Version: `artifact_version:a2a376d0e3072811b84d2107a13511e8`.
- Filename: `four-day-workweek-evidence-review.md`.
- Size: 6,906 bytes.
- Media type: `text/markdown`.
- Preview kind: `MARKDOWN`.

The saved table preserves every returned source record ID, work ID, DOI,
title, authors, design, population, finding, limitation, status, retraction
notice where present, and source URL. It also records the query, per-index
limit, total records, deduplication rule, duplicate pairs, mixed findings,
and evidence gaps. The file was inspected directly through the read-only
development home.

## Acceptance checks

| Requirement | Result |
| --- | --- |
| Read documentation before connecting | Pass. Noema fetched the public fixture documentation before proposing the connection. |
| Generate, review, and accept a bounded read-only connection | Pass. The accepted definition exposes only the profile and two documented GET searches. |
| Read all required sources through the registered connection | Pass. Profile and both six-record searches completed through connection `061250644be8e8f2ea30db41e0b845a3`. |
| Preserve source identity and evidence fields | Pass. Search projections retain source record IDs, work IDs, DOIs, titles, authors, designs, populations, findings, limitations, status, retraction notice, and URLs. |
| Deduplicate cross-index records | Pass. Twelve records were reduced to nine unique works, with three duplicate pairs listed. |
| Mark and exclude the retracted work | Pass. `work-006` and both source records were marked retracted and excluded from substantive support. |
| Compare methods, conflicts, and gaps | Pass. The review distinguishes causal strength, output measures, staffing conditions, mixed findings, and missing evidence. |
| Save and inspect the requested artifact | Pass. A 6.9 KB Markdown artifact was saved and inspected. |
| Avoid external writes | Pass. The fixture exposes only GET operations, and Noema made no service write or contact. |

No real publisher, research, employment, health, payment, or other external
account was used.
