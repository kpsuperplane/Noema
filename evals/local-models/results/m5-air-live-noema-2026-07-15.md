# Real-Noema local-model qualification

- **Suite status:** Historical 12-case live-product snapshot
- **Supersession status:** Latest committed live Noema qualification
- **Current suite:** See [the evaluation guide](../README.md).
- **Recommendation status:** Historical evidence for the E4B catalog choice.

The expanded suite does not automatically invalidate this recommendation.
Use the rerun rules in the parent README when its role contract or a named risk changes.

Run on 2026-07-15 against the production installation, activation, provider,
and runtime paths in `~/.noema` on a MacBook Air `Mac17,3` with an Apple M5 and
32 GB of unified memory. Noema used its pinned llama.cpp b10015 runtime at
commit `12127defda4f41b7679cb2477a4b0d65ee6a0c8f`, Metal, an 8,192-token
evaluation context, temperature 0, and one active generation at a time.

## Decision

Gemma 4 E4B IT Q4_K_M was the only model to pass the 12-case suite, so the decision selected it
as the sole curated catalog entry and the recommendation for both projected 16
GB and measured 32 GB Apple unified-memory tiers. Its 5.46 GiB live peak is
comfortably inside the existing 16 GB fit ceiling. A physical 16 GB acceptance
run remains separate from this 32 GB projection.

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

## Evidence limits

This result used the former Rust backend on one physical 32 GB MacBook Air.
It supports the catalog decision and hardware estimate, not current Go runtime or cross-platform acceptance.
The original installation checks verified the pinned model through the production installer and provider.
Machine-local installation state is not part of the current product contract.

## Onboarding grammar follow-up

The suite originally omitted first-run agent naming. A real onboarding attempt
then exposed that llama.cpp b10015 rejects the `\S` expression from the
canonical name-tool schema while initializing its generated grammar. Noema now
removes regex `pattern` constraints only from the llama.cpp response schema;
the canonical tool contract and runtime validation remain unchanged.

The expanded production-provider suite adds naming as a critical twelfth case.
E4B passed 12/12, including an `update_own_name` call with `{"name":"Momo"}`.
The naming case took 36.34 seconds and 573 output tokens on the concurrently
running development machine. The complete follow-up report is at
`target/noema-grammar-fix-eval-2/report.json`.

## Prompt-cache follow-up

The historical 32 GB probe used a 1 GiB llama.cpp checkpoint cache.
A 4,221-token prompt took 0.130 seconds when warm, then 0.164 seconds after an unrelated prompt.
The intervening cold prompt took 10.251 seconds.
These measurements describe the recorded host and runtime, not current latency guarantees.
The [evaluation guide](../README.md) describes the current cache budget and generation serialization.

## Winning artifact

- Repository: `ggml-org/gemma-4-E4B-it-GGUF`
- Revision: `2714b5519c6c3516b1000e7c5e1eba998dfe1fe8`
- File: `gemma-4-E4B-it-Q4_K_M.gguf`
- SHA-256: `90ce98129eb3e8cc57e62433d500c97c624b1e3af1fcc85dd3b55ad7e0313e9f`
- Bytes: `5335289824`
- License: `Apache-2.0`

The committed summary is the durable evidence record.
Machine-local diagnostic support was written under
`target/noema-live-validation-20260715-151519/` and might not remain present.
