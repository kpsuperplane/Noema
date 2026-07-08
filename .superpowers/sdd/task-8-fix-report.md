# Task 8 Fix Report: GraphQL Memory Settings Review Findings

## Implementation

- Added an explicit `2s` timeout to the user-triggered `checkMemoryService` readiness request.
- Centralized readiness request construction in `memory_service_readiness_request`, which builds the reqwest client with the timeout and returns GraphQL errors for client construction failures.
- Added validation in `save_memory_service_settings` so `reasoningEffort` is rejected when both `providerAccountId` and `modelProfile` are omitted.
- Kept the fix scoped to GraphQL memory settings/status code and GraphQL schema tests.

## Tests

### RED

- `cargo test -p noema-core graphql::schema::tests::save_memory_service_settings_rejects_reasoning_effort_without_model -- --nocapture`
  - Failed before the fix because the mutation succeeded and silently ignored orphan `reasoningEffort`.
- `cargo test -p noema-core graphql::schema::tests::check_memory_service_times_out_when_socket_never_responds -- --nocapture`
  - Failed before the fix because `checkMemoryService` exceeded the test's 3-second timeout against a local socket that accepted the connection but never responded.

### GREEN / Focused

- `cargo test -p noema-core graphql::schema::tests::memory_settings_query_returns_defaults -- --nocapture`
  - PASS
- `cargo test -p noema-core graphql::schema::tests::old_memory_graph_field_is_not_in_schema -- --nocapture`
  - PASS
- `cargo test -p noema-core graphql::schema::tests::save_memory_service_settings_rejects_reasoning_effort_without_model -- --nocapture`
  - PASS
- `cargo test -p noema-core graphql::schema::tests::check_memory_service_times_out_when_socket_never_responds -- --nocapture`
  - PASS

### Required Validation

- `cargo fmt --all --check`
  - PASS
- `cargo check -p noema-core`
  - PASS
- `cargo clippy -p noema-core --all-targets -- -D warnings`
  - PASS
- `git diff --check`
  - PASS

## Files Changed

- `crates/noema-core/src/graphql/memory.rs`
- `crates/noema-core/src/graphql/schema.rs`
- `.superpowers/sdd/task-8-fix-report.md`

## Self-Review

- `checkMemoryService` no longer waits indefinitely on network I/O from a reachable but non-responsive endpoint.
- The timeout is explicit and local to the readiness probe.
- `saveMemoryServiceSettings` now treats `reasoningEffort` as part of a complete provider/model preference and rejects orphan values instead of dropping them.
- New tests exercise the exact review findings without touching frontend, graph modules, SurrealDB-wide code, or unrelated APIs.
- `schema.rs` is already over the preferred 750-line guideline, but the nearby GraphQL test module is the existing local pattern and this fix did not broaden into a test-file split.

## Concerns

- None.
