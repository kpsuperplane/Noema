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
| PA-022–PA-100 | Not run | The operator will run each case in order. |

The test service is synthetic. It uses two fixture accounts and does not
touch real Gmail, Notion, Calendar, banking, travel, or payment accounts.
