# Noema cross-provider model matrix

This matrix runs Noema's production-derived deterministic cases against explicit provider/model/effort candidates, then ranks candidates separately for each persisted model setting. It reuses the same `ProviderHandle` boundary as the runtime: OpenRouter, OpenAI, Codex, and Apple Foundation Models use their production adapters, while `local_models` references the pinned GGUF catalog and existing isolated worker.

The nine setting ids are `primary`, `task_simple`, `task_medium`, `task_difficult`, `task_reviewer`, `web_fetch_summarizer`, `tool_progress_audit`, `action_reviewer`, and `memory_consolidation`. A candidate runs only the roles listed on its manifest entry, which keeps recurring hosted-provider cost bounded.

```bash
# Inspect every candidate, including disabled provider templates.
cargo validate run -p noema-model-evals -- matrix list

# Run every enabled candidate, or name a smaller comparison set.
cargo validate run -p noema-model-evals -- matrix run
cargo validate run -p noema-model-evals -- matrix run \
  openrouter-luna-low openrouter-haiku-4.5 openrouter-terra-medium

# Explicit ids may select a disabled direct, Codex, Foundation, or GGUF target.
cargo validate run -p noema-model-evals -- matrix run direct-openai-luna-low
```

Hosted runs read existing Noema provider credentials without changing accounts or model selections. Close the running Noema app before a hosted matrix so the evaluator is the only process opening the canonical SQLite store. Direct OpenAI additionally requires `NOEMA_OPENAI__API_KEY`. Local GGUF workers retain their isolated `NOEMA_HOME` and never use the human's provider credentials.

Reports are written incrementally under `target/noema-model-evals/model-matrix/<run-id>/` as `matrix.json` and `summary.md`. Ranking is deterministic: critical qualification first, then total case pass rate, estimated cost, median latency, and candidate id. A setting receives a recommendation only when every configured repetition completed and every critical case passed.

Pricing in `candidates.toml` is a decision-time snapshot, not provider billing. Before a decision run, refresh each price from the provider's catalog and keep the generated JSON report with the decision. Cached input uses its explicit rate when present; otherwise the normal input rate is used conservatively. Candidates without prices remain comparable on correctness and latency but sort after equally correct candidates with complete cost data.

The matrix does not apply its recommendations. Changing the nine model settings remains an explicit product operation after a human reviews correctness failures, latency, estimated cost, and any provider-specific constraints.
