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
```

Runs read an existing Noema OpenRouter credential without changing accounts or
model selections. Use an explicit evaluation `NOEMA_HOME`; otherwise close the
running Noema app so the evaluator is the only process opening the resolved
SQLite store. Local GGUF qualification remains a separate workflow under
`evals/local-models/`.

Reports are written incrementally under
`target/noema-model-evals/model-matrix/<run-id>/` as `matrix.json` and
`summary.md`. Explicit-subset exploration reports never emit final
recommendations. A default-decision report emits them only after every selected
candidate and repetition completes; qualification also requires every critical
case and the returned OpenRouter model identity to match the candidate contract.
Ranking is deterministic: qualification, total case pass rate, estimated cost,
median latency, then candidate id.

Pricing in `candidates.toml` is a decision-time snapshot, not provider billing. Before a decision run, refresh each price from the provider's catalog and keep the generated JSON report with the decision. Cached input uses its explicit rate when present; otherwise the normal input rate is used conservatively. Candidates without prices remain comparable on correctness and latency but sort after equally correct candidates with complete cost data.

The matrix does not apply its recommendations. A later plan milestone will emit
a reviewable `recommendations.rs` patch from a completed default decision.
