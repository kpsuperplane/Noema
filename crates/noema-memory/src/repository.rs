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

    /// Return the stable operation that failed.
    #[must_use]
    pub const fn operation(&self) -> &'static str {
        self.operation
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
    fn repository_trait_remains_dyn_compatible() {
        fn accepts_repository(_repository: Option<&dyn MemoryRepository>) {}

        accepts_repository(None);
    }

    #[test]
    fn repository_errors_do_not_expose_backend_details() {
        let error = MemoryRepositoryError::new(
            "load_memory_service_settings",
            MemoryRepositoryErrorKind::Unavailable,
        );

        assert_eq!(error.operation(), "load_memory_service_settings");
        assert_eq!(error.kind(), MemoryRepositoryErrorKind::Unavailable);
        assert_eq!(
            error.to_string(),
            "memory repository operation load_memory_service_settings failed (Unavailable)"
        );
    }
}
