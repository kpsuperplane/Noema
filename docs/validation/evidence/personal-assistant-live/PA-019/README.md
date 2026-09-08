# PA-019 — Project status

Verdict: **Pass**

Run date: 2026-09-08

Backend: Go development instance through `/tmp/noema-codex/graphql.sock`

Fixture: `2026-09-08-gmail-v1-notion-mcp-v8` plus synthetic project ledger
facts in the request

Conversation: `conversation:a39407c5e9686f34225273862829bf0f`

## Execution

Turn `turn:6a712e68f860ce415b49a57bb7102948` read the complete Launch project
record and all seven current Tasks in it. It also read connected Gmail and
Notion records. The response reported the synthetic 1,200/1,500 budget and
calculated 300 remaining, identified the two-day dependency delay, preserved
the September 18 target versus the obsolete September 20 note, and listed
open blockers with owners or explicitly missing owners. It separated
confirmed facts from uncertain records and made no changes.

The reviewed Task inventory included three open Tasks, three completed or
historically completed records, and one cancelled monitor. It preserved the
completed analytics-schema result and the unresolved support, handoff,
metrics, rollback, and launch-report work.

## Acceptance checks

| Check | Result | Evidence |
| --- | --- | --- |
| Read the full project document and Task set | Pass | Project read plus `task.list` returned all seven current project Tasks. |
| Report remaining budget | Pass | The response calculates 1,500 − 1,200 = 300. |
| Identify schedule conflict | Pass | September 18 is retained as approved; September 20 is labelled obsolete. |
| Identify blockers and owners | Pass | Support, handoff, metrics, rollback, dependency, and report risks are listed with owners or missing-owner flags. |
| Reconcile connected sources | Pass | Gmail and Notion records are named, including the support confirmation, metrics dates, and launch-report update. |
| Distinguish facts from uncertainty | Pass | The dependency owner, rollback owner, and report ownership are explicitly marked uncertain or missing. |
| Avoid writes | Pass | The turn used read tools only. |

## Limitation

The two-day delay and 1,200/1,500 ledger were synthetic facts supplied in the
request. The project already contained Tasks from earlier acceptance cases;
Noema still enumerated them instead of silently filtering them.
