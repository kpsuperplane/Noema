//! Opt-in model qualification support for the external Noema eval runner.

mod cases;
mod grade;
mod runner;
mod types;

pub use runner::run_provider_suite;
pub use types::{ModelEvalCaseResult, ModelEvalConfig, ModelEvalReport, ModelEvalToolCall};
