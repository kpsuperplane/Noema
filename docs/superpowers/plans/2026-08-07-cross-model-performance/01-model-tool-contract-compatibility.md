# Model and Tool Contract Compatibility Plan

**Status:** Approved for implementation

**Goal:** Ensure every model is evaluated and run through a request shape that
Noema and the exact OpenRouter model can actually support, while keeping
canonical runtime validation as the execution authority.

**Outcome:** HTTP request incompatibility is distinguishable from model
reasoning failure, optional strict-schema fields decode consistently, and typed
terminal calls receive one bounded repair attempt before failing.

## Boundaries

- Keep OpenRouter as the only hosted evaluation transport.
- Do not add provider-specific recommendation matrices.
- Do not weaken canonical validation, tool policy, authorization, or egress.
- Do not parse typed payloads from assistant prose.
- Do not retry external actions; repair applies only before a terminal payload
  has been accepted or any new external operation is allowed.

## Decisions

### OpenRouter request capabilities

- Treat OpenRouter function arguments as `BestEffort`, not `Strict`. OpenRouter
  standardizes tools but does not expose an exact guarantee for strict function
  argument decoding across every routed model/provider endpoint.
- Continue sending the canonical JSON Schema without `strict: true`; validate
  every returned payload against Noema's canonical contract before dispatch.
- Extend cached OpenRouter model metadata with `supported_parameters`. Omit
  optional top-level parameters such as `temperature` or `reasoning` when the
  selected model does not advertise them.
- A required feature is never silently omitted. If a role requires native
  tools or explicit tool choice and the model lacks that capability, fail the
  request preflight as `model_request_incompatible` before a billed call.
- Set OpenRouter `provider.require_parameters = true` so routing cannot select
  an endpoint that ignores a parameter Noema retained in the request.
- Direct OpenAI, Codex, Foundation Models, and qualified local runtimes retain
  their existing strict capability declarations.

### Canonical schema and decoder agreement

- Keep strict lowering for providers that guarantee it, including the rule
  that optional properties become required and nullable.
- Make production decoders accept exactly that nullable shape. Optional text,
  arrays, objects, and discriminated branches decode as `Option<T>` and
  normalize `null` or absence to the domain default at one boundary.
- Start with `task.report_blocked`: `context_markdown` normalizes to `""` and
  `suggested_answers` normalizes to `[]`. Apply the same audit to every built-in
  terminal tool rather than patching only the observed field.
- Schema construction and decoding remain in their existing authorities. Add a
  compatibility test that lowers each built-in terminal schema and proves a
  minimally valid nullable payload reaches the production decoder.

### Terminal repair

- Preserve the existing one-repair limit for Planner, Executor, and Reviewer.
- Add the same one-repair behavior to progress audit, action review, memory
  consolidation, and other single-tool typed auxiliary calls.
- The repair request exposes only the terminal tool, uses required tool choice,
  disables parallel calls and external tools, and includes the bounded
  validation error plus the canonical schema.
- A second malformed response is final evidence; do not loop or fall back to
  prose parsing.

### Evaluation attribution

- Add `request_incompatible` as a case outcome distinct from `failed` and
  shared provider failure. It disqualifies that candidate/role but contributes
  no semantic quality score.
- Retain bounded ordinary provider diagnostics and request ids in ignored
  evidence. Never retain credentials, headers, request bodies, private model
  input, or raw provider responses.
- Authentication, credit, and rate-limit failures remain resumable shared
  failures. HTTP 400 remains checkpointable candidate evidence, but its report
  label must say request incompatibility rather than model-quality failure.
- Fix the multiple-choice fixture to include the production presentation tool;
  do not grade a tool call that the request made impossible.
- Drive typed-role qualification through the production repair loop. Retain a
  separate one-shot diagnostic only when explicitly requested for debugging.

## Implementation Units

1. Change OpenRouter schema enforcement and serialize model-supported request
   parameters from catalog metadata.
2. Normalize nullable terminal fields at the production decode boundary and
   add the all-terminal-schema compatibility test.
3. Consolidate one-repair execution for typed auxiliary tools using the
   existing task terminal-repair path.
4. Add evaluator outcome attribution and repair-aware typed cases; correct the
   multiple-choice fixture.
5. Run only affected protocol, typed-role, and presentation cases before any
   new default decision.

## Expected Touchpoints

- `crates/noema-providers/src/adapters/openrouter.rs` and
  `adapters/openrouter/catalog.rs`: capability claims, catalog metadata, and
  retained request parameters.
- `crates/noema-providers/src/response_support/strict_schema.rs` and
  `generation/request.rs`: canonical schema lowering and request preflight.
- `crates/noema-runtime/src/daemon/task_run_context.rs` and
  `daemon/runtime/typed_terminal_tools.rs`: nullable normalization and the
  shared bounded terminal-repair path.
- `crates/noema-model-evals/src/hosted_provider.rs`, `matrix_runner.rs`, and
  `matrix_report.rs`: outcome attribution and resumable checkpoint reporting.
- Existing provider/runtime test modules own the focused compatibility tests;
  do not create a parallel protocol-test crate.

## Tests and Acceptance

- OpenRouter omits unsupported optional parameters and rejects missing required
  capabilities before transport.
- OpenRouter tool definitions use best-effort schema enforcement; direct
  strict providers still emit strict definitions.
- Every nullable optional field accepted by lowered schemas is accepted and
  normalized by its production decoder.
- A valid terminal call passes immediately; one malformed call can repair;
  two malformed calls stop without external execution.
- Evaluator reports separately count semantic failures, malformed outputs,
  request incompatibilities, and shared provider failures.
- Sonnet/Opus typed cases no longer appear as generic `Provider returned error`
  entries; their exact compatible behavior is measured.

**Budget:** 250–450 production lines, 140–240 test lines, 6–9 focused tests.

**Stop condition:** If OpenRouter still rejects a best-effort canonical schema,
capture the bounded error and narrow only the unsupported schema keyword. Do
not introduce an Anthropic-specific canonical tool contract.
