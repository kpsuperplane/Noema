use noema_providers::{
    ModelPreferenceSelection, NoemaModelUseCase, ProviderKind, ProviderSelectionSnapshot,
    noema_model_recommendation,
};

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
    pub selection: ModelPreferenceSelection,
    pub fast_mode: bool,
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
        if provider_account_id.is_empty() {
            return Err(invalid_input(
                "task_model_pool.provider_account_id",
                "value cannot be blank",
            ));
        }
        let provider = provider_kind
            .parse::<ProviderKind>()
            .map_err(|_| invalid_input("task_model_pool.provider_kind", "unsupported provider"))?;
        let selection = match &self.selection {
            ModelPreferenceSelection::NoemaRecommended => {
                if noema_model_recommendation(provider, model_use_case(self.complexity)).is_none() {
                    return Err(invalid_input(
                        "task_model_pool.selection",
                        "provider has no Noema recommendation",
                    ));
                }
                ModelPreferenceSelection::NoemaRecommended
            }
            ModelPreferenceSelection::ExplicitProfile {
                model_profile,
                reasoning_effort,
            } => {
                let model = ProviderSelectionSnapshot::explicit(
                    provider_kind.clone(),
                    provider_account_id.clone(),
                    model_profile,
                    *reasoning_effort,
                    Some("task_model_pool".to_string()),
                )
                .normalized()
                .map_err(|error| invalid_input("task_model_pool.model", error.to_string()))?;
                ModelPreferenceSelection::ExplicitProfile {
                    model_profile: model.model_profile.unwrap_or_default(),
                    reasoning_effort: model.reasoning_effort,
                }
            }
        };
        Ok(Self {
            pool_entry_id: normalize_optional(self.pool_entry_id.as_deref()),
            complexity: self.complexity,
            label: normalize_optional(self.label.as_deref()),
            provider_kind,
            provider_account_id,
            selection,
            fast_mode: self.fast_mode,
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
    pub preference: ModelPreferenceSelection,
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

/// Map one task tier to its recommendation workload.
#[must_use]
pub const fn model_use_case(complexity: TaskComplexity) -> NoemaModelUseCase {
    match complexity {
        TaskComplexity::Simple => NoemaModelUseCase::TaskSimple,
        TaskComplexity::Medium => NoemaModelUseCase::TaskMedium,
        TaskComplexity::Difficult => NoemaModelUseCase::TaskDifficult,
    }
}
