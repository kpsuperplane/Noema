//! Persistence boundary for memory settings and article caches.

use std::{future::Future, pin::Pin, sync::Arc};

use thiserror::Error;

use crate::model::{
    MemoryArticleCacheRecord, MemoryServiceSettingsRecord, SaveMemoryArticleCache,
    SaveMemoryServiceSettings,
};

/// Boxed future returned by object-safe memory repository operations.
pub type MemoryRepositoryFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Coarse repository failure class available to memory orchestration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryRepositoryErrorKind {
    /// Durable memory settings or cache state violates an invariant.
    Invariant,
    /// The repository could not complete an operation.
    Unavailable,
}

/// Memory repository failure without backend-specific error exposure.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("memory repository operation {operation} failed ({kind:?})")]
pub struct MemoryRepositoryError {
    operation: &'static str,
    kind: MemoryRepositoryErrorKind,
}

impl MemoryRepositoryError {
    /// Construct a typed repository failure.
    #[must_use]
    pub const fn new(operation: &'static str, kind: MemoryRepositoryErrorKind) -> Self {
        Self { operation, kind }
    }

    /// Return the coarse failure class.
    #[must_use]
    pub const fn kind(&self) -> MemoryRepositoryErrorKind {
        self.kind
    }
}

/// Repository operation result.
pub type MemoryRepositoryResult<T> = Result<T, MemoryRepositoryError>;

/// Durable settings and article-cache operations required by memory services.
pub trait MemoryRepository: Send + Sync + std::fmt::Debug {
    /// Load the singleton memory service settings.
    fn memory_service_settings(
        &self,
    ) -> MemoryRepositoryFuture<'_, MemoryRepositoryResult<MemoryServiceSettingsRecord>>;

    /// Save and return the singleton memory service settings.
    fn save_memory_service_settings(
        &self,
        input: SaveMemoryServiceSettings,
    ) -> MemoryRepositoryFuture<'_, MemoryRepositoryResult<MemoryServiceSettingsRecord>>;

    /// Load an article cache record by governed memory scope.
    fn memory_article_cache(
        &self,
        scope_id: String,
    ) -> MemoryRepositoryFuture<'_, MemoryRepositoryResult<Option<MemoryArticleCacheRecord>>>;

    /// Save an article cache record.
    fn save_memory_article_cache(
        &self,
        input: SaveMemoryArticleCache,
    ) -> MemoryRepositoryFuture<'_, MemoryRepositoryResult<()>>;
}

/// Clonable memory repository handle.
pub type MemoryRepositoryHandle = Arc<dyn MemoryRepository>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_errors_do_not_expose_backend_details() {
        let error = MemoryRepositoryError::new(
            "load_memory_service_settings",
            MemoryRepositoryErrorKind::Unavailable,
        );

        assert_eq!(error.kind(), MemoryRepositoryErrorKind::Unavailable);
        assert_eq!(
            error.to_string(),
            "memory repository operation load_memory_service_settings failed (Unavailable)"
        );
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::sync::Mutex;

    use super::*;
    #[cfg(feature = "service")]
    use crate::MemoryServiceMode;

    #[derive(Debug)]
    pub(crate) struct FakeMemoryRepository {
        settings: Mutex<MemoryServiceSettingsRecord>,
    }

    impl FakeMemoryRepository {
        pub(crate) fn new(settings: MemoryServiceSettingsRecord) -> Self {
            Self {
                settings: Mutex::new(settings),
            }
        }

        pub(crate) fn replace_settings(&self, settings: MemoryServiceSettingsRecord) {
            *self.settings.lock().expect("settings lock") = settings;
        }

        #[cfg(feature = "service")]
        pub(crate) fn set_external_url(&self, base_url: String) {
            self.settings.lock().expect("settings lock").base_url = Some(base_url);
        }

        #[cfg(feature = "service")]
        pub(crate) fn set_managed(&self) {
            let mut settings = self.settings.lock().expect("settings lock");
            settings.mode = MemoryServiceMode::Managed;
            settings.base_url = None;
        }
    }

    impl MemoryRepository for FakeMemoryRepository {
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
                provider_instance_key: None,
                model_profile: input.model_profile,
                reasoning_effort: input.reasoning_effort,
            };
            self.replace_settings(settings.clone());
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
}
