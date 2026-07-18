use noema_providers::{DEFAULT_OPENAI_MODEL, ProviderSelectionSnapshot, ReasoningEffort};

use crate::{TaskComplexity, WorkDomainError, error::invalid_input};

const TASK_MODEL_POOL_SETTING_PREFIX: &str = "task_pool:setting:";

/// Input for creating or updating one executor pool entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewTaskModelPoolEntry {
    /// Optional stable id; one is allocated when absent.
    pub pool_entry_id: Option<String>,
    /// Complexity tier exposed to the primary agent.
    pub complexity: TaskComplexity,
    /// Optional human-facing label.
    pub label: Option<String>,
    /// Provider family for the exact model profile.
    pub provider_kind: String,
    /// Active provider account that owns the model profile.
    pub provider_account_id: String,
    /// Exact provider model/profile.
    pub model_profile: String,
    /// Optional explicit reasoning effort.
    pub reasoning_effort: Option<ReasoningEffort>,
    /// Whether the entry can be selected for new tasks.
    pub enabled: bool,
    /// Human-controlled ordering within a complexity tier.
    pub sort_order: i64,
}

impl NewTaskModelPoolEntry {
    /// Normalize provider/model identity before repository validation.
    ///
    /// # Errors
    ///
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

fn normalize_optional(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

/// Persisted executor model pool entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskModelPoolEntry {
    /// Stable pool-entry id.
    pub pool_entry_id: String,
    /// Complexity tier exposed to the primary agent.
    pub complexity: TaskComplexity,
    /// Optional human-facing label.
    pub label: Option<String>,
    /// Immutable model snapshot selected by this entry.
    pub model: ProviderSelectionSnapshot,
    /// Whether the entry can be selected for new tasks.
    pub enabled: bool,
    /// Human-controlled ordering within a complexity tier.
    pub sort_order: i64,
    /// Creation timestamp.
    pub created_at: String,
    /// Last update timestamp.
    pub updated_at: String,
}

/// Return whether an id belongs to one of the three global executor settings.
#[must_use]
pub fn is_global_task_model_pool_setting_id(pool_entry_id: &str) -> bool {
    pool_entry_id.starts_with(TASK_MODEL_POOL_SETTING_PREFIX)
}

/// One built-in executor choice supplied by a model provider.
#[derive(Debug, Clone, Copy)]
pub struct ProviderDefaultTaskModel {
    /// Complexity tier initialized by this choice.
    pub complexity: TaskComplexity,
    /// Stable human-facing label.
    pub label: &'static str,
    /// Provider model/profile.
    pub model_profile: &'static str,
    /// Optional explicit reasoning effort.
    pub reasoning_effort: Option<ReasoningEffort>,
}

/// Return the ready-to-use task models owned by one provider family.
#[must_use]
pub fn provider_default_task_models(provider_kind: &str) -> Vec<ProviderDefaultTaskModel> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_default_model_pools_cover_each_supported_tier() {
        for provider in ["codex", "openai", "foundation_local"] {
            let defaults = provider_default_task_models(provider);
            assert_eq!(defaults.len(), 3);
            assert_eq!(defaults[0].complexity, TaskComplexity::Simple);
            assert_eq!(defaults[1].complexity, TaskComplexity::Medium);
            assert_eq!(defaults[2].complexity, TaskComplexity::Difficult);
        }
        assert!(provider_default_task_models("unknown").is_empty());
    }

    #[test]
    fn model_pool_input_normalizes_provider_identity_and_rejects_blanks() {
        let input = NewTaskModelPoolEntry {
            pool_entry_id: Some(" pool:1 ".to_string()),
            complexity: TaskComplexity::Simple,
            label: Some(" Fast ".to_string()),
            provider_kind: " OPENAI ".to_string(),
            provider_account_id: " provider_account:openai:default ".to_string(),
            model_profile: " gpt-5.5 ".to_string(),
            reasoning_effort: None,
            enabled: true,
            sort_order: 0,
        };
        let normalized = input.normalized().expect("normalized pool input");
        assert_eq!(normalized.provider_kind, "openai");
        assert_eq!(normalized.pool_entry_id.as_deref(), Some("pool:1"));
        assert_eq!(normalized.label.as_deref(), Some("Fast"));

        assert!(
            NewTaskModelPoolEntry {
                provider_account_id: " ".to_string(),
                ..input
            }
            .normalized()
            .is_err()
        );
    }
}
