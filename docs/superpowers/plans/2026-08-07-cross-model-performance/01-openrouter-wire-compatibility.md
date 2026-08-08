# OpenRouter Wire Compatibility Plan

**Status:** Proposed first implementation slice

**Goal:** Stop judging model quality through a request contract that OpenRouter
does not guarantee, and distinguish request rejection from malformed model
output.

**Observable outcome:** Previously rejected typed cases either reach the model
under a supported tool contract or are reported as request incompatibilities;
canonical runtime validation still rejects invalid arguments before dispatch.

## Scope

- Change OpenRouter function-argument enforcement from `Strict` to
  `BestEffort`.
- Preserve the canonical tool schema and runtime argument validation.
- Attribute provider request rejection separately from semantic and malformed
  output failures in model-eval checkpoints and reports.
- Correct any affected fixture that omits a production tool required by its
  expected answer, then rerun only the previously rejected typed cases.

## Non-Goals

- Do not add another provider or a provider-specific recommendation matrix.
- Do not add a second model-capability registry. The existing OpenRouter
  catalog already checks `supported_parameters` for native tools and tool
  choice.
- Do not add `provider.require_parameters`, omit arbitrary request parameters,
  or generalize parameter negotiation without a remaining concrete rejection.
- Do not weaken canonical schema validation, tool policy, authorization,
  action review, or egress.
- Do not parse typed arguments from assistant prose.

## Decisions

### Provider contract

- `OpenRouterProvider::tool_capabilities` reports best-effort function
  arguments for every routed model.
- OpenRouter tool definitions carry the canonical JSON Schema without a
  universal strict-argument claim.
- Direct providers retain their current capability declarations. This slice
  does not infer that an OpenRouter model has the same wire guarantees as its
  direct-provider counterpart.
- Existing catalog preflight remains responsible for rejecting a model that
  lacks native `tools` or `tool_choice` support before a billed request.

### Evaluator attribution

- Add `request_incompatible` as a case outcome distinct from malformed model
  output, semantic failure, and shared provider failure.
- `ProviderError::InvalidRequest` and deterministic client-status API errors
  400, 404, 405, 409, 415, and 422 are `request_incompatible`. They are request
  evidence, not a model-quality score.
- API status 401, 402, 403, and 429 plus typed authentication/rate-limit errors
  are shared resumable provider failures rather than candidate evidence.
- Other 4xx statuses are shared provider failures unless a later bounded case
  proves that exact status deterministically describes Noema's request shape;
  this prevents unfamiliar provider/account errors from being mislabeled.
- Missing credentials, configuration, connection and timeout failures,
  provider unavailability, request transport failures without a deterministic
  client status, and all 5xx API errors are shared provider failures.
- `MalformedResponse`, `PartialResponse`, protocol errors, and response/protocol
  transport failures are malformed-output or provider-protocol outcomes. They
  are never inferred to be request incompatibilities merely because no valid
  assistant output was available.
- Preserve only the existing bounded ordinary diagnostic and request
  identifier for every class.
- Never persist credentials, headers, request bodies, private model input, or
  raw provider responses in reports or checkpoints.
- A model call that reaches output but returns invalid tool arguments remains a
  malformed-output result and is not relabeled as request incompatibility.

## Implementation Sequence

1. Correct the OpenRouter capability declaration and wire lowering.
2. Add the evaluator outcome and exhaustively map existing `ProviderError`
   variants and deterministic status codes into the classes above.
3. Update report/checkpoint aggregation without changing recommendation score
   semantics for successful cases.
4. Fix only fixtures proven impossible under their supplied tool catalog.
5. Rerun the affected typed cases and inspect whether any request rejection
   remains before proposing parameter-specific work.

## Expected Touchpoints

- `crates/noema-providers/src/adapters/openrouter.rs`
- Existing OpenRouter/provider wire-contract tests
- `crates/noema-model-evals/src/hosted_provider.rs`
- `crates/noema-model-evals/src/matrix_runner.rs`
- `crates/noema-model-evals/src/matrix_report.rs`

## Tests and Acceptance

1. OpenRouter tool definitions do not claim strict argument enforcement while
   direct strict-provider definitions remain unchanged.
2. Table-driven error attribution distinguishes deterministic request errors,
   shared client/provider failures, and malformed/provider-protocol output.
3. Malformed returned arguments remain malformed model output, while shared
   authentication/credit/rate-limit errors remain resumable and do not affect
   candidate quality.

The focused rerun passes when previously rejected cases reach model output or
produce a precise remaining request incompatibility. It need not improve model
quality in this slice.

**Budget:** 40–100 production lines, 30–80 test lines, 2–3 focused tests.

**Stop condition:** If best-effort schemas are still rejected, retain the
bounded rejection as evidence and plan only the exact unsupported keyword or
parameter. Do not introduce a general compatibility layer.
