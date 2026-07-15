# Real-Noema local-model qualification

Run on 2026-07-15 against the production installation, activation, provider,
and runtime paths in `~/.noema` on a MacBook Air `Mac17,3` with an Apple M5 and
32 GB of unified memory. Noema used its pinned llama.cpp b10015 runtime at
commit `12127defda4f41b7679cb2477a4b0d65ee6a0c8f`, Metal, an 8,192-token
evaluation context, temperature 0, and one active generation at a time.

## Decision

Gemma 4 E4B IT Q4_K_M is the only current 11/11 model, so it is the sole
curated catalog entry and the recommendation for both projected 16 GB and
measured 32 GB Apple unified-memory tiers. Its 5.46 GiB live peak is comfortably
inside the existing 16 GB fit ceiling. A physical 16 GB acceptance run remains
separate from this 32 GB projection.

Qualification is a hard gate before performance ranking. Among models that
pass every contract, fitting models are ordered by median case latency and then
peak RSS. Only E4B cleared the gate in this run, so it is rank 1 without a
qualified runner-up.

| Candidate | Critical | All | Load | Median | Long context | Peak RSS | Catalog |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| Gemma 4 E4B IT Q4_K_M | 10/10 | 11/11 | 1.85 s | 5.16 s | 8.83 s | 5.46 GiB | Rank 1 |
| Gemma 4 26B A4B IT Q4_K_M | 10/10 | 10/11 | 4.13 s | 5.34 s | 13.34 s | 16.97 GiB | Excluded: multiple choice |
| Gemma 4 12B IT Q4_K_M | 9/10 | 10/11 | 2.17 s | 12.60 s | 32.12 s | 9.16 GiB | Excluded: memory lookup |
| Ternary Bonsai 8B Q2_KT | 8/10 | 9/11 | 1.13 s | 3.85 s | 19.93 s | 4.07 GiB | Excluded: memory and web safety |

All four resource probes processed roughly 6,500 input tokens and completed 20
follow-up turns. E4B's post-turn RSS stayed between 5,864,833,024 and
5,864,865,792 bytes, a 32 KiB spread.

## Product-path validation

- The final bundled GraphQL catalog exposes one pinned E4B build at priority
  100, recommends it on this 32 GB Metal machine, and generates the expected
  hardware-fit explanation from detected memory.
- Production resumable installation accepted a complete partial transfer,
  independently verified SHA-256 and GGUF structure, atomically installed the
  content-addressed blob, activated it, and started the supervised runtime.
- The active llama-server binds only to loopback, uses Metal offload, runs with
  `--parallel 1` and `--cache-ram 0`, and reports `RUNNING` through GraphQL.
- Activation assigned E4B to the system default, primary agent, task executor,
  task reviewer, progress audit, web summarizer, and all three task-model pool
  tiers. A final database check found zero preference mismatches and
  `PRAGMA quick_check` returned `ok`.
- A full daemon restart started the active model before serving the first
  GraphQL request. The first setup response was already `isReady: true` and
  `runtimeStatus: RUNNING`, with no retry mutation required.
- The installed E4B blob at `~/.noema/models/blobs/` independently hashes to
  `90ce98129eb3e8cc57e62433d500c97c624b1e3af1fcc85dd3b55ad7e0313e9f`.

The two failed Gemma installations and their blobs were removed after the
run. The pre-existing Bonsai installation remains as an inactive manual choice,
while E4B remains installed, active, and the Noema-wide default.

## Winning artifact

- Repository: `ggml-org/gemma-4-E4B-it-GGUF`
- Revision: `2714b5519c6c3516b1000e7c5e1eba998dfe1fe8`
- File: `gemma-4-E4B-it-Q4_K_M.gguf`
- SHA-256: `90ce98129eb3e8cc57e62433d500c97c624b1e3af1fcc85dd3b55ad7e0313e9f`
- Bytes: `5335289824`
- License: `Apache-2.0`

Raw machine-local reports are under
`target/noema-live-validation-20260715-151519/`.
