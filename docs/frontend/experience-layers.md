# Experience Layers

Noema should start as the simplest useful local assistant and grow into a
transparent agent operating system only as the user and task require it.

The product value this IA protects:

```text
Extremely simple first run.
Complexity that grows out of chat.
Deep transparency on demand.
```

## Default First-Run Model

The first experience should be:

```text
You -> Noema -> first chat -> saved memory line -> expanded details when wanted
```

Default identities:

| UI label | Internal identity | Notes |
| --- | --- | --- |
| You | `human:local` | The only visible human in the current slice |
| Noema | `agent:primary` | The only visible agent in the current slice |

Beginner copy should not mention `principal`, `scope`, `daemon`, `socket`,
`context packet`, `egress`, `grant`, provider runtime internals, or
`policy simulator`. Those are technical details inside inspection.

## Onboarding Ramp

The first complete product surface is chat. If setup is incomplete, the UI can
show a guided readiness state before or inside the chat shell, but setup should
not become the enduring primary experience.

Setup checklist:

```text
Provider connection or verified local model
  -> Review model assignments
  -> First chat
  -> First memory line
```

Primary button by state:

| State | Primary CTA | Beginner explanation |
| --- | --- | --- |
| No provider ready | Choose a provider | Use Local, OpenRouter, or Codex for Noema's model work. |
| Provider ready | Confirm models and start chat | Review Noema's role-aware defaults before assigning any workloads. |
| No chat yet | Start chat | Send one message to check that Noema can answer. |
| No saved memory | Save first memory | Say `remember this:` to create an inspectable memory line. |
| Ramp complete | Continue chat | Noema is ready. |

Happy-path first chat:

```text
Prompt: Say hello and tell me Noema is working.
```

Happy-path first memory:

```text
remember this: I prefer concise setup instructions
```

Use explicit memory for the first proof because it is deterministic. Ordinary
memory extraction can remain visible later, but onboarding should not depend on
an inferred candidate appearing.

Explicit `remember this:` from the current authenticated human creates a saved
memory immediately and inserts a `Memory saved` activity line in the chat. It
does not enter the review queue by default unless its access policy is
incomplete or the content is contradictory, action-triggering, externally
sourced, or otherwise requires policy review. Private classification alone does
not require review.

## Post-Ramp Chat

After the first memory, the default surface remains chat. The visible actions
should be conversational and contextual:

1. Continue chatting.
2. Expand the saved memory line.
3. Open memory settings only if the user wants deeper management.

Secondary actions such as local setup, memory browse, and advanced inspection are
available through small utility controls, expanded event details, or settings.
They should not take over the primary frame.

## Default Choices

Use defaults aggressively:

- Default local folder: `~/.noema`, displayed as `Local Noema folder`.
- First-run provider choices: Local, OpenRouter, and Codex with equal priority.
- Model assignments remain drafts until the user confirms the complete setup.
- Default local service: Noema local service, displayed as `Noema`.
- Default user: `You`.
- Default agent: `Noema`.

Hide these behind `Show technical details` during onboarding and settings:

- Full local paths.
- Config file name and YAML.
- Environment variables.
- Model override.
- OpenAI provider setup.
- Base URL, organization ID, project ID.
- Sandbox and ephemeral settings.
- Socket path.
- Provider runtime internals.

OpenAI remains an advanced setup path outside first-run onboarding.

## Beginner Labels

Use:

- Start chat.
- Continue chat.
- Save memory.
- Memory saved.
- What did Noema use?
- Why?
- Open memory settings.
- Show details.
- Start Noema.
- Check again.
- Open local folder.
- Troubleshoot.

Avoid in beginner flows:

- Daemon.
- Socket.
- Principal.
- Scope.
- Run envelope.
- Context packet.
- Egress.
- Grant.
- Provider runtime internals.
- Policy simulator.

## Progressive Disclosure Levels

Reveal complexity by object maturity and user intent, not by exposing every
architecture noun at once.

| Level | Trigger | Primary UI pattern | Management drill-in |
| --- | --- | --- | --- |
| Solo chat | First run and ordinary chat | Single chat pane | Settings utility only |
| Trust event | Memory saved/used/omitted, explicit egress transformation, or denial | Inline activity line in chat | Expanded card, memory settings, access preview |
| Threaded chat | Multiple useful conversations exist | Thread rail beside chat | Conversation list/search |
| Work | User creates or links durable work | Workspace/task panel beside chat | Workspace/project/task pages |
| Governed tools | Capability proposals, grants, invocations, approvals exist | Inline approval/tool cards | Tools and permissions drill-ins |
| Orchestration | Durable runs, task execution, handoffs, proactivity exist | Run timeline inside chat/work context | History/audit/recovery pages |
| Multi-agent / multi-human | More than one active human/agent or delegation exists | Handoff and participant cards in context | Agents/people/shared-scope pages |

Staged surface growth:

```text
The current slice:
Chat
Small utility: Settings
Contextual drill-ins: Memory, What did Noema use?, Advanced inspection

After durable conversations:
Chat + thread rail

After project/task backing:
Chat + workspace/task panel
Workspace/project pages when opened from chat

After capability + approval backing:
Inline tool and approval cards
Tools / Permissions as drill-ins

After durable ledger and recovery:
Run timelines inside chat/work
History / Audit as drill-ins

After multiple agents:
Agent and handoff cards in chat/work
Agents as a drill-in
```

Introduce a new persistent surface only when all three are true:

1. There is backed durable state or a bounded read model.
2. The user has a concrete job that cannot be handled cleanly inside chat,
   an inline detail card, or the current work panel.
3. The surface answers a user-facing question, not just an architecture concept.

## Transparency Ladder

Transparency should be available from every important object, but it should not
be the default surface for beginners.

| Layer | User question | UI pattern | Examples |
| --- | --- | --- | --- |
| Summary | What happened? | Compact line in chat | `Memory saved`, `No external action taken` |
| Explain | Why? | Expandable line, popover, or side drawer | What Noema saw, why included, what was left out |
| Record | Show me the record | Inline object detail or object page | Memory provenance, usage, policy, participants |
| Internals | Show exact internals | Owner/admin advanced settings or Inspect | Context graph, packet rows, raw policy details |
| Portable truth | Give me the record | Export/replay preview | Manifested export, explicit transformations, derived-state omissions |

Plain-language labels:

| Beginner label | Advanced label |
| --- | --- |
| What Noema used | Context packet manifest |
| Used memory | Memory-use records |
| Left out by policy | Context packet omissions |
| Where this came from | Provenance edges |
| Who was involved | Participants |
| What this is about | Subjects/entities |
| Allowed uses | Purpose rules |
| Can leave Noema? | External egress policy |
| Policy health | Stored/effective retrieval policy status |
| Exact graph | Privileged context graph inspector |

## Inspection Trigger Points

Put transparency links where the user naturally asks why:

- Chat turn: `What did Noema use?`, `Memory used`, `External sharing`.
- Memory activity line: `Details`, `Why does Noema know this?`,
  `Where can this be used?`.
- Expanded memory card: `Open memory settings`, `Show provenance`,
  `Preview access`.
- Memory review: `Evidence`, `Risk`, `Advanced policy`.
- Denial: `Why denied?`, `What would make this allowed?`.
- Approval: `Data leaving Noema`, `Policy reason`, `Approval scope`.
- Tool invocation: `Operation`, `Resource`, `Input`, `Result`.
- Export: `Included data`, `Transformations`, `Manifest preview`.

## Power User Growth

Power users should not need different product objects. They need denser views
and faster paths:

- Pin advanced panels.
- Save filters.
- Use object-aware command palette actions.
- Start with access preview presets, then reveal raw policy inputs.
- Drill down from any answer to memory, provenance, policy, use records, run
  events, and exportable records.
- Export filtered records with manifests.

The expert drilldown path should be deterministic:

```text
Chat turn or activity line
  -> Expanded explanation
  -> Object detail
  -> What Noema used
  -> Included memories and omissions
  -> Memory detail
  -> Provenance, participants, subjects
  -> Retrieval policy and grants
  -> Use records
  -> Run or audit events
  -> Exportable record
```

## Acceptance Criteria

- A low-technical-skill user can create the local folder, connect Codex, start
  Noema, send one message, save one memory, and see a memory-saved line in chat
  without reading technical labels.
- The saved memory line can expand in place to show what Noema remembers,
  where it lives, why it was saved, and safe next actions.
- The expanded memory card can route to memory settings for deeper management.
- The current slice shows one visible user and one visible assistant by default.
- Onboarding does not expose projects, tasks, tools, approvals, agents, audit,
  graph, packets, raw IDs, YAML, transport details, or command palette mutations.
- Unsupported controls are hidden, absent, or disabled with exact missing
  dependency text.
