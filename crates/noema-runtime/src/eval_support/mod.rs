//! Opt-in runtime-sensitive model qualification fixtures and grading.

mod cases;
mod grade;
mod runner;
mod types;

/// Version of the production-derived fixture and grading contract.
pub const RUNTIME_EVAL_SUITE_VERSION: u32 = 1;

pub use runner::{run_runtime_suite, run_runtime_suite_for_roles};
pub use types::{RuntimeEvalCaseResult, RuntimeEvalRole, RuntimeEvalToolCall};
