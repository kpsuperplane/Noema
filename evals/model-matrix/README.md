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
cargo validate run -p noema-model-evals -- matrix list

# No ids runs a complete default decision. Explicit ids are exploration only.
cargo validate run -p noema-model-evals -- matrix run
cargo validate run -p noema-model-evals -- matrix run \
  openrouter-luna-low openrouter-haiku-4.5 openrouter-terra-medium

# Reproducible default-maintenance workflow.
cargo validate run -p noema-model-evals -- defaults plan
cargo validate run -p noema-model-evals -- defaults estimate <plan.json>
NOEMA_HOME=/absolute/path/to/an/isolated/eval-home \
  cargo validate run -p noema-model-evals -- defaults run <plan.json>
cargo validate run -p noema-model-evals -- defaults propose <decision-dir>
cargo validate run -p noema-model-evals -- defaults verify <decision-dir>
```

Cadence runs require an absolute, explicit evaluation `NOEMA_HOME`. Put an
OpenRouter account credential in that isolated home; the runner never changes
accounts or model selections. It refreshes the authenticated model catalog
before the first billed request and rejects inaccessible planned models. Local
GGUF qualification remains separate under `evals/local-models/`.

Reports are written incrementally under
`target/noema-model-evals/model-matrix/<run-id>/` as `matrix.json` and
`summary.md`. Explicit-subset exploration reports never emit final
recommendations. A default-decision report emits them only after every selected
candidate and repetition completes; qualification also requires every critical
case and the returned OpenRouter model identity to match the candidate contract.
Default decisions use three repetitions; exploration uses one. Four shared
OpenRouter protocol cases run once per candidate and apply to every role, so
each role has at least five applicable scenarios without repeating identical
billed calls nine times.

`role-policies.toml` is the versioned decision policy. Each role declares its
incumbent, minimum case and quality coverage, provider-error ceiling, p95 latency
ceiling, and challenger replacement margin. Ranking is deterministic:
qualification, quality, reliability, estimated cost, p95 latency, then candidate
id. A qualified challenger below the replacement margin does not displace a
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
`matrix.json` and `summary.md` after every case. Rerunning the same plan resumes
the checkpoint and never repeats a recorded model or judge call. Execution is
sequential, with one provider call in flight and the suite timeout applied to
each call.

Pricing in `candidates.toml` is a decision-time snapshot, not provider billing. Before a decision run, refresh each price from the provider's catalog and keep the generated JSON report with the decision. Cached input uses its explicit rate when present; otherwise the normal input rate is used conservatively. Candidates without prices remain comparable on correctness and latency but sort after equally correct candidates with complete cost data.

The matrix never applies its recommendations. It emits a reviewable
`recommendations.rs` patch from a completed default decision.

`defaults propose` recomputes rankings from the checkpoint, selects the best
qualified candidate with an explicit mapping for each provider/role cell,
applies the incumbent margin independently for that provider, and writes
`recommendations.patch` plus `proposal.md`. It never edits the source tree.
`defaults verify` succeeds only when every shipped hosted-provider default has
qualified mapped evidence and exactly matches the selected profile and effort.
