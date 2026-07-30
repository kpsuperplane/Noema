//! Opt-in runtime-sensitive model qualification fixtures and grading.

mod cases;
mod grade;
mod runner;
mod types;

pub use runner::{run_runtime_suite, run_runtime_suite_for_roles};
pub use types::{RuntimeEvalCaseResult, RuntimeEvalRole, RuntimeEvalToolCall};
