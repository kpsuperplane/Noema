# Memory Consolidation Pipeline Design

## Purpose

Noema already stores durable memory as graph claims with predicate records,
evidence relations, and exact dedupe fingerprints. That prevents exact duplicate
claims, but it does not yet solve the harder problem: ordinary human language
does not arrive already shaped as Noema's graph ontology.

The current live memory database shows the problem. A statement such as "the
user loves planes" can fall through to the generic `has_note` predicate instead
of becoming a canonical `likes` claim. That creates parallel, noisy memories
even though the memory store can already reinforce exact claim fingerprints.

Future memory writes need a first-class consolidation pipeline. LLMs should help
translate messy, multilingual, and domain-specific language into graph-shaped
claims and predicate proposals, while Noema remains the authority that validates
ontology, policy, evidence, and persistence.

## Goals

- Route explicit remembers, ordinary-chat extraction, imported sources, and
  future tool/task memory outputs through one future-write consolidation path.
- Treat the predicate catalog as an ontology, not a phrasebook.
- Let the LLM map natural language to promoted predicates when one clearly fits.
- Let the LLM propose new predicates when the catalog is missing a relationship
  type.
- Keep unpromoted predicates and uncertain claims review-gated.
- Consolidate repeated or equivalent future claims before creating new truth.
- Preserve every supporting source as evidence rather than copying truth.
- Surface disputes, supersession, related claims, and review needs without
  silently overwriting memory.
- Keep matching bounded, auditable, and policy-controlled.
- Attempt all rollout slices in one cohesive implementation pass, while keeping
  slice boundaries for testing, validation, and commits.

## Non-Goals

- Do not repair, merge, or rewrite existing local database claims in this scope.
- Do not make the LLM a direct writer of canonical memory truth.
- Do not scan all private, sensitive, or secret memories for semantic matches.
- Do not promote unknown predicates automatically into ordinary retrieval.
- Do not build the full memory review UI before the write pipeline exists.
- Do not add compatibility migrations for pre-V1 local data unless explicitly
  requested.

## Recommended Approach

Build a future-write memory consolidation pipeline with six stages:

1. claim canonicalization,
2. promoted predicate resolution,
3. predicate proposal creation when needed,
4. bounded existing-claim matching,
5. conservative consolidation decision,
6. validated claim persistence.

The LLM proposes graph shape and ontology extensions. Noema validates the output
against strict Rust types, promoted predicate policy, source evidence, sensitivity
rules, and retrieval eligibility before anything becomes active memory.

## Architecture

### Memory Write Proposal

All memory-producing inputs should first become a shared internal
`MemoryWriteProposal` shape. The shape should preserve source context before
canonical graph decisions are made:

- source kind: explicit chat command, ordinary chat, document import, tool
  output, task output, or future system source,
- source item or object id,
- source actor,
- source excerpt or source object span,
- owner and active context hints,
- raw proposed memory text,
- proposed memory type,
- proposed sensitivity,
- risk flags,
- retrieval hints,
- extractor metadata.

This replaces the current split where explicit remembers and provider proposals
are converted directly into `NewClaimCandidate` by deterministic string-prefix
parsing.

### Claim Canonicalizer

The canonicalizer converts a validated `MemoryWriteProposal` into one or more
graph-shaped candidate claims. It receives the promoted predicate catalog and a
strict output schema.

For each candidate, the canonicalizer must return:

- subject entity,
- object entity,
- predicate resolution,
- canonical fact text,
- evidence basis,
- sensitivity,
- initial lifecycle status,
- retrieval hints,
- confidence,
- rationale.

Predicate resolution is one of:

- `promoted_predicate`: an existing predicate id from the catalog,
- `predicate_proposal`: a structured proposal for a missing relationship type,
- `fallback_note`: a generic note only when the content is genuinely
  unstructured.

The LLM may map multilingual and paraphrased input to promoted predicates. For
example, "I adore trains", "trains are my thing", and equivalent statements in
other languages may all resolve to `likes` when evidence directly supports that
relationship.

### Predicate Resolver

The predicate resolver validates the canonicalizer's predicate choice.

For promoted predicates, Noema checks:

- predicate exists,
- subject entity type is allowed,
- object entity type is allowed,
- requested use modes are allowed,
- default sensitivity and caller sensitivity are compatible,
- predicate review policy permits the proposed lifecycle status,
- conflict and cardinality policy are known.

If a promoted predicate does not validate, the candidate is rejected or turned
into review state rather than being silently stored as `has_note`.

### Predicate Proposal Path

If no promoted predicate fits, the canonicalizer may return a predicate proposal.
Noema should persist it in `predicate_proposals` with enough detail to review or
merge it later:

- label,
- description,
- allowed subject and object types,
- allowed use modes,
- default sensitivity,
- conflict and cardinality policy,
- review policy,
- inverse behavior,
- proactivity default,
- merge hints,
- synonym and multilingual hints,
- extraction examples,
- source item or object id,
- rationale,
- proposed claim preview.

Claims that depend on unpromoted predicates should remain candidate-only and
excluded from ordinary retrieval until the predicate is approved or merged into a
promoted predicate.

### Bounded Candidate Matcher

Before creating a new promoted-predicate claim, Noema should find a small set of
plausible existing claims:

- same owner or active human subject,
- same promoted predicate or policy-compatible predicate,
- overlapping subject/object entities,
- overlapping retrieval hints or fact tokens,
- statuses eligible for consolidation, such as candidate, active, or confirmed,
- strict result limit, initially around 10 to 20 claims.

The matcher must not perform broad private, sensitive, or secret memory scans.
Sensitive classes should use exact identity and explicit scope constraints until
the policy layer has stronger review surfaces.

### Consolidation Decider

The decider receives the new canonical candidate plus the bounded match set. It
applies deterministic predicate policy first, then may use an LLM comparison when
policy permits.

The decision vocabulary is:

| Decision | Meaning | Persistence behavior |
| --- | --- | --- |
| `create` | The candidate is distinct. | Insert a new claim. |
| `reinforce` | Existing claim already expresses the same truth. | Add evidence and strengthen status/confidence when allowed. |
| `supersede` | New claim replaces older truth under predicate policy. | Link claims and retire the superseded claim when allowed. |
| `dispute` | New evidence conflicts with existing truth. | Mark/link disputed state without overwriting truth. |
| `relate` | Claims are meaningfully related but should stay separate. | Keep both claims and add a relation edge or reviewable relation. |
| `needs_review` | Noema cannot safely decide. | Store candidate/review state, not active truth. |

This vocabulary is intentionally broader than simple dedupe. "Kevin likes
planes", "Kevin likes commercial aviation", and "Kevin thinks the Su-57 is
pretty" are related, but they are not automatically the same claim.

### Claim Writer

The writer persists the validated decision:

- `create`: call the existing claim insert path with a canonical fingerprint,
- `reinforce`: add support evidence to the existing claim and update status,
  sensitivity, and confidence only according to policy,
- `supersede`: create or keep the replacement claim, mark the old claim
  superseded when allowed, and add a `supersedes` relation,
- `dispute`: preserve the incoming evidence and add a contradiction relation or
  disputed candidate,
- `relate`: preserve both claims and add a relation edge or reviewable relation,
- `needs_review`: store enough candidate data for review without promoting it
  into ordinary retrieval.

Evidence verification is mandatory. If the source excerpt or source object span
cannot be verified, no active claim is written.

## Data Flow

1. A source event arrives from explicit memory, ordinary-chat extraction,
   imported material, or future tool/task output.
2. Noema validates source evidence, actor, owner, active context, sensitivity,
   and whether the source may write memory.
3. The source becomes a `MemoryWriteProposal`.
4. The claim canonicalizer runs with promoted predicate catalog context and a
   strict output schema.
5. Noema validates the canonical output.
6. Known predicates continue through promoted predicate validation.
7. Unknown predicates create `predicate_proposals`; dependent claims remain
   candidate/review-gated.
8. For validated promoted predicates, Noema computes deterministic exact
   fingerprints.
9. Exact fingerprint matches reinforce immediately.
10. If no exact match exists, Noema loads a bounded match set.
11. Deterministic predicate consolidation rules run first.
12. If still uncertain and policy permits, an LLM compares the candidate against
    the bounded set and returns a strict consolidation decision.
13. Noema validates the decision against predicate policy and source evidence.
14. The claim writer persists the decision.
15. Transcript and inspection surfaces report the outcome: created,
    reinforced, superseded, disputed, related, or needs review.

## Error Handling

- Invalid canonicalizer JSON fails closed.
- Unsupported enum values fail validation.
- Unknown promoted predicate ids do not fall back to `has_note`.
- Predicate proposals are review-gated and do not unlock ordinary retrieval.
- Bad evidence excerpts reject the write.
- Match search failure may still allow `create` only for low-risk promoted
  predicates with no plausible conflict.
- Consolidation comparison failure defaults to `create` only for low-risk,
  promoted, non-conflict-prone predicates.
- Sensitive, secret, and conflict-prone claims default to `needs_review` when
  comparison is unavailable or uncertain.
- Supersession and dispute decisions require predicate policy support.
- Event logging failures must not create duplicate exact claims.

## Policy And Safety

The LLM is advisory. Noema owns authority.

The LLM may:

- translate language into candidate graph shape,
- choose among promoted predicates,
- propose new predicates,
- suggest consolidation decisions over bounded candidates,
- provide short rationales.

Noema must:

- validate source evidence,
- validate subject and object entity types,
- validate predicate policy,
- validate sensitivity and lifecycle status,
- enforce retrieval eligibility,
- cap candidate matching,
- preserve provenance,
- decide what is active, candidate, disputed, superseded, archived, or deleted.

This split keeps Noema flexible across languages and domains without letting the
ontology become ungoverned model state.

## Testing

Repository and pure-module tests:

- Predicate proposals validate required fields and enum vocabularies.
- Promoted predicate validation rejects wrong subject/object types.
- Exact fingerprints still reinforce identical canonical claims.
- Bounded match search respects subject, predicate, status, sensitivity, and
  limit constraints.
- `supersede`, `dispute`, `relate`, and `needs_review` decisions persist the
  expected graph state.

Runtime tests:

- "The user loves planes" maps to `likes`, not `has_note`.
- Paraphrased and multilingual preference statements can map to promoted
  predicates when evidence supports them.
- Unknown relationship types create predicate proposals and candidate-only
  claims.
- Repeated equivalent facts reinforce one claim and add evidence.
- Related but non-identical facts stay separate and are linked or marked for
  review.
- Contradictions become disputed/reviewable rather than overwriting truth.
- Invalid LLM output, unsupported predicate shape, bad evidence excerpts, and
  policy violations fail closed.
- Sensitive and secret proposals skip semantic consolidation unless explicitly
  allowed.
- Transcript metadata reports created, reinforced, superseded, disputed,
  related, and needs-review outcomes accurately.

## Rollout

Implementation should attempt all slices in one cohesive pass so the write path
does not stall halfway between parser patches and ontology-aware consolidation.
The slices remain useful as milestone, validation, and commit boundaries:

1. Define internal proposal, canonicalization, predicate proposal, and
   consolidation decision types. Preserve existing deterministic behavior while
   routing it through the new shapes.
2. Add LLM canonicalization against the promoted predicate catalog and persist
   `predicate_proposals` for missing relationship types.
3. Add bounded match search and deterministic consolidation rules.
4. Add conservative LLM consolidation decisions plus basic inspection surfaces
   for predicate proposals and uncertain memory outcomes.

The first implementation should still keep the blast radius controlled:
deterministic exact dedupe remains the repository safety net, predicate proposal
claims remain candidate-only, and uncertain semantic outcomes prefer review over
silent memory mutation.

## Relationship To Earlier Design

This design supersedes the future-write direction from
`2026-06-28-memory-consolidation-dedupe-design.md`. The earlier exact-dedupe
design remains useful historical context, and the implemented fingerprint layer
continues to serve as the lowest-level idempotency guard.
