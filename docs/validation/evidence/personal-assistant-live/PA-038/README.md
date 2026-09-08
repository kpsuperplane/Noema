# PA-038 — Newsletter reading digest

Verdict: **Pass**

Date: 2026-09-08

Backend: live Go Noema development instance through
`/tmp/noema-codex/graphql.sock`.

## Fixture

- Service: synthetic newsletter API, fixture
  `2026-09-08-newsletter-api-v1`.
- Documentation: `https://residence-loving-pic-commission.trycloudflare.com/docs`.
- Read endpoints: `/v1/profile` and `/v1/newsletters?edition=...`.
- Fixture source: [`scripts/acceptance/run-mock-newsletter-api.ts`](../../../../../scripts/acceptance/run-mock-newsletter-api.ts).
- The fixture is public, unauthenticated, read-only, and synthetic. It has no
  subscription, publishing, messaging, or payment operation.
- Conversation: `conversation:a39407c5e9686f34225273862829bf0f`.

The profile defines the user's preferred topics as technology and design, a
10-minute reading budget, deduplication by `story_id`, and a change rule that
reports only new or revised stories in the updated edition. The baseline has
12 rows and 9 unique story IDs. The updated edition has 13 rows and 10 unique
story IDs. The updated edition changes `story-tech-002` and adds
`story-design-003`.

## Connection setup

Noema read the service documentation and proposed a bounded public connection.
The proposal exposed only these two read-only operations:

- `get_newsletter_profile` — `GET /v1/profile`
- `list_newsletter_stories` — `GET /v1/newsletters` with required `edition`
  query value `baseline` or `updated`.

The proposal turn was `turn:e5167c46c7147c6054680bec7acd1acb`. The reviewed
proposal digest was
`7dc86a4bfeeba5a7f5f6485db994a5d3adf48441548b1aa511e3f4ba61359b6a`. The
accepted reviewed semantic digest was
`6650b7340ecdd6bbc99119abe955bdffb836bce06015da07ff6002595cd1017e`.

The final connection is `8829f05d7d7ae6c98e8124f0d6abe734`, at connection
revision 2 and policy revision 2. Its data policy is `allow_automatically`;
unsafe actions remain `always_ask`. It exposes exactly the two documented
read operations. The initial proposal attempts exceeded the manifest output
limit, then converged to this bounded definition without fetching stories.

## Baseline read

The successful baseline turn was
`turn:cd4075dea303305350fce74e1656b45f`. Both tool calls completed:

- `get_newsletter_profile` returned the documented preferences and rules.
- `list_newsletter_stories` used `edition=baseline` and returned all 12 rows.

Noema reduced the rows to 9 unique stories, then selected four preferred-topic
stories within the budget:

| Topic | Story | Reading time | Original link |
|---|---|---:|---|
| Technology | Local-first sync without silent conflicts | 2 min | https://news.example.test/story-tech-003 |
| Design | Why calm defaults improve form completion | 3 min | https://news.example.test/story-design-002 |
| Technology | Open protocols make personal data easier to move | 3 min | https://news.example.test/story-tech-002 |
| Design | Dense product screens can remain accessible | 2 min | https://news.example.test/story-design-001 |

The selected total is exactly 10 minutes. Duplicate rows for
`story-tech-001` and `story-design-001` were not repeated, and one original
link was retained for each selected story.

## Updated read

The updated comparison turn was
`turn:2df46ab4bfbfd9c51aebb428fb8b4e4c`. Noema called
`list_newsletter_stories` with `edition=updated` and received all 13 rows.
It reported exactly:

- Revised technology story: `story-tech-002`, with its September 8 date and
  revised summary.
- New design story: `story-design-003`, a two-minute story with its original
  link.

All unchanged stories were suppressed. No external write occurred.

## Saved artifact

Turn `turn:69cdba682196c5a91792e5a2628cc84f` saved a local Markdown artifact
through `artifact.create_local_file`. The result was:

- Artifact: `artifact:a75cab935198a719d01247eae1f7bb58`.
- Version: `artifact_version:f298d78a3df4fe0fb68f4d71a1f92f27`.
- Filename: `newsletter-reading-digest.md`.
- Size: 1,649 bytes.
- Media type: `text/markdown`.

The file was inspected through the read-only development home. It contains the
four selected stories, their original links, the 10-minute total, the
deduplication note, and a “What changed” section containing only the revised
technology story and new design story.

## Acceptance checks

| Requirement | Result |
| --- | --- |
| Read service documentation before connecting | Pass. The agent opened the supplied guide before proposing the connection. |
| Generate, review, and accept a bounded read-only connection | Pass. The final definition has exactly two documented GET operations. |
| Read the profile and baseline edition | Pass. The profile and all 12 baseline rows were returned. |
| Deduplicate by story ID | Pass. The baseline reduced from 12 rows to 9 unique stories. |
| Respect preferences and reading budget | Pass. Four technology/design stories total exactly 10 minutes. |
| Keep original links | Pass. Each selected story has its source URL. |
| Detect only updated information | Pass. One revised and one new story were reported; unchanged stories were suppressed. |
| Save and inspect the digest | Pass. The 1,649-byte Markdown artifact contains the required sections and values. |
| Avoid external writes | Pass. The service is read-only and only a local Noema artifact was created. |
