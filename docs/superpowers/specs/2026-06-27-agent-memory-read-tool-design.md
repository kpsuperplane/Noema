# Agent Memory Read Tool Design

## Summary

Agents should be able to read Noema memories through an ordinary tool call named
`search_memory`. The first slice is tool-only: no automatic pre-turn memory
injection, no direct table access, and no special memory-only UI surface. Noema
intercepts the tool call, builds a trusted retrieval envelope from runtime state,
runs the existing governed memory retrieval path, records the retrieval, and
returns a normal tool result.

This makes memory access visible, auditable, and small enough to ship before
adding automatic low-risk retrieval later.

## Goals

- Let agents intentionally search approved memories during a turn.
- Keep all memory reads behind `PostgresMemoryRepository::retrieve_memories`.
- Show memory access as ordinary tool call and tool result transcript items.
- Return policy-approved memories plus generic policy omissions.
- Record context packet and memory-use records for inspection.
- Preserve a clean path for later automatic retrieval to reuse the same request
  builder.

## Non-Goals

- Do not inject memories automatically before every turn in this slice.
- Do not expose private, sensitive, or secret memories by default.
- Do not let the agent decide trusted retrieval fields.
- Do not create a special memory-card rendering path for agent reads.
- Do not add migrations or compatibility layers during pre-V1 development.

## Tool Contract

The agent calls `search_memory` like any other tool.

```json
{
  "query": "trains preference",
  "purpose": "answer_human_question",
  "limit": 8
}
```

`query` is required. `purpose` defaults to `answer_human_question`. Unsupported
purpose values are rejected with a failed tool result. `limit` is optional and
capped server-side.

The successful tool result is JSON with approved memories and generic omissions.

```json
{
  "memories": [
    {
      "id": "mem_...",
      "title": "Train preference",
      "content": "Kevin is a big fan of trains.",
      "scope": "human:local",
      "sensitivity": "normal",
      "why": "participant_overlap"
    }
  ],
  "omissions": [
    {
      "reason": "policy_restricted_context"
    }
  ]
}
```

Policy denials are not tool failures. They are successful results with generic
omissions. Denied memory ids, titles, topics, subjects, content, and exact denial
details remain audit-only.

## Retrieval Envelope

The agent supplies untrusted hints. Noema supplies trusted fields from runtime
state.

Trusted request fields for the first slice:

- `requesting_principal_id`: current agent, initially `agent:primary`.
- `active_humans`: `human:local`.
- `active_agents`: current agent.
- `active_object_refs`: current human, current agent, current conversation, and
  cwd-derived project when available.
- `purpose`: validated purpose, defaulting to `answer_human_question`.
- `trigger_type`: tool call during a human-message turn.
- `explicit_memory_request`: true only when the current human explicitly asked
  the agent to remember, search, or recall memory.
- `sensitivity_ceiling`: `normal`.
- `include_candidate_memories`: false.

Untrusted hints:

- `query_text`: the tool `query`.
- `fuzzy_topics`: optional later extension.
- `fuzzy_entities`: optional later extension.

This keeps the first tool conservative while still exercising active scope,
same-human participant overlap, public hint search, explicit grants, context
packets, and audit records.

## Runtime Flow

1. The provider emits `ToolCall { name: "search_memory", payload }`.
2. Noema persists a normal `Tool call: search_memory` transcript item.
3. Noema validates arguments and builds the memory retrieval request.
4. Noema calls `PostgresMemoryRepository::retrieve_memories`.
5. Noema records a context packet and memory-use records for included memories
   and audit-only denials.
6. Noema persists a normal `Tool result: search_memory` transcript item.
7. Noema returns the tool result to the provider so the agent can continue the
   response.

The transcript and UI do not need a special memory surface. Existing tool-event
rendering should be enough for this slice.

## Error Handling

Tool argument and execution errors produce failed tool results instead of
crashing the turn.

Failure cases include:

- Missing or empty `query`.
- Invalid JSON payload.
- Unsupported or unsafe purpose.
- Repository failure.
- Retrieval timeout.

Safe error results must avoid exposing internals. Policy denials are successful
tool results with generic omissions, not errors.

## Testing

Tests should cover:

- A provider `search_memory` call becomes a persisted tool call/result pair.
- An approved normal memory is returned to the agent.
- Private, sensitive, and secret memories are withheld under the initial normal
  sensitivity ceiling.
- Generic omissions appear without leaking denied memory ids, titles, content,
  subjects, or exact denial details.
- Context packet and memory-use records are written.
- Invalid tool arguments produce a failed tool result and do not crash the turn.

## Future Extension

Automatic low-risk retrieval can later reuse the same request builder with a
narrower purpose and sensitivity ceiling. That later slice should decide when
Noema may prefetch memories without an explicit agent tool call and how the UI
explains that hidden retrieval to the human.
