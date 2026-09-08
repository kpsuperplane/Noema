# PA-028 — Job-search pipeline

Verdict: **Pass after connector revision, Go adapter enablement repair, and
synthetic end-to-end rerun**

Run date: 2026-09-08
Backend: Go development instance through `/tmp/noema-codex/graphql.sock`
Fixture: `2026-09-08-job-search-api-v1` (synthetic only)
Conversation: `conversation:a39407c5e9686f34225273862829bf0f`

## Scenario

The service contains one person's job-search constraint, five jobs, and three
applications. The constraint allows San Francisco or fully remote work in the
United States. Two jobs are onsite outside San Francisco. Beacon Labs has a
changed deadline. The service can update that deadline and send one synthetic
recruiter follow-up with a receipt.

The service cannot contact a job board, employer, recruiter, or mail provider.
All records and writes are synthetic.

## Fixture and connector setup

| Item | Evidence |
| --- | --- |
| Fixture source | [`scripts/acceptance/run-mock-job-search-api.ts`](../../../../../scripts/acceptance/run-mock-job-search-api.ts) |
| Fixture version | `2026-09-08-job-search-api-v1` |
| Documentation URL used by Noema | `https://prospect-cheese-websites-ant.trycloudflare.com/docs` |
| Final accepted proposal | v2, accepted semantic digest `7bb4b374302b12e3ec0d50542d3f2312fdab04622d83c2faa3a4d7d1cba30ff2` |
| Final connection | `513d7368a6544fb5b5539234e0df3f3a`, revision 2 |
| Connection policy | Sharing allowed automatically; unsafe actions always ask |

Noema first proposed a read-only v1 connector. The operator accepted it and
Noema read the profile, jobs, and applications. A v2 revision added only the
documented deadline update, follow-up creation, and receipt-read operations.
The Go adapter retained the three previously allowed operations when the
revision was accepted. This exposed a parity defect: the new operations were
visible in settings but could not be requested or enabled in the live tool
catalog.

The repair now exposes a separate `enable.<operation>` binding for each
disabled operation. The enable action requires human review. A focused parity
test verifies that direct invocation is rejected, reviewed enablement restores
the operation, and stale authority is rejected. The live run then reconnected
the synthetic service with all six v2 operations enabled so the case could
exercise the deadline and follow-up contracts.

## Execution

1. Noema read the service documentation through its browser. The operator
   approved only that documentation read.
2. Noema proposed and the operator accepted the read-only v1 API connector.
3. In `turn:d2f1911a890f02a8a2038997a3d679b9`, Noema read:
   - the San Francisco or remote-U.S. location constraint;
   - all five jobs, including Beacon's changed deadline; and
   - all three applications with their stages and next actions.
4. Noema saved a read-only tracker and follow-up draft as
   `artifact:bb460e28ff55f45415315e8f0f526fbf`, version
   `artifact_version:620e0313b33eded8aeb21d89a3798db7`.
5. The operator accepted the v2 connector revision listed above. The old
   connection was deleted and the synthetic connector was reconnected so the
   live run had all six documented operations available.
6. In `turn:f03eba05fb7edc4b2842bb2e2bd64d87`, Noema showed a multiple-choice
   approval for exactly `job-002` and `2026-09-18`. The operator selected the
   approval option. Noema then created governed action
   `action:967b92b18df3cf0bf341bc9ce0bb422d`, revision 1. The operator approved
   it. The connector returned `{"id":"job-002","application_deadline":"2026-09-18"}`.
7. The operator asked Noema to send the drafted Beacon follow-up and read its
   receipt. Noema created governed action
   `action:514cbf899e829935da65ee9a9181de7b`, revision 1. Its reviewed payload
   targeted `application-002` and contained the saved draft. The operator
   approved it once. The connector returned `follow-up-001` with status
   `SENT`.
8. Noema immediately read `follow-up-001`. It returned receipt ID
   `receipt-follow-up-001`, application `application-002`, status `SENT`, and
   the same message. Two independent read-back requests returned identical
   records, each with `submission_count: 1`.

## Acceptance checks

| Check | Result | Evidence |
| --- | --- | --- |
| Generate an API proposal from service documentation | Pass | v1 and v2 reviewed proposals and accepted digest above |
| Preserve the user's location rule | Pass | Profile read returned San Francisco or fully remote within the United States |
| Read all five jobs | Pass | `list_jobs` returned `job-001` through `job-005` |
| Exclude unsuitable roles | Pass | Cinder Research (New York onsite) and Delta Commerce (Austin onsite) were excluded |
| Preserve all three application stages and next actions | Pass | `list_applications` returned `SCREENING`, `APPLIED`, and `INTERVIEW` records with next actions |
| Identify the changed Beacon deadline | Pass | `job-002` changed from `2026-09-10` to `2026-09-18` |
| Save a useful tracker and draft | Pass | Artifact ID and version above |
| Show the exact deadline action before changing it | Pass | Multiple-choice prompt named `job-002` and `2026-09-18` |
| Update the deadline with human review | Pass | Governed action `action:967b92b18df3cf0bf341bc9ce0bb422d` succeeded |
| Show the follow-up before sending | Pass | Reviewed action payload contained the complete Beacon draft |
| Send one follow-up only | Pass | Governed action `action:514cbf899e829935da65ee9a9181de7b` succeeded once |
| Read the returned receipt | Pass | `follow-up-001` and `receipt-follow-up-001` read through Noema |
| Avoid duplicate submission | Pass | Fixture read-back remained `submission_count: 1` on both reads |
| Avoid real external effects | Pass | The fixture has no external job, employer, recruiter, email, or payment connection |

## Implementation check

Focused Go checks passed after the repair:

```text
go test ./internal/adapter ./internal/graphql ./internal/runtime
go test ./internal/adapter -run TestRustAdapters_reviewed_enablement_restores_one_disabled_adapter_tool -count=1 -v
```

The focused test covers the cross-backend contract that was missing in Go:
disabled operations remain visible as human-reviewed enable actions, direct
enablement cannot bypass review, and approved enablement restores one direct
operation without changing the connection identity.
