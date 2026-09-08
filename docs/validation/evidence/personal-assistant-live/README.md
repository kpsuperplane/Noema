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
| PA-006–PA-100 | Not run | The operator will run each case in order. |

The test service is synthetic. It uses two fixture accounts and does not
touch real Gmail, Notion, Calendar, banking, travel, or payment accounts.
