//! Provider-owned default executor model choices.

use noema_providers::{DEFAULT_OPENAI_MODEL, ReasoningEffort};

use crate::task::TaskComplexity;

/// One built-in executor choice supplied by a model provider.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ProviderDefaultTaskModel {
    pub(crate) complexity: TaskComplexity,
    pub(crate) label: &'static str,
    pub(crate) model_profile: &'static str,
    pub(crate) reasoning_effort: Option<ReasoningEffort>,
}

/// Return the ready-to-use task models owned by one provider family.
#[must_use]
pub(crate) fn provider_default_task_models(provider_kind: &str) -> Vec<ProviderDefaultTaskModel> {
    match provider_kind {
        "codex" => vec![
            ProviderDefaultTaskModel {
                complexity: TaskComplexity::Simple,
                label: "GPT-5.6-Luna · medium",
                model_profile: "gpt-5.6-luna",
                reasoning_effort: Some(ReasoningEffort::Medium),
            },
            ProviderDefaultTaskModel {
                complexity: TaskComplexity::Medium,
                label: "GPT-5.6-Luna · max",
                model_profile: "gpt-5.6-luna",
                reasoning_effort: Some(ReasoningEffort::XHigh),
            },
            ProviderDefaultTaskModel {
                complexity: TaskComplexity::Difficult,
                label: "GPT-5.6-Sol · high",
                model_profile: "gpt-5.6-sol",
                reasoning_effort: Some(ReasoningEffort::High),
            },
        ],
        "openai" => vec![
            ProviderDefaultTaskModel {
                complexity: TaskComplexity::Simple,
                label: "OpenAI default · medium",
                model_profile: DEFAULT_OPENAI_MODEL,
                reasoning_effort: Some(ReasoningEffort::Medium),
            },
            ProviderDefaultTaskModel {
                complexity: TaskComplexity::Medium,
                label: "OpenAI default · high",
                model_profile: DEFAULT_OPENAI_MODEL,
                reasoning_effort: Some(ReasoningEffort::High),
            },
            ProviderDefaultTaskModel {
                complexity: TaskComplexity::Difficult,
                label: "OpenAI default · max",
                model_profile: DEFAULT_OPENAI_MODEL,
                reasoning_effort: Some(ReasoningEffort::XHigh),
            },
        ],
        "foundation_local" => [
            TaskComplexity::Simple,
            TaskComplexity::Medium,
            TaskComplexity::Difficult,
        ]
        .into_iter()
        .map(|complexity| ProviderDefaultTaskModel {
            complexity,
            label: "Default on-device",
            model_profile: "default",
            reasoning_effort: None,
        })
        .collect(),
        _ => Vec::new(),
    }
}
