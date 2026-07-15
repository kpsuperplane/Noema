# Noema local-model evaluations

This suite qualifies GGUF candidates against the model-sensitive behavior Noema actually uses. It runs the production `LocalModelsProvider` and pinned `llama.cpp` runtime in one isolated worker process per model, without changing the user's installed models or default selections.

The current direct-provider suite covers strict final responses, visible streaming, multiple choice, memory selection and continuation, task executor/reviewer/blocked terminal contracts, progress-audit JSON, prompt-injection-resistant web summarization, and context compaction. Every case uses deterministic typed or sentinel predicates. Runtime compatibility, correctness, and latency remain separate results.

Candidates live in `candidates.toml`; suite-wide resource limits live in `suite.toml`. Candidate entries pin the Hugging Face repository, immutable revision, exact file, SHA-256, size, license, and provenance. These candidates are experimental and do not alter Noema's curated recommendation catalog.

```bash
# Inspect the matrix without downloading weights.
cargo run -p noema-model-evals -- list

# Download and verify all candidates into target/noema-model-evals/cache.
cargo run -p noema-model-evals -- prepare

# Run all candidates, or append one or more candidate ids for a subset.
cargo run -p noema-model-evals -- run
cargo run -p noema-model-evals -- run ternary-bonsai-8b-q2kt ternary-bonsai-27b-q2-g64
```

Reports are written incrementally under `target/noema-model-evals/runs/<run-id>/` as `matrix.json`, `summary.md`, and one raw JSON report per candidate/repetition. Downloads are resumable and content-addressed through Noema's installer. The runner checks Noema's existing verified blob store before downloading a duplicate artifact.

Committed machine snapshots live under `results/`. They record the exact hardware,
runtime, artifacts, suite configuration, and raw-report locations used for a decision;
they are evidence for catalog changes, not additional catalog policy.

V1 deliberately scores model-sensitive behavior only. Catalog selection, downloads, SQLite mechanics, and UI rendering already have deterministic product tests and are not model quality. Web/MCP execution is also excluded because the current local provider exposes only Noema's builtin fallback envelope; that integration limit must not be counted as a model failure.
