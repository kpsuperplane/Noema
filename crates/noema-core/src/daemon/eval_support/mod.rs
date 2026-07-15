//! Opt-in model qualification support for the external Noema eval runner.

mod cases;
mod grade;
mod memory;
mod resource_probe;
mod runner;
mod types;

pub use runner::run_provider_suite;
pub use types::{
    ModelEvalCaseResult, ModelEvalConfig, ModelEvalReport, ModelEvalResourceProbe,
    ModelEvalRuntimeMemory, ModelEvalToolCall,
};
