# PA-027 — Credential renewal

Verdict: **Pass after connector contract repair and full synthetic renewal**

Run date: 2026-09-08  
Backend: Go development instance through `/tmp/noema-codex/graphql.sock`  
Fixture: `2026-09-08-credential-api-v1` (synthetic only)  
Conversation: `conversation:a39407c5e9686f34225273862829bf0f`

## Scenario

The service represents one professional credential. It has no licensing
authority and cannot contact a registrar, course provider, or professional
board. All records and state changes are synthetic.

The credential requires 12 credits by October 8, 2026. The fixture contains
eight valid credits, two duplicate records, two expired records, and three
candidate courses. Two eligible two-credit courses cover the four-credit gap.
One later four-credit course is ineligible.

## Connector setup

| Item | Evidence |
| --- | --- |
| Fixture source | [`scripts/acceptance/run-mock-credential-api.ts`](../../../../../scripts/acceptance/run-mock-credential-api.ts) |
| Fixture version | `2026-09-08-credential-api-v1` |
| Documentation URL used by Noema | `https://certified-marble-glasgow-offered.trycloudflare.com/docs` |
| Final proposal revision | `v3`, proposed in `turn:turn-811c0d522b45117174b1d75763b57d22` |
| Accepted definition digest | `63fba62a17642f5a2b20bfe8a44d2cb4f13321ef489fb2d7f5bb8ad7c6df1ec3` |
| Connection | `d20b82d888dd374f7849510e19de7dbd`, revision 4 |
| Connection policy | Sharing allowed automatically; unsafe actions always ask |

The first fixture process used a different tunnel. Its accepted connector
pointed at stale response fields. The operator added compatible response
fields, started the corrected fixture, and accepted a new connector revision.
The final connector reads passed before any state-changing action.

## Execution

1. Noema reviewed the documentation through its browser. The operator approved
   only that documentation read. No API data was read through the browser.
2. Noema proposed the API connector from the documentation. The operator
   accepted the reviewed `v3` proposal and saved its connection policy.
3. In `turn:turn-b760fea7229b79a2d7c93534b01bf590`, the connector read:
   - `get_credential`: 12 required credits and a 2026-10-08 deadline;
   - `list_credits`: eight `VALID` records, two `DUPLICATE` records, and two
     `EXPIRED` records; and
   - `list_courses`: two eligible two-credit courses and one ineligible late
     course.
4. The operator asked Noema to schedule only the eligible courses. Governed
   actions `action:6124b2e83cf72ecfb7936d76f7717d82` and
   `action:55ac233b015237a4b4e0afffb92cdba9` were approved. They returned:
   - `course-privacy-2` → `enrollment-001`;
   - `course-safety-2` → `enrollment-002`.
5. Noema read enrollments after both schedules. The connector returned
   `COMPLETED` for both enrollments and evidence IDs
   `evidence-course-privacy-2` and `evidence-course-safety-2`.
6. Noema showed this exact renewal payload before submission:

   ```json
   {
     "credit_ids": [
       "credit-001", "credit-002", "credit-003", "credit-004",
       "credit-005", "credit-006", "credit-007", "credit-008"
     ],
     "enrollment_ids": ["enrollment-001", "enrollment-002"]
   }
   ```

7. The operator approved governed action
   `action:10dd9df609ee835ab0c208241658ec81`. The connector returned
   `renewal-001` with status `SUBMITTED`.
8. Noema read `renewal-001` through the connector. It returned status
   `APPROVED` in `turn:05b4423cac33cecaf68f11298c58b506`.

## Acceptance checks

| Check | Result | Evidence |
| --- | --- | --- |
| Generate an API proposal from service documentation | Pass | Final `v3` proposal and accepted digest above |
| Connect and configure the reviewed API | Pass | Connection `d20b82d888dd374f7849510e19de7dbd`, revision 4 |
| Read the requirement and deadline | Pass | `get_credential` returned 12 and `2026-10-08` |
| Count only valid credits | Pass | Eight valid one-credit records; duplicates and expired records excluded |
| Select eligible courses | Pass | `course-privacy-2` and `course-safety-2`; late course excluded |
| Schedule only approved courses | Pass | Two governed actions; no late-course action |
| Collect completion evidence | Pass | Two completed enrollments with evidence IDs |
| Show the exact renewal before submission | Pass | Eight valid credit IDs and two enrollment IDs |
| Submit once with approval | Pass | One governed action returned `renewal-001` |
| Verify the final status | Pass | Connector read returned `APPROVED` |
| Avoid real external effects | Pass | The fixture cannot contact a registrar, course provider, board, or payment service |
