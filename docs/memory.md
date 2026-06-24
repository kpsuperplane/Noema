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

Ask five questions for every memory:

```
Home: where is this memory managed?
Subject: who or what is this memory about?
Participants: which humans and agents were in scope when it was formed?
Provenance: where did it come from?
Access: who or what may use it?
```

Participants are not ownership and not subject identity. A human can
participate in a memory that is about a project, an agent, a task, or another
entity. Participant bindings let Noema reuse normal conversation memories in
later conversations involving the same human without copying the memory or
promoting every useful conversation memory to human scope.

Default ownership rules:

| Memory | Default home |
| --- | --- |
| Personal facts and preferences | Human |
| Project decisions, goals, constraints, risks | Project |
| Task-local execution context and open loops | Task |
| Scheduled trigger state and cron-specific rules | Cron |
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
Task
Cron
Conversation
Agent
Relationship
Tool
```

Scopes are control boundaries for visibility, retrieval, proactivity, permissions, and auditability.

## Participants

Participants record who was in scope when a memory was formed.

Initial participant roles:

```
human_in_scope
agent_in_scope
originator
observer
```

Default participant behavior:

- Same-human participant overlap makes normal memories eligible for
  cross-conversation retrieval.
- Current active scope memories rank above memories retrieved through
  participant overlap.
- Same-agent overlap alone does not grant cross-human retrieval.
- Explicit deny grants override participant overlap.
- Participant bindings should be stored separately from subjects. For example,
  "Kevin discussed the Noema memory design" has Kevin as a participant, but
  the subject may be the Noema project or memory system.

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

Sensitivity is the privacy tier used by access and retrieval policy:

| Sensitivity | Default behavior |
| --- | --- |
| `public` | Broadly reusable where grants allow |
| `normal` | Reusable across same-human contexts by default |
| `private` | Restricted to home scope unless explicitly granted |
| `sensitive` | Considered only through deterministic high-relevance gates |
| `secret` | Excluded unless explicitly requested and approved |

Sensitive retrieval must be deterministic at read time. LLMs may help extract
hints or propose policy when a memory is created, but they should not review all
memories to decide whether sensitive content is relevant during retrieval.

## Retrieval hints and policy

Retrieval metadata has two layers with different trust levels:

| Layer | Purpose | Can unlock private/sensitive memory? |
| --- | --- | --- |
| Retrieval hints | Candidate generation, ranking, search, display | No |
| Retrieval policy | Deterministic access, privacy, and egress gates | Yes |

Retrieval hints can include fuzzy topics, keywords, embedding references,
summaries, and broad entity labels. They are useful for finding candidates, but
they are not authority-bearing. A topic such as `health`, `doctor`, or
`appointment` must never unlock sensitive memory by itself.

Retrieval policy is typed, versioned, and auditable. It includes:

- `retrieval_policy_status`: `valid`, `stale`, `invalid`, or `needs_review`.
- `retrieval_policy_version`.
- `retrieval_policy_fingerprint`.
- `retrieval_policy_extractor_principal_id`.
- `retrieval_policy_extractor_version`.
- `retrieval_policy_validated_at`.
- `participant_visibility_policy`: `any_active_human`, `all_original_humans`,
  `owner_only`, or `explicit_grant_only`.
- `external_egress_policy`: `allow`, `approval_required`, or `deny`.
- Purpose allow/deny rules using the closed purpose vocabulary.
- Trusted object links resolved by Noema, such as active tasks, projects,
  conversations, calendar events, documents, artifacts, tools, or sources. These
  links include the trusted relation and, when relevant, the scope that
  authorized the link.

Invalid, missing, stale, or unreviewed retrieval policy must fail closed for
private, sensitive, and secret memories. Hints may still help retrieve normal
memory candidates when grants and privacy permit.

A `valid` retrieval policy requires:

- Non-null policy fingerprint, extractor principal, extractor version, and
  validation timestamp.
- A policy fingerprint over memory content, sensitivity, subject IDs and roles,
  participant principal IDs and roles, purpose rules, trusted object links,
  participant visibility policy, egress policy, and provenance basis.
- A participant visibility policy intentionally selected by the validator.

Private, sensitive, and secret memories should not validate with the default
`explicit_grant_only` visibility unless that restriction is intentional. Normal
memories may be validated with `any_active_human` when same-human reuse is
appropriate.

Purpose policy uses this precedence:

```text
explicit deny grant
> blocked purpose
> sensitivity ceiling
> allowed purpose
> default deny for private, sensitive, and secret memory
```

Initial purpose vocabulary:

```text
answer_human_question
draft_internal_content
general_personalization
manage_task
manage_calendar
draft_external_content
use_tool
proactive_suggestion
external_action
debug_audit
```

## Retrieval contract

Agents should request memory with context:

```json
{
  "requesting_principal": "agent:architect",
  "trusted": {
    "active_humans": ["human:kevin"],
    "active_agents": ["agent:architect"],
    "active_scopes": [
      "human:kevin",
      "workspace:noema",
      "project:memory-system",
      "conversation:123"
    ],
    "purpose": "answer_human_question",
    "trigger_type": "human_message",
    "explicit_memory_request": false,
    "canonical_entity_ids": ["project:noema", "concept:memory_system"],
    "active_object_links": [
      {
        "object_type": "project",
        "object_id": "project:noema",
        "relation": "active_context",
        "authorized_scope_id": "project:memory-system"
      }
    ],
    "allowed_proactivity_level": 2,
    "include_candidate_memories": false,
    "sensitivity_ceiling": "normal"
  },
  "untrusted_hints": {
    "query_text": "User is asking to design memory retrieval metadata",
    "fuzzy_topics": ["memory", "retrieval", "privacy"],
    "fuzzy_entities": ["Noema", "memory system"]
  }
}
```

The memory runtime decides what is in scope, what is eligible through
participant overlap, what is denied, what is stale, and what may be used for
personalization or proactive behavior.

Only trusted request fields may satisfy private, sensitive, or secret retrieval
policy gates. Untrusted hints from user text, external documents, tool results,
LLM extraction, or fuzzy matching may generate candidates and influence ranking
after policy inclusion, but they must not unlock sensitive memory.

Default retrieval ranking:

1. Current active scope memories.
2. Active project, task, workspace, relationship, and human memories.
3. Same-human cross-conversation memories.
4. Agent skill or operational memories granted to the requesting agent.
5. Broader workspace and system memories.

For normal memories, same-human participant overlap is enough to make the
memory retrievable. For sensitive memories, participant overlap only makes the
memory eligible for consideration.

Sensitive memories may be included only when all hard gates pass:

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
scores cannot unlock sensitive memory. Canonical entities may help rank already
allowed sensitive memories, but they are not primary unlock signals on their
own.

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

Retrieval results should explain inclusion and omission with fields such as
`eligibility_reason`, `privacy_decision`, and `rank_reason`.

Agent-visible omissions should be redacted for private, sensitive, and secret
memories. A context packet may say that some memory was unavailable because of
policy, but it should not expose sensitive memory IDs, titles, topics, entity
names, or exact denial reasons unless the receiving principal can inspect that
memory. Full denial details belong in the governed audit trail.

Retrieval policy becomes stale when memory content, sensitivity, participants,
subjects, purpose rules, trusted object links, egress policy, participant
visibility policy, or provenance change. Stale policy must be regenerated or
revalidated before it can unlock private, sensitive, or secret memory.

## Dashboard requirements

The memory dashboard should support:

- Memory search and filtering
- Scope-aware memory views
- Candidate memory review
- Provenance inspection
- Access grant inspection
- Participant inspection
- Retrieval policy inspection
- Retrieval hint redaction preview
- Version history
- Usage history
- Edit, archive, delete, and restore
- Export by scope, source, type, or date range
- Preview of what a given agent can access in a given context
- Explanation of whether a memory was available through active scope,
  same-human participant overlap, or explicit grant

## Storage

Canonical memory state lives in `db/noema.sqlite`.

Object-owned documents and artifacts live under the relevant object folder.

Rebuildable retrieval state lives under `system/`.

## V1 core plan

V1 should include graph-shaped memory primitives in the core, but not a
separate graph engine.

The Noema context graph is a policy-aware view over canonical SQLite state:

```text
episodes and messages -> provenance-bearing source stream
entities -> graph nodes
relationships -> graph claim edges
memory_items -> durable claims, preferences, decisions, procedures, and notes
memory_subjects -> what a memory is about
memory_participants -> who was in scope when the memory formed
memory_provenance_edges -> why Noema believes it
retrieval policy -> whether it may be used
```

SQLite remains the source of truth. FTS is the only required retrieval index in
V1. Embeddings, external graph databases, Graphiti adapters, learned ontology as
authority, and autonomous multi-hop graph reasoning are out of V1.

### Scoped graph claims

A relationship is a claim edge, not authority by itself.

Relationship rules:

- A relationship may be `candidate` without supporting memory.
- A relationship cannot become `active` or `confirmed` unless it links to a
  supporting `memory_id`.
- The supporting memory must have provenance before the relationship can be
  treated as active graph context.
- Relationship retrieval inherits the supporting memory's scope, participants,
  sensitivity, status, authority, purpose rules, participant visibility policy,
  grants, validity window, and egress policy.
- No edge, predicate, alias, neighboring entity, or graph path should be
  model-visible unless the backing memory is includable for the run.
- Deleted, archived, superseded, disputed, or policy-stale supporting memories
  make their relationship edges unavailable as current graph context.

This gives Noema a Graphiti-like temporal graph shape while keeping memory
truth, privacy, and lifecycle in the canonical memory subsystem.

### V1 retrieval flow

V1 retrieval should stay deliberately small:

1. Build the trusted run envelope and untrusted hint set.
2. Compute the allowed search aperture from active scopes, participants, grants,
   and trusted active object links.
3. Use FTS and structured filters as the primary candidate generators.
4. Optionally perform one-hop graph expansion from trusted active object links
   or already-allowed memory subjects.
5. For every traversed relationship, require the supporting memory to pass the
   same policy gates as ordinary memory before the edge is used.
6. Return candidate memory IDs only from graph traversal.
7. Reapply memory policy before inclusion in the context packet.
8. Redact denied details from model-visible omissions.
9. Record retrieval, inclusion, shown-to-agent, used-in-reply, and
   used-for-action events.

Two-hop traversal and graph-derived action reasoning are V1.x features after
adversarial retrieval tests pass.

The current durable retrieval bridge is
`SqliteMemoryRepository::retrieve_memories`. It loads canonical SQLite memory
items, subjects, participants, purpose rules, trusted object links, access
grants, provenance, and relationship claim edges into the shared deterministic
policy engine before evaluating the request. This avoids policy drift between
in-memory adversarial tests and durable state while the V1 FTS candidate
generator is still being built. Retrieval recomputes `valid` policy
fingerprints from the canonical rows and treats mismatches as stale, so changed
content, subjects, participants, object links, purpose rules, egress policy, or
provenance cannot continue to unlock private or stronger memory until the
policy is refreshed.

### V1 extraction flow

Extraction should produce proposals, not unchecked truth:

- Entity candidates.
- Relationship candidates.
- Memory candidates.
- Subject and participant bindings.
- Provenance links.
- Retrieval hints.
- Retrieval policy proposals.

Only authenticated current-human "remember this" commands may directly create
confirmed memory. External content quoting "remember this" can only create
candidates. Inferred, sensitive, action-triggering, or contradiction-prone
memories require review or explicit policy before promotion.

### V1 inspection

V1 inspection should expose:

- Memory item detail.
- Subject and participant bindings.
- Supporting provenance.
- Relationship claim edges.
- Retrieval hints and typed retrieval policy.
- Grants and participant-overlap policy.
- Context packet and redacted omissions.
- Audit-only denied details.
- Memory-use records by run.

The first CLI inspection surface is:

```bash
noema context graph --limit 50
```

It renders the persisted context graph view from the canonical SQLite tables:
memory nodes with stored and effective retrieval-policy status, entity nodes,
subject edges, participant edges, provenance edges, trusted object-link policy
edges, purpose rules, access grants that affect the inspected memories or their
home scopes, context packet manifests, context-packet memory and omission
edges, typed memory-use records, memory events, and relationship claim edges.
During active development, schema changes update the canonical schema directly;
local development databases can be recreated rather than migrated.

This CLI graph is privileged local owner/admin debug output. Agent-visible
context packets and omissions must continue to use redacted model-facing
surfaces rather than this inspection view.

This is enough to debug why a graph claim was retrieved without making graph
state opaque or globally authoritative.

## Storage examples

Conversation-local preference, reusable later because Kevin participated:

```json
{
  "memory_id": "mem_001",
  "home_scope": "conversation:conv_memory_design",
  "type": "preference",
  "content": "Kevin prefers current-conversation memory to take precedence over older cross-conversation memory.",
  "sensitivity": "normal",
  "subjects": [
    { "entity": "human:kevin", "role": "about" },
    { "entity": "concept:memory_retrieval", "role": "about" }
  ],
  "participants": [
    { "principal": "human:kevin", "role": "human_in_scope" },
    { "principal": "agent:primary", "role": "agent_in_scope" }
  ],
  "provenance": [
    { "source": "message:msg_123", "authority": "explicit_human_statement" }
  ]
}
```

Project decision promoted to project scope:

```json
{
  "memory_id": "mem_002",
  "home_scope": "project:noema",
  "type": "decision",
  "content": "Noema memory should use Home, Subject, Participants, Provenance, and Access as its core dimensions.",
  "sensitivity": "normal",
  "subjects": [
    { "entity": "project:noema", "role": "about" },
    { "entity": "concept:memory_system", "role": "about" }
  ],
  "participants": [
    { "principal": "human:kevin", "role": "human_in_scope" },
    { "principal": "agent:primary", "role": "agent_in_scope" }
  ],
  "provenance": [
    { "source": "conversation:conv_memory_design", "authority": "project_decision" }
  ]
}
```

Sensitive memory with deterministic retrieval policy:

```json
{
  "memory_id": "mem_003",
  "home_scope": "conversation:conv_health",
  "type": "open_loop",
  "content": "Kevin needs to follow up about a doctor appointment.",
  "sensitivity": "sensitive",
  "subjects": [
    { "entity": "human:kevin", "role": "about" },
    { "entity": "task:schedule_checkup", "role": "target" }
  ],
  "participants": [
    { "principal": "human:kevin", "role": "human_in_scope" },
    { "principal": "agent:primary", "role": "agent_in_scope" }
  ],
  "retrieval_hints": {
    "topics": ["health", "doctor", "appointment"],
    "keywords": ["follow up", "appointment"]
  },
  "retrieval_policy": {
    "status": "valid",
    "version": 1,
    "fingerprint": "sha256:...",
    "extractor": "system:memory_extractor",
    "extractor_version": "2026-06-24",
    "validated_at": "2026-06-24T00:00:00Z",
    "participant_visibility_policy": "owner_only",
    "external_egress_policy": "approval_required",
    "purpose_rules": [
      { "purpose": "answer_human_question", "effect": "allow" },
      { "purpose": "manage_calendar", "effect": "allow" },
      { "purpose": "general_personalization", "effect": "deny" }
    ],
    "trusted_object_links": [
      {
        "object_type": "task",
        "object_id": "task:schedule_checkup",
        "relation": "open_loop_for",
        "authorized_scope_id": "conversation:health"
      }
    ]
  },
  "provenance": [
    { "source": "message:msg_200", "authority": "explicit_human_statement" }
  ]
}
```

Agent skill memory, not automatically Kevin memory:

```json
{
  "memory_id": "mem_004",
  "home_scope": "agent:researcher",
  "type": "skill",
  "content": "When summarizing technical documents, produce a claims/evidence table before recommendations.",
  "sensitivity": "normal",
  "subjects": [
    { "entity": "agent:researcher", "role": "about" }
  ],
  "participants": [
    { "principal": "agent:researcher", "role": "agent_in_scope" }
  ],
  "provenance": [
    { "source": "run:run_789", "authority": "repeated_observation" }
  ]
}
```

## Retrieval policy test cases

Adversarial retrieval scenarios:

- Prompt-injected topics or entities from email, browser, files, or tool results
  do not unlock sensitive memory.
- Topic-only matches never include sensitive memory.
- Same-human participant overlap alone never includes sensitive memory.
- Mentioned or forged object IDs do not count unless the harness loaded them as
  active authorized objects with the required trusted relation and authorized
  scope.
- Broad canonical entities such as the active human, workspace, project, or a
  general concept do not unlock sensitive memory.
- Ambiguous entity resolution does not count as a trusted active object link.
- Invalid, missing, stale, or unreviewed retrieval policy defaults to deny for
  private, sensitive, and secret memory.
- Blocked purpose always overrides allowed purpose.
- Multi-human private, sensitive, or secret memory is not available in a
  one-human context unless policy explicitly permits it.
- Agent-visible manifests do not reveal denied private, sensitive, or secret
  memory IDs, titles, topics, entity names, object links, or exact denial
  reasons.
- Content, sensitivity, subject, participant, trusted object link, purpose rule,
  egress policy, or provenance changes invalidate the retrieval policy
  fingerprint until regenerated or revalidated.
- External egress using private or sensitive memory requires the egress policy
  decision and any required approval, even if retrieval was allowed.
- Active or confirmed relationship edges without supporting memory are rejected.
- Relationship edges backed by deleted, archived, superseded, disputed, or
  policy-stale memory are not available as current graph context.
- Denied relationship predicates, neighboring entities, aliases, and graph paths
  are not visible to the agent.
- One-hop graph expansion returns candidate memory IDs only and cannot directly
  populate context.
- Two-hop graph traversal is disabled in V1.

## Rollout phases

### Phase 1: Canonical memory core

Schema, scopes, principals, memory items, entities, relationship claim edges,
participants, provenance, access grants, versions, audit events.

### Phase 2: Extraction and consolidation

Candidate extraction, promotion policy, contradiction detection, deduplication, confirmation queue.

### Phase 3: Retrieval integration

Scope-aware and participant-aware retrieval, deterministic sensitive-memory
gates, FTS search, provenance-aware ranking, explanation of memory use.

### Phase 4: Proactivity

Open-loop detection, routine detection, project risk detection, per-scope proactivity rules.

### Phase 5: Collaboration

Workspaces, shared projects, relationship memory, contested memories, multi-agent coordination.

### Phase 6: Advanced adapters

Vector indexes, derived graph indexes, Postgres mode, external memory provider
adapters, encryption mode.
