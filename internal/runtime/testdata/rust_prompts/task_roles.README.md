# Task prompt reference

`task_roles.json` contains complete outputs from Rust commit `4d29f6ba1f70a30b5959a0e460217feeeb5e8c04`.

The source functions are `format_planner_prompt`, `format_executor_prompt`, and `format_reviewer_prompt` in `crates/noema-runtime/src/daemon/task_run_context.rs`.
Each input includes the persistence suffix from `build_task_role_prompt`.
The finalization outputs come from `build_task_finalization_prompt` in `crates/noema-runtime/src/daemon/runtime/task_continuation.rs`.

The fixed inputs use task `task:reference`, title `Exact task`, and source `Human source request`.
The workspace is `Personal` with an empty description.
The project name is `Project café`, and its description is `日本語`.
The source environment is explicit fixture data, with date `2026-08-12`, time `12:00:00`, and timezone `UTC`.
Finalization uses reason `ceiling` and original input `original input`.

Expected strings were extracted and expanded from Rust source without calling Go prompt builders.
Do not regenerate this reference from Go output.
