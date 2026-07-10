# Phase 2: Integrations and Provider Consolidation

**Target time:** 2 hours
**Hard cap:** 2.5 aggregate agent-hours
**LOC target:** at least 1,500 net lines deleted
**Prerequisite:** Phase 1 is validated and committed

## Goal

Consolidate duplicated Responses-provider behavior, use standard OAuth/schema/
secret primitives, and remove custom integration machinery that the existing
`reqwest`, `rmcp`, and FastAPI stacks already provide.

## Files in Scope

Provider consolidation:

- `crates/noema-core/src/provider/contract.rs`
- `crates/noema-core/src/provider/adapters/responses.rs`
- `crates/noema-core/src/provider/adapters/codex_responses.rs`
- `crates/noema-core/src/provider/adapters/openai.rs`
- `crates/noema-core/src/provider/adapters/noema_response_stream.rs`
- `crates/noema-core/src/provider/adapters/codex_oauth.rs`
- provider auth/token support modules

MCP and web integrations:

- `crates/noema-core/src/mcp/http.rs`
- `crates/noema-core/src/mcp/client.rs`
- `crates/noema-core/src/mcp/oauth.rs`
- `crates/noema-core/src/mcp/setup.rs`
- `crates/noema-core/src/capability/gateway.rs`
- `crates/noema-core/src/provider/tools.rs`
- `crates/noema-core/src/web_fetch/**`
- Exa/search adapters

Memory sidecar:

- `crates/noema-core/mnemosyne-sidecar/noema_mnemosyne_sidecar/app.py`
- sidecar configuration and Rust lifecycle/client modules

## Sequence

### Checkpoint 2A — Shared Responses dialect (50 minutes)

- [x] Identify exact OpenAI/Codex duplication in request assembly, native tool
  input/output conversion, stream event normalization, required-response
  parsing, usage, and error mapping.
- [x] Keep one provider-neutral Responses dialect module with small explicit
  adapter hooks for Codex-only encrypted reasoning, prompt-cache behavior, and
  authentication headers.
- [ ] Move shared data types out of the 1,700-line provider contract into
  focused modules only when doing so eliminates duplicate definitions or test
  fixtures; file movement alone does not count.
- [ ] Consolidate repeated stream parser test fixtures and assertions without
  deleting distinct behavior coverage.
- [ ] Remove obsolete legacy JSON-tool and response compatibility paths that
  current context explicitly says are rejected.
- [x] Run provider adapter and response-stream tests; commit before moving to
  integration auth.

Checkpoint target: at least 900 net lines deleted.

### Checkpoint 2B — HTTP, OAuth, and secrets (35 minutes)

- [ ] Build one shared `reqwest::Client` policy for connection/request deadline,
  redirect policy, response-byte cap helpers, and sensitive diagnostics.
- [ ] Replace per-adapter client construction and hand-coded retry/redirect
  behavior. Do not add `reqwest-middleware` unless it removes more code than a
  small shared wrapper.
- [ ] Use `oauth2` 5.0 with default features disabled for CSRF/state, PKCE,
  expiry, and token response types in provider
  and MCP flows touched here. Keep provider-specific device authorization where
  the standard crate does not model it.
- [ ] Use `secrecy` for token/client-secret fields crossing adapter boundaries;
  do not refactor every stored string in this phase.
- [ ] Make Codex token refresh single-flight using one account-keyed async lock
  and persist successful refreshes through the Phase 3 atomic writer seam if it
  already exists; otherwise keep the existing writer and defer atomicity.
- [ ] Remove parallel OAuth primitives and helpers made obsolete.

### Checkpoint 2C — MCP and Mnemosyne boundaries (35 minutes)

- [x] Prefer `rmcp` streamable HTTP/SSE, auth, pagination, and protocol types
  over `mcp/http.rs`. Delete the custom transport only if current `rmcp` covers
  all required operations without a compatibility wrapper.
- [ ] Add `jsonschema 0.47` with all default features disabled for reviewed MCP
  tool arguments and structured results. Do not enable remote or file schema
  resolution.
- [ ] Bound MCP pages/cursors, SSE buffer, response bytes, and total operation
  deadline through library/client configuration and one small policy layer.
- [ ] Use FastAPI's `HTTPBearer` dependency and Python `secrets.compare_digest`
  for the managed Mnemosyne capability instead of handwritten header parsing.
- [ ] Keep Noema-derived memory ownership fields authoritative; Pydantic models
  reject conflicting client metadata.
- [ ] Remove duplicate MCP HTTP/auth/schema code and sidecar request parsing.

If `rmcp` cannot replace `mcp/http.rs` within 25 minutes, stop and defer that
deletion. Do not maintain two MCP transports or wrap every `rmcp` type.

## Policy Scope

This phase implements a small fail-closed gateway:

- disabled/unhealthy/unauthenticated/stale tools cannot dispatch;
- arguments must match the reviewed schema;
- write/export tools remain blocked unless the existing approval state can be
  consumed exactly once;
- approval identity comes from the authenticated request/run context;
- ambiguous side-effect failures are not automatically retried.

Do not build a general policy DSL, content-taint lattice, or multi-level role
system. Record those ideas in the deferred backlog.

## Validation

```bash
cargo fmt --all --check
cargo check -p noema-core -p noema-desktop
cargo clippy -p noema-core --all-targets -- -D warnings
cargo test -p noema-core provider:: -- --test-threads=1
cargo test -p noema-core mcp:: -- --test-threads=1
cargo test -p noema-core web_fetch:: -- --test-threads=1
cargo test -p noema-core daemon::memory -- --test-threads=1
cd crates/noema-core/mnemosyne-sidecar && python3 -m unittest discover -s tests
git diff --check
```

## Exit Evidence

- [x] At least 1,500 net source lines deleted.
- [ ] No duplicate provider request/stream dialect remains.
- [x] No new duplicate reqwest/TLS stack.
- [ ] Standard OAuth and schema-validation types are used where claimed.
- [x] MCP/Mnemosyne negative cases have focused unit coverage.
- [x] Relevant security, governance, integration, and performance tracker rows
  updated.

## Completion Record

Completed at `92,157` maintained source lines, down `1,581` lines during this
phase and `1,767` lines from the program baseline. Commits: `675401cc`,
`f1c741e2`, `9b84e674`, and `49913c64`.

The phase reused `rmcp` for the supported MCP transports, removed the legacy
HTTP+SSE implementation, consolidated the OpenAI/Codex Responses dialect, and
deleted an unreachable hosted-search implementation plus its false capability
advertisement. Full workspace formatting, check, clippy, and 684 unit tests
passed.

Direct `oauth2`/`secrecy` adoption, a second SSE crate, generic HTTP middleware,
schema validation, refresh single-flight, and Mnemosyne bearer authentication
were not forced into this phase: the evaluated libraries would not replace
enough current code within the timebox, or the work is a focused reliability
addition assigned to a later phase/backlog. Remaining Responses request-builder
duplication and the unenforced MCP ownership UI are assigned to later
architecture/frontend simplification rather than extending this phase.

## Stop Conditions

- Stop a public-crate adoption when it needs a parallel compatibility path.
- Stop provider consolidation if Codex/OpenAI behavior can no longer be stated
  as explicit adapter differences.
- Stop governance work when it requires new product semantics rather than
  enforcement of existing persisted fields.
- Preserve the last validated checkpoint at the hard cap.
