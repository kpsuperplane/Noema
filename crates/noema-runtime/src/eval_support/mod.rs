//! Opt-in runtime-sensitive model qualification fixtures and grading.

mod cases;
mod grade;
mod runner;
mod types;

/// Version of the production-derived fixture and grading contract.
pub const RUNTIME_EVAL_SUITE_VERSION: u32 = 6;

/// Cases in this category exercise the shared OpenRouter/model protocol and
/// apply to every model setting evaluated for the candidate.
pub const OPENROUTER_PROTOCOL_CATEGORY: &str = "openrouter_protocol";

pub use runner::{
    run_runtime_case_for_roles, run_runtime_suite, run_runtime_suite_for_roles,
    runtime_eval_case_descriptors_for_roles,
};
pub use types::{
    RuntimeEvalCaseDescriptor, RuntimeEvalCaseResult, RuntimeEvalRole, RuntimeEvalToolCall,
};
