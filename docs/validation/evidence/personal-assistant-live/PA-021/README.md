# PA-021 — Decision history

Verdict: **Pass**

Run date: 2026-09-08

Backend: Go development instance through `/tmp/noema-codex/graphql.sock`

Fixture: `2026-09-08-gmail-v1-notion-mcp-v8` plus synthetic decision facts in
the request

Conversation: `conversation:a39407c5e9686f34225273862829bf0f`

## Execution

Turn `turn:938c5f6c6c13be4899415abbdbc09d42` saved
`artifact:eb04d607cd9c8a37204908f34e722582` as **Launch Vendor Decision D1**.
It records D1 selecting Vendor A, Alex as owner, the support-requirement
rationale, and the supplied synthetic source. It explicitly excludes a
discussion-only mention of Vendor B.

The later approved D2 update was handled in turn
`turn:6135b2768344169e37ae76df9696e39e`. Noema created
`artifact:b39ffff9f1b26b88a2d80cb0b35951ec` as **Launch Vendor Decision History
— D1 and D2**. That record keeps D1 as superseded and records D2 selecting
Vendor B as the current decision, with Alex as owner, a lower-total-cost
rationale, a September 9 approval date, and its source. It preserves the
future-dated approval as supplied and continues to exclude the earlier
discussion-only mention. No vendor was contacted.

## Acceptance checks

| Check | Result | Evidence |
| --- | --- | --- |
| Save D1 with rationale, owner, and source | Pass | D1 artifact `artifact:eb04d607cd9c8a37204908f34e722582`. |
| Exclude discussion-only proposal | Pass | D1 and the history artifact label the earlier Vendor B mention as unaccepted. |
| Record D2 as current replacement | Pass | History artifact `artifact:b39ffff9f1b26b88a2d80cb0b35951ec` marks D2 current and D1 superseded. |
| Preserve D1 rather than delete it | Pass | The D1 section remains in the D1/D2 history artifact. |
| Preserve rationale, owner, approval date, and source | Pass | Both decisions list the requested fields. |
| Avoid external vendor contact | Pass | Both turns created local artifacts only. |

## Limitation

The decision facts and approval were synthetic inputs. The live artifact tool
does not append to an existing artifact by ID, so the replacement is preserved
in a new history artifact that contains both decisions.
