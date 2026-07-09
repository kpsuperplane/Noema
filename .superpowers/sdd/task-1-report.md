# Task 1 Report: SQLite Schema And Artifact Domain Types

## Scope Completed

- Added the `artifacts` and `artifact_versions` SQLite tables plus the owner/version indexes in `crates/noema-core/src/store/schema.rs`.
- Extended the `conversation_items.kind` schema check to allow `artifact_reference`.
- Added `crates/noema-core/src/store/artifacts.rs` with the requested domain/input/record types:
  - `ArtifactOwnerRef`
  - `ArtifactStorageKind`
  - `ArtifactVersionStorage`
  - `ArtifactSource`
  - `NewArtifact`
  - `NewArtifactVersion`
  - `ArtifactRecord`
  - `ArtifactVersionRecord`
- Added the requested artifact-specific `StoreError` variants in `crates/noema-core/src/store/error.rs`.
- Wired the new artifact module through `crates/noema-core/src/store.rs` and re-exported the public artifact types from `crates/noema-core/src/lib.rs`.

## Files Changed

- `crates/noema-core/src/store/schema.rs`
- `crates/noema-core/src/store/artifacts.rs`
- `crates/noema-core/src/store/error.rs`
- `crates/noema-core/src/store.rs`
- `crates/noema-core/src/lib.rs`
- `crates/noema-core/src/store/tests.rs`

## TDD Evidence

### RED

- Added the exact test from the brief to `crates/noema-core/src/store/tests.rs`:
  - `sqlite_schema_creates_artifact_tables`
- Ran:
  - `cargo test -p noema-core store::tests::sqlite_schema_creates_artifact_tables --lib`
- Observed failure:
  - assertion failed with `left: []` and `right: ["artifact_versions", "artifacts"]`
  - this confirmed the tables did not yet exist before implementation

### GREEN

- Implemented the schema changes and artifact domain types.
- Re-ran:
  - `cargo test -p noema-core store::tests::sqlite_schema_creates_artifact_tables --lib`
- Observed success:
  - `test store::tests::sqlite_schema_creates_artifact_tables ... ok`

## Validation Run

Ran after the final code state:

- `git diff --check` -> passed
- `cargo fmt --all --check` -> passed
- `cargo check --workspace` -> passed
- `cargo clippy --workspace --all-targets -- -D warnings` -> passed
- `cargo test --workspace --no-fail-fast` -> passed

Workspace test summary from the final run:

- `noema_core`: 655 passed, 0 failed
- `noema_dev`: 10 passed, 0 failed
- `noema_desktop`: 6 passed, 0 failed
- remaining bin/doc test targets: 0 tests, all passed

## Staged Diff Review

Before commit I inspected:

- `git diff --cached --stat`
- `git diff --cached --name-status`

The staged set contained only the six task code files listed above.

## Remaining Unstaged Files

- None reported by `git status --short --branch` after commit

## Concerns

- None on the implemented slice. This task intentionally stops at schema/types only; repository writes, artifact path generation, transcript references, GraphQL, and rendering remain for later tasks.

## Commit

- `087d2f88 feat: add artifact store schema`

## Review Fix Addendum

### What Changed

- Added `ConversationItemKind::ArtifactReference` to `crates/noema-core/src/conversation/status.rs` with storage string `artifact_reference`.
- Updated the enum parser and `as_str()` implementation so `ConversationItemKind::parse("artifact_reference")` round-trips cleanly.
- Added a focused unit test covering the new parse/round-trip behavior.
- Added exhaustive handling for the new variant in the existing runtime/test matches by treating it as a no-op in replay/prompt-context paths and as an agent-authored item in the test helper.

### Tests Run And Results

- `cargo test -p noema-core conversation_item_kind_parse_round_trips_artifact_reference --lib` -> passed
- `cargo test -p noema-core store::tests::sqlite_schema_creates_artifact_tables --lib` -> passed
- `cargo fmt --all --check` -> passed
- `git diff --check` -> passed

### Files Changed

- `crates/noema-core/src/conversation/status.rs`
- `crates/noema-core/src/daemon/runtime/prompt_context.rs`
- `crates/noema-core/src/daemon/tests.rs`
- `crates/noema-core/src/daemon/web/replay.rs`

### Commit Created

- `fix: add artifact_reference conversation kind`
