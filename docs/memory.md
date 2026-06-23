# Memory System Plan

## Purpose

Noema’s memory system should let multiple agents coordinate through durable, scoped, inspectable context without turning memory into opaque global state.

Memory is a first-party product layer. Third-party memory tools can be adapters, but they should not define Noema’s memory model, lifecycle, permissions, or dashboard UX.

## Core principles

- Memory is automatic by default, but visible and correctable by humans.
- Memories live with the smallest durable scope that legitimately owns them.
- Agents usually access memory through grants; they should not own human truth.
- Every durable memory has provenance.
- Proactivity is governed by explicit policy.
- Derived search, vector, and graph state is rebuildable.

## Ownership model

Ask four questions for every memory:

```
Home: where is this memory managed?
Subject: who or what is this memory about?
Provenance: where did it come from?
Access: who or what may use it?
```

Default ownership rules:

| Memory | Default home |
| --- | --- |
| Personal facts and preferences | Human |
| Project decisions, goals, constraints, risks | Project |
| Team norms, shared policies, member roles | Workspace |
| Thread-local context and working assumptions | Conversation |
| Skills, workflows, tool-use lessons | Agent |
| Interaction preferences and delegation rules | Relationship |

## Scope types

```
System
Human
Workspace
Project
Conversation
Agent
Relationship
Tool
```

Scopes are control boundaries for visibility, retrieval, proactivity, permissions, and auditability.

## Memory types

Initial memory types:

```
fact
preference
person
organization
project
routine
goal
open_loop
procedure
constraint
trigger
decision
skill
policy
note
```

## Lifecycle

```
Observation
  -> Candidate
  -> Active
  -> Confirmed
  -> Superseded / Archived / Deleted
```

Default promotion policy:

| Input | Default handling |
| --- | --- |
| Explicit “remember this” | Confirmed |
| Low-risk durable fact or preference | Active |
| Inference or behavioral pattern | Candidate or inferred |
| Sensitive information | Confirmation required |
| Action-triggering memory | Permission required |
| Contradiction | Resolution or supersession required |

## Provenance

Every durable memory should answer:

```
Why does Noema know this?
Who or what created it?
Was it stated, imported, inferred, summarized, or edited?
What source supports it?
When was it learned?
When was it true?
Who can see it?
Where has it been used?
```

Authority levels:

```
human_correction
explicit_human_statement
workspace_policy
project_decision
document_source
repeated_observation
agent_inference
weak_inference
system_rule
```

Conflict priority:

```
human correction
> explicit human statement
> workspace/project policy
> source document
> repeated observation
> agent inference
```

## Access model

Memory access should use grants rather than copies.

A grant can allow or deny:

```
read
write
propose
confirm
delete
use_for_retrieval
use_for_proactivity
use_for_external_action
```

## Retrieval contract

Agents should request memory with context:

```json
{
  "requesting_principal": "agent:architect",
  "active_human": "human:kevin",
  "active_workspace": "workspace:noema",
  "active_project": "project:memory-system",
  "active_conversation": "conversation:123",
  "purpose": "answer_human_question",
  "allowed_proactivity_level": 2,
  "include_candidate_memories": false,
  "sensitivity_ceiling": "normal"
}
```

The memory runtime decides what is in scope, what is denied, what is stale, and what may be used for personalization or proactive behavior.

## Dashboard requirements

The memory dashboard should support:

- Memory search and filtering
- Scope-aware memory views
- Candidate memory review
- Provenance inspection
- Access grant inspection
- Version history
- Usage history
- Edit, archive, delete, and restore
- Export by scope, source, type, or date range
- Preview of what a given agent can access in a given context

## Storage

Canonical memory state lives in `db/noema.sqlite`.

Object-owned documents and artifacts live under the relevant object folder.

Rebuildable retrieval state lives under `system/`.

## Rollout phases

### Phase 1: Canonical memory core

Schema, scopes, principals, memory items, provenance, access grants, versions, audit events.

### Phase 2: Extraction and consolidation

Candidate extraction, promotion policy, contradiction detection, deduplication, confirmation queue.

### Phase 3: Retrieval integration

Scope-aware retrieval, FTS search, provenance-aware ranking, explanation of memory use.

### Phase 4: Proactivity

Open-loop detection, routine detection, project risk detection, per-scope proactivity rules.

### Phase 5: Collaboration

Workspaces, shared projects, relationship memory, contested memories, multi-agent coordination.

### Phase 6: Advanced adapters

Vector indexes, graph indexes, Postgres mode, external memory provider adapters, encryption mode.