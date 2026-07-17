//! Memory repository port backed by the embedded SQLite store.

use noema_memory::{
    MemoryArticleCacheRecord, MemoryRepository, MemoryRepositoryError, MemoryRepositoryErrorKind,
    MemoryRepositoryFuture, MemoryRepositoryResult, MemoryServiceSettingsRecord,
    SaveMemoryArticleCache, SaveMemoryServiceSettings,
};

use super::{NoemaStore, StoreError};

impl MemoryRepository for NoemaStore {
    fn memory_service_settings(
        &self,
    ) -> MemoryRepositoryFuture<'_, MemoryRepositoryResult<MemoryServiceSettingsRecord>> {
        Box::pin(async move {
            NoemaStore::memory_service_settings(self)
                .await
                .map_err(|error| repository_error("load_memory_service_settings", &error))
        })
    }

    fn save_memory_service_settings(
        &self,
        input: SaveMemoryServiceSettings,
    ) -> MemoryRepositoryFuture<'_, MemoryRepositoryResult<MemoryServiceSettingsRecord>> {
        Box::pin(async move {
            NoemaStore::save_memory_service_settings(self, input)
                .await
                .map_err(|error| repository_error("save_memory_service_settings", &error))
        })
    }

    fn memory_article_cache(
        &self,
        scope_id: String,
    ) -> MemoryRepositoryFuture<'_, MemoryRepositoryResult<Option<MemoryArticleCacheRecord>>> {
        Box::pin(async move {
            NoemaStore::memory_article_cache(self, &scope_id)
                .await
                .map_err(|error| repository_error("load_memory_article_cache", &error))
        })
    }

    fn save_memory_article_cache(
        &self,
        input: SaveMemoryArticleCache,
    ) -> MemoryRepositoryFuture<'_, MemoryRepositoryResult<()>> {
        Box::pin(async move {
            NoemaStore::save_memory_article_cache(self, input)
                .await
                .map_err(|error| repository_error("save_memory_article_cache", &error))
        })
    }
}

fn repository_error(operation: &'static str, error: &StoreError) -> MemoryRepositoryError {
    let invalid_memory_vocabulary = matches!(
        error,
        StoreError::Sqlite(rusqlite::Error::FromSqlConversionFailure(_, _, source))
            if source.downcast_ref::<noema_memory::MemorySettingsError>().is_some()
    );
    let kind = if error.is_system_invariant() || invalid_memory_vocabulary {
        MemoryRepositoryErrorKind::Invariant
    } else {
        MemoryRepositoryErrorKind::Unavailable
    };
    MemoryRepositoryError::new(operation, kind)
}

#[cfg(test)]
mod tests {
    use noema_memory::{MemoryRepository, MemoryRepositoryErrorKind};

    #[tokio::test]
    async fn sqlite_store_implements_memory_repository_without_store_errors() {
        let store = crate::store::tests::test_store().await;
        let repository: &dyn MemoryRepository = &store;

        let settings = repository
            .memory_service_settings()
            .await
            .expect("memory settings");

        assert_eq!(settings.settings_id, "default");
    }

    #[test]
    fn store_invariants_map_to_typed_repository_kind() {
        let error = super::repository_error(
            "load_memory_service_settings",
            &crate::StoreError::InvariantViolation {
                message: "invalid row".to_string(),
            },
        );

        assert_eq!(error.kind(), MemoryRepositoryErrorKind::Invariant);
        assert!(!error.to_string().contains("invalid row"));
    }
}
