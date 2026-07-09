# Task 5 Report: Typed Artifact Transcript References

## What I Implemented

- Added `TurnTranscriptItem::ArtifactReference` to the daemon/web protocol with the typed display snapshot fields from the task brief:
  - `artifact_id`
  - `artifact_version_id`
  - `title`
  - `artifact_kind`
  - `storage_kind`
  - `external_url`
  - `download_url`
  - `media_type`
- Kept replay synchronous by decoding artifact reference display snapshots directly from `conversation_items.payload_json` in `crates/noema-core/src/daemon/web/replay.rs`.
- Added a dedicated replay payload decoder:
  - `ReplayArtifactReferencePayload`
- Mapped persisted `ConversationItemKind::ArtifactReference` rows into typed replay items instead of dropping them from transcript replay.
- Added `GraphqlArtifactReference` and included it in the `GraphqlTranscriptItem` union.
- Mapped `TurnTranscriptItem::ArtifactReference` through GraphQL chat conversion and GraphQL conversation-item telemetry labeling.
- Updated transcript persistence and a couple of exhaustive test/runtime helpers so the new transcript variant compiles and round-trips consistently when used elsewhere.

## What I Tested And Test Results

- `cargo test -p noema-core daemon::web::replay::tests::conversation_replay_maps_artifact_reference --lib`
  - PASS after implementation
- `cargo test -p noema-core conversation_transcript_page_exposes_artifact_reference_item --lib`
  - PASS
- `cargo test -p noema-core conversation_replay_maps_artifact_reference --lib`
  - PASS
- `cargo fmt --all --check`
  - PASS
- `cargo check --workspace`
  - PASS
- `cargo clippy --workspace --all-targets -- -D warnings`
  - PASS
- `cargo test --workspace --no-fail-fast`
  - PASS
- Pre-commit checks:
  - `git status --short --branch`
  - `git diff --check`
  - PASS / reviewed

## TDD Evidence

### RED

Command:

```bash
cargo test -p noema-core daemon::web::replay::tests::conversation_replay_maps_artifact_reference --lib
```

Result:

```text
error[E0599]: no variant named `ArtifactReference` found for enum `protocol::TurnTranscriptItem`
```

This failed for the expected reason: the new replay test referenced the typed transcript variant before the protocol/replay/GraphQL implementation existed.

### GREEN

Command:

```bash
cargo test -p noema-core daemon::web::replay::tests::conversation_replay_maps_artifact_reference --lib
```

Result:

```text
running 1 test
test daemon::web::replay::tests::conversation_replay_maps_artifact_reference ... ok

test result: ok. 1 passed; 0 failed
```

Additional green verification:

```bash
cargo test -p noema-core conversation_transcript_page_exposes_artifact_reference_item --lib
```

```text
running 1 test
test graphql::schema::tests::conversation_transcript_page_exposes_artifact_reference_item ... ok

test result: ok. 1 passed; 0 failed
```

## Files Changed

- `crates/noema-core/src/daemon/protocol.rs`
- `crates/noema-core/src/daemon/runtime/transcript_persistence.rs`
- `crates/noema-core/src/daemon/tests.rs`
- `crates/noema-core/src/daemon/web/replay.rs`
- `crates/noema-core/src/graphql/chat.rs`
- `crates/noema-core/src/graphql/schema.rs`

## Self-Review Findings

- Verified `ConversationItemKind::ArtifactReference` and the SQLite `artifact_reference` schema entry already existed and matched the task needs, so I did not duplicate that enum/schema groundwork.
- Kept artifact transcript replay store-independent and synchronous, using display snapshot fields in `payload_json` exactly as the task brief required.
- Did not add migrations, compatibility layers, frontend rendering, generated frontend types, or artifact-intent inference from filenames/text/URLs.

## Issues Or Concerns

- None.

## Commit

- `feat: add artifact transcript references`
