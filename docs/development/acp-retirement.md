# Retired ACP execution

Noema does not launch, authenticate, configure, or run ACP executors.
Go migration 38 removes ACP configuration and authentication tables.
It stops unfinished ACP Tasks, closes their active items and requests, and pauses associated recurrences.
It preserves Task history and completed results. Provider Tasks retain their state.

Stored ACP placement markers remain only to reject unsupported execution.
They cannot silently select a provider. Remove them when supported stored objects no longer need that check.
Older immutable migrations retain the original table definitions.

`TestRetiredExecutorUpgradePreservesHistoryAndStopsExecution` covers version-37 upgrades and fresh-schema convergence.
It also checks preserved evidence, paused recurrences, rejected executor selection, and provider recovery.
ACP-only Rust parity tests were retired with that capability; mixed tests retain their provider assertions.

Source: [retirement migration and tests](../../internal/store/retired_executor_test.go).
