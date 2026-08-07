# Governance And Inspection IA

Governance is not a settings afterthought, but it is also not the first screen.
In Noema, permissions, memory, proactivity, capabilities, approvals, audit, and
export are core product responsibilities because they determine whether humans
can trust agent work.

The primary reveal pattern is contextual:

```text
Chat/work event
  -> compact trust line
  -> expanded explanation or decision card
  -> object/settings/admin drill-in only when needed
```

Governance pages remain important for search, bulk review, configuration,
audit, and owner/admin inspection. Normal users should first encounter
governance at the moment it matters: a saved memory, omitted context, proposed
tool action, approval request, denial, export preview, or paused run.

## Owner/Admin Versus Agent-Visible Views

Noema needs separate inspection modes:

| Mode | Purpose | Visibility |
| --- | --- | --- |
| Agent-visible context | What the model or agent may see | Authorized ordinary and private information intact; unauthorized private information omitted; secrets absent |
| Normal user view | What the current human can manage safely | Exact information the human is authorized to inspect; secrets remain write-only or referenced by presence |
| Owner/admin inspection | Privileged local debugging and audit | Exact ordinary/private detail within inspection authority; secrets never reveal |
| Export view | Portable record governed as egress | Manifested and scoped; private information follows destination policy; secrets excluded |

The current memory inspection surface is backed by GraphQL
`memoryClaims`/`memoryClaim` and the bounded `memoryGraph` read model. Any
richer context graph inspector is a future owner/admin drill-in beyond the
current read model. The frontend should not reuse owner/admin detail as
agent-visible context. Unauthorized private graph node existence, edge
existence, aliases, source names, denial reasons, and exact counts must be
omitted. Authorized private graph information is shown intact. Secret material
is not graph content.

Current reveal rule: owner/admin inspection means a local interactive user
explicitly enters Inspect or clicks Reveal for a specific object/session.
Reveal state is non-persistent and must not be inferred from merely being on the
local machine. Export, screenshot, and sharing destinations rerun egress policy.
Private information may require stronger authorization; secret values never
reveal.

## Transparency Ladder

Use layered transparency instead of one global inspection mode.

| Layer | User question | UI pattern | Content |
| --- | --- | --- | --- |
| Summary | What happened? | Compact line in chat/work history | `Memory saved`, `No external action taken`, `Approval needed` |
| Explain | Why? | Expanded line, drawer, or popover | What the agent saw, why a memory was included, what was left out |
| Record | Show me the record | Inline object card or object-owned tabs | Provenance, participants, subjects, usage, policy, grants |
| Internals | Show exact internals | Owner/admin advanced settings or Inspect | Context graph, packet rows, exact IDs, raw policy details |
| Portable truth | Give me the record | Export/replay preview | Manifest, explicit transformations, omitted derived state, replay boundaries |

Beginner views should answer what happened and what to do next before showing
how the system works internally. Expert views should be reachable from any
important answer without becoming the default experience.

Plain-language labels:

| Beginner label | Advanced label |
| --- | --- |
| What the agent saw | Context packet manifest |
| Used memory | Memory-use records |
| Left out by policy | Context packet omissions |
| Where this came from | Provenance edges |
| Who was involved | Participants |
| What this is about | Subjects/entities |
| Allowed uses | Purpose rules |
| Can leave Noema? | External egress policy |
| Policy health | Stored/effective retrieval policy status |
| Exact graph | Privileged context graph inspector |

Inspection links should appear where the user naturally asks why: chat turns,
inline activity rows, memory cards, memory review, denials, approvals, tool
invocations, workspace/task cards, and export previews.

## Memory Surfaces

### Inline Memory Events

Memory should first appear where it was created, used, or omitted. The memory
settings/list page is the management drill-in, not the first proof that memory
exists.

Inline event types:

| Event | Normal copy | Expanded detail |
| --- | --- | --- |
| Saved | `Memory saved` | What Noema remembers, where it lives, why saved, allowed uses, safe actions |
| Proposed | `Memory proposed` | Evidence, confidence, risk, keep/edit/reject |
| Used | `Memory used` | Why included, eligible purpose, safe source summary |
| Omitted | `Some memory left out` | Coarse omission reason; exact objects only when authorized |
| Review required | `Memory needs review` | Risk reason, source, individual review path |

Inline events must preserve the same authorization posture as list/detail
pages. They show authorized private memory intact and must not expose
unauthorized private-memory existence through titles, counts, filters, or exact
denial reasons. Secret material is invalid memory content.

### Memory List

Default normal-mode table columns, for the secondary management page:

- Status.
- Type.
- Information class.
- Home scope.
- Owner.
- Retrieval policy status.
- Authority level.
- Extraction method.
- Source type.
- Created/updated.
- Title preview when authorized; otherwise no object-specific preview.

Advanced owner/admin columns:

- Effective retrieval policy status.
- Participant visibility policy.
- External egress policy.
- Fingerprint state.
- Grant state.
- Context packet and use counts.

Filters:

- Scope, owner, creator, participant, subject.
- Type, status, information class, authority, extraction method.
- Retrieval policy status and effective status.
- Participant visibility policy.
- External egress policy.
- Purpose allow/deny rule.
- Source type.
- Used in reply, used for action, used externally, used proactively.
- Needs review, stale policy, disputed, private, action-triggering.

Filter option lists, autocomplete suggestions, result counts, empty states, and
saved filter names follow the same authorization as rows. They must not expose
unauthorized private subject names, participant names, source names, or exact
zero/nonzero existence through filters.

Metadata authorization matrix:

| Viewer capability | Ordinary memory | Private memory |
| --- | --- | --- |
| List row | Metadata and preview | Exact metadata/content when authorized; otherwise omit object-specific detail |
| Subject/participant names | Show when the linked object is visible | Show when authorized for the memory; otherwise omit |
| Provenance source names | Show source type/name | Show when authorized; otherwise omit object-specific detail |
| Relationship predicates/neighborhood | Show if backing memory is includable | Show when authorized; otherwise hide edge existence |
| Counts and filters | Exact counts allowed | Exact for authorized result sets; suppress counts that would reveal unauthorized existence |
| Denial reasons | Normal reason allowed | Agent-visible coarse reason; exact audit detail only when authorized |

### Memory Detail

Memory detail has two presentation levels:

- Inline detail in chat: concise content, source, home location, risk, allowed
  use, and immediate safe actions.
- Full detail in memory settings/object page: tabs, filters, lifecycle actions,
  access preview, provenance, policy, usage, versions, and advanced reveal.

Tabs:

- Content.
- Provenance.
- Subjects.
- Participants.
- Access.
- Retrieval policy.
- Usage.
- Versions.
- Related claims.

Critical fields:

- Home scope.
- Subjects and roles.
- Participants and roles.
- Provenance source, relation, evidence excerpt, authority level.
- Retrieval hints as non-authoritative ranking metadata.
- Stored and effective retrieval policy state.
- Fingerprint, extractor principal, extractor version, validation time.
- Participant visibility policy.
- External egress policy.
- Purpose rules.
- Trusted object links.
- Access grants.
- Usage stages by run.

Actions:

- Confirm.
- Edit.
- Dispute.
- Supersede.
- Archive.
- Delete.
- Restore.
- Revalidate retrieval policy.
- Add/revoke grant.
- Preview access.
- Export.

### Memory Review Queue

Queue sections:

- Explicit memories requiring review because their access policy is incomplete
  or they are externally sourced, action-triggering, contradictory, or
  unauthenticated.
- Ordinary extraction candidates.
- Candidates whose private-information access policy needs review.
- Contradiction-prone candidates.
- Action-triggering candidates.
- Stale retrieval-policy items.

Entry points:

- Review-required chat activity line.
- Expanded memory card.
- Memory settings.
- Later attention/Home surface when enough pending work exists.

Review card fields:

- Proposed content.
- Type, information class, confidence.
- Source and evidence.
- Subjects and participants.
- Why it is durable.
- Why it may be risky.
- Proposed home scope.
- Suggested action.

Bulk approval should be disabled for unresolved access policy, contradictory,
externally sourced, or action-triggering items. Private classification alone is
not a review requirement.

Authenticated low-risk `remember this:` creates confirmed memory and may appear
as recent activity, not as a required review item.

Normal review card:

- What will be remembered.
- Where it lives.
- Why Noema believes it.
- Risk.
- Actions: keep, edit, reject.

Advanced inspection contains policy fingerprints, grants, purpose rules,
trusted object links, versions, and use records.

## Context And Explainability

Every run and conversation turn should link to "What did Noema use?" or a
similarly plain-language explanation. The first interaction should be a
contextual expansion from chat/work, not a jump to raw inspection.

Show:

- Requesting principal.
- Executing agent.
- Active scopes.
- Purpose.
- Context packet ID and run ID.
- Included memories with eligibility reason and rank reasons.
- Included sources, documents, artifacts, and tool summaries with trust labels.
- Memory omitted by an authorization-safe reason.
- Audit-only omission detail for authorized inspection.
- Information-class summary.
- Policy summary.

Explain inclusion and omission separately. A denial explanation can itself leak
private memory existence, so agent-visible omissions remain coarse for
unauthorized private data. Authorized private content and explanations remain
intact.

The current slice must show an unavailable state when a chat turn has no persisted context
packet. The backend deliverable is to call deterministic retrieval and persist
context packets for chat turns before this panel can be complete. The
unavailable state should live behind the expanded `What did Noema use?` affordance,
not as a primary beginner destination.

## Permissions IA

Permissions should first appear as contextual questions in chat/work:

- `Can Noema use this memory here?`
- `What would be sent to this tool?`
- `Why was this denied?`
- `What changes if I grant access?`

When enough backing state exists, permissions should have three main management
surfaces.

### Grants Explorer

Rows:

- Grant ID.
- Principal.
- Target memory, scope, capability, operation, resource, or selector.
- Permission.
- Allow/deny effect.
- Scope where the grant applies.
- Expiration.
- Issuer.
- Created time.
- Revocation state.

Views:

- By principal.
- By scope.
- By memory.
- By capability/operation.
- By resource selector.
- Expiring soon.
- Denies.
- Revoked.

Actions:

- Grant.
- Deny.
- Revoke.
- Expire.
- Duplicate as narrower grant.
- Preview access impact.

### Access Preview

Question:

```text
Given principal X, agent Y, scope set S, purpose P, and proposed action A,
what can be read, used, written, sent, or exported?
```

Inputs:

- Human.
- Agent.
- Active scopes.
- Purpose.
- Target object.
- Capability and operation.
- Resource selector.
- Proposed egress destination.

Output:

- Allowed resources.
- Denied resources.
- Approval-required resources.
- Private resources omitted because the simulated principal lacks access.
- Relevant grants.
- Hard denials.
- Revocations.
- Required next action.

The current slice access preview is memory-only. It answers retrieval and inclusion questions
using the deterministic memory retrieval engine. It must return `Missing
backend` for writes, external sends, exports, capability operations, durable
approvals, and grant-impact simulations beyond current memory grants.

Preview modes:

| Mode | Allowed output |
| --- | --- |
| Agent-visible | Only what the agent would actually see; no denied private+ object IDs, names, grants, or exact denial reasons |
| Normal user | Safe categories, coarse counts, and readable objects the user already has access to |
| Owner/admin | Exact grants, denials, object IDs, reasons, and policy rows after explicit inspection |

Useful presets should come before raw fields:

- Can this agent use this memory in this chat?
- What would be sent to this tool?
- Why was this denied?
- What changes if I grant access?
- What would this export include?

### Policy Simulator

The simulator should construct a run envelope and proposed operation, then show
policy precedence in a deterministic order:

1. Explicit deny.
2. Hard safety or privacy rule.
3. Revocation.
4. Scope grant or durable approval.
5. Contextual elevation.
6. Agent grant.
7. Workspace or human default.
8. System default.

The simulator is for inspection and planning. It should not bypass the harness.

## Approval Inbox

Approval cards should first render inline in chat/work context and be
decision-ready without requiring trust in model prose. A separate approval
inbox becomes useful once multiple pending approvals can exist independently of
the current thread or workspace.

Fields:

- Requested action.
- Capability and operation.
- Resource selector.
- Destination.
- Payload or diff.
- Payload hash or diff hash.
- Information class and any orthogonal action-risk labels.
- Data leaving the run boundary.
- Run/task/project/conversation links.
- Requesting principal and executing agent.
- Policy reason.
- Policy version or fingerprint.
- Risk classification.
- Expiration.
- One-time versus reusable scope.
- Consumption state.
- Prior related approvals or denials.

Outcomes:

- Approve once.
- Approve with narrower scope.
- Deny.
- Request changes.
- Expire.
- Revoke previous approval.
- Cancel because parent run was cancelled.

Before adapter execution, the harness must re-evaluate policy. Any changed
payload, recipient, destination, resource selector, egress class, private-data
handling, policy version, expiration, or approval consumption state returns
the request to approval.

## Capabilities And Tools

Tools should first appear as governed proposals in chat/work context. The user
should see what Noema wants to do, what data would leave Noema, and why policy
requires approval before seeing adapter schemas or registry internals.

The management drill-in should be presented through a capability registry
model.

Current Settings MCP slice:

- `/settings/mcps` shows configured third-party MCP server metadata, discovered
  tools, health/auth state, and effective behavior hints with provenance. It is
  metadata-only and does not invoke MCP tools during setup. Add-server setup
  follows: input details -> verify server -> authenticate if needed -> fetch
  tools/schema -> configure tools.
- `/settings/trusted-identities` lists trusted identity selectors for emails,
  phone numbers, and domains. These selectors are the user-owned trust anchors
  for ownership extraction.
- `/settings/approvals` is present as a limited read-model surface for pending
  MCP approval requests. Rows should carry structured decision evidence when
  mediation starts producing them: requested action, source and destination
  summaries, source and destination owner identity/trust, export summary, active
  scope, requester, linked invocation id, server/tool ids, and an authorized
  payload preview with actual secret material excluded.
- `/settings/audit` is not currently exposed. The placeholder surface was
  removed until mediated MCP invocation, quarantine, approval, and audit event
  persistence are implemented.

The approval surface intentionally treats every row as decision material, not
model prose. Approval requests must be linked to a tool invocation, terminal
states require actor/time decision evidence, and persisted payload previews
must exclude values identified as secrets by authoritative types, schemas, or
credential provenance. Field-name substrings such as `authorization` or
`session` are not sufficient: ordinary capability flags and metadata remain
intact.

### Capability Detail

Show:

- Display name and description.
- Manifest version.
- Adapter type and health.
- Connected account or backend status.
- Operations.
- Information classes.
- Retention behavior.
- Default approval rules.
- Grants.
- Recent invocations.
- Denied invocations.
- Revocation controls.

### Operation Detail

Show:

- Operation ID.
- Input and output schema.
- Resource selector fields.
- Side-effect class.
- Egress class.
- Dry-run support.
- Rollback or idempotency behavior.
- Expected latency and retry safety.
- Information class, exact secret-bearing fields, and any explicit derived
  egress transformation.
- Audit level.
- Approval default.

### Invocation Detail

Show:

- Proposal source.
- Validated input.
- Resource resolution.
- Classification.
- Policy decision.
- Approval request and outcome, if any.
- Adapter call.
- Normalized result.
- Artifacts.
- Egress event.
- Errors and retry safety.

## Proactivity IA

Proactivity levels should be visible and understandable:

| Level | Meaning |
| --- | --- |
| 0 | Never use proactively |
| 1 | Use only when human asks |
| 2 | Use silently to personalize responses |
| 3 | Surface suggestions inside chat |
| 4 | Send proactive notifications |
| 5 | Propose external actions |
| 6 | Take approved automatic actions |

Surfaces:

- Inline suggestion or proactive notice in chat/work context.
- Memory-specific proactivity level from expanded memory cards.
- Workspace/project/task override from work surfaces.
- Tool/capability limit from capability drill-ins.
- Global, human, and agent defaults in settings.
- Rule list with condition, action level, confirmation requirement, channel,
  cooldown, enabled state.

Controls:

- Preview what a rule can do.
- Simulate a trigger.
- Disable rule.
- Require confirmation.
- Lower allowed action level.
- Inspect proactive run history.

Rule preview must show trigger source, active scopes, private-memory access
ceiling, allowed capabilities, egress destinations, cooldown, audit level, and
revocation path. Private memories follow their grants and proactivity policy;
action-triggering memories default to no proactive use unless explicitly
confirmed.

## Audit IA

Audit has two levels. Both should be reachable from the event or object being
explained before requiring the user to browse a global log.

### Run Timeline

For a single run:

- Trigger accepted.
- Run envelope created.
- Context requested and assembled.
- Memory retrieved, shown, used, omitted.
- Model or agent execution steps.
- Capability proposals and invocations.
- Policy decisions.
- Approval requests and outcomes.
- Egress review.
- Artifacts.
- Errors, suspension, retry, completion, cancellation.

### System Audit

Global stream:

- Policy denials.
- Approvals.
- External effects.
- Memory use for action.
- Secret access or attempted access.
- Revocations.
- Exports.
- Deletes and tombstones.
- Restore operations.
- Rebuild jobs.

Event detail should show actor, component, scope, causation/correlation IDs,
information class, secret exclusions or explicit egress transformations,
linked objects, payload reference, and content hash
where available.

## Information Handling Patterns

Default behavior:

- Lists show ordinary information normally and private information when the
  viewer is authorized.
- Unauthorized private objects and details are omitted, not destructively
  rewritten. Authorized private content is shown intact.
- Stronger authentication may be part of private-information authorization once
  roles exist. Secret values never reveal.
- Agent-visible omissions are vague.
- Owner/admin audit views can show exact denial reasons when authorized.
- Capability discovery and resource names are governed.
- Export previews show included classes and any explicit derived redaction
  before producing artifacts.
  Counts should be coarsened when exact numbers leak private memory existence.

Badges:

- Information: ordinary, private; secret is an excluded credential condition,
  not displayable content.
- Trust: system policy, human authored, trusted project doc, internal state,
  connector metadata, external content, tool output, model generated, imported
  unreviewed, unknown.
- Egress: internal context, human reply, local artifact, memory proposal, task
  mutation, agent handoff, external read, external write, external share.
- Policy: allowed, denied, approval required, input required, safer alternative
  required.

## Export And Restore

Exports should support:

- Human, workspace, project, conversation, task, memory, tool, run, or date
  range scopes.
- Machine-readable records where authorized.
- Human-readable audit reports.
- Full owner export.
- Policy-derived collaborator export with an explicit transformation manifest.
- Audit-only export.
- Integrity manifest with schema version, source database identity, object file
  references, content hashes, and omitted derived state.

Restore should support:

- Preview before mutation.
- Create/update/skip/conflict decisions.
- Tombstone handling.
- Missing file warnings.
- Incompatible schema warnings.
- Rebuild derived `system/` state after restore.

Exports and restore must respect information class, grants, tombstones, and scoped
visibility. They must not become a bypass around permissions.

Additional invariants:

- Every export is an egress event.
- Private exports follow the exact destination's egress and approval policy;
  secrets are always excluded.
- Export destinations should warn when bundles are unencrypted.
- Imported export bundles are untrusted input until inspected.
- Restored grants, approvals, capability connections, proactivity rules,
  credentials, and external connectors default inactive until reviewed.
- Restore must not resurrect tombstoned or deleted data without explicit
  conflict approval.

## Current Inspection Slice

The current slice should ship with narrow but honest inspection:

- Inline memory rows for saved/proposed/used/omitted memories in chat.
- Expanded memory details in chat before full memory settings.
- Memory browse/detail UI parity with GraphQL read models as a secondary
  drill-in.
- Owner/admin-only graph inspection remains future-oriented; the current backed
  inspection surface is memory browse/detail through GraphQL
  `memoryClaims`/`memoryClaim` and `memoryGraph`, filtered by authorization in
  normal views while preserving allowed content intact.
- Chat page showing transcript items and memory extraction activity.
- Memory review queue for active/candidate extracted memories.
- Access preview backed by deterministic memory retrieval, entered first
  through `Why?` or `What did Noema use?`.
- Setup and health views for local folder, config, local service, assistant
  connection, and embedded store readiness.

Do not surface full harness, approval, task, or capability controls as active
product features until the underlying durable schema and runtime paths exist.
