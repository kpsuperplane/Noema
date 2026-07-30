# Noema local-model evaluations

This suite qualifies GGUF candidates against the model-sensitive behavior Noema actually uses. It runs the production `LocalModelsProvider` and pinned `llama.cpp` runtime in one isolated worker process per model, without changing the user's installed models or default selections.

The current direct-provider suite covers strict final responses, visible streaming, multiple choice, onboarding name persistence, memory selection and continuation, simple/medium/difficult task execution, task review and blocked terminal contracts, progress auditing, action review, memory consolidation, prompt-injection-resistant web summarization, and context compaction. Every case uses deterministic typed or sentinel predicates. Runtime compatibility, correctness, and latency remain separate results.

The production runtime enables prompt reuse with a bounded llama.cpp checkpoint
cache. The budget is one thirty-second of detected system RAM, capped at 2 GiB,
so 16 GB and 32 GB machines use 512 MiB and 1 GiB respectively instead of the
upstream 8 GiB default. Noema keeps one active local generation and prioritizes
queued interactive work over background memory, task, audit, compaction, and
task-originated summarization requests.

Candidates live in `candidates.toml`; suite-wide resource limits live in `suite.toml`. Candidate entries pin the Hugging Face repository, immutable revision, exact file, SHA-256, size, license, and provenance. These candidates are experimental and do not alter Noema's curated recommendation catalog.

```bash
# Inspect the matrix without downloading weights.
cargo validate run -p noema-model-evals -- list

# Download and verify all candidates into target/noema-model-evals/cache.
cargo validate run -p noema-model-evals -- prepare

# Run all candidates, or append one or more candidate ids for a subset.
cargo validate run -p noema-model-evals -- run
cargo validate run -p noema-model-evals -- run ternary-bonsai-8b-q2kt ternary-bonsai-27b-q2-g64

# After a candidate passes, repeat the 21 cases plus an unscored resource soak.
cargo validate run -p noema-model-evals -- soak nemotron-3-nano-4b-q4-k-m
```

`soak` keeps the correctness denominator at 21, then sends one
tokenizer-calibrated near-context request and 20 distinct short turns. The raw
report records observed input tokens, long-request latency, completed turns, and
the post-turn resident-set range; the process-level peak still covers the entire
worker.

Reports are written incrementally under `target/noema-model-evals/runs/<run-id>/` as `matrix.json`, `summary.md`, and one raw JSON report per candidate/repetition. Downloads are resumable and content-addressed through Noema's installer. The runner checks Noema's existing verified blob store before downloading a duplicate artifact.

Committed machine snapshots live under `results/`. They record the exact hardware,
runtime, artifacts, suite configuration, and raw-report locations used for a decision;
they are evidence for catalog changes, not additional catalog policy.

V1 deliberately scores model-sensitive behavior only. Catalog selection,
downloads, SQLite mechanics, and UI rendering already have deterministic product
tests and are not model quality. Live web/MCP execution is excluded because the
direct-provider runner does not start external MCP servers or depend on network
state. Local built-ins and MCP tools use the same typed request catalog in
production; memory selection exercises that catalog and transport without making
external services part of the qualification result.
