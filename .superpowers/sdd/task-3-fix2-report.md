# Task 3 Fix 2 Report: MCP Tool Server Move Enabled Recompute

## Implementation

- Added explicit `server_changed` detection in `upsert_discovered_mcp_tool` by comparing the previously stored `mcp_server_id` with the incoming tool server id.
- Kept calibration review invalidation scoped to metadata fingerprint changes only.
- Recomputed MCP server `enabled` state when either metadata changed or server membership changed.
- On affected upserts, recomputation still covers both the previous server and the current server when the tool moved.

## Tests

- Added `same_fingerprint_tool_move_recomputes_old_and_new_server_enabled_state`.
- The test creates two MCP servers, saves a ready calibration for a tool on the first server, then rediscover-upserts the same tool id onto the second server with the same metadata fingerprint.
- It verifies the old server becomes disabled, the new server becomes enabled, and the ready calibration review remains intact.

## Validation

- Red evidence:
  - `cargo test -p noema-core same_fingerprint_tool_move_recomputes_old_and_new_server_enabled_state -- --nocapture`
  - Failed before the production fix because `mcp_server:google` remained enabled after the same-fingerprint move.
- Final verification:
  - `cargo test -p noema-core same_fingerprint_tool_move_recomputes_old_and_new_server_enabled_state -- --nocapture`: passed.
  - `cargo test -p noema-core store::tests::mcp -- --nocapture`: passed, 25 tests.
  - `cargo check -p noema-core`: passed.
  - `cargo fmt --all --check`: passed.
  - `git status --short --branch`: only the scoped MCP files and this report were modified before staging.
  - `git diff --check`: passed.

## Files Changed

- `crates/noema-core/src/store/mcp/tools.rs`
- `crates/noema-core/src/store/tests/mcp.rs`
- `.superpowers/sdd/task-3-fix2-report.md`

## Self-Review

- The fix is tightly scoped to the requested enabled-state recomputation path.
- Same-fingerprint moves now update ready-calibration membership for both old and new servers.
- Metadata-change invalidation behavior remains unchanged because invalidation still runs only under `metadata_changed`.
- No compatibility layer, migration, smoke test, fixture test, sccache bypass, or Rust wrapper change was introduced.

## Concerns

- I ran the focused requested validation rather than the full workspace Rust validation suite.
