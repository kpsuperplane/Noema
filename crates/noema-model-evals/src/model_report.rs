use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ModelEvalConfig {
    pub(crate) model_id: String,
    pub(crate) model_path: PathBuf,
    pub(crate) runtime_root: PathBuf,
    pub(crate) context_window_tokens: u32,
    pub(crate) timeout_seconds: u64,
    pub(crate) startup_timeout_seconds: u64,
    pub(crate) run_resource_probe: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ModelEvalRuntimeMemory {
    pub(crate) metric: String,
    pub(crate) ready_bytes: u64,
    pub(crate) peak_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ModelEvalResourceProbe {
    pub(crate) target_input_tokens: u32,
    pub(crate) observed_input_tokens: Option<u64>,
    pub(crate) near_context_latency_ms: Option<u64>,
    pub(crate) steady_turns_requested: u32,
    pub(crate) steady_turns_completed: u32,
    pub(crate) post_turn_min_bytes: Option<u64>,
    pub(crate) post_turn_max_bytes: Option<u64>,
    pub(crate) failure: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct ModelEvalReport {
    pub(crate) model_id: String,
    pub(crate) llama_cpp_release: String,
    pub(crate) llama_cpp_commit: String,
    pub(crate) backend: Option<String>,
    pub(crate) runtime_load_ms: u64,
    pub(crate) runtime_memory: Option<ModelEvalRuntimeMemory>,
    pub(crate) resource_probe: Option<ModelEvalResourceProbe>,
    pub(crate) runtime_error: Option<String>,
    pub(crate) cases: Vec<noema_runtime::eval_support::RuntimeEvalCaseResult>,
    pub(crate) passed_cases: usize,
    pub(crate) total_cases: usize,
    pub(crate) passed_critical_cases: usize,
    pub(crate) total_critical_cases: usize,
}
