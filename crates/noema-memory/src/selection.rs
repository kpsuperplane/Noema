//! Memory model selection derived from persisted memory settings.

use noema_providers::{
    ProviderRouteError, ProviderSelectionLoaderHandle, ProviderSelectionSnapshot,
    provider_selection_loader,
};

use crate::{model::MemoryServiceSettingsRecord, repository::MemoryRepositoryHandle};

/// Derive the provider-owned route selection for memory model work.
#[must_use]
pub fn memory_provider_selection(
    settings: &MemoryServiceSettingsRecord,
    default_provider_kind: &str,
) -> ProviderSelectionSnapshot {
    let provider_kind = settings
        .provider_kind
        .as_deref()
        .unwrap_or(default_provider_kind);
    let provider_account_id = settings
        .provider_account_id
        .clone()
        .unwrap_or_else(|| format!("provider_account:{provider_kind}:default"));
    match settings.model_profile.clone() {
        Some(model_profile) => ProviderSelectionSnapshot::explicit(
            provider_kind,
            provider_account_id,
            model_profile,
            settings.reasoning_effort,
            Some("memory_service_settings".to_string()),
        ),
        None => ProviderSelectionSnapshot::provider_default(
            provider_kind,
            provider_account_id,
            settings.reasoning_effort,
            Some("memory_service_settings".to_string()),
        ),
    }
}

/// Build a fresh-per-resolution selection loader from the memory repository.
#[must_use]
pub fn memory_provider_selection_loader(
    repository: MemoryRepositoryHandle,
    default_provider_kind: impl Into<String>,
) -> ProviderSelectionLoaderHandle {
    let default_provider_kind = default_provider_kind.into();
    provider_selection_loader(move || {
        let repository = repository.clone();
        let default_provider_kind = default_provider_kind.clone();
        Box::pin(async move {
            let settings = repository.memory_service_settings().await.map_err(|_| {
                ProviderRouteError::SelectionLoad {
                    operation: "load_memory_model_selection",
                }
            })?;
            Ok(memory_provider_selection(&settings, &default_provider_kind))
        })
    })
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use noema_providers::{ProviderSelectionMode, ReasoningEffort};

    use super::*;
    use crate::{
        model::{
            MemoryArticleCacheRecord, MemoryServiceMode, SaveMemoryArticleCache,
            SaveMemoryServiceSettings,
        },
        repository::{MemoryRepository, MemoryRepositoryFuture, MemoryRepositoryResult},
    };

    #[derive(Debug)]
    struct FakeRepository {
        settings: Mutex<MemoryServiceSettingsRecord>,
    }

    impl MemoryRepository for FakeRepository {
        fn memory_service_settings(
            &self,
        ) -> MemoryRepositoryFuture<'_, MemoryRepositoryResult<MemoryServiceSettingsRecord>>
        {
            let settings = self.settings.lock().expect("settings lock").clone();
            Box::pin(async move { Ok(settings) })
        }

        fn save_memory_service_settings(
            &self,
            input: SaveMemoryServiceSettings,
        ) -> MemoryRepositoryFuture<'_, MemoryRepositoryResult<MemoryServiceSettingsRecord>>
        {
            let settings = MemoryServiceSettingsRecord {
                settings_id: "default".to_string(),
                mode: input.mode,
                base_url: input.base_url,
                port: input.port,
                provider_account_id: input.provider_account_id,
                provider_kind: input.provider_kind,
                model_profile: input.model_profile,
                reasoning_effort: input.reasoning_effort,
            };
            *self.settings.lock().expect("settings lock") = settings.clone();
            Box::pin(async move { Ok(settings) })
        }

        fn memory_article_cache(
            &self,
            _scope_id: String,
        ) -> MemoryRepositoryFuture<'_, MemoryRepositoryResult<Option<MemoryArticleCacheRecord>>>
        {
            Box::pin(async { Ok(None) })
        }

        fn save_memory_article_cache(
            &self,
            _input: SaveMemoryArticleCache,
        ) -> MemoryRepositoryFuture<'_, MemoryRepositoryResult<()>> {
            Box::pin(async { Ok(()) })
        }
    }

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
            model_profile: model_profile.map(str::to_string),
            reasoning_effort: Some(ReasoningEffort::Low),
        }
    }

    #[tokio::test]
    async fn selection_loader_re_reads_repository_for_each_request() {
        let repository = Arc::new(FakeRepository {
            settings: Mutex::new(settings(Some("foundation_local"), Some("memory-v1"))),
        });
        let loader = memory_provider_selection_loader(repository.clone(), "codex");

        let first = loader.load_selection().await.expect("first selection");
        assert_eq!(first.provider_kind, "foundation_local");
        assert_eq!(first.model_profile.as_deref(), Some("memory-v1"));

        *repository.settings.lock().expect("settings lock") =
            settings(Some("codex"), Some("memory-v2"));
        let second = loader.load_selection().await.expect("second selection");

        assert_eq!(second.provider_kind, "codex");
        assert_eq!(second.model_profile.as_deref(), Some("memory-v2"));
        assert_eq!(
            second.selection_mode,
            ProviderSelectionMode::ExplicitProfile
        );
    }

    #[test]
    fn unset_model_uses_provider_default_selection() {
        let selection = memory_provider_selection(&settings(None, None), "codex");

        assert_eq!(selection.provider_kind, "codex");
        assert_eq!(
            selection.selection_mode,
            ProviderSelectionMode::ProviderDefault
        );
        assert_eq!(selection.model_profile, None);
    }
}
