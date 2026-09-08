# ACP retirement

The human removed ACP support on 2026-09-08.
Noema no longer launches, authenticates, configures, or runs ACP executors.
The server API, web Settings, and iOS Settings no longer expose ACP controls.

Go migration 38 removes ACP configuration and authentication tables.
It fails unfinished Tasks and runs that selected ACP.
It interrupts active run items, supersedes open gates, and pauses related recurrences.
Task history, evidence, names, and completed results remain stored.
Provider Tasks keep their state and can resume after server restart.

Stored ACP placement markers remain only to reject unsupported execution.
These markers belong to Tasks and recurrences created before migration 38.
They cannot select a provider automatically.
They can be removed when no stored objects need this retirement check.
Immutable older migrations retain their original ACP table definitions.
Archived plans and validation evidence describe past support.

## Rust test mapping exceptions

The removal retires these 11 ACP-only Rust test mappings.
They are explicit exceptions to the previous complete Rust test port.
Mixed tests retain their non-ACP assertions and use built-in executor fixtures.

| Former Go test | Rust crate |
| --- | --- |
| `TestRustRuntime_fake_stdio_acp_covers_probe_auth_malformed_crash_and_timeout` | `noema-runtime` |
| `TestRustRuntime_fake_stdio_acp_covers_streaming_terminal_launch_and_cancellation` | `noema-runtime` |
| `TestRustRuntime_terminal_tokens_are_scoped_expiring_and_one_use` | `noema-runtime` |
| `TestRustServer_blocked_tool_schema_restricts_gate_kinds` | `noema-server` |
| `TestRustAPI_delete_is_revision_fenced_and_removes_acp_setup_state` | `noema-api` |
| `TestRustAPI_delete_rejects_current_task_and_schedule_references` | `noema-api` |
| `TestRustAPI_acp_agent_setup_is_revision_fenced_and_never_exposes_credentials` | `noema-api` |
| `TestRustStore_acp_executors_upgrade_v28_preserves_provider_history_and_converges` | `noema-store` |
| `TestRustStore_acp_executor_resolves_launch_at_start_and_uses_task_directory_precedence` | `noema-store` |
| `TestRustStore_missing_or_disabled_acp_executors_are_rejected_before_capture` | `noema-store` |
| `TestRustStore_acp_permission_decisions_match_exactly_and_approvals_are_consumed_once` | `noema-store` |

`TestRetiredExecutorUpgradePreservesHistoryAndStopsExecution` checks the replacement contract.
It checks version-37 upgrades, fresh schema convergence, preserved evidence, paused recurrences, rejected executor selection, and provider recovery.
