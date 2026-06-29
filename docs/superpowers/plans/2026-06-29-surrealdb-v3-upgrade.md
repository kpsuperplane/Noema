# SurrealDB v3 Upgrade Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Upgrade Noema's embedded SurrealDB Rust SDK dependency from stable v2 to the latest stable v3 line, with no backwards compatibility layer for local v2 development databases.

**Architecture:** Keep the current embedded-store boundary intact: `NoemaStore` owns an embedded `RocksDb` client at `NOEMA_HOME/db`, bootstraps strict SurrealQL schema, and exposes domain-focused repository methods to daemon, GraphQL, CLI, and memory code. The upgrade should be dependency-first, then repair only the compile, schema, and runtime behavior that v3 exposes.

**Tech Stack:** Rust 2024, Cargo workspace dependencies, SurrealDB Rust SDK v3 with `default-features = false` and `kv-rocksdb`, embedded RocksDB, Tokio, async-graphql, existing Noema CLI and daemon tests.

---

## Mode and Ground Rules

Implementation mode: `implement`, followed by adversarial review before shipping.

- Work on `main`.
- Preserve unrelated dirty or untracked files.
- Do not add migrations, compatibility tables, dual-read paths, or v2 database open fallbacks.
- Do not introduce a remote SurrealDB server or Surrealist debug server.
- Do not redesign graph-memory schema except where SurrealDB v3 requires a syntax or behavior repair.
- If an existing local v2 database fails under v3, delete the local development database and rebuild it. Do not write compatibility code.
- Keep `surrealdb = { version = "3", default-features = false, features = ["kv-rocksdb"] }` in the manifest; use Cargo to regenerate `Cargo.lock`.
- The approved design observed `3.1.5` as latest stable and `3.2.0-beta.2` as beta. Reconfirm with Cargo during implementation. If Cargo reports a newer non-prerelease v3 stable, use that stable version in lockfile update commands and keep the manifest on `version = "3"`.

## Recommended Subagent Split

- Implementation Agent A, dependency/runtime lane, model preference `5.5-medium`: `Cargo.toml`, `Cargo.lock`, `crates/noema-core/src/store/runtime.rs`, `crates/noema-core/src/store/error.rs`.
- Implementation Agent B, schema/store lane, model preference `5.5-medium`: `crates/noema-core/src/store/schema.rs`, `crates/noema-core/src/store/{claims,retrieval,conversations,ontology,provider_accounts}.rs`, store tests.
- Implementation Agent C, integration lane, model preference `5.5-medium`: daemon, GraphQL, CLI, generated schema/types only if the Rust upgrade changes exported GraphQL output.
- Review Agent D, adversarial review, model preference `5.5-high`: inspect the final diff for accidental compatibility code, policy weakening, schema looseness, and missed validation. Review only; do not edit files.

The main agent owns sequencing, resolving overlapping edits, running final validation, updating durable context, staging, and committing.

## File Structure

Expected direct edits:

- Modify `Cargo.toml`: change the workspace `surrealdb` dependency from v2 to v3.
- Modify `Cargo.lock`: regenerate through Cargo.

Possible compile/runtime repair files:

- Modify `crates/noema-core/src/store/runtime.rs`: only if v3 changes local `RocksDb` open or close API expectations.
- Modify `crates/noema-core/src/store/error.rs`: only if runtime repair needs one precise new error variant.
- Modify `crates/noema-core/src/store/schema.rs`: only if v3 rejects existing SurrealQL bootstrap syntax or schema behavior.
- Modify `crates/noema-core/src/store/claims.rs`, `crates/noema-core/src/store/retrieval.rs`, `crates/noema-core/src/store/conversations.rs`, `crates/noema-core/src/store/ontology.rs`, or `crates/noema-core/src/store/provider_accounts.rs`: only if v3 changes query result shapes, typed deserialization, or query syntax.
- Modify `crates/noema-core/src/store/tests.rs` and `crates/noema-core/src/store/tests/claims.rs`: only to preserve the current behavior assertions under v3 error text or deterministic output changes.
- Modify `crates/noema-core/src/daemon/tests.rs`, `crates/noema-core/src/graphql/schema.rs`, or `crates/noema-cli/src/inspection_tests.rs`: only if v3 affects persisted behavior visible through these tests.
- Modify `docs/context/current.md`: record the durable fact that the embedded store uses stable SurrealDB v3 and that v2 local development databases may be rebuilt instead of migrated.

Do not edit frontend files unless GraphQL SDL generation changes and Rust validation identifies stale generated assets.

## Task 1: Preflight and Stable Target Confirmation

**Files:** no edits.

- [ ] Run:

```bash
git status --short --branch
```

Expected: current branch is `main`; any unrelated changes are noted and preserved.

- [ ] Read the approved design and current context:

```bash
sed -n '1,260p' docs/superpowers/specs/2026-06-29-surrealdb-v3-upgrade-design.md
sed -n '1,220p' docs/context/current.md
```

Expected: design says embedded RocksDB remains the architecture and no v2 compatibility is needed.

- [ ] Reconfirm the stable v3 target:

```bash
cargo info surrealdb
cargo info surrealdb@3.1.5
```

Expected: `3.1.5` is a stable v3 release and any newer `3.2.0-beta.*` release is not selected. If Cargo lists a newer non-prerelease stable v3, use that stable version for the `cargo update --precise` command in Task 2.

- [ ] Capture the current dependency baseline:

```bash
cargo tree -p noema-core | rg "surrealdb"
rg -n "surrealdb = " Cargo.toml Cargo.lock
```

Expected before the upgrade: `Cargo.toml` still uses `version = "2"` and `cargo tree` resolves SurrealDB v2.

## Task 2: Upgrade the Manifest and Lockfile

**Files:**

- Modify `Cargo.toml`
- Modify `Cargo.lock`

- [ ] Change the workspace dependency in `Cargo.toml` from:

```toml
surrealdb = { version = "2", default-features = false, features = ["kv-rocksdb"] }
```

to:

```toml
surrealdb = { version = "3", default-features = false, features = ["kv-rocksdb"] }
```

- [ ] Regenerate the lockfile through Cargo. If Task 1 confirmed `3.1.5` is still the latest stable v3, run:

```bash
cargo update -p surrealdb --precise 3.1.5
```

If Task 1 confirmed a newer non-prerelease stable v3, use that exact stable version in the same command.

- [ ] Verify the dependency line and lockfile result:

```bash
rg -n "surrealdb = |name = \"surrealdb\"|name = \"surrealdb-core\"" Cargo.toml Cargo.lock
cargo tree -p noema-core | rg "surrealdb"
```

Expected: `Cargo.toml` uses `version = "3"`, the lockfile contains stable v3 SurrealDB crates, and no `alpha`, `beta`, `rc`, or `pre` SurrealDB crate is selected.

- [ ] Run the first compile signal:

```bash
cargo check -p noema-core --tests
```

Expected: either PASS, or compiler errors isolated to SurrealDB API/type changes. Do not commit until Task 3 has a passing `cargo check -p noema-core --tests`.

## Task 3: Repair Compile Breakage at the Smallest Surface

**Files:** edit only the files named by compiler errors.

- [ ] If `cargo check -p noema-core --tests` passes after Task 2, record that no source-level API repair was needed and skip to Task 4.

- [ ] If the compiler rejects the `RocksDb` endpoint argument in `crates/noema-core/src/store/runtime.rs`, keep the same architecture and change only the endpoint conversion. The repair should remain inside `NoemaStore::open`.

Preferred shape if v3 requires a string path:

```rust
let db_path = config
    .path
    .to_str()
    .ok_or_else(|| StoreError::Schema("store path is not valid UTF-8".to_string()))?;
let db = Surreal::new::<RocksDb>(db_path).await?;
```

- [ ] If `NoemaStore::close` fails to compile because `invalidate()` changed, use the v3 local-client API for closing or disconnecting when one exists. If v3 local clients close by dropping the handle, remove the explicit invalidation call and keep `close(self) -> Result<(), StoreError>` returning `Ok(())`.

- [ ] If typed query response errors move to new concrete error types, add the narrowest `StoreError` conversion needed. Do not collapse store errors into strings.

- [ ] Re-run:

```bash
cargo check -p noema-core --tests
```

Expected: PASS.

- [ ] Commit the dependency and compile repairs:

```bash
git status --short
git diff --check
git add Cargo.toml Cargo.lock crates/noema-core/src/store/runtime.rs crates/noema-core/src/store/error.rs
git commit -m "chore: upgrade surrealdb to v3"
```

If `runtime.rs` or `error.rs` did not change, omit them from `git add`.

## Task 4: Validate Store Open and Strict Schema Bootstrap

**Files:**

- Possible edit: `crates/noema-core/src/store/schema.rs`
- Possible edit: `crates/noema-core/src/store/tests.rs`

- [ ] Run the fresh embedded-store tests:

```bash
cargo test -p noema-core store::tests::opens_embedded_store_under_noema_db_dir -- --exact
cargo test -p noema-core store::tests::embedded_store_config_is_stable_for_reopen -- --exact
```

Expected: both tests PASS and the store opens under the temp `NoemaPaths::db_dir()` path.

- [ ] Run strict schema smoke tests:

```bash
cargo test -p noema-core store::tests::strict_schema_rejects_invalid_sensitivity -- --exact
cargo test -p noema-core store::tests::strict_schema_accepts_minimal_claim_with_datetime_fields -- --exact
cargo test -p noema-core store::tests::strict_schema_rejects_invalid_claim_timestamp -- --exact
cargo test -p noema-core store::tests::strict_schema_rejects_duplicate_claim_dedupe_fingerprint -- --exact
cargo test -p noema-core store::tests::strict_schema_rejects_invalid_evidence_authority -- --exact
cargo test -p noema-core store::tests::strict_schema_accepts_valid_evidence_source_shape -- --exact
cargo test -p noema-core store::tests::built_in_personal_predicates_are_seeded -- --exact
cargo test -p noema-core store::tests::built_in_predicate_seed_is_idempotent_when_bootstrap_replays -- --exact
```

Expected: all tests PASS.

- [ ] If a SurrealQL schema statement fails under v3, edit only the failing statement in `STORE_SCHEMA_SQL`. Preserve the same table, field, index, assertion, and seed semantics. Keep `SCHEMAFULL`, sensitivity assertions, evidence source-shape assertions, predicate `use_mode` assertions, and unique indexes intact.

- [ ] After each schema edit, rerun the exact failing test first, then rerun the full strict schema group from this task.

- [ ] Commit schema-only repairs:

```bash
git status --short
git diff --check
git add crates/noema-core/src/store/schema.rs crates/noema-core/src/store/tests.rs
git commit -m "fix: align store schema with surrealdb v3"
```

If no schema/test files changed, do not create an empty commit.

## Task 5: Validate Graph Claim Writes, Evidence, and Retrieval

**Files:**

- Possible edit: `crates/noema-core/src/store/claims.rs`
- Possible edit: `crates/noema-core/src/store/retrieval.rs`
- Possible edit: `crates/noema-core/src/store/objects.rs`
- Possible edit: `crates/noema-core/src/store/ontology.rs`
- Possible edit: `crates/noema-core/src/store/conversations.rs`
- Possible edit: `crates/noema-core/src/store/tests/claims.rs`

- [ ] Run graph-claim write tests:

```bash
cargo test -p noema-core store::tests::claims::known_predicate_claim_gets_evidence -- --exact
cargo test -p noema-core store::tests::claims::reinforcing_existing_claim_adds_evidence -- --exact
cargo test -p noema-core store::tests::claims::fallback_note_punctuation_variants_reinforce_one_claim -- --exact
cargo test -p noema-core store::tests::claims::confirmed_reinforcement_promotes_candidate_claim -- --exact
cargo test -p noema-core store::tests::claims::concurrent_same_fingerprint_writes_reinforce_one_claim -- --exact
cargo test -p noema-core store::tests::claims::unknown_predicate_claim_is_rejected -- --exact
cargo test -p noema-core store::tests::claims::missing_source_item_claim_is_rejected -- --exact
```

Expected: all tests PASS.

- [ ] Run graph entity, retrieval, list, and detail tests:

```bash
cargo test -p noema-core store::tests::claims::colliding_record_fragment_entity_ids_remain_distinct -- --exact
cargo test -p noema-core store::tests::claims::entity_upsert_preserves_existing_aliases_and_metadata -- --exact
cargo test -p noema-core store::tests::claims::retrieval_includes_normal_active_claim_for_allowed_use_mode -- --exact
cargo test -p noema-core store::tests::claims::retrieval_redacts_policy_denied_sensitive_claim -- --exact
cargo test -p noema-core store::tests::claims::retrieval_respects_use_mode_predicate_policy -- --exact
cargo test -p noema-core store::tests::claims::retrieval_limit_keeps_fact_match_over_earlier_hint_only_match -- --exact
cargo test -p noema-core store::tests::claims::list_claims_filters_status_predicate_query_and_clamps_limit -- --exact
cargo test -p noema-core store::tests::claims::claim_detail_includes_support_evidence_and_unknown_claim_is_none -- --exact
```

Expected: all tests PASS.

- [ ] If v3 changes query result shapes, update the local row structs next to the failing query. Do not loosen data validation. If a query returns `NONE` differently, convert at the row boundary and keep public Rust types unchanged.

- [ ] If v3 rejects an existing query, repair the exact SurrealQL text in the repository method that owns the query. Preserve deterministic ordering by IDs, retrieval ranking behavior, and policy-denied omission counts.

- [ ] Commit store query repairs:

```bash
git status --short
git diff --check
git add crates/noema-core/src/store/claims.rs crates/noema-core/src/store/retrieval.rs crates/noema-core/src/store/objects.rs crates/noema-core/src/store/ontology.rs crates/noema-core/src/store/conversations.rs crates/noema-core/src/store/tests/claims.rs
git commit -m "fix: keep graph claims working on surrealdb v3"
```

If no files changed, do not create an empty commit.

## Task 6: Validate Daemon, GraphQL, and CLI Surfaces

**Files:**

- Possible edit: `crates/noema-core/src/daemon/runtime.rs`
- Possible edit: `crates/noema-core/src/daemon/memory_pipeline.rs`
- Possible edit: `crates/noema-core/src/daemon/memory_tool.rs`
- Possible edit: `crates/noema-core/src/daemon/tests.rs`
- Possible edit: `crates/noema-core/src/graphql/schema.rs`
- Possible edit: `crates/noema-core/src/graphql/{resolvers,types}.rs`
- Possible edit: `crates/noema-cli/src/inspection.rs`
- Possible edit: `crates/noema-cli/src/inspection_tests.rs`

- [ ] Run daemon memory integration tests:

```bash
cargo test -p noema-core daemon::tests::runtime_primary_conversation_sends_recent_durable_context_after_restart -- --exact
cargo test -p noema-core daemon::tests::explicit_remember_creates_claim_with_source_evidence -- --exact
cargo test -p noema-core daemon::tests::repeated_explicit_memory_reinforces_one_claim -- --exact
cargo test -p noema-core daemon::tests::runtime_actor_persists_ordinary_provider_memory_as_graph_claim -- --exact
cargo test -p noema-core daemon::tests::provider_memory_mislabelled_secret_stays_candidate_and_unretrievable -- --exact
cargo test -p noema-core daemon::tests::runtime_actor_persists_provider_memory_proposals_as_graph_claims -- --exact
cargo test -p noema-core daemon::tests::runtime_actor_persists_natural_remember_provider_proposals_as_graph_claims -- --exact
cargo test -p noema-core daemon::tests::runtime_actor_executes_search_memory_as_local_tool_result -- --exact
```

Expected: all tests PASS. If a sandboxed run fails with local socket `PermissionDenied`, rerun the same command with socket permissions and report that distinction.

- [ ] Run GraphQL memory inspection tests:

```bash
cargo test -p noema-core graphql::schema::tests::memory_claim_query_returns_seeded_detail -- --exact
cargo test -p noema-core graphql::schema::tests::memory_claims_list_redacts_non_public_facts -- --exact
cargo test -p noema-core graphql::schema::tests::memory_claims_rejects_negative_limit -- --exact
```

Expected: all tests PASS and redaction behavior remains unchanged.

- [ ] Run CLI inspection output tests:

```bash
cargo test -p noema-cli inspection_tests::graph_claim_list_redacts_non_public_facts -- --exact
cargo test -p noema-cli inspection_tests::graph_claim_detail_includes_unredacted_evidence -- --exact
```

Expected: all tests PASS. CLI memory inspection still goes through GraphQL, not direct SurrealDB access.

- [ ] If GraphQL SDL changes after Rust compilation, regenerate web GraphQL artifacts:

```bash
cd crates/noema-core/web
bun run gen:types
```

Expected: generated files change only if the Rust schema changed. Do not run frontend browser inspection for this backend dependency upgrade.

- [ ] Commit integration repairs:

```bash
git status --short
git diff --check
git add crates/noema-core/src/daemon/runtime.rs crates/noema-core/src/daemon/memory_pipeline.rs crates/noema-core/src/daemon/memory_tool.rs crates/noema-core/src/daemon/tests.rs crates/noema-core/src/graphql/schema.rs crates/noema-core/src/graphql/resolvers.rs crates/noema-core/src/graphql/types.rs crates/noema-cli/src/inspection.rs crates/noema-cli/src/inspection_tests.rs crates/noema-core/web/src/generated
git commit -m "fix: validate noema integrations on surrealdb v3"
```

If generated files or integration files did not change, omit them from `git add`. If no files changed, do not create an empty commit.

## Task 7: Update Durable Context

**Files:**

- Modify `docs/context/current.md`

- [ ] Add this settled decision or update the existing embedded-store decision:

```markdown
- The embedded store uses the stable SurrealDB v3 Rust SDK with `kv-rocksdb`; pre-stable local development databases created by v2 may be deleted and rebuilt instead of migrated.
```

- [ ] Keep the context concise; do not paste validation logs.

- [ ] Commit the context update:

```bash
git status --short
git diff --check
git add docs/context/current.md
git commit -m "docs: note surrealdb v3 store baseline"
```

## Task 8: Full Validation and Adversarial Review

**Files:** no planned edits after this task unless review finds an issue.

- [ ] Run formatting check:

```bash
cargo fmt --all --check
```

Expected: PASS.

- [ ] Run workspace compile:

```bash
cargo check --workspace
```

Expected: PASS.

- [ ] Run clippy:

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: PASS.

- [ ] Run unit tests:

```bash
cargo test --workspace --no-fail-fast
```

Expected: PASS. Do not run smoke tests or fixture tests unless the user explicitly requests them.

- [ ] Run dependency scans:

```bash
cargo tree -p noema-core | rg "surrealdb"
rg -n "surrealdb = |name = \"surrealdb\"|name = \"surrealdb-core\"" Cargo.toml Cargo.lock
```

Expected: SurrealDB and SurrealDB core resolve to stable v3; no SurrealDB prerelease is selected.

- [ ] Run final diff checks:

```bash
git status --short --branch
git diff --check
```

Expected: only intended files are modified before final staging.

- [ ] Dispatch or run adversarial review focused on:

```text
Review the SurrealDB v3 upgrade diff for accidental v2 compatibility code, loosened schema/policy assertions, remote server mode, direct CLI SurrealDB access, unstable beta dependency selection, missing context update, and test gaps around store open, schema bootstrap, graph claim writes, retrieval, daemon, GraphQL, and CLI inspection.
```

Expected: no blocking findings. Address any blocking finding with a focused patch and rerun the relevant focused test plus the full validation command that covers the touched area.

## Task 9: Final Commit Hygiene

**Files:** all completed implementation files.

- [ ] Inspect staged scope before any final commit:

```bash
git diff --cached --stat
git diff --cached --name-status
```

Expected: staged files match the completed unit of work.

- [ ] Confirm no untracked or unstaged implementation files remain:

```bash
git status --short --branch
```

Expected: clean worktree, or only unrelated user files explicitly reported.

- [ ] If review fixes were committed separately, no final squashing is required. Leave small, scoped commits in place unless the user asks for history cleanup.

## Success Criteria

- `Cargo.toml` uses `surrealdb = { version = "3", default-features = false, features = ["kv-rocksdb"] }`.
- `Cargo.lock` resolves SurrealDB crates to a stable v3 release, not beta.
- `NoemaStore::open` still opens embedded RocksDB under `NOEMA_HOME/db`.
- Strict schema bootstrap succeeds under v3.
- Graph-claim writes, evidence, deterministic retrieval, policy omissions, inspection reads, daemon memory persistence, GraphQL memory queries, and CLI memory output keep their current behavior.
- `docs/context/current.md` records the v3 baseline and no-migration stance.
- Full Rust validation passes.
- No backwards compatibility layer for v2 local databases is introduced.
