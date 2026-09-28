# Noema OpenRouter model default matrix

This matrix runs Noema's production-derived cases against explicit OpenRouter
model/effort candidates, then ranks candidates separately for each persisted
model setting. Every model-quality call uses Noema's production OpenRouter
adapter. Explicit target mappings say which OpenRouter, direct OpenAI, or Codex
profile a qualified candidate may update; mappings are never inferred from
model names.

The nine setting ids are `primary`, `task_simple`, `task_medium`, `task_difficult`, `task_reviewer`, `web_fetch_summarizer`, `tool_progress_audit`, `action_reviewer`, and `memory_consolidation`. A candidate runs only the roles listed on its manifest entry, which keeps recurring hosted-provider cost bounded.

```bash
# Inspect every OpenRouter candidate and its recommendation targets.
go run ./cmd/noema-model-evals matrix list

# No ids runs a complete default decision. Explicit ids are exploration only.
go run ./cmd/noema-model-evals matrix run
go run ./cmd/noema-model-evals matrix run \
  openrouter-luna-low openrouter-haiku-4.5 openrouter-terra-medium

# Reproducible default-maintenance workflow.
go run ./cmd/noema-model-evals defaults plan
go run ./cmd/noema-model-evals defaults estimate <plan.json>
OPENROUTER_API_KEY=<key> \
  go run ./cmd/noema-model-evals defaults run <plan.json>
go run ./cmd/noema-model-evals defaults propose <decision-dir>
go run ./cmd/noema-model-evals defaults verify <decision-dir>
```

Hosted matrix and cadence runs read `OPENROUTER_API_KEY` directly. They do not
open Noema's account store or depend on `NOEMA_HOME`. The runner validates the
key and authenticated model catalog before the first billed request, then
rejects inaccessible planned models. Local GGUF qualification remains separate
under `evals/local-models/`.

Reports are written incrementally under
`target/noema-model-evals/<run-id>/` as `report.json` and
`report.md`. Explicit-subset exploration reports never emit final
recommendations. A default-decision report emits them only after every selected
candidate and repetition completes; qualification also requires every critical
case and the returned OpenRouter model identity to match the candidate contract.
Default decisions use three repetitions; exploration uses one. Four shared
OpenRouter protocol cases run once per candidate and apply to every role, so
each role has at least five applicable scenarios without repeating identical
billed calls nine times.

## Evidence validity

The current runner uses Go suite v10.
Committed decision summaries are sufficient durable evidence.
Machine-local plans, checkpoints, responses, comparisons, and generated patches are optional diagnostic support.
Their absence does not invalidate a committed decision summary.

A suite change does not automatically invalidate an earlier recommendation.
Rerun when case coverage changes a recommended role's material contract or a named risk.
A later applied decision supersedes only the provider and role cells that it changes.

Each committed decision records its suite and supersession status.
Current shipped recommendations remain authoritative in `internal/provider/recommendations.go`.

Primary qualification includes seven non-compensable stateful scenarios. They
cover a cross-timezone flight and public event discovered on the web, an email
meeting added to a calendar, an existing meeting updated from the latest email,
a delivery date retrieved without a write, a source-grounded reminder, and a
missing appointment that must not produce an invented calendar event. Each case
starts with a broad provider-visible tool catalog and enforces its exact source,
identifier, time, mutation, and terminal-response contract. A direct write,
invented identifier or time, stale-source selection, unnecessary mutation,
skipped inspection, clarification instead of available discovery, or failed
terminal continuation fails the whole candidate regardless of its aggregate
score. Prompts intentionally describe the user's outcome rather than prescribing
tools; choosing the right discovery and action sequence is qualification behavior.
The Markdown report includes a candidate-by-scenario pass matrix and the exact
failure from every unsuccessful repetition so recurring runs expose narrow gaps
instead of only an aggregate quality score.

`role-policies.toml` is the versioned decision policy. Each role declares its
incumbent, minimum case and quality coverage, provider-error ceiling, p95 latency
ceiling, and challenger replacement margin. Ranking is deterministic:
qualification, deterministic score, quality, error rate, estimated cost, p95 latency, then candidate ID. A qualified challenger below the replacement margin does not displace a
qualified incumbent; the report records that reason.

Open-ended cases named by a role policy receive one blinded incumbent-versus-
challenger comparison from the pinned OpenRouter judge. Candidate ids are
deterministically shuffled out of the prompt, outputs are treated as untrusted
data, and the response must satisfy a strict bounded JSON contract. Missing,
malformed, or wrong-identity judge results keep a default decision incomplete.
Typed safety and terminal-contract roles use deterministic graders only.

The immutable plan embeds Git state, candidates, prices, suite and policy
versions, a conservative maximum-token cost estimate, a matching spend ceiling,
and a content fingerprint. The evidence directory copies that plan and updates
`report.json` and `report.md` after every case. Rerunning the same plan resumes
the checkpoint and never repeats a recorded model or judge call. Execution is
sequential, with one provider call in flight and the suite timeout applied to
each case. An interrupted request keeps an uncertain-cost checkpoint.
Create a new plan before another attempt at that request.
Completed cases remain recorded, including malformed outputs and provider failures.
 The cost ceiling multiplies stateful
cases by their maximum provider round count rather than treating each case as
one request.

Pricing in `candidates.toml` is a decision-time snapshot, not provider billing. Before a decision run, refresh each price from the provider's catalog and keep the generated JSON report with the decision. Cached input uses its explicit rate when present; otherwise the normal input rate is used conservatively. Candidates without prices remain comparable on correctness and latency but sort after equally correct candidates with complete cost data.

The matrix never applies its recommendations. It emits a reviewable
`recommendations.go` patch from a completed default decision.

`defaults propose` recomputes rankings from the checkpoint, selects the best
qualified candidate with an explicit mapping for each provider/role cell,
applies the incumbent margin independently for that provider, and writes
`recommendations.patch` plus `proposal.md`. It never edits the source tree.
`defaults verify` succeeds only when every shipped hosted-provider default has
qualified mapped evidence and exactly matches the selected profile and effort.

Go plans use schema 2. Go reports use schema 3.
Old Rust plans cannot resume under the Go runner.
The Go suite uses current production prompts, tool schemas, and provider adapters.
