# Frontend Information Architecture Plan

This directory defines the frontend information architecture for Noema. It is
grounded in the current repository state and in the product architecture in
`docs/project.md`, `docs/memory.md`, `docs/postgres.md`, and `docs/harness/`.

Noema should not open as an admin dashboard. The primary user-facing
experience is a quiet chat workspace: one person talking to Noema, with memory,
threads, tasks, projects, approvals, tools, and inspection revealed inside that
experience only when the conversation creates a reason for them.

Long term, the frontend is still the control and inspection layer for humans,
agents, conversations, projects, tasks, tools, permissions, memory, runs,
approvals, audit, export, and restore. The difference is placement:
management pages are drill-ins from chat and work context, not the first thing
a normal user has to understand.

## Document Map

- [Experience layers](experience-layers.md): chat-first onboarding,
  progressive reveal, and transparency-on-demand model.
- [Navigation and workflows](navigation-workflows.md): primary chat surface,
  incremental navigation, command palette, key flows, and empty/error states.
- [Object model](object-model.md): frontend objects, ownership boundaries,
  relationships, detail pages, metadata, and actions.
- [Governance and inspection](governance-inspection.md): memory, permissions,
  capabilities, proactivity, audit, privacy, and export/restore surfaces.
- [Current contract](current-contract.md): current frontend/backend contract, backed
  sources, read models, route support, and implementation boundaries.
- [Review notes](review-notes.md): orchestration notes, adversarial review
  findings, and cleanup decisions.

## User Mental Model

The user-facing model should be simpler than the architecture vocabulary:

| User question | Primary UI answer | Internal terms |
| --- | --- | --- |
| What do I do first? | Chat with Noema | Conversation, daemon chat stream |
| What happened? | A line in the chat history | Message, activity event, memory event, run event |
| What did Noema remember? | Expand the memory line in chat | Memory item, provenance, participants |
| Where do I manage that memory? | Open memory settings from the expanded card | Memory detail, lifecycle actions |
| Is this becoming a project? | Open the workspace/task panel beside chat | Workspace, project, task, scope |
| What needs my decision? | Inline approval/review line item, then detail panel | Approval, policy decision, grant |
| Why did Noema do that? | `Why?`, `What did Noema use?`, or expanded event details | Context packet, policy, provenance |
| What can leave Noema? | Data leaving Noema preview on the relevant event | Egress, capability, grant, approval |

Terms such as `principal`, `run envelope`, `context packet`, and `egress` are
inspection labels. They should not be primary navigation copy for normal users.

The default first-run mental model is:

```text
You -> Noema -> first chat -> saved memory line -> expanded details -> settings only if needed
```

The current slice should create one visible local human named `You` and one visible primary
assistant named `Noema` in the UI. Multi-human, multi-agent, project, task,
tool, approval, and audit concepts appear only when backed state and user
intent make them useful.

## IA Thesis

The frontend is chat-led and object-backed.

Chat is the primary entry surface because it is where the user expresses intent
and where Noema explains live activity. Durable objects still own canonical
state:

```text
Conversation event -> Object record -> Evidence -> Policy -> Optional management page
```

- `Conversation event` is what the user sees first: a reply, saved memory,
  proposed task, approval request, tool result, denial, or recovery notice.
- `Object record` is the durable source of truth: memory, task, workspace,
  run, approval, capability, export, or audit event.
- `Evidence` answers why Noema believes something or why it acted.
- `Policy` answers what was allowed, denied, approval-gated, or redacted.
- `Management page` is a secondary drill-in when the user needs search,
  bulk review, configuration, or owner/admin inspection.

This keeps the interface aligned with Noema's core promise: transparent systems
over mysterious AI state, without making the first experience a control panel.

## Product Stance

The first frontend should feel like a focused local assistant, not a marketing
page, not a novelty chat toy, and not an admin dashboard. It should begin as a
single chat pane. As work accumulates, the interface grows in place:

1. Chat starts as the whole experience.
2. Memory appears as compact activity lines inside chat.
3. Clicking an activity line expands the object summary inline.
4. Deeper management opens from that object summary.
5. Multiple chats introduce a thread rail.
6. Project-like work introduces a workspace/task panel beside chat.
7. Mature workspaces can become their own management surface while retaining a
   chat dock or `Ask Noema` affordance.
8. Settings and owner/admin inspection remain secondary utilities.

Canonical state remains outside the frontend:

- Structured source of truth lives in Postgres.
- Durable object-owned files live under the owning object directory.
- `system/` state is derived and rebuildable.
- The frontend displays projections, explains decisions, and submits actions.
- The frontend must not become the hidden source of truth for runs, approvals,
  memory, or permissions.

## Non-Negotiable IA Principles

1. Start with one chat pane.
2. Reveal complexity through conversation events before adding navigation.
3. Treat memory as visible product state: saved, used, omitted, and reviewed
   memories appear where they affect the conversation.
4. Keep durable objects authoritative; chat events point to records instead of
   storing product truth only in the transcript.
5. Make scopes visible wherever trust depends on them, but introduce the label
   only when the user is managing or inspecting that boundary.
6. Treat runs as the execution spine for chat, tasks, schedules, proactive
   rules, imports, maintenance, and handoffs.
7. Make approvals and policy denials operationally prominent inline before
   routing to specialized governance pages.
8. Separate agent-visible explanations from owner/admin inspection details.
9. Show provenance before asking users to trust memory, decisions, or actions.
10. Route every tool/integration through capability, operation, resource,
    grant, approval, invocation, result, and audit concepts.
11. Prefer object pages with linked evidence over log-only debugging.
12. Keep export, deletion, rebuild, restore, and audit available as normal
    system operations, but not as the primary beginner experience.
13. Mark future surfaces clearly when the underlying Rust slice does not exist
    yet.

## Primary Surface Ladder

The product should not begin with a full top-level app nav. Surfaces appear as
state accumulates.

| Stage | Visible structure | Trigger | Purpose |
| --- | --- | --- | --- |
| First run | Single chat pane with small setup/status affordance | Setup is healthy enough to chat | Prove Noema works |
| Trust event | Chat plus inline memory/review/why line items | Noema saves, uses, omits, or proposes memory | Make learning inspectable |
| Object detail | Inline expanded card or side drawer | User clicks the event | Show what happened and safe actions |
| Management drill-in | Memory/settings page, review queue, or object page | User needs search, bulk review, edit, archive, reveal | Manage durable state |
| Multiple threads | Thread rail beside chat | More than one useful conversation exists | Resume work without a dashboard |
| Work reveal | Workspace/task panel beside chat | User asks to plan, track, or delegate work | Promote chat into durable work |
| Workspace mode | Project/task/workspace surface with chat dock | Work has enough durable state | Manage tasks, decisions, artifacts, open loops |
| Governed tools | Inline approval/tool cards plus capability drill-ins | Tool registry, grants, approvals exist | Decide external effects safely |
| History/audit | Object-linked timelines and system audit | Durable ledger/recovery exists | Recover, review, export |
| Multi-agent | Agent/person/handoff surfaces | More than one active human/agent exists | Coordinate delegation |

## Current Visible IA

The current slice should default to chat, not Home.

```text
Default: Chat
Small utility: Settings
Contextual drill-ins: Memory detail/review, What did Noema use?, local health
Owner/admin utility: Advanced inspection
```

The current slice can still implement routes for `/memory`, `/settings`, and `/inspect`, but
normal users should reach them through chat events, object details, or utility
controls. They should not read as the primary product shell.

Home as an operational overview can exist later, but it should not be the first
post-ramp destination. It becomes useful only after there are enough threads,
tasks, approvals, runs, or reviews that a user needs an attention surface.

## Current Implementation Anchors

The current repository implements only a narrow slice:

- `noema config` initializes the Noema directory and config.
- `noema start` runs a foreground local daemon and serves the basic local web
  chat.
- `noema chat` starts daemon-backed Codex chat through Noema-owned OAuth and
  direct Codex Responses API calls.
- One-shot prompts can use the configured provider.
- Explicit `remember this:` and `/remember` messages are persisted.
- Ordinary chat memory extraction can produce persisted candidates.
- `noema memory list`, `noema memory show <id>`, and
  `noema context graph --limit N` provide owner/admin inspection.
- The daemon protocol already has transcript activity notices and an `A2uiCard`
  placeholder for future structured UI cards.
- The first implemented frontend shell is the core-hosted React chat in
  `crates/noema-core/web`, built and linted with Bun.

The frontend plan intentionally includes target surfaces that are not fully
backed by current Rust tables yet. Those surfaces should be staged and revealed
from chat/work context rather than pretending they already exist.

## Backing Status

| Area | Status | Notes |
| --- | --- | --- |
| Setup/config health | Current Rust-backed | `noema config`, `NoemaPaths`, config loading, daemon socket paths |
| Live chat | Current Rust-backed | GraphQL mutations/subscriptions, Noema runtime commands, and direct Codex Responses provider runtime |
| Transcript activity | Current Rust-backed | Assistant text, activity notices, errors, turn completion, future `A2uiCard` payloads |
| Persisted chat history | Current Rust-backed | Durable chat history is reconstructed from `conversation_items`; live turn coordination comes from GraphQL subscriptions and `agent_status` |
| Explicit memory save | Current Rust-backed | `remember this:` and `/remember` persist memory with provenance |
| Memory list/detail | Current Rust-backed | Postgres repository and CLI inspection exist; UI should open from inline events first |
| Context graph inspection | Current Rust-backed, owner/admin-only | Backed by persisted memory graph tables; redacted by default outside privileged inspection |
| Context packets | Table-backed, not chat-wired | Tables and repository method exist, but chat turns do not yet persist packets in production flow |
| Runs, approvals, capabilities, task system | Harness-doc target | Architecture exists in docs; durable runtime/schema paths are not implemented as active product controls |
| Memory versions, proactive rules, tombstones, FTS | Schema-doc target | Defined in `docs/postgres.md`; not all are created by the current Rust bootstrap schema |

## Current Frontend Slice

The current first shell is a local web chat hosted by the core daemon. The
first practical frontend slice should keep proving the chat-led mental model:

1. Guided setup or pre-chat readiness state for local folder, Codex connection,
   local service, first chat, and first saved memory.
2. Chat as the default landing surface after setup.
3. Transcript rendering for assistant text, activity notices, error notices,
   turn completion, and structured cards when present.
4. Inline memory activity lines for saved memory, proposed memory, memory use,
   memory omission, and review-required memory.
5. Expandable memory details in the transcript with safe actions: keep, edit,
   stop using here, open memory settings, and explain why.
6. Memory list/detail parity with `noema memory list/show` as a secondary
   management drill-in.
7. Memory review queue for active/candidate memories, opened from chat or
   settings when review is needed.
8. Owner/admin-only graph inspector backed by `noema context graph` semantics,
   redacted by default in normal views.
9. Memory-only access preview using the existing deterministic retrieval
   engine, presented first as a `Why?` or `What did Noema use?` explanation.
10. Settings for assistant connection, local paths, backups, rebuildable state,
    and disabled export/restore entry points that explain the missing governed
    export pipeline.

Current route support is detailed in [Current contract](current-contract.md).

## Later Product Slices

Later slices should add:

- Durable conversation rows and a thread rail.
- Workspace/project/task panels that can be created from chat.
- Run ledger, projections, replay, cancellation, and recovery controls.
- Capability registry, operation schemas, grants, invocation history, and
  approval gates rendered first as inline proposal/approval cards.
- Full project/task system with Kanban, dependencies, recurrence, delegation,
  and provenance.
- Agent handoff graph and child-run inspection.
- Proactivity rules by system, human, agent, workspace, project, task, cron,
  and tool scopes.
- Multi-human roles, collaborator redaction, and scoped exports.

## Open Product Questions

- Should a desktop app or launcher eventually own starting/stopping the
  core-hosted local web chat?
- What minimum local health UI is required before chat can be the default
  landing surface?
- When should the thread rail appear: after the second chat, after pinned
  chats, or after durable conversation rows exist?
- When should a chat become a workspace rather than just a thread?
- Which workspace signals are strong enough to reveal task/project management:
  explicit user request, detected plan, recurring work, linked directory, or
  durable task creation?
- What user roles exist before multi-human workspaces ship?
- Should sensitive or secret memory reveal require re-authentication?
- Which capability ships first after memory: filesystem, tasks, or an external
  connector?
- Which export formats are required first?
