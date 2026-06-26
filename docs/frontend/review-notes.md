# IA Review Notes

This file records how the frontend IA plan was produced and how adversarial
review findings were resolved.

## Orchestration

The plan was drafted through parallel subagent work:

- Domain inventory: primary objects, secondary objects, relationships,
  metadata, and navigation taxonomy.
- Workflow model: top user jobs, setup, chat, memory, run, approval, task,
  handoff, recovery, command palette, and empty/error states.
- Governance model: memory inspection, permissions, access preview,
  capabilities, audit, proactivity, privacy, export, and restore.

After synthesis, three adversarial reviewers inspected the draft:

- Usability and mental model.
- Governance, privacy, and egress safety.
- IA cleanliness and implementation sequencing.

A second refinement pass used the same workflow for the project values:

- Extremely simple onboarding with one default human and one default agent.
- Complexity that scales with the task and the user.
- Deep transparency when the user asks to understand what is happening.

Three refinement subagents covered:

- The onboarding ramp for a low-technical-skill user.
- Progressive disclosure as work, tools, approvals, and multi-agent use appear.
- The transparency ladder from summaries to exact owner/admin internals.

## Major Corrections From Review

### Current slice Navigation Was Overpromising

Original issue:

- The current slice navigation listed runs, approvals, tools, and context graph as first-class
  active destinations.
- Current Rust backs setup/config, daemon chat, memory persistence, and memory
  graph inspection, but not durable run envelopes, approvals, tool registry, or
  task execution.

Resolution:

- The current slice no longer lists runs, approvals, tools, and context graph as first-class
  active destinations.
- `docs/frontend/current-contract.md` defines route status and disabled future
  routes.
- Runs, approvals, tools, projects, tasks, and audit remain target IA until
  their durable runtime paths exist.
- This correction was later tightened: The current slice defaults to chat, while memory,
  settings, and owner/admin inspection are contextual or utility drill-ins
  rather than primary beginner navigation.

### Mental Model Was Too Architect-Facing

Original issue:

- `Scope + Principal + Object + Run + Evidence + Policy` is useful internally
  but not how a first user decides where to click.

Resolution:

- The hub doc now starts with a user mental model:
  `Where am I working? Who is acting? What does Noema know? Why does it know
  that? What needs my decision? What can leave Noema?`
- Architecture terms are explicitly treated as inspection labels, not primary
  navigation copy.

### Context Graph Could Leak Debug State

Original issue:

- "Parity with `noema context graph`" could imply normal user or agent-visible
  graph views exposing memory IDs, entities, relationships, provenance,
  omissions, grants, and denial reasons.

Resolution:

- Context graph is owner/admin-only.
- It is redacted by default in normal views.
- Private, sensitive, or secret node/edge existence, aliases, source names,
  denial reasons, and counts require authorized reveal.
- Graph remains nested under Inspect/Memory, not a top-level normal-user object.

### Access Preview Could Become An Oracle

Original issue:

- Access preview can leak denied private objects, memory existence, resource
  names, grant IDs, or exact denial reasons.

Resolution:

- Access preview now has agent-visible, normal user, and owner/admin modes.
- Agent and normal modes show only allowed context or coarse categories/counts.
- Exact denied objects and grant rows require authorized owner/admin reveal.

### Memory Review Was Too Dense

Original issue:

- Memory list/detail exposed many policy and graph internals before answering
  the review question: should Noema remember this?

Resolution:

- Normal review card is reduced to:
  `What will be remembered`, `Where it lives`, `Why Noema believes it`, `Risk`,
  and `Keep/Edit/Reject`.
- Policy fingerprints, grants, purpose rules, object links, versions, and use
  records are advanced inspection.

### Approval Execution Needed Revalidation

Original issue:

- Approval cards showed enough data to decide, but did not require revalidation
  before execution.

Resolution:

- Approval now records payload/diff hash, resource selector, destination, data
  classes, policy version/fingerprint, expiration, and consumption state.
- Any changed payload, recipient, destination, resource selector, egress class,
  sensitive data class, policy version, or expiration returns to approval.

### Export And Restore Needed Anti-Bypass Rules

Original issue:

- Export/restore could bypass sensitivity, tombstones, grants, and egress
  review if treated as simple file operations.

Resolution:

- Every export is an egress event.
- Full private/sensitive/secret exports require explicit confirmation.
- Unencrypted export destinations need warnings.
- Imported export bundles are untrusted input.
- Restored grants, approvals, capability connections, proactivity rules,
  credentials, and external connectors default inactive until reviewed.
- Restore cannot resurrect tombstoned or deleted data without explicit conflict
  approval.

### Schema And Runtime Boundaries Were Blurry

Original issue:

- The target Postgres doc includes surfaces that current Rust bootstrap/runtime
  does not fully create or use.
- Context packet tables exist, but chat turns do not yet persist context
  packets consistently.

Resolution:

- The hub doc and object model now include backing status tables.
- current contract identifies table-backed-but-not-flow-wired surfaces.
- Context packet views must show unavailable states when no packet exists.

### Onboarding Needed A Smaller Success State

Original issue:

- Setup still read like a technical readiness checklist rather than a simple
  first success path.

Resolution:

- Added [experience-layers.md](experience-layers.md).
- The current slice now starts with `You -> Noema -> first chat -> saved memory -> details
  when wanted`.
- `/setup` is a guided checklist: local folder, assistant connection, local
  service, first chat, first memory.
- The happy path uses Codex, a seeded first prompt, and an explicit
  `remember this:` message so success does not depend on inferred memory.

### Complexity Needed A Growth Ladder

Original issue:

- The target IA could still feel like every architecture object wanted a future
  top-level navigation slot.

Resolution:

- Added progressive levels: Solo, Trust, Work, Governed Tools, Orchestration,
  Multi-Agent / Multi-Human.
- Primary surfaces appear only when backed state, user intent, and a concrete
  job all exist.
- The current slice keeps `Inspect` as a secondary owner/admin utility rather than part of the
  first successful path.

### Transparency Needed Layers

Original issue:

- "Inspect" was doing too much conceptual work.

Resolution:

- Added a transparency ladder: Summary, Explain, Record, Internals, Portable
  truth.
- Beginner labels now map to advanced labels, such as `What the agent saw` to
  context packet manifest and `Where this came from` to provenance edges.
- Deep inspection remains deterministic and available, but it is not the
  default beginner experience.

### Second Review: Beginner Ramp Still Leaked Technical Concepts

Original issue:

- The revised docs still used `daemon`, `socket`, `provider`, `Noema home`, and
  `context packet` in places that could become beginner-facing labels.
- `Inspect` was still ambiguous because routes existed even though the primary
  nav demoted it.

Resolution:

- Beginner-facing labels now use local folder, assistant connection, local
  service, what Noema used, and Show technical details.
- `/inspect` remains a current owner/admin route but must not appear in primary
  navigation during onboarding or normal beginner use.
- Added the first-shell assumption: The current slice must provide a guided shell that can
  perform beginner setup actions, or show one plain-language next step plus a
  copyable command under Show technical details.

### Second Review: Current slice Boundaries Needed Sharper Edges

Original issue:

- Export was both described as a current settings capability and future-gated.
- Access preview sounded broader than current memory retrieval support.
- Owner/admin reveal was not defined for the one-user current model.
- Explicit memory lifecycle was contradictory.

Resolution:

- The current slice export and restore are disabled entry points until a governed export
  pipeline exists.
- The current slice access preview is now explicitly memory-only.
- Owner/admin reveal is explicit, session/object-scoped, and non-persistent.
- Low-risk authenticated `remember this:` creates saved memory immediately;
  review queues are for inferred, risky, external, contradictory, or
  unauthenticated cases.

### Third Review: IA Was Still Dashboard-First

Original issue:

- The prior docs made a good settings/admin dashboard, but not the primary
  product experience.
- `Home`, `Memory`, `Settings`, and `Inspect` were still treated as visible
  top-level surfaces too early.
- Memory proof routed too quickly to a memory page instead of appearing in the
  chat history where the user caused it.
- Projects/tasks/workspaces were described as separate navigation destinations
  rather than something that should reveal from a chat becoming durable work.

Resolution:

- The hub doc now defines Noema as chat-led and object-backed.
- The current slice defaults to `Chat`; settings, memory, and owner/admin inspection are
  secondary drill-ins.
- Explicit `remember this:` creates a `Memory saved` activity line in chat.
  Clicking the line expands memory details inline before offering a memory
  settings drill-in.
- Multiple threads appear only after there is more than one useful
  conversation.
- Workspaces, projects, tasks, approvals, tools, and recovery states first
  appear as chat/work activity rows or side panels, then open full management
  surfaces when there is backed durable state.
- The object model now distinguishes chat as the primary entry/reveal surface
  from durable objects as the source of truth.

## Remaining Open Questions

- Which shell implements the required guided local UI: desktop app, local web
  chat, or launcher-backed inspector?
- When should the thread rail appear?
- What exact signals promote a chat into a workspace or task panel?
- What user roles exist before multi-human workspaces ship?
- Should sensitive or secret reveal require re-authentication?
- Which capability ships first after memory: filesystem, tasks, or an external
  connector?
- Which export formats ship first?
- Which memory lifecycle mutations should be implemented before the frontend
  exposes keep/edit/reject?
- Which exact lifecycle mutations should back `Review saved memory` before it
  becomes editable instead of inspect-only?

## Review Standard For Future Changes

Future frontend IA changes should pass these checks:

- Does this surface have a backing runtime/read model today?
- If not, is it clearly marked future or disabled?
- Can the page leak private object existence through metadata, counts, graph
  edges, filters, or denial reasons?
- Does every external effect go through egress review?
- Does every trust-critical command have preview and confirmation?
- Can a normal user understand the next action without reading architecture
  vocabulary?
- Does the page point to evidence and provenance when trust is required?
