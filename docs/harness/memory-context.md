# Memory and Context Integration

The harness is responsible for assembling runtime context. The memory subsystem
is responsible for durable memory truth.

The boundary should stay crisp:

```text
Harness asks: "What memory may this run use, and how was it used?"
Memory answers: "Here are allowed memories, with provenance and constraints."
Harness proposes: "This run observed something that may deserve memory."
Memory decides: "Create, promote, confirm, supersede, archive, or reject."
```

This file extends the [Memory System Plan](../memory.md) for runtime use.

## Goals

The harness-memory integration should:

- Make memory useful during agent work.
- Keep memory scoped and inspectable.
- Avoid opaque global personalization.
- Prevent agents from owning human truth.
- Record which memories influenced which runs.
- Preserve provenance and authority.
- Preserve participant bindings for cross-conversation retrieval.
- Support candidate review.
- Support future multi-agent and multi-human collaboration.

## ContextPacket

A `ContextPacket` is the bounded, governed information package supplied to an
agent execution step.

It should include both content and a manifest.

Conceptual shape:

```json
{
  "context_packet_id": "ctx_01...",
  "run_id": "run_01...",
  "created_at": "2026-06-23T20:00:03Z",
  "purpose": "answer_human_question",
  "active_object_refs": [
    { "object_type": "human", "object_id": "human:kevin" },
    { "object_type": "agent", "object_id": "agent:primary" },
    { "object_type": "conversation", "object_id": "conversation_1" },
    { "object_type": "workspace", "object_id": "workspace:noema" },
    { "object_type": "project", "object_id": "project:harness" }
  ],
  "sections": [
    {
      "section_type": "trigger",
      "source_ref": "message_01...",
      "trust": "human_authored",
      "sensitivity": "normal"
    },
    {
      "section_type": "memory",
      "source_ref": "memory_01...",
      "trust": "explicit_human_statement",
      "sensitivity": "normal",
      "allowed_uses": ["answer_human_question"],
      "eligibility_reason": "same_human_participant",
      "rank_reason": "same_human_memory"
    },
    {
      "section_type": "capability_summary",
      "source_ref": "cap_google_docs",
      "trust": "internal_state",
      "sensitivity": "normal"
    }
  ],
  "agent_visible_omissions": [
    {
      "reason": "policy_restricted_context"
    }
  ],
  "audit_omissions_ref": "audit_omissions:ctx_01..."
}
```

The model-visible packet should use `agent_visible_omissions`. Full omission
details, including memory IDs and precise denial reasons, belong in an audit
record referenced by `audit_omissions_ref`.

Audit omission shape:

```json
{
  "audit_omissions_id": "audit_omissions:ctx_01...",
  "items": [
    {
      "source_ref": "memory_02...",
      "reason": "scope_denied"
    }
  ]
}
```

The model-visible prompt or tool context can be rendered from the packet, but
the packet itself should remain structured.

## Context assembly inputs

The context assembler may consider:

- Trigger payload.
- Conversation messages.
- Project documents.
- Workspace policy.
- Task state.
- Human preferences.
- Agent instructions.
- Agent skills.
- Relationship memory.
- Participant-linked memory.
- Retrieved memories.
- Entity relationships.
- Recent run results.
- Capability summaries.
- Pending approvals.
- External documents.
- Tool results.
- Artifact references.

Considering context is not the same as including it. Omitted context should be
recorded when omission affects behavior or explainability.

## Context assembly algorithm

Recommended high-level algorithm:

1. Load the run envelope.
2. Resolve active scopes.
3. Load deterministic state for linked objects.
4. Ask governance runtime which sources may be considered.
5. Request memory from the memory runtime.
6. Rank and select context within budget.
7. Apply sensitivity and trust labels.
8. Summarize or compact as needed.
9. Create the context packet and manifest.
10. Record context and memory-use events.
11. Render the agent-specific execution input from the packet.

The algorithm should preserve the difference between source selection,
retrieval, inclusion, model visibility, and use.

## Graph retrieval in V1

Entity relationships are part of V1 context assembly, but only as scoped graph
claims backed by memory.

V1 graph retrieval rules:

- FTS and structured filters are the primary candidate generators.
- Graph expansion is optional and limited to one hop.
- Expansion may start only from trusted active object links or already-allowed
  memory subjects.
- Traversal returns candidate memory IDs, not final context.
- Every traversed relationship must have a supporting memory.
- The supporting memory must pass the same scope, grant, sensitivity, status,
  purpose, participant visibility, retrieval policy, validity, and egress gates
  as ordinary memory before the edge, predicate, neighboring entity, alias, or
  path can be model-visible.
- Policy is reapplied after graph expansion and before inclusion.
- Denied graph edges are redacted from agent-visible omissions.

Two-hop traversal and graph-derived action reasoning should wait until V1.x,
after adversarial tests show that one-hop traversal does not leak edge
existence.

## Memory retrieval request

The harness should request memory using the run envelope.

Conceptual shape:

```json
{
  "run_id": "run_01...",
  "requesting_object": { "object_type": "agent", "object_id": "agent:primary" },
  "trusted": {
    "active_human_ids": ["human_kevin"],
    "active_agent_ids": ["agent_primary"],
    "active_object_refs": [
      { "object_type": "human", "object_id": "human:kevin" },
      { "object_type": "conversation", "object_id": "conversation_1" },
      { "object_type": "workspace", "object_id": "workspace:noema" },
      { "object_type": "project", "object_id": "project:harness" }
    ],
    "purpose": "answer_human_question",
    "trigger_type": "human_message",
    "explicit_memory_request": false,
    "canonical_entity_ids": ["project_noema", "concept_memory_system"],
    "active_object_links": [
      {
        "object_type": "project",
        "object_id": "project_noema",
        "relation": "active_context",
        "authorized_object_ref": {
          "object_type": "project",
          "object_id": "project:harness"
        }
      }
    ],
    "allowed_proactivity_level": 2,
    "include_candidate_memories": false,
    "sensitivity_ceiling": "normal"
  },
  "untrusted_hints": {
    "query_text": "User is asking to design harness architecture docs",
    "fuzzy_entities": ["Noema", "runtime harness", "capability registry"],
    "fuzzy_topics": ["memory", "retrieval", "privacy"]
  }
}
```

The memory runtime should decide:

- Which scopes are in bounds.
- Which memories are eligible through participant overlap.
- Which grants allow retrieval.
- Which privacy tier applies.
- Which memories are stale or superseded.
- Which candidate memories may be included.
- Which sensitivity ceiling applies.
- Which memories should be denied.
- Which memories need confirmation before use.

Participant overlap is a retrieval path, not ownership. Normal memories from
another conversation involving the same human may be retrieved when the current
purpose permits it. Current active-scope memories should rank above
participant-overlap memories.

Sensitive memories need deterministic high-relevance gates. The memory runtime
should distinguish non-authoritative retrieval hints, such as topics and fuzzy
entities, from typed retrieval policy, such as purpose rules, participant
visibility policy, trusted object links, and egress policy. It should not
require an LLM to review all sensitive memories at retrieval time.

The request must separate trusted fields from untrusted hints. Trusted fields
come from the run envelope, current-human UI actions, grants, canonical entity
resolution, and active objects that the harness loaded through governed
references. Untrusted hints come from raw user text, model extraction, external
documents, tool results, fuzzy entities, topics, and query text. Untrusted hints
may generate candidates or affect ranking after inclusion, but they must not
unlock private, sensitive, or secret memory.

Sensitive memory inclusion should require:

```text
participant, scope, or grant match
AND retrieval policy status is valid
AND purpose is allowed by closed-enum policy
AND participant visibility policy is satisfied
AND one primary trusted unlock signal exists:
  - explicit memory request from the current human or UI
  - trusted active object link match
AND no deny rule applies
```

Topics, same-human overlap, fuzzy entities, query text, broad canonical
entities, active human IDs, participant IDs, workspace/project IDs, and additive
scores cannot unlock sensitive memory. Scoring is only for ranking
already-allowed memories.

Suggested deterministic ranking signals after inclusion:

```text
+100 explicit memory request from current human or UI
+80 trusted active object link match
+60 trusted canonical entity match
+40 exact topic match
+30 active open-loop match
+20 same-human participant match

Normal include threshold: 40
Private include rule: active scope or explicit grant only
Sensitive include rule: hard gates only, then rank
Secret include rule: explicit request plus approval, then rank
```

## Memory retrieval result

The result should be structured.

Conceptual shape:

```json
{
  "request_id": "memreq_01...",
  "included": [
    {
      "memory_id": "memory_01...",
      "owner": { "object_type": "project", "object_id": "project:harness" },
      "memory_type": "decision",
      "title": "Harness uses ledger-first persistence",
      "content": "Harness runs should emit append-only events...",
      "status": "confirmed",
      "authority_level": "explicit_human_statement",
      "confidence": 1.0,
      "sensitivity": "normal",
      "allowed_uses": ["answer_human_question", "draft_project_doc"],
      "provenance_refs": ["message_01..."],
      "participant_refs": ["human_kevin", "agent_primary"],
      "eligibility_reason": "active_scope",
      "privacy_decision": "allowed",
      "retrieval_policy_status": "valid",
      "rank_reason": "direct_project_decision"
    }
  ],
  "denied_for_audit": [
    {
      "memory_id": "memory_02...",
      "reason": "participant_mismatch"
    },
    {
      "memory_id": "memory_03...",
      "reason": "privacy_denied"
    }
  ],
  "agent_visible_omissions": [
    {
      "reason": "policy_restricted_context"
    }
  ],
  "warnings": [
    {
      "kind": "candidate_excluded",
      "count": 3
    }
  ]
}
```

The harness should record the request and result without necessarily copying
all memory content into the run ledger.

The full audit record may include exact denied memory IDs and denial reasons.
The agent-visible context packet should redact private, sensitive, and secret
denials so it does not leak memory titles, topics, entity names, object links,
or existence through omission details.

## Memory-use stages

Noema should distinguish memory stages.

| Stage | Meaning |
| --- | --- |
| `retrieved` | Memory runtime returned it |
| `included_in_packet` | Context assembler selected it |
| `shown_to_agent` | Agent executor received it |
| `used_in_reply` | It influenced a human-visible response |
| `used_for_action` | It influenced a tool call, task mutation, or external effect |
| `used_for_proactivity` | It contributed to proactive trigger/action |

These distinctions matter. A memory can be retrieved but omitted. It can be
shown to an agent but not used. It can be used in a reply but not for action.

## MemoryUseRecord

The harness should create memory-use records.

Conceptual fields:

- `memory_use_id`
- `run_id`
- `memory_id`
- `stage`
- `agent_id`
- `purpose`
- `used_for_object_type`
- `used_for_object_id`
- `policy_decision_id`
- `created_at`
- `details`

This supports dashboard questions:

- Which memories did this run see?
- Which runs used this memory?
- Which actions depended on this memory?
- Which memories are frequently used?
- Did an agent use a memory outside its intended scope?

## Memory proposals

The harness should submit memory proposals after relevant observations.

Proposal types:

- Create memory.
- Update memory.
- Confirm memory.
- Supersede memory.
- Mark stale.
- Archive memory.
- Link memory to entity.
- Add provenance.
- Flag contradiction.

The proposal should include:

- Proposed home scope.
- Subject entities.
- Participant actor refs.
- Memory type.
- Proposed content.
- Structured value.
- Retrieval hints for search and ranking.
- Retrieval policy proposal, including purpose rules, participant visibility
  policy, trusted object links, egress policy, extractor identity, and policy
  status.
- Authority level.
- Extraction method.
- Confidence.
- Sensitivity.
- Provenance.
- Why the harness thinks it matters.
- Whether human confirmation is required.

The memory subsystem should evaluate the proposal according to memory policy.

## Candidate extraction

Candidate extraction may happen:

- During the run.
- After the run completes.
- In a background worker.
- During import.
- During explicit "remember this" commands.

The harness should not block ordinary replies on expensive extraction unless
the user explicitly requested memory work.

Candidate extraction should be careful with:

- Inferences.
- Sensitive information.
- Third-party personal data.
- Action-triggering preferences.
- Contradictions.
- Temporary task context.
- Jokes, speculation, or brainstorming.

Explicit "remember this" statements can bypass some uncertainty but should
still preserve scope and provenance.

## Context provenance

Every context packet should be explainable.

For each included item, Noema should know:

- Source object.
- Source type.
- Home scope.
- Participant bindings.
- Trust label.
- Sensitivity.
- Authority.
- Eligibility reason.
- Retrieval policy status.
- Retrieval reason.
- Inclusion reason.
- Policy decision.
- Whether it was model-visible.
- Whether it was summarized.
- Whether it was redacted.

This enables the dashboard to answer "why did the agent know that?"

## Scope behavior

Default ownership rules from the memory plan still apply:

```text
Human facts live with the human.
Project facts live with the project.
Workspace facts live with the workspace.
Conversation-local context lives with the conversation.
Agent skills live with the agent.
Interaction preferences live with the relationship.
Provenance links everything.
```

The harness should request memory from the smallest relevant scope first, then
expand through active scopes, participant overlap, and explicit grants.

For example:

- A project run can use project decisions.
- A conversation run can use conversation-local assumptions.
- A conversation run can use normal same-human memories from earlier
  conversations.
- An agent can use its own skill memories.
- A reply to a human can use relationship preferences.
- A workspace policy can constrain all project runs in that workspace.

But a run should not silently use private memory from another conversation or
project merely because the same human participated. Same-human overlap grants
normal memory eligibility, not unrestricted privacy bypass.

## Context compaction

When context is too large, compaction should preserve provenance.

Compaction outputs should record:

- Source refs included.
- Source refs omitted.
- Summarization method.
- Created by principal or component.
- Sensitivity.
- Trust labels.
- Whether the summary is durable or temporary.
- Whether it can be used as memory evidence.

Summaries are not automatically equivalent to source truth. Important actions
should prefer direct source references or confirmed memory.

## Stale, disputed, and contradicted memory

The harness should treat non-active memory states carefully.

- `candidate`: use only when policy allows; usually not for external action.
- `inferred`: may personalize low-risk replies; avoid high-stakes action.
- `stale`: avoid unless explaining history.
- `superseded`: do not use as current truth.
- `disputed`: do not use as fact without surfacing dispute.
- `archived`: exclude by default.
- `deleted`: never use.

The memory runtime should enforce most of this, but the harness should preserve
state labels in context and use records.

## Memory and egress

Using memory in a human-visible reply is egress to that human. Using memory in
an external email, document, notification, or API call is stronger egress.

Before memory-derived content leaves the run boundary, the harness should
check:

- Is the destination allowed?
- Is the memory allowed for this purpose?
- Does sensitivity require approval?
- Is the memory candidate, inferred, stale, or disputed?
- Does the output reveal cross-scope information?
- Does the output reveal participant-overlap memory from another conversation?
- Does the memory include third-party personal data?

This is especially important for proactive runs.

## Memory and proactivity

Memory can trigger proactive behavior, but only through policy.

Examples:

- Open-loop memory suggests a follow-up.
- Routine memory suggests a recurring reminder.
- Project risk memory suggests a notification.
- Preference memory changes response style silently.
- Task memory causes a scheduled check.

The harness should include the allowed proactivity level in memory retrieval
requests. The memory runtime should filter or annotate memories that cannot be
used proactively.

Action-triggering memory should require confirmation or explicit policy.

## Failure handling

Memory integration can fail.

Failure cases:

- Memory runtime unavailable.
- Retrieval timeout.
- Access denied.
- Candidate conflict.
- Too much context.
- Sensitive memory requires confirmation.
- Provenance missing.

The harness should decide whether to:

- Continue without memory.
- Ask the human.
- Pause the run.
- Fail the run.
- Retry later.
- Surface a warning.

For ordinary chat, continuing with a transparent limitation may be acceptable.
For external action, missing or uncertain memory may require a pause.

## Dashboard surfaces

Useful memory/context surfaces:

- Context packet viewer.
- "What did the agent see?" view.
- Memory retrieved by run.
- Memory omitted by reason.
- Memory used in reply.
- Memory used for action.
- Memory proposal review.
- Memory provenance graph.
- Memory participant graph.
- Agent access preview.
- Scope access preview.
- Participant-overlap preview.
- Context compaction trace.

These surfaces are core to making memory trustworthy.

## V1 memory/context slice

A practical first slice:

- Context packet manifest for conversation-triggered runs.
- Memory retrieval request using active humans, agents, conversation,
  workspace, project, and participant-overlap paths.
- Memory-use records for retrieved and shown memories.
- Simple memory proposals for explicit "remember this" and obvious project
  decisions.
- Dashboard or CLI inspection of context packet and memory use.
- Egress check before using memory in external tool calls.

This gives Noema useful memory without creating opaque global state.
