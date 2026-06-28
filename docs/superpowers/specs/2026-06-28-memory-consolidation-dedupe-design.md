# Memory Consolidation And Deduplication Design

## Purpose

Noema currently appends every explicit or extracted memory proposal as a fresh
`memory_items` row. If the same human repeats the same durable preference in the
same conversation, the system can create identical memories with the same
participants, provenance neighborhood, and agent. This makes memory feel noisy
and less trustworthy.

The memory subsystem should consolidate proposals before creating rows. Exact
duplicates should be impossible to persist accidentally, and semantic repeats
should reinforce or reuse existing memory instead of creating parallel truth.

## Goals

- Prevent duplicate memories for repeated statements such as "I like ice cream."
- Handle near-semantic repeats such as "Ice cream is one of my favorite
  desserts."
- Preserve provenance for repeated evidence without copying memory truth.
- Surface conflicts for review instead of silently overwriting existing memory.
- Keep deterministic exact dedupe database-backed and testable.
- Keep semantic comparison bounded, auditable, and conservative.
- Use one consolidation path for explicit remembers, provider memory proposals,
  and ordinary-chat extraction.

## Non-Goals

- Build the full memory review UI.
- Add long-term schema migrations or backwards compatibility layers.
- Use semantic comparison for broad private, sensitive, or secret memory scans.
- Merge old duplicate rows already present in a user's database.
- Automatically rewrite or supersede established memories without review.

## Recommended Approach

Use a two-layer consolidation path:

1. Deterministic exact dedupe in the repository.
2. Bounded semantic consolidation in the daemon memory pipeline.

The repository owns canonical identity and idempotency. The daemon pipeline owns
context-aware semantic comparison because it already has the turn context,
proposal source, and provider/model access.

## Architecture

Add a `memory_consolidation` module near the daemon memory pipeline. It accepts a
`NewMemoryCandidate`, gathers plausible existing memories, and returns a
consolidation decision.

Decision vocabulary:

| Decision | Meaning | Persistence behavior |
| --- | --- | --- |
| `Create` | The proposal is distinct. | Insert a new memory row. |
| `Reuse` | Existing memory already fully covers the proposal. | Do not create a memory row; optionally record an event. |
| `Reinforce` | The proposal restates existing truth with useful new evidence. | Add supporting provenance and an event to the existing memory. |
| `Conflict` | The proposal is related but incompatible. | Create a reviewable candidate or disputed item linked to the existing memory. |

The initial implementation can represent `Reuse` and `Reinforce` as Rust enum
variants and transcript outcomes before building a full review UI. `Conflict`
should create reviewable state without promoting it to active truth.

## Canonical Fingerprint

Add a canonical dedupe fingerprint for exact duplicates. The fingerprint should
be derived from stable fields:

- owner object type and id,
- memory type,
- normalized content,
- sensitivity,
- subject ids and roles,
- participant actor ids and roles.

Normalization should be deterministic and conservative:

- trim surrounding whitespace,
- collapse internal whitespace,
- lowercase ASCII text,
- remove only low-risk trailing sentence punctuation.

The fingerprint should not include volatile fields such as source item id,
turn id, confidence, observed timestamp, retrieval hints, title, or metadata.
Those fields may differ across repeated evidence for the same memory.

Store the fingerprint on `memory_items` and enforce uniqueness for non-deleted
rows. If schema support is staged separately, the repository should still query
by fingerprint inside the append transaction before allocating a new id.

## Match Search

Before semantic comparison, gather a small bounded match set:

- same owner scope and memory type,
- same-human participant memories for normal/public memory,
- same conversation/source neighborhood for turn-local repeats,
- overlapping subject ids,
- FTS overlap against title, content, and retrieval hints.

The match set should have a strict limit, such as 12 candidates. Exact
fingerprint matches should bypass semantic comparison.

Sensitive and secret proposals should use exact dedupe only for the first slice.
Broader semantic comparison over those memories should wait for explicit policy
and review surfaces.

## Semantic Comparator

The semantic comparator receives the new proposal and the bounded match set. It
must return strict JSON with:

- `decision`: `create`, `reuse`, `reinforce`, or `conflict`,
- `existing_memory_id`: present for reuse, reinforce, and conflict,
- `confidence`: bounded 0.0 to 1.0,
- `rationale`: short audit-facing reason.

The comparator should be conservative:

- choose `create` when there is not a clear semantic match,
- choose `reinforce` for paraphrases that preserve the same meaning,
- choose `conflict` when both statements cannot be true at the same time,
- never choose `reuse` or `reinforce` for different subjects or owners.

If semantic comparison fails or returns invalid JSON, Noema should fall back to
`Create` only when no exact duplicate exists. The created row metadata should
include `semantic_consolidation_failed: true` and a safe error category.

## Data Flow

1. Validate extractor/provider output as today.
2. Convert each proposal into `NewMemoryCandidate`.
3. Compute the canonical dedupe fingerprint.
4. Check for an existing active, confirmed, or candidate memory with the same
   fingerprint.
5. If an exact match exists, return `Reuse` or `Reinforce`.
6. Load plausible matches for semantic comparison.
7. Run semantic comparison when policy allows.
8. Persist the decision:
   - `Create`: call the repository insert path.
   - `Reuse`: avoid inserting a row and record outcome metadata.
   - `Reinforce`: add a `supports` or `derived_from` provenance edge and a
     memory event to the existing memory.
   - `Conflict`: create a candidate/disputed review item and add a
     `contradicted_by` provenance relation to the existing memory.
9. Return structured outcomes to runtime callers.
10. Render transcript payloads with created, reused, reinforced, and conflict
    counts instead of only `created_memory_ids`.

## Runtime Integration

Explicit remembers and extracted/provider proposals should share the same
consolidation path. This keeps `/remember`, natural language "remember this",
provider-structured memory proposals, and ordinary-chat background extraction
consistent.

Existing transcript cards currently report created memory ids. They should move
toward a neutral `memory_outcomes` payload:

```json
[
  {
    "outcome": "reinforced",
    "memory_id": "mem_...",
    "proposal_content": "Kevin likes ice cream.",
    "reason": "semantic_repeat"
  }
]
```

The UI can initially display this with existing memory activity/card surfaces.
Detailed review and merge controls can arrive later.

## Error Handling

- Exact duplicate handling must be deterministic and transactional.
- Semantic comparison failure must not block the user-visible chat response.
- If no exact duplicate exists and semantic comparison fails, create the memory
  with failure metadata rather than losing the observation.
- If an exact duplicate exists, do not create a second row even if event logging
  fails.
- Conflict decisions must not update or supersede existing memory automatically.
- Sensitive/secret proposals skip broad semantic comparison until policy allows
  it.

## Testing

Repository tests:

- Appending the same canonical candidate twice yields one memory row.
- The second exact append returns the existing memory id or a reuse outcome.
- Fingerprints ignore source item, turn id, title, confidence, and metadata.

Runtime tests:

- Repeating "I like ice cream" in the same conversation does not create a
  second memory.
- "I like ice cream" followed by "Ice cream is one of my favorite desserts"
  reuses or reinforces the existing memory.
- "I like ice cream" followed by "I hate ice cream" creates conflict/review
  state rather than silently overwriting the existing memory.
- Provider-structured proposals and ordinary-chat extraction both use the same
  consolidation outcome path.
- Transcript payloads report `created`, `reused`, `reinforced`, and `conflict`
  outcomes accurately.

## Rollout

1. Add deterministic fingerprint computation and repository-level exact dedupe.
2. Route all memory writes through consolidation outcome types.
3. Add bounded match search.
4. Add semantic comparator for normal/public memories.
5. Add conflict outcome persistence.
6. Update transcript payloads and tests.

This order makes the first milestone useful even before semantic comparison:
exact repeats stop duplicating immediately, while the semantic layer can be
added without changing the public write path again.
