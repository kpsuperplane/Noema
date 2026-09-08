# Live personal-assistant acceptance ledger

Updated: 2026-09-08

This ledger records the fresh Go-backend acceptance run. It does not import
the superseded replay report. Every case is run through the live Noema
development instance and inspected before the next case starts.

| Case | Verdict | Evidence |
| --- | --- | --- |
| Setup 1 — Gmail API | In progress | OAuth and live reads work. Full setup recovery, rate-limit, account-boundary, and delegated reuse checks remain. |
| Setup 2 — Notion MCP | In progress | OAuth, discovery, pagination, nested reads, expiry recovery, and account isolation work. Full setup evidence and delegated reuse remain. |
| Calendar prerequisite | Pass | Documentation proposal, reviewed v3 revision, account-A read, pagination correction, and no-write rerun all work. Full downstream reuse remains. |
| PA-001 — Daily brief | Pass | [Case evidence](PA-001/) |
| PA-002 — Promise register | Pass | [Case evidence](PA-002/) |
| PA-003 — Incoming actions | Pass | [Case evidence](PA-003/) |
| PA-004 — Reply queue | Pass after repair and rerun | [Case evidence](PA-004/) |
| PA-005 — Calendar audit | Pass after connector revision and rerun | [Case evidence](PA-005/) |
| PA-006 — Daily plan | Pass after correction and full-document reread | [Case evidence](PA-006/) |
| PA-007 — Disruption replan | Pass after delay, draft inspection, and unchanged rerun | [Case evidence](PA-007/) |
| PA-008 — Weekly review | Pass after bounded-range correction and full source review | [Case evidence](PA-008/) |
| PA-009 — Deadline tracking | Pass after source disambiguation, one approved overdue notice, and restart recovery | [Case evidence](PA-009/) |
| PA-010 — Goal review | Pass after week-boundary correction | [Case evidence](PA-010/) |
| PA-011 — Conflicting requests | Pass | [Case evidence](PA-011/) |
| PA-012 — Conversation monitoring | Pass after resolved and overdue branches | [Case evidence](PA-012/) |
| PA-013 — Meeting coordination | Pass after reviewed synthetic write setup and read-back | [Case evidence](PA-013/) |
| PA-014 — Meeting preparation | Pass | [Case evidence](PA-014/) |
| PA-015 — Audience updates | Pass | [Case evidence](PA-015/) |
| PA-016 — Meeting follow-through | Pass | [Case evidence](PA-016/) |
| PA-017 — Relationship brief | Pass | [Case evidence](PA-017/) |
| PA-018 — Relationship cadence | Pass | [Case evidence](PA-018/) |
| PA-019 — Project status | Pass | [Case evidence](PA-019/) |
| PA-020 — Project risks | Pass | [Case evidence](PA-020/) |
| PA-021 — Decision history | Pass | [Case evidence](PA-021/) |
| PA-022 — Ambiguous goal | Pass | [Case evidence](PA-022/) |
| PA-023 — Deliverable assembly | Pass after Gmail attachment capability was added and the packet was revised | [Case evidence](PA-023/) |
| PA-024 — Approval coordination | Pass after synthetic review routing, two v2 approvals, and finalization | [Case evidence](PA-024/) |
| PA-025 — Expense submission | Pass after adding a synthetic expense API connector, reconciling receipts and bank records, submitting one governed claim, and verifying paid status | [Case evidence](PA-025/) |
| PA-026 — Handoff package | Pass after synthetic delivery through the existing Gmail thread | [Case evidence](PA-026/) |
| PA-027 — Credential renewal | Pass after connector contract repair and full synthetic renewal | [Case evidence](PA-027/) |
| PA-028 — Job-search pipeline | Pass after connector revision, Go adapter enablement repair, and synthetic end-to-end rerun | [Case evidence](PA-028/) |
| PA-029 — Application packet | Pass after working-directory repair and synthetic end-to-end rerun | [Case evidence](PA-029/) |
| PA-030 — Offer comparison | Pass after connector policy setup and fresh API read | [Case evidence](PA-030/) |
| PA-031 — Purchase comparison | Pass after four connector projection revisions, fresh API reads, and a saved recommendation | [Case evidence](PA-031/) |
| PA-032 — Research brief | Pass after four bounded connector revisions and a saved brief | [Case evidence](PA-032/) |
| PA-033 — Topic monitoring | Pass | [Case evidence](PA-033/) |
| PA-034 — Literature review | Pass after a focused projection revision, live reads from two indexes, deduplication, retraction handling, and a saved evidence table | [Case evidence](PA-034/) |
| PA-035 — Fact-checking | Pass after documented connection setup, source-linked five-claim review, and a saved report | [Case evidence](PA-035/) |
| PA-036 — Mixed inventory | Pass after five bounded connector revisions, explicit nullable-serial handling, and independent artifact inspection | [Case evidence](PA-036/) |
| PA-037 — Personal spending analysis | Pass after a profile contract repair, duplicate and refund reconciliation, period comparison, and artifact inspection | [Case evidence](PA-037/) |
| PA-038 — Newsletter reading digest | Pass after bounded API setup, budgeted selection, changed-story detection, and artifact inspection | [Case evidence](PA-038/) |
| PA-039 — Adaptive learning plan | Pass after progress projection repair, prerequisite-aware rescheduling, pacing change, and quiet repeat | [Case evidence](PA-039/) |
| PA-040 — Course choice | Pass after a focused course projection repair, clean comparison rerun, and one approved synthetic enrollment with receipt verification | [Case evidence](PA-040/) |
| PA-041 — Cash flow | Pass after a focused bill-ID projection repair, two-week cash-flow planning, duplicate-notice reconciliation, and one approved synthetic payment with receipt verification | [Case evidence](PA-041/) |
| PA-042 — Tax packet | Pass after two reviewed tax-API projection repairs, a complete five-record read, duplicate and supersession handling, missing-form retrieval, and independent packet inspection | [Case evidence](PA-042/) |
| PA-043 — Subscription cancellation | Pass after three connector repairs, one approved monthly cancellation, complete receipt capture, and post-cycle verification | [Case evidence](PA-043/) |
| PA-044–PA-100 | Not run | The operator will run each case in order. |

The test service is synthetic. It uses two fixture accounts and does not
touch real Gmail, Notion, Calendar, banking, travel, or payment accounts.
