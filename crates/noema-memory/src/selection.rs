//! Memory model selection derived from persisted memory settings.

use noema_providers::{
    ProviderRouteError, ProviderSelectionLoaderHandle, ProviderSelectionSnapshot,
    provider_selection_loader,
};

use crate::{model::MemoryServiceSettingsRecord, repository::MemoryRepositoryHandle};

/// Derive the provider-owned route selection for memory model work.
///
/// # Errors
///
/// Returns [`ProviderRouteError::SelectionLoad`] until initialization has
/// persisted a complete exact memory selection.
pub fn memory_provider_selection(
    settings: &MemoryServiceSettingsRecord,
) -> Result<ProviderSelectionSnapshot, ProviderRouteError> {
    let missing = || ProviderRouteError::SelectionLoad {
        operation: "load_memory_model_selection",
    };
    let provider_kind = settings.provider_kind.as_deref().ok_or_else(missing)?;
    let provider_account_id = settings
        .provider_account_id
        .as_deref()
        .ok_or_else(missing)?;
    let model_profile = settings.model_profile.as_deref().ok_or_else(missing)?;
    let provider_instance_key = settings.provider_instance_key.clone().ok_or_else(missing)?;
    let mut selection = ProviderSelectionSnapshot::explicit(
        provider_kind,
        provider_account_id,
        model_profile,
        settings.reasoning_effort,
        Some("memory_service_settings".to_string()),
    );
    selection.provider_instance_key = Some(provider_instance_key);
    selection
        .normalized_for_persistence()
        .map_err(|_| ProviderRouteError::SelectionLoad {
            operation: "load_memory_model_selection",
        })
}

/// Build a fresh-per-resolution selection loader from the memory repository.
#[must_use]
pub fn memory_provider_selection_loader(
    repository: MemoryRepositoryHandle,
) -> ProviderSelectionLoaderHandle {
    provider_selection_loader(move || {
        let repository = repository.clone();
        Box::pin(async move {
            let settings = repository.memory_service_settings().await.map_err(|_| {
                ProviderRouteError::SelectionLoad {
                    operation: "load_memory_model_selection",
                }
            })?;
            memory_provider_selection(&settings)
        })
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use noema_providers::{ProviderSelectionMode, ReasoningEffort, provider_account_instance_key};

    use super::*;
    use crate::{model::MemoryServiceMode, repository::test_support::FakeMemoryRepository};

    fn settings(
        provider_kind: Option<&str>,
        model_profile: Option<&str>,
    ) -> MemoryServiceSettingsRecord {
        MemoryServiceSettingsRecord {
            settings_id: "default".to_string(),
            mode: MemoryServiceMode::Managed,
            base_url: None,
            port: None,
            provider_account_id: provider_kind
                .map(|kind| format!("provider_account:{kind}:default")),
            provider_kind: provider_kind.map(str::to_string),
            provider_instance_key: provider_kind.map(|kind| {
                provider_account_instance_key(&format!("provider_account:{kind}:default"))
                    .expect("provider key")
            }),
            model_profile: model_profile.map(str::to_string),
            reasoning_effort: Some(ReasoningEffort::Low),
        }
    }

    #[tokio::test]
    async fn selection_loader_re_reads_repository_and_rejects_unset_model() {
        let repository = Arc::new(FakeMemoryRepository::new(settings(
            Some("foundation_local"),
            Some("memory-v1"),
        )));
        let loader = memory_provider_selection_loader(repository.clone());

        let first = loader.load_selection().await.expect("first selection");
        assert_eq!(first.provider_kind, "foundation_local");
        assert_eq!(first.model_profile.as_deref(), Some("memory-v1"));

        repository.replace_settings(settings(Some("codex"), Some("memory-v2")));
        let second = loader.load_selection().await.expect("second selection");

        assert_eq!(second.provider_kind, "codex");
        assert_eq!(second.model_profile.as_deref(), Some("memory-v2"));
        assert_eq!(
            second.selection_mode,
            ProviderSelectionMode::ExplicitProfile
        );
        let error = memory_provider_selection(&settings(None, None))
            .expect_err("missing initialized memory selection");

        assert!(matches!(
            error,
            ProviderRouteError::SelectionLoad {
                operation: "load_memory_model_selection"
            }
        ));
    }
}
