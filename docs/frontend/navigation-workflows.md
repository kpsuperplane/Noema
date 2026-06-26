# Navigation And Workflows

This document maps the frontend's navigational structure to the jobs a human
needs to complete in Noema.

## Top User Jobs

- Set up Noema locally with one default human and one default assistant.
- Send one chat message and confirm Noema works.
- Save one explicit memory and see it appear in the chat history.
- Expand that memory line to understand what was saved and why.
- Drill into memory settings only when deeper management is needed.
- Continue across multiple chat threads once more than one useful thread
  exists.
- Promote a chat into a project/task/workspace when the user starts managing
  durable work.
- Talk to an agent and understand live activity.
- Review, correct, and govern memory.
- Inspect what context an agent saw and why.
- Grant, preview, and revoke tool access.
- Resolve approvals and policy denials.
- Recover from failed, paused, or ambiguous runs.
- Export, restore, delete, and rebuild local-first state.

## Navigation Model

The default navigation model is not a dashboard. It is a chat surface that can
grow side rails and side panels.

User-facing labels should favor clear product nouns over architecture nouns.
In V1, the primary visible destination is `Chat`. `Settings`, `Memory`, and
`Inspect` exist, but they are reached through utility controls, inline memory
events, `Why?` links, or direct owner/admin routes. Terms like `Governance`,
`Audit`, `RunEnvelope`, and `ContextPacket` belong in advanced inspection
panels until the user has a reason to inspect them.

### Surface Order

| Surface | When it appears | Role |
| --- | --- | --- |
| Chat | Default after setup, and the first meaningful screen | Conversation, live activity, inline memory/work/governance events |
| Inline event detail | User clicks a chat activity line | Explain what happened and offer safe next actions |
| Thread rail | More than one useful conversation exists | Resume recent conversations without leaving chat |
| Work panel | User asks to plan, track, or delegate durable work | Show project/task/workspace state beside the conversation |
| Object page | User opens a memory, task, project, approval, tool, or run | Manage durable state and linked evidence |
| Settings | User needs local setup, account, storage, backup, or memory management | Secondary utility surface |
| Owner/admin inspection | User explicitly asks for exact internals | Redacted privileged inspection |
| Home/attention view | Later, when there is enough activity to summarize | Optional overview, not the first post-ramp experience |

## Chat

Chat is the primary experience.

Include:

- Single-pane first-run chat with one visible human (`You`) and one visible
  assistant (`Noema`).
- Chat transcript with assistant text, activity notices, error notices, and
  future structured cards.
- Inline activity rows for saved memory, proposed memory, memory use, omitted
  context, task creation, workspace creation, tool proposal, approval request,
  denial, recovery state, and export preview.
- Expandable object summaries inside the transcript.
- Links from expanded objects to their management pages.
- A small local/setup/settings utility affordance.
- Thread rail only after durable or useful multiple conversations exist.
- Work panel only after the user creates or links durable work.

Primary actions:

- Send message.
- Start a new chat.
- Expand an activity row.
- Remember selected content.
- Open memory settings from an expanded memory card.
- Link or promote the chat to a workspace/project/task.
- Ask why Noema used or omitted something.

V1 conversation contract:

- The daemon WebSocket is the live source for current turn updates.
- Durable chat history is reconstructed from `conversation_items`.
- `agent_status` is live coordination state and is not replayed as transcript
  history.
- `conversation_items` include user text, assistant text, durable activity
  rows, A2UI cards, tool calls/results, approvals, and meaningful errors.
- After daemon restart, the frontend can reconstruct durable chat history from
  `conversation_items`, but cannot assume the provider thread remains resumable
  unless the daemon/runtime persists that mapping.
- Context packet links appear only when packets exist; otherwise show an
  unavailable state and a backend requirement.

## Inline Activity Rows

Inline activity rows are the main reveal mechanism. They keep the user in the
primary chat context while making system state visible.

Required row patterns:

| Event | Compact row | Expanded detail | Drill-in |
| --- | --- | --- | --- |
| Saved memory | `Memory saved` plus title preview | What Noema remembers, where it lives, why saved, allowed uses | Memory settings/detail |
| Proposed memory | `Memory proposed` plus risk label | Evidence, confidence, risk, keep/edit/reject | Memory review |
| Memory used | `Memory used` | Why included, purpose, rank/eligibility summary | What Noema used / memory detail |
| Memory omitted | `Some memory left out` | Safe omission category; exact detail only when authorized | Access preview / owner admin |
| Workspace created | `Workspace created` | Tasks, decisions, open loops, linked thread | Workspace |
| Task proposed | `Task proposed` | Suggested owner, status, source, blockers | Task detail |
| Tool proposed | `Tool access needed` | Operation, resource, data classes, destination | Approval/tool detail |
| Policy denied | `Action blocked` | Deterministic reason and safer path | Policy detail |
| Run paused | `Noema paused` | Last safe event and recovery choices | Run timeline |

Rows should be scannable and non-alarming. They should not feel like logs. A
row should answer "what happened?" first and reveal internals only on demand.

## Memory

Memory is a first-party product layer, but it should first appear in context.

Default memory reveal path:

```text
User says "remember this:"
  -> chat shows "Memory saved"
  -> user clicks the line
  -> inline card explains content, source, scope, risk, and actions
  -> user opens memory settings only if deeper management is needed
```

Memory management pages include:

- Search and filters.
- Candidate review queue.
- Memory detail.
- Provenance, participants, subjects, grants, retrieval policy, versions, and
  usage history.
- Access preview.
- Owner/admin context graph only behind explicit reveal.

Primary actions:

- Keep, edit, reject, or stop using a memory in this thread.
- Confirm, dispute, supersede, archive, delete, restore when backend mutation
  exists.
- Add or revoke grant when grant mutation exists.
- Revalidate retrieval policy when supported.
- Export filtered memories after the governed export pipeline exists.

## Threads

Threads appear after the single-chat experience has more than one useful
conversation to resume. They should not be required for first run.

Include:

- Recent thread list with title, participants, linked workspace/project, and
  recent activity.
- Badges for memory saved, review needed, workspace linked, approval pending,
  or paused run.
- Search/filter only when there are enough threads to justify it.

Primary actions:

- New chat.
- Resume chat.
- Rename/pin/archive thread when durable conversation rows exist.
- Link thread to project or task.

## Work

Work groups workspaces, projects, tasks, documents, artifacts, decisions, and
open loops. It should appear as an extension of chat.

Default work reveal path:

```text
User asks to plan, track, or delegate work
  -> Noema creates or proposes a workspace/task
  -> chat shows "Workspace created" or "Task proposed"
  -> user opens a side panel beside chat
  -> mature work can open as a workspace page
```

Include:

- Workspace/project switcher once more than one workspace exists.
- Project overview.
- Task Kanban and dependency views.
- Decisions, risks, constraints, goals, procedures, open loops.
- Project documents and artifacts.
- Project-scoped agents, grants, memories, and runs when backed.

Primary actions:

- Create or confirm project/task from chat.
- Assign human or agent.
- Link artifact or conversation.
- Review project memory.
- Grant project-scoped capability access.

## Tools And Approvals

Tools are governed capabilities, not just integrations. They should first
appear as inline proposals or approval requests in chat/work context.

Inline approval cards must show:

- Requested action.
- Capability and operation.
- Resource selector.
- Destination.
- Payload or diff.
- Data leaving Noema.
- Risk.
- Policy reason.
- Expiration.
- One-time versus reusable scope.

Before adapter execution, the harness must re-evaluate policy. Any changed
payload, recipient, destination, resource selector, egress class, sensitive
data class, policy version, expiration, or approval consumption state returns
the request to approval.

Tools/permissions pages become useful only after capability registry, grants,
invocations, denials, and revocations exist.

## Agents

V1 presents one visible assistant: Noema. Agent directories, handoffs, and
multi-agent controls appear only after more than one active agent or delegation
exists.

Agent activity should first appear in chat/work context:

- `Noema handed this to...`
- `Agent requested access...`
- `Agent paused for approval...`
- `Agent completed task...`

Full agent pages can then show instructions, skills, provider config, grants,
activity, memory, handoff rules, and proactivity limits.

## Settings

Settings is a secondary utility surface, not the main experience.

Include:

- Back to chat.
- Local folder path.
- Config file status.
- Database status.
- Local service status.
- Assistant connection and authentication status.
- Backup and rebuild controls.
- Memory management entry points opened from inline memory cards.
- Disabled export/restore entry points that explain the missing governed
  export pipeline.
- Advanced inspection entry points for owner/admin users.

Primary actions:

- Create local folder.
- Edit assistant connection in advanced settings.
- Start, stop, or restart Noema.
- Open local directory.
- Rebuild derived indexes.
- Create backup.

## Home / Attention View

Home is optional and later-stage. It is not the post-ramp default.

Home becomes useful when there is enough durable state that the user needs an
overview:

- Pending memory reviews.
- Pending approvals.
- Paused or failed runs.
- Recently active threads.
- Active tasks and blockers.
- Workspace updates.
- Local service health.

Home should avoid duplicating every object page. It links users into the
chat, workspace, memory, approval, run, or setting that owns the work.

## Command Palette

The command palette should be object-aware and scope-aware. In a chat-first IA,
commands should default to acting on the current chat, selected activity row,
expanded object, or current workspace panel.

Every trust-critical palette action must create a governed proposal before it
mutates state or causes external effects. The proposal must include current
scope, target object, diff or preview, required permission, sensitivity/egress
summary, and confirmation.

Special rules:

- `Remember this` preserves source trust. Selected external, tool, imported, or
  model-generated text can create a candidate, not confirmed memory.
- `Approve`, `grant`, `revoke`, `export`, `delete`, `restore`, `edit config`,
  and `restart Noema` require preview and confirmation.
- Disabled palette commands should explain the missing backend, permission, or
  object state.

V1 enabled commands:

- New chat.
- Remember this.
- Open current saved memory.
- Open memory settings for selected memory.
- Review memory candidates.
- Create local folder.
- Check Codex sign-in.
- Start Noema.
- Open local folder.

V1 inspect-only commands:

- Preview agent memory access.
- Preview what Noema used when a context packet or bounded explanation exists.
- Explain policy decision.
- Rebuild indexes.
- Show technical details.

Future disabled commands:

- Ask agent in current project.
- Run one-shot prompt.
- Create project.
- Create task.
- Link conversation to task.
- Link conversation to project.
- Show active runs.
- Cancel run.
- Resume run.
- Retry failed run.
- Review approvals.
- Approve selected request.
- Deny selected request.
- Revoke approval.
- Install capability.
- Grant capability.
- Revoke grant.
- Export scope.
- Export filtered memories.
- Create export.
- Edit config.
- Restart Noema.

## Key Workflows

### First-Run Setup

```text
Create local folder
  -> connect Codex
  -> start Noema
  -> send first message
  -> save first memory
  -> see Memory saved in chat
```

Success path:

```text
Create local folder
  -> Check Codex sign-in
  -> Start Noema
  -> Start chat
  -> Send "Say hello and tell me Noema is working."
  -> Send "remember this: I prefer concise setup instructions"
  -> Click "Memory saved"
  -> Optionally open memory settings
```

Primary CTAs:

1. `Create local folder`.
2. `Check Codex sign-in`.
3. `Start Noema`.
4. `Start chat`.
5. `Click Memory saved`.

Required screens/states:

- Pre-chat local-first storage explanation when setup is missing.
- Guided setup checklist: local folder, assistant connection, local service,
  first chat, first memory.
- Beginner path using Codex as the assistant connection.
- Chat as the first complete product frame.
- `Show technical details` drawer for paths, config, provider, and daemon
  details.
- Health check using beginner labels first.

Setup and health screens must never render secret values. Environment-derived
credentials, provider account identifiers, socket paths, and local home paths
should be redacted in screenshots, exports, and shared audit views unless an
authorized owner/admin explicitly reveals them.

Hide during onboarding:

- Top-level agents, workspaces, projects, tasks, tools, governance, audit,
  runs, approvals, exports, restore, context graph, context packets, capability
  grants, proactivity rules, raw IDs, provider thread IDs, socket paths, YAML,
  API key fields, and command palette mutations.

Beginner labels:

| Use | Avoid |
| --- | --- |
| Noema | Daemon |
| Local folder | Noema home |
| Assistant connection | Provider |
| Saved memory | Memory item |
| Show technical details | Inspect, context graph, context packet |
| What happened | Run timeline, ledger |
| Can leave Noema? | Egress |

### Chat With Memory

```text
User sends message
  -> response streams
  -> activity notices appear
  -> memory extraction runs
  -> saved/proposed/omitted memory lines appear
  -> user expands a line to inspect details
  -> user drills into settings only when needed
```

The transcript should show assistant output and activity without making memory
state feel hidden or automatic. Memory activity lines link to candidate
details, provenance, and management pages.

### Explicit Remember

```text
User says "remember this:" or "/remember"
  -> create saved memory when authenticated current human initiated it
  -> attach conversation provenance
  -> attach participants and subjects
  -> insert "Memory saved" activity line in chat
  -> expanded card shows content, source, risk, and actions
```

External content quoting "remember this" must not create confirmed memory.
Authenticated low-risk `remember this:` creates saved memory immediately and
does not enter the review queue by default. Inferred candidates, externally
sourced candidates, quoted `remember this` text, contradictions, sensitive
candidates, and memories requiring policy review belong in the review queue.

### Investigate An Answer

```text
Open chat turn or activity line
  -> open "What did Noema use?"
  -> review active context
  -> inspect included memory and sources
  -> inspect omissions and redactions
  -> inspect policy decisions
  -> open provenance for the underlying memory/source
```

The user should be able to answer "why did Noema know that?" without reading
raw logs.

### Memory Review

```text
Open review-required memory line or memory settings
  -> inspect content, sensitivity, provenance, subjects, participants
  -> confirm, edit, dispute, archive, delete, or leave pending
  -> optionally adjust retrieval policy or grants
```

Review should support bulk triage only for low-risk memories. Sensitive,
secret, action-triggering, contradictory, or externally sourced candidates need
individual inspection.

Normal review card:

- What will be remembered.
- Where it lives.
- Why Noema believes it.
- Risk.
- Actions: keep, edit, reject.

Advanced inspection contains policy fingerprints, grants, purpose rules,
trusted object links, versions, and use records.

### Chat To Workspace

```text
User asks Noema to plan, track, or manage work
  -> Noema proposes project/task/workspace state
  -> chat shows Workspace created or Task proposed
  -> side panel appears beside chat
  -> user opens full workspace only when the work needs it
```

Tasks should expose agent activity and governance beside the work. A task is
not complete just because a chat reply exists.

### Tool Action And Approval

```text
Agent proposes operation
  -> chat/work shows approval card
  -> schema and resource selector are validated
  -> side-effect and egress classes are shown
  -> policy decision is computed
  -> approval is requested if needed
  -> human approves, denies, narrows, or requests change
  -> adapter executes only after revalidation
  -> result is normalized and audited
```

Approval cards must show operation, capability, resource, destination, payload
or diff, data leaving boundary, risk, policy reason, expiration, and whether
approval is one-time or reusable.

### Agent Handoff

```text
Source agent proposes child run
  -> chat/work shows handoff card
  -> target agent, transferred context, and requested capabilities are shown
  -> policy checks prevent privilege laundering
  -> approval is requested when required
  -> child run links back to parent run
```

Handoff views should make inherited context and newly requested access obvious.

### Recovery

```text
Run fails or pauses
  -> chat/work shows paused recovery line
  -> show last durable event
  -> show safe resume point
  -> show unknown external-effect risk
  -> offer cancel, resume, retry, or inspect
```

Never make blind retry the default when an external write may have partially
succeeded.

### Export And Restore

```text
Choose scope/object/date range
  -> preview included data by sensitivity and object type
  -> choose redaction mode
  -> create export with manifest
  -> restore preview shows create/update/skip/conflict/tombstone outcomes
```

Exports should not include derived `system/` data as if it were canonical
backup data.

Exports are egress events. Full private, sensitive, or secret exports require
explicit confirmation and should support encryption or destination warnings.
Restored grants, approvals, capabilities, proactivity rules, credentials, and
external connectors default inactive until reviewed. Restore must not resurrect
tombstoned or deleted data without explicit conflict approval.

## Empty And Error States

| State | User-facing behavior |
| --- | --- |
| No local folder | `Noema needs a local folder before it can save chats or memory.` CTA: `Create local folder`. |
| Assistant not connected | `Noema cannot chat until Codex is connected.` CTA: `Check Codex sign-in`. |
| Noema stopped | `Noema needs to be running while you chat.` CTA: `Start Noema`. |
| Start failed | `Noema could not start.` CTA: `Troubleshoot`; secondary: `Show technical details`. |
| No database | `Noema has not created local memory storage yet.` CTA: `Start chat`. |
| No chat yet | `Send one message to check that Noema works.` CTA: `Start chat`. |
| No saved memory | `Save one memory to see how Noema remembers things.` CTA: `Try remember this`. |
| No durable threads | Stay in single-chat mode; do not render an empty thread rail. |
| No workspace | Stay in chat; reveal work panel only when a chat creates or links durable work. |
| No durable runs | Explain that V1 chat activity exists, but full run ledger is future-backed. |
| No "what Noema used" record | Explain that detailed context records appear after governed runs are persisted. |
| Waiting on approval | Make the pending approval the main inline call to action. |
| Policy denied | Show deterministic reason, affected scope/resource, and safer path. |
| Context omitted | Show agent-visible redacted reason; show audit detail only if authorized. |
| Capability unavailable | Distinguish not installed, unauthenticated, ungranted, revoked, unhealthy. |
| External effect unknown | Pause recovery and require inspection before retry. |
| Secret detected | Deny egress by default, redact content, and link audit trail. |

## V1 Route Support Matrix

Routes may exist before they are primary navigation. The route contract is about
addressability and backing, not what the first shell emphasizes.

| Route | Visible label | Backed by | Capability | Status |
| --- | --- | --- | --- | --- |
| `/` | Chat | setup health and local service state | route to chat when ready; show setup readiness if blocked | V1 |
| `/setup` | Setup | local folder, assistant connection, local service checks | create/update setup through guided setup flow | V1 |
| `/chat` | Chat | local service stream | live chat, transcript activity, inline memory extraction rows | V1 |
| `/chat/:id` | Chat detail | active daemon conversation; persisted `conversation_items` where available | live while daemon conversation exists; durable chat history after acknowledged turns | V1 limited |
| `/memory` | Memory settings | SQLite memory repository | secondary list and supported filters with redacted metadata; full FTS search waits for backend support | V1 |
| `/memory/:id` | Memory detail | SQLite memory repository | opened from chat line or memory settings; inspect, reveal when authorized, limited lifecycle actions as backend supports | V1 |
| `/memory/review` | Review memory | persisted candidates/active extracted memories | opened from review-required chat lines or settings; keep/edit/reject once mutation endpoints exist | V1 limited |
| `/inspect` | Advanced inspection | CLI-equivalent read models | owner/admin inspection hub; not primary navigation during onboarding or normal beginner use | V1 |
| `/inspect/context-graph` | Context graph | persisted graph tables and `noema context graph` semantics | owner/admin-only, redacted by default | V1 |
| `/inspect/context-packets` | Context packets | context packet tables if populated | inspect when rows exist; unavailable state otherwise | V1 limited |
| `/settings` | Settings | config, paths, local service health | local setup, maintenance, memory management entry points, advanced drill-ins | V1 |

Future route groups:

```text
/threads
/workspaces
/workspaces/:id
/projects
/projects/:id
/tasks
/tasks/:id
/agents
/agents/:id
/runs
/runs/:id
/approvals
/tools/:capabilityId
/tools/:capabilityId/operations/:operationId
/governance/grants
/governance/policy-simulator
/audit
/exports
/restore
```
