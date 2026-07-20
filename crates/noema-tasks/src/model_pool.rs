use noema_providers::{DEFAULT_OPENAI_MODEL, ProviderSelectionSnapshot, ReasoningEffort};

use crate::{
    TaskComplexity, WorkDomainError, error::invalid_input,
    validation::optional as normalize_optional,
};

const TASK_MODEL_POOL_SETTING_PREFIX: &str = "task_pool:setting:";

/// Input for creating or updating one executor pool entry.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct NewTaskModelPoolEntry {
    pub pool_entry_id: Option<String>,
    pub complexity: TaskComplexity,
    pub label: Option<String>,
    pub provider_kind: String,
    pub provider_account_id: String,
    pub model_profile: String,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub enabled: bool,
    pub sort_order: i64,
}

impl NewTaskModelPoolEntry {
    /// Normalize provider/model identity before repository validation.
    /// # Errors
    /// Returns a task-domain error for blank account/model fields or a malformed
    /// provider selection.
    pub fn normalized(&self) -> Result<Self, WorkDomainError> {
        let provider_kind = self.provider_kind.trim().to_ascii_lowercase();
        let provider_account_id = self.provider_account_id.trim().to_string();
        let model_profile = self.model_profile.trim().to_string();
        if provider_account_id.is_empty() {
            return Err(invalid_input(
                "task_model_pool.provider_account_id",
                "value cannot be blank",
            ));
        }
        if model_profile.is_empty() {
            return Err(invalid_input(
                "task_model_pool.model_profile",
                "value cannot be blank",
            ));
        }
        let model = ProviderSelectionSnapshot::explicit(
            provider_kind.clone(),
            provider_account_id.clone(),
            model_profile.clone(),
            self.reasoning_effort,
            Some("task_model_pool".to_string()),
        )
        .normalized()
        .map_err(|error| invalid_input("task_model_pool.model", error.to_string()))?;
        Ok(Self {
            pool_entry_id: normalize_optional(self.pool_entry_id.as_deref()),
            complexity: self.complexity,
            label: normalize_optional(self.label.as_deref()),
            provider_kind,
            provider_account_id,
            model_profile,
            reasoning_effort: model.reasoning_effort,
            enabled: self.enabled,
            sort_order: self.sort_order,
        })
    }
}

/// Persisted executor model pool entry.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct TaskModelPoolEntry {
    pub pool_entry_id: String,
    pub complexity: TaskComplexity,
    pub label: Option<String>,
    pub model: ProviderSelectionSnapshot,
    pub is_override: bool,
    pub enabled: bool,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// Return whether an id belongs to one of the three global executor settings.
#[must_use]
pub fn is_global_task_model_pool_setting_id(pool_entry_id: &str) -> bool {
    pool_entry_id.starts_with(TASK_MODEL_POOL_SETTING_PREFIX)
}

/// One built-in executor choice supplied by a model provider.
#[derive(Debug, Clone, Copy)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct ProviderDefaultTaskModel {
    pub complexity: TaskComplexity,
    pub label: &'static str,
    pub model_profile: &'static str,
    pub reasoning_effort: Option<ReasoningEffort>,
}

/// Return the ready-to-use task models owned by one provider family.
#[must_use]
pub fn provider_default_task_models(provider_kind: &str) -> Vec<ProviderDefaultTaskModel> {
    match provider_kind {
        #[rustfmt::skip]
        "codex" => defaults([
            ("GPT-5.6-Luna · medium", "gpt-5.6-luna", ReasoningEffort::Medium),
            ("GPT-5.6-Luna · max", "gpt-5.6-luna", ReasoningEffort::XHigh),
            ("GPT-5.6-Sol · high", "gpt-5.6-sol", ReasoningEffort::High),
        ]),
        #[rustfmt::skip]
        "openai" => defaults([
            ("OpenAI default · medium", DEFAULT_OPENAI_MODEL, ReasoningEffort::Medium),
            ("OpenAI default · high", DEFAULT_OPENAI_MODEL, ReasoningEffort::High),
            ("OpenAI default · max", DEFAULT_OPENAI_MODEL, ReasoningEffort::XHigh),
        ]),
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

fn defaults<const N: usize>(
    values: [(&'static str, &'static str, ReasoningEffort); N],
) -> Vec<ProviderDefaultTaskModel> {
    [
        TaskComplexity::Simple,
        TaskComplexity::Medium,
        TaskComplexity::Difficult,
    ]
    .into_iter()
    .zip(values)
    .map(
        |(complexity, (label, model_profile, reasoning_effort))| ProviderDefaultTaskModel {
            complexity,
            label,
            model_profile,
            reasoning_effort: Some(reasoning_effort),
        },
    )
    .collect()
}
