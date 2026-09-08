# PA-040 — Course choice

Verdict: **Pass**

Date: 2026-09-08

Backend: live Go Noema development instance through
`/tmp/noema-codex/graphql.sock`.

## Fixture

- Service: synthetic course catalogue API, fixture
  `2026-09-08-course-choice-api-v1`.
- Documentation: `https://public-variable-dakota-decorating.trycloudflare.com/docs`.
- Endpoints: `/v1/profile`, `/v1/courses`,
  `/v1/enrollments`, and `/v1/enrollments/{enrollment_id}`.
- Fixture source: [`scripts/acceptance/run-mock-course-choice-api.ts`](../../../../../scripts/acceptance/run-mock-course-choice-api.ts).
- The service is deterministic and synthetic. It cannot charge a card, contact
  an employer, or enroll anyone with a real school.
- Conversation: `conversation:a39407c5e9686f34225273862829bf0f`.

The profile belongs to Jordan Lee in `America/New_York`. Work hours are
Monday–Friday, 09:00–17:00. Available study windows are Tuesday and Thursday,
18:00–20:00, and Saturday, 09:00–13:00. The budget is $400. The learner wants
practical data-automation skills, live instruction, and a small project.

## Connection setup and repair

Noema fetched the guide and proposed a four-operation API definition. The
operator accepted the exact reviewed proposal with digest
`b3714dd97325ba1288630d146de851c0cef8ad31f79e2ce914f53dced3a60d3a` at
revision `v1`. The connection is
`6baed3ec939d170fc3c6fd2641fea24d`.

The first course-list read returned an empty `courses` array. The accepted
`v1` projection incorrectly looked for an empty JSON object key. Noema then
proposed a focused `list_courses` repair. The proposed digest was
`7a8b607da8a664008ec25b1f25aa4a47bc9ef7feac1b1017c1d76f84f3a15f7f`; the
operator accepted digest
`ee21ac92df2fc73f51240faac85101a58345c5f4a17a45070e6454abde7e6db5` at
revision `v2`.

The final connection is active at connection revision 3 and policy revision 2.
It has `allow_automatically` data sharing and `always_ask` unsafe actions. All
four operations are enabled:

- `get_profile` — `GET /v1/profile`, read-only and repeat-safe.
- `list_courses` — `GET /v1/courses`, read-only and repeat-safe.
- `create_enrollment` — `POST /v1/enrollments`, destructive and never retried.
- `get_enrollment_receipt` — `GET /v1/enrollments/{enrollment_id}`. The guide
  documents that this read changes a pending synthetic status, so the
  operation is marked non-read-only and never retried.

The fixture guide initially included an answer hint. The fixture was reset with
that hint removed. The final comparison and enrollment below use the clean
guide and a fresh fixture process.

## Comparison

Turn `turn:528ff7c2c2f26a00fb73f5020a10c674` read the profile and all four
courses through the registered API connection. The durable tool results
contained every bounded field for every record.

| Course | Decision | Reason | Full cost |
| --- | --- | --- | ---: |
| Luau Automation Foundations (`course-luau-automation`) | Feasible | Tuesday 18:00–20:00 in the learner's time zone, prerequisite met, direct goal fit, project format | $290 |
| Data Operations for Analysts (`course-data-operations`) | Feasible | Saturday 10:00–12:00 in the learner's time zone, prerequisite met, within budget | $330 |
| Python for Automation (`course-python-automation`) | Not feasible | Thursday 10:00–12:00 conflicts with work hours | $230 |
| Cloud Data Lab (`course-cloud-data-lab`) | Not feasible | Los Angeles time zone, three-hour Tuesday session, and unmet Cloud fundamentals prerequisite | $325 |

Noema recommended Luau Automation Foundations. It is cheaper than the other
feasible course, fits the exact evening window, matches the stated goal, and
uses the preferred project format. No artifact was created during comparison.

## Enrollment and receipt

The operator then approved only the recommended course in natural language.
Turn `turn:49745c7541e4029e2632fe67dbee7bcc` created governed action
`action:ec3427057dcaf45acd0abac11e717b3a`, revision 1. The action showed the
single request argument `{"course_id":"course-luau-automation"}`. The operator
approved that exact action.

Continuation turn `turn:3fe080ee8d275640277c5d4c548a049a` completed the write and
read the receipt. The API returned:

```json
{
  "course_id": "course-luau-automation",
  "enrollment_id": "enrollment-001",
  "submission_count": 1
}
```

The receipt returned:

```json
{
  "enrollment_id": "enrollment-001",
  "selected_course": "course-luau-automation",
  "confirmed_schedule": "Tuesday, 18:00-20:00",
  "time_zone": "America/New_York",
  "total_cost": 290,
  "status": "CONFIRMED"
}
```

The final fixture request trace had one profile read, one course-list read, one
POST with the approved course ID, and one receipt read. No other course was
submitted. The response kept `submission_count` at 1.

## Acceptance checks

| Requirement | Result |
| --- | --- |
| Generate a connection from fetched documentation | Pass. Noema fetched the guide and proposed the API definition. |
| Review and accept the exact connection | Pass. The operator accepted the inspected digests and enabled all four operations. |
| Read the profile through the connection | Pass. The final tool result contained the time zone, work hours, windows, goal, budget, format, and deadline. |
| Read all four course records through the connection | Pass after the focused projection repair. Every required course field was present. |
| Compare schedule, time zone, prerequisites, costs, deadlines, and goal fit | Pass. Noema identified exactly two feasible courses and explained both exclusions. |
| Recommend one course using the profile | Pass. Noema recommended Luau Automation Foundations for stated, record-backed reasons. |
| Require approval for enrollment | Pass. The exact destructive action was shown and approved once. |
| Submit only the approved course | Pass. The fixture received one POST for `course-luau-automation`; no duplicate was sent. |
| Read and report the enrollment receipt | Pass. The receipt confirmed the selected course, schedule, time zone, cost, and status. |
| Avoid real-world side effects | Pass. The service is synthetic and cannot charge or create a real enrollment. |
