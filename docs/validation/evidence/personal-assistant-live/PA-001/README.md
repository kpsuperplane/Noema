# PA-001 — Daily brief

Verdict: **Pass**  
Run date: 2026-09-07  
Backend: Go development instance through `/tmp/noema-codex/graphql.sock`  
Backend revisions: `663e72e4`, `374ad146`, `c1f9ecbe`  
Fixture: `2026-09-07-gmail-v1-notion-mcp-v4`

## Scenario

Account A contained two Calendar meetings, two same-topic Gmail messages,
and the approved Notion launch deadline. The second Gmail message was a
duplicate reminder. Native Memory contained the durable weekday 16:00 work
stop preference.

The run used these saved connections:

| Source | Registered connection | Accepted source revision |
| --- | --- | --- |
| Gmail API | `7e1a192dfe8cb6c687de09a01a2e1e94` | `bb233ceed394b42a61289597aab81faf0f4977f9dea98f6355762d9b6698895a` |
| Calendar API | `9dca0c77c15f569bda1ba0e65077d754` | `90e3ea6ad3f84374f9d625352b0c1e7e87ddb9f8d7e2c565ec62071322d4f9fb` |
| Notion MCP | `3425e24fbf06121ffa2436420b90ca2b` | authenticated server revision `a473daf5fe43707c2346075000b281e9` |

Credential values and OAuth URLs are intentionally absent.

## Inspected turns

The operator sent the natural request, then used ordinary follow-ups after
recovering expired synthetic credentials. These turns supplied the source
evidence used by the final brief:

- `turn:43bd2596f4e26b617e3ad266ef723d94` read the two Calendar events.
- `turn:73a6074b267ab21c9d55c4a954b7f9ea` read Gmail messages `a-msg-004` and
  `a-msg-005` with `after:2026/09/06 before:2026/09/08`.
- `turn:60e836ac91f6aad4ce4b67d8c77ba00a` fetched the launch notes and the
  approved 2026-09-18 deadline.
- `turn:de2fd6459ef468d4d0f0fd2d9727f1dc` assembled the final brief.

No write operation was requested or issued. Calendar and Gmail traces contain
read requests only. Notion protocol traffic was read-only tool discovery and
fetching.

## Acceptance checks

| Check | Result | Evidence |
| --- | --- | --- |
| Read all four required sources through Noema | Pass | Calendar, Gmail, and Notion calls above; Memory root contains the 16:00 preference and the final brief uses it. |
| Count the deadline once | Pass | The final brief names one approved September 18 deadline and labels September 20 as stale. |
| Identify preparation still open | Pass | Support coverage, rollback owner, and analytics event schema are listed. |
| Identify urgency | Pass | The noon launch-support request is identified as due today. |
| Identify work beyond capacity | Pass | The brief protects the 16:00 stop and says schema work needs an effort estimate or later scheduling. |
| Collapse duplicate mail | Pass | The two launch-support messages are reported as one action. |
| Preserve account scope | Pass | Fixture traces show account A only; no account B record appears. |

## Final inspected result

The assistant produced a brief with the 09:00 planning meeting, the 13:00
client launch review, the 16:00 work stop, one noon launch-support action,
the September 18 launch deadline, three preparation items, and a capacity
warning for unsized schema work.

## Recovery notes

The first attempt used an expired saved OAuth grant. The operator completed
normal synthetic Gmail and Notion reauthentication. The first fresh Gmail
date search also exposed a fixture gap: `after:` and `before:` were treated as
text. The fixture now parses both date forms, with focused and broad Go tests
passing. The final rerun used the corrected fixture and current connections.

