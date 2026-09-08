# PA-022 — Ambiguous goal

Verdict: **Pass**

Run date: 2026-09-08

Backend: Go development instance through `/tmp/noema-codex/graphql.sock`

Fixture: `2026-09-08-gmail-v1-notion-mcp-v8` (no external writes)

Conversation: `conversation:a39407c5e9686f34225273862829bf0f`

## Execution

The initial turn `turn:9902f4f5384bb9aeaf5a56b73d63070e` asked only three
questions that could change the plan: the primary audience, launch date, and
minimum first release. It did not create a project or tasks.

The follow-up turn `turn:7f31407db725d8e3c67ae3b8327ff874` used the supplied
answers: current local club members as the main audience, October 15, 2026 as
the target, and Home, About, Events, and Contact as the first release. It
excluded accounts and payments and kept the estimate within the requested ten
hours.

Noema created project `project:e6d7f8a9f70db2af4f39037389f18d01`, **Club website
launch**, and seven Inbox tasks. The task estimates total eight hours, leaving
about two hours of contingency. Each task has a dependency and measurable
acceptance criteria. The tasks remain unstarted.

## Acceptance checks

| Check | Result | Evidence |
| --- | --- | --- |
| Ask only material questions before planning | Pass | Initial turn asks audience, deadline, and first-release scope only. |
| Incorporate the supplied answers | Pass | Project description and document record the audience, date, scope, and exclusions. |
| Save a native project | Pass | Project `project:e6d7f8a9f70db2af4f39037389f18d01`; revision 1. |
| Create a sequenced plan with measurable completion | Pass | Seven tasks are titled 1–7. `tasks list --project` and `tasks get` show objectives, acceptance criteria, and dependencies. |
| Keep the plan feasible | Pass | Estimates total 8.0 hours against a 10-hour limit, with 2.0 hours contingency. |
| Avoid premature execution or external effects | Pass | All seven tasks are in the personal Inbox. No messages or external service writes occurred. |

## Limitation

The site implementation and the required content or service accounts were not
provided, so this case validates planning and native task capture only. It does
not claim that the website is built or published.
