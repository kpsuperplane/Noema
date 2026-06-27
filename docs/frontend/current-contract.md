# Current Frontend Contract

This contract keeps the first frontend aligned with the current Rust runtime.
It should be updated whenever the daemon protocol, Postgres bootstrap schema, or
inspection commands change.

For the current slice, routes may exist for memory, settings, and inspection,
but the default user-facing experience is chat. Management and admin routes are
secondary drill-ins from chat activity, object details, utility controls, or
explicit owner/admin entry points.

## Status Terms

| Status | Meaning |
| --- | --- |
| Current | Backed by current Rust code or a small bounded read model over current Postgres state |
| Current limited | Partly backed; must show unavailable/inspect-only states for missing behavior |
| Future | Do not expose as an active control until backend/runtime support exists |
| Owner/admin-only | Local privileged inspection; redacted or hidden for normal views |

## Current Sources

| Source | Current authority |
| --- | --- |
| GraphQL client API | Local status, onboarding, provider auth, chat startup, chat turns, transcript items, memory extraction activity, daemon errors |
| Daemon web server | Local React shell, GraphQL HTTP, and GraphQL WebSocket subscription endpoints |
| `A2uiCard` payloads | Future structured cards inside the chat stream |
| `config.yaml` and environment-derived config | Provider/model/default setup state; secrets remain environment-only |
| `NoemaPaths` and database config | Noema home, run directory, socket path, and Postgres connection/readiness state |
| Postgres memory tables | Concrete object rows, conversations, conversation_turns, conversation_items, memory items, entities, relationships, object provenance, grants, context graph inspection |
| `PostgresMemoryRepository` | Memory list/detail, durable conversation item replay, memory candidates, context graph read model, deterministic retrieval |
| CLI inspection commands | Reference behavior for memory and context graph display/redaction |

## Default Current Identities

| UI label | Internal identity | Notes |
| --- | --- | --- |
| You | `human:local` | The only visible human during current onboarding |
| Noema | `agent:primary` | The only visible assistant during current onboarding |

The frontend should not ask a beginner to create humans, agents, scopes, or
workspaces before first chat. Those are later customization and inspection
concepts.

## Route Matrix

Routes describe addressable states and backend backing. They do not imply
primary navigation priority.

| Route | Label | Backing | Capability | Status |
| --- | --- | --- | --- | --- |
| `/` | Chat | setup health, local service state | route to chat when ready; show guided readiness state if blocked | Current |
| `/setup` | Setup | local folder, assistant connection, local service checks | initialize or update setup through explicit setup flow | Current |
| `/chat` | Chat | local service stream | start chat, render transcript items, show inline memory/activity rows | Current |
| `/chat/:id` | Chat detail | active daemon conversation; persisted `conversation_items` after acknowledged turns | live while active; durable item replay after turn | Current limited |
| `/memory` | Memory settings | memory list read model | secondary memory management opened from chat or settings; redacted list, filters, review entry points | Current |
| `/memory/:id` | Memory detail | memory detail read model | opened from inline memory event or memory settings; inspect content/provenance/policy with reveal rules | Current |
| `/memory/review` | Review memory | memory candidates and active extracted memories | opened from review-required chat events or settings; inspect now; mutation only where repository/API supports it | Current limited |
| `/inspect` | Advanced inspection | read models over current Postgres state | owner/admin inspection hub; never required for ordinary chat success | Current |
| `/inspect/context-graph` | Context graph | context graph read model | owner/admin-only graph inspection, redacted by default | Current |
| `/inspect/context-packets` | Context packets | context packet tables if populated | inspect when rows exist; unavailable state otherwise | Current limited |
| `/settings` | Settings | paths/config/local service/assistant connection/database health | local maintenance, memory-management entry points, advanced drill-ins | Current |

Future route groups:

- `/threads`, `/threads/:id`.
- `/workspaces`, `/workspaces/:id`.
- `/projects`, `/projects/:id`.
- `/tasks`, `/tasks/:id`.
- `/runs`, `/runs/:id`.
- `/approvals`.
- `/tools`, `/tools/:capabilityId`.
- `/governance/grants`.
- `/governance/policy-simulator`.
- `/audit`.
- `/exports`, `/restore`.
- `/agents`, `/agents/:id`.

Future routes may appear as disabled rows only when doing so helps explain why
a feature is unavailable. Disabled rows must not present mutation controls.

## First Shell Assumption

The current implementation starts with a core-hosted local web chat served by
`noema start`. Longer term, the initial product shell should guide setup
without requiring a beginner to open a terminal after launch. That may remain
local web chat, or become a desktop app or launcher-backed inspector if the
launcher can perform the beginner actions.

The frontend owns these beginner actions:

- Create local folder.
- Check Codex sign-in.
- Start Noema.
- Start chat.
- View saved memory inline.

If the shell cannot perform one of these actions directly, the screen must show
one plain-language next step and a copyable command under `Show technical
details`. CLI-only setup does not satisfy the beginner onboarding goal unless
those commands are wrapped by a guided shell.

## Setup Health Read Model

The setup route is a blocking readiness state, not the product home. Once setup
is healthy enough, `/` should route to chat.

Checklist:

```text
Local folder
  -> Assistant connection
  -> Local service
  -> First chat
  -> First memory line
```

Default happy path:

1. Create local folder.
2. Check Codex sign-in.
3. Start Noema.
4. Start chat.
5. Send `Say hello and tell me Noema is working.`
6. Send `remember this: I prefer concise setup instructions`.
7. Click the `Memory saved` line.

Use Codex as the only beginner-path assistant connection. Other provider
settings belong behind `Show technical details` until they support the same
first-chat behavior.

Show:

- Local folder status, with the full path redacted by default outside
  owner/admin reveal.
- Config file existence and assistant connection status.
- Whether config was initialized by defaults.
- Postgres connection/readiness state.
- Local service reachable/unreachable.
- Provider readiness in beginner language: connected, not connected, timed out,
  or error.

Actions:

- Create local folder.
- Check Codex sign-in.
- Start Noema.
- Start chat.
- View saved memory inline.
- Force rewrite config only with explicit confirmation.
- Start Noema if the frontend shell owns that lifecycle.
- Show technical details.
- Open troubleshooting.

Never show:

- API keys.
- Environment variable secret values.
- Full account identifiers in normal shared views.
- Socket paths, runtime internals, YAML, or raw environment variables in the
  beginner path.

## Chat Stream Contract

Inputs:

- Optional initial prompt.
- Optional model override.
- Current working directory or project hint when available.

Events to render:

- User text.
- Assistant text.
- Activity notice with status: started, completed, failed.
- Saved memory line.
- Proposed memory line.
- Memory used line.
- Memory omitted or unavailable-context line.
- Structured cards when `A2uiCard` payloads exist.
- Error notices.
- Turn completion.

Current behavior:

- The current web frontend uses Noema's GraphQL client API. Queries provide
  scoped read models, mutations execute explicit Noema commands, and
  subscriptions stream conversation and activity events.
- Static assets are served over ordinary HTTP; product state and product
  actions go through GraphQL.
- The web home chat loads `human:local.primary_conversation_id`.
- The daemon GraphQL subscription endpoint is the live source for current turn
  updates.
- Durable chat history is reconstructed from `conversation_items`.
- Daemon runtime state is live coordination state only. After restart, Noema
  reactivates the durable conversation and assembles context from Postgres.
- Provider runtime ids are not part of the current product contract.
- `agent_status` is live coordination state and is not replayed as transcript
  history.
- `conversation_items` include user text, assistant text, durable activity
  rows, A2UI cards, tool calls/results, approvals, and meaningful errors.

Unavailable state:

- If a user opens a persisted conversation while Postgres replay is
  unavailable, show an unavailable state for durable history and avoid falling
  back to provider runtime resume assumptions.

## Inline Activity Row Contract

Every meaningful system-side object created, proposed, used, denied, or
blocked during chat should be representable as a compact row in the transcript.
The current slice must support memory rows; later slices add work, tool, approval, and run
rows.

Minimum row fields:

- Event type.
- Plain-language title.
- Short preview.
- Status.
- Source turn or message when available.
- Primary next action: expand, review, why, retry, or open.
- Redaction state.

Minimum row interactions:

- Expand inline when detail can be shown safely.
- Open object page or settings drill-in when deeper management is needed.
- Show unavailable state when a backing record does not exist.
- Never expose private, sensitive, or secret object existence through row text
  unless the viewer is authorized.

Current memory row types:

| Row | Trigger | Expanded content |
| --- | --- | --- |
| `Memory saved` | Low-risk authenticated `remember this:` or supported explicit save | What Noema remembers, where it lives, why saved, allowed use, keep/edit/stop using/open settings |
| `Memory proposed` | Inferred or risk-bearing candidate | Proposed content, evidence, risk, keep/edit/reject when supported |
| `Memory used` | Retrieval result affects answer | Included memory summary, eligibility reason, purpose, link to `What did Noema use?` |
| `Memory left out` | Context omitted or denied | Safe omission summary, coarse reason, owner/admin detail only after reveal |

## Memory List Read Model

Memory list is a secondary management surface, not the first proof that memory
works. The first proof is the inline `Memory saved` row in chat.

Minimum fields:

- Memory ID.
- Status.
- Type.
- Sensitivity.
- Home scope.
- Title preview with redaction.
- Content preview with redaction.
- Conversation/source link when available.
- Created time.
- Retrieval policy status.

Normal view:

- Redact private, sensitive, and secret previews.
- Coarsen metadata that would reveal private memory existence.
- Preserve a link back to the chat/event that created or used the memory when
  available.

Owner/admin advanced view:

- Effective retrieval policy status.
- Participant visibility policy.
- External egress policy.
- Authority/extraction method.
- Subjects, participants, provenance counts, and grant state after reveal.

Empty state:

`No memories yet. Start a chat or use "remember this" to create an inspectable
memory line.`

## Memory Detail Read Model

Memory detail can appear inline first and as a full object/settings page when
deeper management is needed.

Minimum inline detail:

- What Noema remembers.
- Where it lives.
- Why Noema believes it.
- How it may be used.
- Risk/sensitivity.
- Keep/edit/stop using/open settings actions when supported.

Minimum full-page tabs:

- Summary.
- Content.
- Provenance.
- Subjects and participants.
- Retrieval policy.
- Access.
- Usage when records exist.

Normal-mode summary:

- What will be remembered.
- Where it lives.
- Why Noema believes it.
- Risk.
- Keep/edit/reject actions when mutation endpoints exist.

Advanced inspection:

- Fingerprints.
- Purpose rules.
- Trusted object links.
- Grants.
- Context packets.
- Memory-use records.
- Raw JSON metadata where authorized.

Mutation boundary:

- The UI may show target actions such as confirm, edit, dispute, archive,
  delete, restore, revalidate, and grant, but each action must be disabled or
  routed through an implemented repository/API path.
- Disabled actions must explain the missing backend operation.

## Memory Review Queue

Backed by:

- Memory records with candidate or review-worthy lifecycle states.
- Explicit remember records and ordinary extraction candidates.

Entry points:

- Review-required activity row in chat.
- Expanded memory card.
- Memory settings.
- Optional later attention/Home surface.

Card fields:

- Proposed memory.
- Proposed home scope.
- Source/evidence.
- Sensitivity.
- Confidence when present.
- Subjects and participants, redacted where required.
- Risk reason.

Actions:

- Keep.
- Edit.
- Reject.
- Open advanced inspection.

Bulk rules:

- Bulk actions are allowed only for low-risk normal memories.
- Sensitive, secret, contradictory, external, action-triggering, or policy-stale
  memories require individual review.

## Context Graph Inspector

Status: current owner/admin-only.

Backed by:

- The same persisted graph read model used by `noema context graph`.

Sections:

- Memories.
- Retrieval policy.
- Entities.
- Subject edges.
- Participant edges.
- Provenance edges.
- Trusted object links.
- Purpose rules.
- Access grants.
- Context packets if present.
- Context packet memory edges if present.
- Context packet omissions if present.
- Memory-use records if present.
- Memory events.
- Relationship claims.

Rules:

- Do not make this a top-level normal-user navigation item.
- Do not use it as the primary explanation for a beginner chat event.
- Default to redacted normal view.
- Require owner/admin reveal for private, sensitive, and secret node/edge
  details.
- Do not expose graph paths to agents unless every backing memory is includable
  for the run.

## Context Packet Inspector

Status: Current limited.

Backed by:

- `context_packets`, `context_packet_memory_edges`,
  `context_packet_omissions`, and `memory_use_records` when populated.

Current limitation:

- Current chat flow records chat turn provenance and memory extraction, but
  does not yet consistently persist context packets for each turn.

Unavailable state:

`No persisted context packet exists for this turn yet. The backend needs to
assemble deterministic retrieval context and record the packet before this view
can explain exactly what Noema saw.`

Backend deliverable:

- Call deterministic retrieval for chat turns.
- Persist context packet, included memories, omissions, and use records.
- Link the packet to a run or stable turn ID.

## Memory Access Preview Contract

Status: Current limited.

Backed by:

- Existing deterministic memory retrieval engine.

The current slice preview answers memory retrieval and inclusion only. It must return
`Missing backend` for writes, external sends, exports, capability operations,
durable approvals, and grant-impact simulations beyond current memory grants.

Preset questions:

- Can Noema use this memory in this chat?
- Why was this memory included?
- Why was this memory omitted?
- What would change if I grant access?

Modes:

- Agent-visible: exact agent context only, no denied private+ enumeration.
- Normal user: coarse categories and readable objects only.
- Owner/admin: exact objects, grants, denials, and policy rows after reveal.

Outputs:

- Allowed.
- Denied.
- Approval required, when supported.
- Redacted.
- Missing backend.

## Disabled/Future Controls

These controls must be disabled until durable runtime support exists:

- Create thread from persisted conversation list.
- Promote chat to durable workspace.
- Create task.
- Start task run.
- Approve tool call.
- Grant capability access.
- Revoke capability access.
- Invoke external tool.
- Retry durable run.
- Replay run.
- Create export.
- Export scope.
- Export filtered memories.
- Restore export bundle.
- Enable proactive automatic action.

Disabled controls should name the missing dependency, such as "requires
capability registry" or "requires durable run ledger."

## Acceptance Criteria

Current IA implementation is acceptable when:

- A low-technical-skill user can create the local folder, connect Codex, start
  Noema, send one message, save one explicit memory, and see the `Memory saved`
  row in chat without reading technical labels.
- The first successful experience shows one visible user, `You`, and one
  visible assistant, `Noema`.
- The saved memory row expands inline to show what was saved, why it was saved,
  where it lives, and safe next actions.
- Memory management remains reachable from the expanded card or settings, but
  does not replace chat as the first success state.
- Onboarding does not expose projects, tasks, tools, approvals, agents, audit,
  graph, packets, raw IDs, YAML, socket paths, or command palette mutations.
- Unsupported run/approval/tool/task controls are absent or clearly disabled.
- Memory list/detail matches CLI redaction posture or is stricter.
- Context graph is owner/admin-only and not exposed as agent-visible context.
- Context packet views show unavailable states when no packet exists.
- Access preview cannot enumerate denied private, sensitive, or secret objects
  in normal or agent-visible modes.
- Export, delete, grant, approve, and external-effect actions cannot bypass
  governance via command palette or disabled screens.
- Every advanced view is reachable from a plain-language `Show details`,
  `Why?`, or object-specific action rather than from primary beginner nav.
