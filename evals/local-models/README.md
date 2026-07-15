# Noema local-model evaluations

This suite qualifies GGUF candidates against the model-sensitive behavior Noema actually uses. It runs the production `LocalModelsProvider` and pinned `llama.cpp` runtime in one isolated worker process per model, without changing the user's installed models or default selections.

The current direct-provider suite covers strict final responses, visible streaming, multiple choice, memory selection and continuation, task executor/reviewer/blocked terminal contracts, progress-audit JSON, prompt-injection-resistant web summarization, and context compaction. Every case uses deterministic typed or sentinel predicates. Runtime compatibility, correctness, and latency remain separate results.

The production runtime disables llama.cpp's checkpoint prompt cache explicitly.
The upstream 8 GiB default can grow a seemingly compatible model into unified-memory
pressure over repeated requests, while Noema already serializes local generations
and needs predictable headroom more than cross-request checkpoint retention.

Candidates live in `candidates.toml`; suite-wide resource limits live in `suite.toml`. Candidate entries pin the Hugging Face repository, immutable revision, exact file, SHA-256, size, license, and provenance. These candidates are experimental and do not alter Noema's curated recommendation catalog.

```bash
# Inspect the matrix without downloading weights.
cargo run -p noema-model-evals -- list

# Download and verify all candidates into target/noema-model-evals/cache.
cargo run -p noema-model-evals -- prepare

# Run all candidates, or append one or more candidate ids for a subset.
cargo run -p noema-model-evals -- run
cargo run -p noema-model-evals -- run ternary-bonsai-8b-q2kt ternary-bonsai-27b-q2-g64

# After a candidate passes, repeat the 11 cases plus an unscored resource soak.
cargo run -p noema-model-evals -- soak nemotron-3-nano-4b-q4-k-m
```

`soak` keeps the correctness denominator at 11, then sends one
tokenizer-calibrated near-context request and 20 distinct short turns. The raw
report records observed input tokens, long-request latency, completed turns, and
the post-turn resident-set range; the process-level peak still covers the entire
worker.

Reports are written incrementally under `target/noema-model-evals/runs/<run-id>/` as `matrix.json`, `summary.md`, and one raw JSON report per candidate/repetition. Downloads are resumable and content-addressed through Noema's installer. The runner checks Noema's existing verified blob store before downloading a duplicate artifact.

Committed machine snapshots live under `results/`. They record the exact hardware,
runtime, artifacts, suite configuration, and raw-report locations used for a decision;
they are evidence for catalog changes, not additional catalog policy.

V1 deliberately scores model-sensitive behavior only. Catalog selection, downloads, SQLite mechanics, and UI rendering already have deterministic product tests and are not model quality. Web/MCP execution is also excluded because the current local provider exposes only Noema's builtin fallback envelope; that integration limit must not be counted as a model failure.
