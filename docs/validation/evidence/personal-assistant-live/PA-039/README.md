# PA-039 — Adaptive learning plan

Verdict: **Pass**

Date: 2026-09-08

Backend: live Go Noema development instance through
`/tmp/noema-codex/graphql.sock`.

## Fixture

- Service: synthetic learning API, fixture `2026-09-08-learning-api-v1`.
- Documentation: `https://extend-consultant-performs-modifications.trycloudflare.com/docs`.
- Read endpoints: `/v1/profile`, `/v1/plan`, and
  `/v1/progress?snapshot=...`.
- Fixture source: [`scripts/acceptance/run-mock-learning-api.ts`](../../../../../scripts/acceptance/run-mock-learning-api.ts).
- The fixture is public, unauthenticated, read-only, and synthetic. It has no
  enrollment, grading, payment, or learner-account write operation.
- Conversation: `conversation:a39407c5e9686f34225273862829bf0f`.

The profile describes an eight-week Luau course with a 180-minute weekly cap.
The baseline plan has required and optional blocks in each week. The initial
progress snapshot has a 55-point prerequisite score against a 70-point passing
score, two missed 60-minute sessions, and no completed catch-up sessions. The
follow-up snapshot has an 82-point retry, both catch-up sessions completed, and
current week 5. The repeat snapshot is identical to the follow-up.

## Connection setup and projection repair

Noema read the service documentation and proposed a public connection with
three read-only operations:

- `get_learning_profile` — `GET /v1/profile`
- `get_learning_plan` — `GET /v1/plan`
- `get_learning_progress` — `GET /v1/progress` with required `snapshot` query
  value `initial`, `followup`, or `repeat`.

The first accepted definition was reviewed at semantic digest
`3f78355fd3e15eb129394c65d139364aebda3b71071561bcd429e1f976c0ba02`. Its
progress projection did not map the fixture's nested quiz and catch-up fields.
The initial read exposed that gap: it returned only current week, missed
sessions, and snapshot. Noema then proposed a focused revision rather than
changing the other operations.

The repair proposal digest was
`2d048db72032fb96e0b431167068c35bafe3aeeb027a1b69f034f357043de205`. The
accepted reviewed semantic digest is
`c4e915f403925757f9aa34aa18f4afb1aa5f060347d6551fd717a259ed2ed0f4` at
definition revision `v4`. The final connection is
`e983da690d0d440d38af1dba7e63e7de`, at connection revision 3 and policy
revision 2. Its data policy is `allow_automatically`; unsafe actions remain
`always_ask`. The final progress tool maps the nested quiz score and passing
score and counts completed catch-up entries.

## Initial recovery plan

The first planning turn was `turn:fa884ea096803463384f4dee12e68188`. Its
initial projection exposed the omission above, so it is not used as the final
source read. After the repair, turn
`turn:7c91a044c75ed60aeb9574682457da9e` read the initial snapshot and the
durable tool result contained:

- score: 55;
- passing score: 70;
- missed sessions: 2;
- completed catch-up sessions: 0;
- current week: 3.

The recovery plan uses the remaining 60-minute week-3 slot for prerequisite
review. It puts one missed session in each of the week-4 and week-5 optional
blocks, keeps Module 2 locked until the score reaches 70, and keeps every week
at exactly 180 minutes.

The verified local artifact is:

- Artifact: `artifact:3bdc3487f65cba2953318614d2bece75`.
- Version: `artifact_version:f5c4398db82c393cbb9d582402f544cc`.
- Filename: `adaptive-learning-plan-verified.md`.
- Size: 1,755 bytes.

It was inspected through the read-only development home. The file contains
the corrected tool values, the two make-up blocks, the prerequisite lock, and
the weekly arithmetic.

## Follow-up pacing change

Turn `turn:716661b6b6f848c9dac49a7c8c5d7ff0` read the follow-up snapshot
through the repaired connector. The durable tool result contained score 82,
passing score 70, zero missed sessions, two completed catch-up sessions, and
current week 5.

Noema moved Module 2 into week 5, the current week, and removed the completed
make-up blocks. It retained 120 required minutes plus 60 optional minutes in
week 5 and kept every other week at or below 180 minutes.

The follow-up artifact is:

- Artifact: `artifact:9d5206d12ed04486170dfa275c53c09f`.
- Version: `artifact_version:76ea485f0e6df8a0864a5928f03c8db9`.
- Filename: `adaptive-learning-plan-followup.md`.
- Size: 1,666 bytes.

The file was inspected through the read-only development home. It explicitly
states that Module 2 starts in week 5 because the prerequisite passed and the
catch-up work is complete.

## Unchanged check

Turn `turn:2881b5f1986852823ae2a419a6a18cc4` read `snapshot=repeat` through the
repaired connector. The result exactly matched the follow-up values. No
artifact tool call occurred, and no artifact was created or revised.

## Acceptance checks

| Requirement | Result |
| --- | --- |
| Read service documentation before connecting | Pass. Noema opened the supplied guide before proposing the connection. |
| Generate, review, and accept a bounded read-only connection | Pass. The final connection has exactly three documented GET operations. |
| Read the eight-week plan | Pass. The tool returned all eight week records with required and optional minutes. |
| Expose the failed prerequisite and missed sessions through the connector | Pass after a focused projection repair. The final tool returned 55 versus 70, two misses, and zero catch-up sessions. |
| Address the prerequisite gap | Pass. The recovery plan reserves review and keeps Module 2 locked until 70. |
| Reschedule missed sessions within the weekly limit | Pass. One 60-minute make-up block is placed in each of weeks 4 and 5; no week exceeds 180 minutes. |
| Save the revised plan | Pass. The verified Markdown artifact was saved and inspected. |
| Change pacing after later progress | Pass. Score 82 and completed catch-up work unlock Module 2 in week 5. |
| Stay quiet when progress is unchanged | Pass. The repeat snapshot matches exactly and creates no artifact revision. |
| Avoid external writes | Pass. The service is read-only, and only local Noema artifacts were created. |
