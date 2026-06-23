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
  "active_scopes": [
    "human_kevin",
    "agent_primary",
    "conversation_01...",
    "workspace_noema",
    "project_harness"
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
      "allowed_uses": ["answer_human_question"]
    },
    {
      "section_type": "capability_summary",
      "source_ref": "cap_google_docs",
      "trust": "internal_state",
      "sensitivity": "normal"
    }
  ],
  "omissions": [
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

## Memory retrieval request

The harness should request memory using the run envelope.

Conceptual shape:

```json
{
  "run_id": "run_01...",
  "requesting_principal_id": "agent_primary",
  "active_human_id": "human_kevin",
  "active_agent_id": "agent_primary",
  "active_scopes": [
    "human_kevin",
    "conversation_01...",
    "workspace_noema",
    "project_harness"
  ],
  "purpose": "answer_human_question",
  "trigger_type": "human_message",
  "allowed_proactivity_level": 2,
  "include_candidate_memories": false,
  "sensitivity_ceiling": "normal",
  "query": {
    "text": "User is asking to design harness architecture docs",
    "entities": ["Noema", "runtime harness", "capability registry"],
    "object_links": {
      "project_id": "project_noema"
    }
  }
}
```

The memory runtime should decide:

- Which scopes are in bounds.
- Which grants allow retrieval.
- Which memories are stale or superseded.
- Which candidate memories may be included.
- Which sensitivity ceiling applies.
- Which memories should be denied.
- Which memories need confirmation before use.

## Memory retrieval result

The result should be structured.

Conceptual shape:

```json
{
  "request_id": "memreq_01...",
  "included": [
    {
      "memory_id": "memory_01...",
      "home_scope_id": "project_harness",
      "memory_type": "decision",
      "title": "Harness uses ledger-first persistence",
      "content": "Harness runs should emit append-only events...",
      "status": "confirmed",
      "authority_level": "explicit_human_statement",
      "confidence": 1.0,
      "sensitivity": "normal",
      "allowed_uses": ["answer_human_question", "draft_project_doc"],
      "provenance_refs": ["message_01..."],
      "rank_reason": "direct_project_decision"
    }
  ],
  "denied": [
    {
      "memory_id": "memory_02...",
      "reason": "outside_active_scope"
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
- `scope_id`
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
- Memory type.
- Proposed content.
- Structured value.
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
- Trust label.
- Sensitivity.
- Authority.
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
expand only through explicit active scopes and grants.

For example:

- A project run can use project decisions.
- A conversation run can use conversation-local assumptions.
- An agent can use its own skill memories.
- A reply to a human can use relationship preferences.
- A workspace policy can constrain all project runs in that workspace.

But a run should not silently use private memory from another project merely
because the same human owns both projects.

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
- Agent access preview.
- Scope access preview.
- Context compaction trace.

These surfaces are core to making memory trustworthy.

## V1 memory/context slice

A practical first slice:

- Context packet manifest for conversation-triggered runs.
- Memory retrieval request using active human, agent, conversation, workspace,
  and project scopes.
- Memory-use records for retrieved and shown memories.
- Simple memory proposals for explicit "remember this" and obvious project
  decisions.
- Dashboard or CLI inspection of context packet and memory use.
- Egress check before using memory in external tool calls.

This gives Noema useful memory without creating opaque global state.

