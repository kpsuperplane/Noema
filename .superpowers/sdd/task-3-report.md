# Task 3 Report: Local File Artifacts And Download Route

## What I implemented

- Added conversation artifact path helpers to `NoemaPaths`:
  - `conversations_dir`
  - `conversation_dir`
  - `conversation_artifacts_dir`
  - `conversation_artifact_version_dir`
- Added `safe_artifact_filename` plus `NoemaPathError::UnsafeArtifactFilename` so first-slice local artifact filenames must be a single safe path segment.
- Created `crates/noema-core/src/artifacts.rs` with:
  - `NewConversationLocalFileArtifact`
  - `ArtifactWriteError`
  - `artifact_download_url`
  - `create_conversation_local_file_artifact`
- Implemented local artifact file writing to:
  - `${NOEMA_HOME}/conversations/<conversation_id>/artifacts/<artifact_id>/versions/1/<filename>`
  - with preallocated `artifact_id` and `artifact_version_id`
  - SHA-256 persisted on the first version row
  - metadata written only after file write succeeds
  - best-effort file rollback when metadata persistence fails
- Exported the new artifact helpers from `crates/noema-core/src/lib.rs`.
- Added `write_binary_response` in `crates/noema-core/src/daemon/web/http.rs`.
- Added read-only local download handling in `crates/noema-core/src/daemon/web/mod.rs` for:
  - `GET /artifacts/<artifact_version_id>/download`
  - loads artifact version + canonical artifact row
  - rejects missing/deleted/non-local versions with `404`
  - resolves local paths under `NoemaPaths`
  - returns bytes with content type and attachment filename

## TDD evidence

- RED phase:
  - Added the requested path tests in `crates/noema-core/src/paths.rs`
  - Added `conversation_local_file_artifact_writes_bytes_and_metadata` in `crates/noema-core/src/store/tests.rs`
  - Ran the task brief’s intended test set via Cargo-compatible targeted runs
- Note:
  - The brief’s exact command
    - `cargo test -p noema-core conversation_artifact_version_dir_lives_under_conversation_artifacts safe_artifact_filename_rejects_path_traversal conversation_local_file_artifact_writes_bytes_and_metadata --lib`
    - is not valid Cargo syntax because Cargo accepts one test filter, not several positional filters
- Actionable RED run:
  - `cargo test -p noema-core --lib conversation_local_file_artifact_writes_bytes_and_metadata`
  - Result before implementation: compile failed with missing
    - `create_conversation_local_file_artifact`
    - `NewConversationLocalFileArtifact`
    - `conversation_artifact_version_dir`
    - `conversation_artifacts_dir`
    - `safe_artifact_filename`
- GREEN phase:
  - `cargo test -p noema-core --lib conversation_local_file_artifact_writes_bytes_and_metadata`
    - passed
  - `cargo test -p noema-core --lib conversation_artifact_version_dir_lives_under_conversation_artifacts`
    - passed
  - `cargo test -p noema-core --lib safe_artifact_filename_rejects_path_traversal`
    - passed
  - Added focused web-route tests in `crates/noema-core/src/daemon/web/mod.rs`
  - `cargo test -p noema-core --lib artifact_download_route_serves_local_artifact_bytes`
    - passed

## Validation run

- `cargo fmt --all --check`
  - passed
- `git diff --check`
  - passed
- `cargo check --workspace`
  - passed
- `cargo clippy --workspace --all-targets -- -D warnings`
  - passed
- `cargo test --workspace --no-fail-fast`
  - passed
  - relevant new coverage included:
    - `store::tests::conversation_local_file_artifact_writes_bytes_and_metadata`
    - `paths::tests::conversation_artifact_version_dir_lives_under_conversation_artifacts`
    - `paths::tests::safe_artifact_filename_rejects_path_traversal`
    - `daemon::web::tests::artifact_download_route_accepts_get_path`
    - `daemon::web::tests::artifact_download_route_serves_local_artifact_bytes`

## Files changed

- `crates/noema-core/src/artifacts.rs`
- `crates/noema-core/src/daemon/web/http.rs`
- `crates/noema-core/src/daemon/web/mod.rs`
- `crates/noema-core/src/lib.rs`
- `crates/noema-core/src/paths.rs`
- `crates/noema-core/src/store/tests.rs`
- `.superpowers/sdd/task-3-report.md`

## Commit created

- `12bab7e4` — `feat: write local artifact files`

## Concerns

- None.

---

## Review Fix Follow-up: Local Artifact Download Hardening

### What changed

- Tightened local download path validation so the web route only serves files that match the exact expected conversation artifact version subtree for the loaded artifact/version, instead of trusting any safe relative path under `NOEMA_HOME`.
- Tightened `safe_artifact_filename` to reject quote and control characters in addition to traversal/path-separator cases.
- Added a defensive attachment header helper that escapes quotes and control characters before building `Content-Disposition`.
- Added focused regression tests covering:
  - rejecting forged local artifact paths outside the expected artifact subtree
  - rejecting unsafe filename characters for artifact creation
  - rejecting corrupt stored filenames that would otherwise inject response headers

### Tests run and results

- `cargo test -p noema-core --lib safe_artifact_filename_rejects_header_unsafe_characters` — passed
- `cargo test -p noema-core --lib attachment_content_disposition_escapes_quotes_and_controls` — passed
- `cargo test -p noema-core --lib artifact_download_route_rejects_local_path_outside_artifact_version_subtree` — passed
- `cargo test -p noema-core --lib artifact_download_route_sanitizes_attachment_filename` — passed
- `cargo test -p noema-core --lib conversation_local_file_artifact_writes_bytes_and_metadata` — passed
- `cargo test -p noema-core --lib artifact_download_route_` — passed
- `cargo test -p noema-core --lib safe_artifact_filename_` — passed
- `cargo fmt --all --check` — passed

### Files changed

- `crates/noema-core/src/artifacts.rs`
- `crates/noema-core/src/daemon/web/http.rs`
- `crates/noema-core/src/daemon/web/mod.rs`
- `crates/noema-core/src/paths.rs`
- `.superpowers/sdd/task-3-report.md`

### Commit created

- `fix: harden local artifact downloads`

---

## Second Review Fix: Header-Safe Download Content Type

### What changed

- Hardened `write_response` and `write_binary_response` so invalid `Content-Type` values are rejected at emission time and fall back to `application/octet-stream`.
- Added a focused unit test for invalid header values in `crates/noema-core/src/daemon/web/http.rs`.
- Added a route-level regression test that forges a stored artifact version `media_type` containing CR/LF and verifies the download response still emits `Content-Type: application/octet-stream`.

### Tests run and results

- `cargo fmt --all --check` — passed
- `cargo test -p noema-core --lib invalid_content_type_falls_back_to_octet_stream` — passed
- `cargo test -p noema-core --lib artifact_download_route_falls_back_to_octet_stream_for_unsafe_media_type` — passed

### Files changed

- `crates/noema-core/src/daemon/web/http.rs`
- `crates/noema-core/src/daemon/web/mod.rs`
- `.superpowers/sdd/task-3-report.md`
