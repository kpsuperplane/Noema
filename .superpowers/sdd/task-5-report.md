# Task 5 Report: Add The Progress Auditor

## Status

DONE_WITH_CONCERNS

## Commit

- `12c7b4ae feat: add progress audit model runner`

## Implementation

- Added `daemon::runtime::progress_audit` with:
  - `ProgressAuditDecision`, `ProgressAuditOutcome`, and `ProgressAuditError`.
  - `CodexRuntimeActor::run_progress_audit`, using `crate::store::TOOL_PROGRESS_AUDIT_TASK_ID`.
  - Auxiliary provider execution with no tools and `require_noema_response: false`.
  - Strict JSON parsing for the audit response decision and required non-empty `user_summary`.
  - Progress-audit prompt and no-tools finalization prompt helpers.
- Registered the module in `crates/noema-core/src/daemon/runtime.rs`.
- Added parser and prompt unit tests.

## Validation

- Red test observed first:
  - `cargo test -p noema-core daemon::runtime::progress_audit --no-fail-fast`
  - Failed because the new parser/prompt API was not yet implemented.
- Focused test:
  - `cargo test -p noema-core daemon::runtime::progress_audit --no-fail-fast`
  - Passed: 4 tests passed.
- Formatting:
  - `cargo fmt --all --check`
  - Passed.
- Whitespace:
  - `git diff --check`
  - Passed.

## Concerns

- The focused test emits existing warnings from unrelated unintegrated progress-marker helpers in `crates/noema-core/src/daemon/runtime/transcript_persistence.rs`.
- Unrelated dirty work remains in `crates/noema-core/web/src/components/shell/AppShell.tsx`; it was not modified or staged by this task.

## Final Review Fix

### Findings Addressed

- Scoped OpenAI file-config `reasoning_effort` so it is used only when the request does not pass an explicit model and the adapter uses its config-owned default model. Explicit request `reasoning_effort` still takes priority.
- Added OpenAI request-capture coverage for config default model reasoning, explicit request model without request reasoning, and explicit request model with request reasoning.
- Gated GraphQL profile reasoning metadata to OpenAI provider accounts. Codex profile metadata may still contain `reasoning_efforts`, but GraphQL now exposes an empty list/default and rejects saved reasoning effort for Codex until adapter support is verified.
- Updated existing GraphQL reasoning persistence/validation tests to use OpenAI provider metadata instead of Codex metadata.

### Test Results

- `cargo test -p noema-core provider::adapters::openai --no-fail-fast` - passed, 24 tests.
- `cargo test -p noema-core graphql::schema --no-fail-fast` - passed, 65 tests.
- `cargo fmt --all --check` - passed.
- `git diff --check` - passed.
