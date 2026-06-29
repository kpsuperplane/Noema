# Search Memory Scope And Marker Design

## Summary

Noema's current `search_memory` tool can return no memories even when relevant
memories exist because the agent may pass a sentence-shaped query and the
retrieval store currently treats the whole query as a literal text match. The
chat UI can also show a vague `Memory updated` marker without naming the memory
that was saved.

The fix should keep memory reads explicit and tool-visible, but make the tool
contract more structured. The agent should query concrete Noema scope IDs that
Noema exposes in the turn prompt, and the backend should validate those IDs
against trusted runtime context before retrieval.

## Goals

- Teach the agent to form reliable `search_memory` calls for broad profile
  questions and topical memory questions.
- Add reusable concrete scope constraints to the tool contract without adding
  English-intent branching in the backend.
- Preserve deterministic policy gates for status, predicate use mode,
  sensitivity, active context, and omissions.
- Make `query` narrow within `scope_ids` using AND semantics.
- Allow empty-query retrieval only when scoped by concrete IDs.
- Render memory markers with a safe saved fact preview when one memory is saved
  or updated.
- Preserve the current local, visible, tool-only memory-read slice.

## Non-Goals

- Do not inject memories automatically before turns.
- Do not branch on English phrases such as "what memories do you have of me?"
  in backend retrieval code.
- Do not let the agent invent or authorize scope IDs.
- Do not broaden search silently when the agent supplies an invalid scope.
- Do not build the full memory-management UI in this slice.
- Do not add compatibility migrations for pre-V1 local data.

## Tool Contract

Extend `search_memory` with optional `scope_ids`:

```json
{
  "scope_ids": ["human:local"],
  "query": "",
  "purpose": "answer_human_question",
  "limit": 8
}
```

`scope_ids` are concrete Noema entity or object IDs that constrain retrieval.
They are not aliases and not semantic labels. The agent may only use IDs that
Noema exposed in the current turn's active retrieval metadata or IDs returned by
prior Noema tools.

`query` narrows within the selected scopes. The semantics are:

```text
allowed by policy
AND connected to a requested scope_id, when scope_ids are supplied
AND matched by query, when query is non-empty
```

An empty `query` means "return the highest-ranked allowed memories in this
scope." Empty `query` is valid only when `scope_ids` is non-empty.

If `scope_ids` is omitted, `query` must be non-empty. In that case Noema should
use the existing trusted active retrieval envelope and policy gates, rather than
performing an unscoped empty browse.

Examples:

```json
{
  "scope_ids": ["human:local"],
  "query": "",
  "purpose": "answer_human_question",
  "limit": 8
}
```

This asks for allowed memories about the current local human.

```json
{
  "scope_ids": ["human:local"],
  "query": "aviation",
  "purpose": "answer_human_question",
  "limit": 8
}
```

This asks for allowed local-human memories narrowed to aviation.

## Agent Guidance

Noema should include active retrieval metadata in the structured turn prompt:

```text
Active retrieval IDs:
- human:local
- conversation:<current conversation id>
- project:<cwd-derived project id, when available>
```

The prompt should teach the agent:

- Use `scope_ids` to choose the concrete memory owner or context.
- Use `query` only to narrow within those IDs.
- For broad questions about what Noema remembers about the user, call
  `search_memory` with `scope_ids: ["human:local"]` and `query: ""`.
- For topical questions about the user, keep `scope_ids: ["human:local"]` and
  use a concise topic query such as `"aviation"` or `"planes"`.
- Never invent IDs. Use only IDs listed in active retrieval metadata or returned
  by prior Noema tools.
- Do not tell the user Noema has no memories unless the scoped tool result is
  empty for the scope actually being discussed.

This keeps search intelligence in the agent while keeping authorization and
runtime truth in Noema.

## Backend Validation

`search_memory` should parse `query`, `scope_ids`, `purpose`, and `limit`.

Validation rules:

- `query` must be present, but may be an empty string when `scope_ids` is
  non-empty.
- Empty `query` with empty or missing `scope_ids` is rejected.
- Non-empty `query` without `scope_ids` remains valid and uses the trusted
  active retrieval envelope.
- Every `scope_id` must be present in the trusted active retrieval envelope or
  be otherwise explicitly allowed by future grants.
- Unknown or out-of-context `scope_ids` produce a safe failed tool result.
- Tool argument failures do not crash the turn.
- Policy denials remain successful results with redacted omissions.

Noema should not silently fall back to broader retrieval if validation fails.

## Retrieval Behavior

The store retrieval API should accept the trusted retrieval request, query text,
requested scope IDs, and limit.

Retrieval order:

1. Load active and confirmed claims.
2. Apply deterministic policy gates for predicate use mode, sensitivity ceiling,
   and active context.
3. If `scope_ids` is supplied, require each included claim to be connected to at
   least one requested `scope_id`, using subject or object entity IDs in the
   first slice.
4. If `query` is non-empty, tokenize query terms and score matches across claim
   fact text, predicate label, entity display names, and retrieval hints.
5. Rank scoped empty-query results by deterministic usefulness, preferring
   higher confidence, newer updates, stronger status, and stable claim ID
   ordering.
6. Return included claim facts and generic omission counts.

The first implementation can keep matching simple and deterministic. It should
not add vector search or LLM-based retrieval decisions.

## Memory Marker UX

Memory persistence activities should include enough claim outcome data for the
chat transcript to render a truthful compact marker without an extra query:

```json
{
  "claim_outcomes": [
    {
      "claim_id": "claim:...",
      "outcome": "created",
      "fact_preview": "User likes planes",
      "sensitivity": "normal"
    }
  ],
  "created_claim_count": 1,
  "reinforced_claim_count": 0,
  "failed_proposal_count": 0
}
```

For a single safe preview, the compact marker should say:

```text
Memory saved: User likes planes
Memory updated: Kevin likes commercial aviation
```

For multiple claims, render a count:

```text
Memory saved: 2 memories
```

For partial failures:

```text
Memory saved: 1 memory; 1 failed
```

For full failures:

```text
Memory update failed
```

The expanded attachment should list claim previews, claim IDs, created versus
reinforced outcomes, failed proposal count, and source. If a preview is not safe
or not available, the UI should fall back to counts and claim IDs.

`fact_preview` should be derived from the persisted claim after the write
summary is available, not directly from the model's proposal text. This keeps
the marker aligned with the canonical graph claim.

## Data Flow

1. Runtime builds trusted active retrieval metadata for the turn.
2. Runtime includes those IDs and tool guidance in the provider system prompt.
3. Agent emits `search_memory` with `scope_ids` and `query`.
4. Noema validates the supplied `scope_ids` against trusted active metadata.
5. Store retrieves claims using policy, scope, and optional query narrowing.
6. Tool result returns included claim facts, omissions, and context packet ID.
7. Provider continuation answers from the returned memories.
8. Memory write activities include claim outcome previews.
9. React transcript renders concrete memory marker labels from activity metadata.

## Error Handling

- Invalid JSON or unknown fields produce a failed tool result with a safe error.
- Empty `query` without `scope_ids` produces a failed tool result.
- Unknown or unauthorized `scope_ids` produce a failed tool result.
- Policy-restricted memories produce successful results with generic omissions.
- No matches produce a successful result with `memories: []`; the continuation
  should describe that as no matching memories in the requested scope/query.
- Memory preview rendering should fall back to counts if metadata is missing,
  malformed, redacted, or contains multiple previews.

## Testing

Backend tests should cover:

- Broad profile retrieval with `scope_ids: ["human:local"]` and `query: ""`
  returns active normal human claims.
- `scope_ids` and `query` are ANDed: a topical query returns matching scoped
  memories but not unrelated memories in the same scope.
- Empty `query` without `scope_ids` fails safely.
- Out-of-context `scope_ids` fail safely.
- Tool results include claim facts and omit the legacy `unavailable` field.
- Policy denials remain redacted omissions, not failed tool calls.
- Prompt text exposes active retrieval IDs and instructs the agent how to use
  empty scoped queries.

Frontend tests should cover:

- One created claim with `fact_preview` renders `Memory saved: <fact preview>`.
- One reinforced claim with `fact_preview` renders
  `Memory updated: <fact preview>`.
- Multiple claim outcomes render count-based labels.
- Partial and full failures render failure-aware labels.
- Expanded marker details show previews and claim IDs when available.

## Rollout Notes

This is a pre-V1 schema and protocol change. No migration or backwards
compatibility layer is required unless explicitly requested. Existing transcript
activities without `claim_outcomes` should continue to render with the current
count-based fallback.
