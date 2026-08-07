# Frontend Object Model

This document defines the objects the frontend should expose, how they relate,
and what each detail page needs to show.

The IA is chat-led and object-backed. Chat is the primary entry surface and the
place where users first see activity. Durable objects remain the source of
truth for memory, work, approvals, tools, runs, audit, and settings.

## Core Object Groups

| Group | Objects | Frontend role |
| --- | --- | --- |
| Identity | Humans, agents, groups, system, tools, services, importers | Who can own, act, delegate, approve, or be audited |
| Governable context | System, human, agent, conversation, workspace, project, task, cron, tool, relationship, custom | Visibility, memory, permissions, proactivity, audit, and retrieval boundary implemented by concrete objects |
| Work | Conversations, workspaces, projects, tasks, cron rules, artifacts | Coordination surfaces that grow out of chat |
| Execution | Runs, run envelopes, context packets, handoffs, worker state, artifacts | What agents did, are doing, or may resume |
| Memory | Memory items, entities, relationships, subjects, participants, provenance, versions, use records | Durable, scoped, inspectable context |
| Governance | Policies, grants, approvals, proactivity rules, egress reviews, denials, revocations | Trust-critical controls and decisions |
| Capabilities | Capabilities, adapters, operations, resource selectors, invocation records | Governed tool and integration access |
| System | Conversation items, object provenance, events, exports, restore plans, tombstones, rebuild jobs | Provenance, portability, recovery, and maintenance |

## Default Identity Model

The current slice should present only one visible human and one visible agent:

| UI label | Internal identity | User-facing role |
| --- | --- | --- |
| You | `human:local` | The local person using Noema |
| Noema | `agent:primary` | The primary assistant |

Humans and agents remain first-class objects in the target architecture, but
they should not be setup prerequisites. Additional humans, agents, groups,
handoffs, and shared scopes appear only after the user enables multi-person or
multi-agent work.

## Backing Status

| Status | Meaning | Objects/surfaces |
| --- | --- | --- |
| Current Rust-backed | Current code can serve or inspect this with bounded frontend work | Setup/config, local runtime health, live chat stream, chat transcript persistence through `conversation_items`, and memory browse/detail through GraphQL `memoryClaims`/`memoryClaim` |
| Store-backed, not flow-wired | Store records and repository methods exist, but normal chat/runtime flows do not populate them consistently yet | Context packets, packet omissions, memory-use records |
| Future store target | Not fully created or exercised by current Rust bootstrap/runtime | Memory versions, proactive rules, deletion tombstones, richer search |
| Harness-doc target | Designed in harness architecture docs, not available as active durable product controls yet | Runs, approvals, capability registry, policy decisions, event ledger, replay/recovery |
| Future product | Product goal with no current runtime contract | Full workspaces/projects/tasks, multi-human roles, agent handoff graph, connector marketplace |

Each object page below describes the target IA. Implementation specs should
split fields and actions into current and future groups before building screens.

## Cross-Cutting Inspectors

These concepts should be available from chat activity rows, inline object
cards, and object pages rather than hidden in one settings area.

| Inspector | Answers |
| --- | --- |
| Scope inspector | What boundary owns this? What parent scope applies? What default visibility and proactivity level apply? |
| Principal inspector | Who is acting, owning, creating, approving, or receiving access? |
| Provenance inspector | Why does Noema know this? Which source, episode, message, document, tool result, import, or rule supports it? |
| Policy inspector | Why was an action allowed, denied, approval-gated, omitted by authorization, or intentionally transformed for egress? |
| Egress inspector | What information left the run boundary, where did it go, who can see it, and which policy allowed it? |

## Relationship Map

```text
Principal
  owns Scope
  acts in Run
  creates Memory
  receives Grant
  resolves Approval

Scope
  owns Memory
  contains Conversation, Workspace, Project, Task, Cron, Tool
  provides policy, visibility, proactivity, and retrieval boundaries

Conversation
  contains Messages and Episodes
  shows Activity Events
  creates Run triggers
  produces Memory proposals
  links to Tasks, Projects, Artifacts, and Context Packets

Run
  has RunEnvelope
  assembles ContextPacket
  retrieves Memory
  invokes Capabilities
  requests Approvals
  produces Events, Artifacts, External Effects, and Memory proposals

Memory
  belongs to one home Scope
  has Subjects via Entities
  has Participants via Principals
  has Provenance edges
  has Grants, Purpose Rules, Trusted Object Links, Versions, and Use Records

Capability
  has Operations
  uses Adapter
  accepts Resource Selectors
  is constrained by Grants and Approval defaults
  produces Invocations, Results, Artifacts, and Audit Events
```

## Chat Event To Object Pattern

Most durable objects should have a lightweight representation inside chat or
work context before the user reaches a full object page.

1. Compact event row: what happened, status, short preview, safe label.
2. Inline expansion: summary, source, risk, scope/location, immediate actions.
3. Object drill-in: full management page only when search, bulk review, edit,
   reveal, export, recovery, or configuration is needed.
4. Owner/admin inspection: exact internals only after explicit reveal.

Examples:

| Chat/work event | Durable object | First expansion | Drill-in |
| --- | --- | --- | --- |
| `Memory saved` | Memory | Content, source, use, keep/edit/stop using | Memory settings/detail |
| `Task proposed` | Task | Goal, owner, source, blockers | Task page |
| `Workspace created` | Workspace/project | Tasks, decisions, open loops | Workspace page |
| `Approval needed` | Approval | Operation, resource, data leaving Noema, risk | Approval detail |
| `Tool result` | Invocation/audit event | Operation, result, errors, egress | Tool invocation detail |
| `Noema paused` | Run | Last event, safe resume point, unknown external-effect risk | Run timeline |

## Reusable Object Page Pattern

Every full object page should follow the same hierarchy:

1. Status summary: current state, owner, scope, information class, and health.
2. Why this matters: one sentence or compact panel explaining why the object is
   on screen now.
3. Primary next action: the best current action, or no action when inspection
   is the point.
4. Secondary inspection: tabs and linked evidence for deeper review.
5. Disabled/future actions: visible only when useful, with the missing backend
   or permission stated plainly.
6. Danger zone: destructive, external, export, revoke, delete, or restore
   actions with preview and confirmation.

Normal users should usually reach these pages from a chat/work event or inline
detail card. They should see review and decision surfaces first. Policy
fingerprints, grant rows, packet omissions, and raw event payloads belong
behind advanced inspection affordances.

## Object Detail Requirements

### Human

Availability: future product, with a default local human implicit in the current slice.

Purpose: a person who owns, uses, collaborates through, or is represented in
Noema.

Show:

- Profile, handle, active status, owned governable contexts, default visibility.
- Conversations, workspaces, projects, tasks, approvals, and recent activity.
- Memories about the human, memories owned by the human, and memories where the
  human was a participant.
- Grants issued by or affecting the human.
- Proactivity defaults and overrides.
- Export, deletion, and audit actions.

Actions:

- Edit profile.
- Preview what an agent can access for this human.
- Export human scope.
- Review memories involving the human.
- Manage grants and proactivity.

### Agent

Availability: partial current concept for the primary agent; full agent
directory and handoff model are future product.

Purpose: an actor-capable concrete object with model/provider config, skills,
policies, operational memory, and grants.

Show:

- Instructions, skills, model/provider, runtime health, owner object.
- Allowed tools and resource selectors.
- Recent runs, failed runs, handoffs, and activity.
- Memory owned by the agent and memory granted to the agent.
- Proactivity limits by global, scope, project, and task context.
- Approval history and denials.

Actions:

- Edit instructions or provider settings.
- Preview access in a scope or run envelope.
- Grant or revoke capability access.
- Pause, disable, or archive agent.
- Review handoff rules.

### Conversation

Availability: current live chat stream and persisted turn provenance; durable
conversation list/detail model is future-slice.

Purpose: the primary entry surface for ordinary use and a first-class
coordination surface containing transcript, context, decisions, activities, and
memory proposals.

Show:

- Transcript and activity stream.
- Inline activity rows for memory saved/proposed/used/omitted, task/workspace
  proposals, approvals, tool activity, denials, and recovery states as backing
  exists.
- Inline object expansions for the rows above.
- `human:local.primary_conversation_id` as the current home conversation in
  the initial slice.
- Participants, active governable contexts, working directory or project hint.
- Provider changes should preserve the Noema conversation id.
- Memory extraction activity and proposed/created memories.
- Linked tasks, projects, artifacts, and runs.
- Context packets and "what did the agent see?" panel.
- Tool calls and external effects once the harness surfaces are durable.

Actions:

- Send message.
- Expand activity row.
- Remember selected content.
- Link to project or task.
- Review extracted memories.
- Inspect context packet.
- Export transcript and attached artifacts.

### Workspace and Project

Availability: future product, except project hints currently inferred from a
conversation working directory for memory extraction.

Purpose: durable shared or goal-oriented environments for work, policies,
documents, tasks, memory, and agent activity. Workspaces usually begin as a
chat-side panel or proposal, then become a full surface when there is enough
state to manage.

Show:

- Linked originating conversations and chat-derived decisions.
- Overview, members, agents, policies, proactivity defaults, grants.
- Documents, imports, artifacts, decisions, goals, constraints, risks, open
  loops.
- Tasks, dependencies, approvals, recurring work, and active runs.
- Project memory, provenance, and context graph slice.
- Capability grants scoped to the workspace or project.

Actions:

- Promote or confirm from chat.
- Create task or document.
- Assign agent.
- Review project decisions and memory candidates.
- Grant narrow tool access.
- Export or archive scope.
- Rebuild derived project indexes.

### Task

Availability: harness/product target; not current Rust-backed.

Purpose: first-party work item, not a lightweight reminder.

Show:

- Originating chat event or workspace.
- Workflow stage, priority, owner, assignees, dependencies, and blockers.
- Approval checkpoints, recurrence, due dates, linked project/conversation.
- Agent runs, handoffs, tool calls, artifacts, memory use, provenance.
- Required capability grants and current policy state.

Actions:

- Create or confirm from chat.
- Move status.
- Assign human or agent.
- Add dependency or blocker.
- Request agent work.
- Approve, deny, or revise pending checkpoint.
- Convert conversation outcome to task.

### Run

Availability: harness-doc target; the current slice can show activity placeholders, but not a
full durable run page until run envelopes and events are persisted.

Purpose: one governed attempt by an agent to do work under a specific context.

Show:

- Originating chat/work event and current inline status row.
- Status, trigger, requesting actor, executing agent, owner object.
- Active governable contexts, proactivity limit, memory constraints, egress
  constraints.
- Timeline of run events.
- Context packet manifest.
- Memories retrieved, included, shown, used, and omitted.
- Capability proposals, invocations, policy decisions, approvals, results.
- Outputs, artifacts, external effects, errors, recovery state.

Actions:

- Cancel, suspend, resume, retry when safe.
- Inspect context.
- Resolve approval.
- Export run.
- Open linked conversation, task, memory, capability, or artifact.

### Memory

Availability: current Rust-backed for list/detail, candidate creation,
provenance, participants, subjects, graph inspection, and access policy fields.
Versions and FTS search are schema-doc targets.

Purpose: durable scoped context with provenance and policy.

Show:

- Inline summary first when memory is saved, proposed, used, or omitted in
  chat.
- Header: ID, title, type, status, information class, confidence, authority level,
  extraction method, home scope, owner, creator.
- Content and structured value.
- Subjects, participants, provenance, source excerpts, validity windows.
- Retrieval hints and typed retrieval policy.
- Participant visibility, external egress policy, purpose rules, object links,
  grants, effective policy status, fingerprint validation.
- Context packets and use records by run.
- Versions, lifecycle events, related relationship claims.

Actions:

- Expand inline summary.
- Open memory settings/detail.
- Stop using in the current chat or workspace when supported.
- Confirm, edit, dispute, supersede, archive, delete, restore.
- Revalidate retrieval policy.
- Add or revoke grant.
- Preview availability for an agent, scope, purpose, or run envelope.
- Export selected memory.

### Entity and Relationship

Availability: current Rust-backed for graph inspection; relationship graph
exploration is advanced owner/admin inspection unless policy authorizes display.

Purpose: graph nodes and memory-backed relationship claims used for context
retrieval and inspection.

Show:

- Entity canonical name, aliases, type, home scope, linked principal.
- Relationship subject, predicate, object, status, confidence, validity window.
- Supporting memory and provenance.
- Policy inherited from supporting memory.

Actions:

- Open supporting memory.
- Merge aliases.
- Dispute or archive relationship claim.
- Preview whether claim is visible in a run envelope.

Rule: no relationship edge or graph path should be visible to an agent unless
the supporting memory is includable for that run.

### Capability and Operation

Availability: harness-doc target; current frontend should not present active
tool mutation controls until the registry and gateway exist.

Purpose: governed tool and integration access.

Show:

- Inline tool proposal or invocation summary before full registry details.
- Capability manifest, version, adapter, health, connected accounts.
- Operations, input/output schemas, side-effect class, egress class.
- Resource selector types, dry-run support, rollback/idempotency, audit level.
- Grants by principal, scope, resource, operation, and expiration.
- Approval defaults, revocations, invocation history, denied invocations.

Actions:

- Install, enable, disable, revoke.
- Grant or deny operation access.
- Create narrower resource selector.
- Preview access for a run envelope.
- Open invocation detail.

### Approval

Availability: harness-doc target; current frontend should not present active
approval controls until durable approvals exist.

Purpose: durable gate for an action that needs human or delegated decision.

Show:

- Inline approval card in chat/work context before full approval inbox/detail.
- Requested operation, run/task, requester, target principal, destination.
- Payload or diff, data leaving boundary, risk classification, policy reason.
- One-time versus reusable scope, expiration, comments, outcome.
- Related capability, resource selector, memory, task, project, and run.

Actions:

- Approve once.
- Approve with narrower scope.
- Deny.
- Request changes or clarification.
- Revoke prior approval.
- Expire approval.

### Audit Event

Availability: partial current memory events; full run ledger and system audit
are harness-doc targets.

Purpose: inspectable record of trust-critical state and actions.

Show:

- Event type, actor principal, component, scope, timestamp, sequence.
- Causation/correlation IDs, linked run, linked object.
- Information class, secret exclusions or explicit egress transformations,
  trust labels, content hash or object reference.
- Human-readable summary and machine-readable payload where authorized.

Actions:

- Open linked object.
- Export filtered event set.
- Rebuild projection from ledger.
- Report or annotate suspicious event.

## Metadata Badges And Filters

The frontend should make these closed vocabularies visible as badges, filters,
and sort keys:

- Scope type.
- Principal type.
- Memory type, status, information class, authority level, extraction method.
- Retrieval policy status and effective retrieval policy status.
- Participant role and subject role.
- Participant visibility policy.
- External egress policy.
- Purpose.
- Grant permission and effect.
- Relationship status.
- Proactivity level 0-6.
- Trust label.
- Side-effect class and egress class.
- Run status.
- Approval status.
- Event category and event type.

Important computed values:

- Eligibility reason.
- Rank reasons.
- Denial reason.
- Omission reason.
- Authorization-omission or explicit egress-transformation state.
- Stale policy fingerprint.
- Source trust level.
- Validity and expiry windows.
- Unknown external-effect risk.

## Modeling Boundaries

- Do make chat the primary entry and reveal surface for ordinary use.
- Do not make the transcript the canonical store for all agent work. Chat
  events point to durable objects; objects own source-of-truth state.
- Do not force every object into primary navigation just because it exists.
- Do not make agents the authority on their own permissions.
- Do not copy memory into every scope that can use it. Use grants and
  participant bindings.
- Do not treat graph relationships as authority without supporting memory.
- Do not model connected accounts as capability grants.
- Do not hide approvals inside a transcript-only log; inline approval cards
  must link to durable approval records and audit.
- Do not make derived indexes, caches, vectors, or temporary files appear as
  durable source-of-truth objects.
