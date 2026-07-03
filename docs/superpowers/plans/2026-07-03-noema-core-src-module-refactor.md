# Noema Core Src Module Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split the agreed oversized Rust modules under `crates/noema-core/src` into focused submodules without changing behavior or public crate APIs.

**Architecture:** Preserve idiomatic Rust facade files (`config.rs`, `conversation.rs`, `mcp.rs`, `store.rs`, `daemon.rs`) and move implementation-heavy internals under matching folders. Re-export existing public types/functions from facades so downstream code continues to compile. Keep each milestone behavior-preserving and commit after focused validation.

**Tech Stack:** Rust 2024, Tokio, SurrealDB Rust SDK, serde/serde_json, async-graphql, thiserror.

---

### Task 1: Split Store MCP Persistence

**Files:**
- Modify: `crates/noema-core/src/store.rs`
- Replace: `crates/noema-core/src/store/mcp.rs`
- Create: `crates/noema-core/src/store/mcp/model.rs`
- Create: `crates/noema-core/src/store/mcp/rows.rs`
- Create: `crates/noema-core/src/store/mcp/servers.rs`
- Create: `crates/noema-core/src/store/mcp/tools.rs`
- Create: `crates/noema-core/src/store/mcp/calibrations.rs`
- Create: `crates/noema-core/src/store/mcp/trusted_identities.rs`
- Create: `crates/noema-core/src/store/mcp/approvals.rs`

- [ ] Move MCP record/input enums and structs to `model.rs`.
- [ ] Move Surreal row structs and row-to-record parsers to `rows.rs`.
- [ ] Move MCP server CRUD to `servers.rs`.
- [ ] Move discovered-tool CRUD to `tools.rs`.
- [ ] Move calibration write/read/validation to `calibrations.rs`.
- [ ] Move trusted identity selector persistence to `trusted_identities.rs`.
- [ ] Move approval request persistence and preview sanitization to `approvals.rs`.
- [ ] Keep `store/mcp.rs` as the module facade and preserve `store.rs` re-exports.
- [ ] Run `cargo fmt --all --check` and `cargo check -p noema-core`.
- [ ] Commit as `refactor(core): split store mcp module`.

### Task 2: Move Daemon Memory Logic Into A Domain Subtree

**Files:**
- Modify: `crates/noema-core/src/daemon.rs`
- Move: `crates/noema-core/src/daemon/memory_pipeline.rs` to `crates/noema-core/src/daemon/memory/pipeline.rs`
- Move: `crates/noema-core/src/daemon/memory_tool.rs` to `crates/noema-core/src/daemon/memory/tool.rs`
- Move: `crates/noema-core/src/daemon/runtime/memory_writes.rs` to `crates/noema-core/src/daemon/memory/writes.rs`
- Create: `crates/noema-core/src/daemon/memory.rs`
- Modify imports in `daemon/runtime/turn.rs`, `daemon/runtime/actor.rs`, and other daemon modules as needed.

- [ ] Create `daemon::memory` facade with `pipeline`, `tool`, and `writes`.
- [ ] Update `CodexRuntimeActor` impl imports so runtime memory write methods remain available.
- [ ] Keep behavior and visibility equivalent to the previous `daemon` private module layout.
- [ ] Run `cargo fmt --all --check` and `cargo check -p noema-core`.
- [ ] Commit as `refactor(core): group daemon memory modules`.

### Task 3: Split Config Into Focused Submodules

**Files:**
- Replace: `crates/noema-core/src/config.rs`
- Create: `crates/noema-core/src/config/error.rs`
- Create: `crates/noema-core/src/config/env.rs`
- Create: `crates/noema-core/src/config/model.rs`
- Create: `crates/noema-core/src/config/raw.rs`
- Keep: `crates/noema-core/src/config/tests.rs`

- [ ] Move public config structs/enums/constants to `model.rs`.
- [ ] Move `ConfigError` to `error.rs`.
- [ ] Move env key normalization/provider construction to `env.rs`.
- [ ] Move raw/file config structs and loading/resolution helpers to `raw.rs`.
- [ ] Keep `Config` facade and public re-exports in `config.rs`.
- [ ] Run `cargo fmt --all --check` and `cargo check -p noema-core`.
- [ ] Commit as `refactor(core): split config module`.

### Task 4: Split Conversation Domain Types

**Files:**
- Replace: `crates/noema-core/src/conversation.rs`
- Create: `crates/noema-core/src/conversation/status.rs`
- Create: `crates/noema-core/src/conversation/records.rs`

- [ ] Move status enums and parse helpers to `status.rs`.
- [ ] Move `NewConversation`, turn/item records, and replay mode to `records.rs`.
- [ ] Re-export the same public names from `conversation.rs`.
- [ ] Run `cargo fmt --all --check` and `cargo check -p noema-core`.
- [ ] Commit as `refactor(core): split conversation domain types`.

### Task 5: Move MCP Trusted Identity Helpers

**Files:**
- Modify: `crates/noema-core/src/mcp.rs`
- Create: `crates/noema-core/src/mcp/trusted_identity.rs`

- [ ] Move `TrustedIdentitySelectorKind`, `OwnerExtractor`, `OwnerExtractorSource`, and trusted identity normalization helpers to `trusted_identity.rs`.
- [ ] Re-export trusted identity types and `normalize_trusted_identity_value` from `mcp.rs`.
- [ ] Run `cargo fmt --all --check`, `cargo check -p noema-core`, and focused tests if any module moved tests.
- [ ] Commit as `refactor(core): split mcp trusted identity helpers`.

### Task 6: Final Validation

- [ ] Run `cargo fmt --all --check`.
- [ ] Run `cargo check --workspace`.
- [ ] Run `cargo clippy --workspace --all-targets -- -D warnings`.
- [ ] Run `cargo test --workspace --no-fail-fast`.
- [ ] Run `git status --short --branch` and report remaining worktree state.
