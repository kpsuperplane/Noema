//! Request-scoped access to the configured memory service.

use std::{future::Future, pin::Pin, sync::Arc};

use crate::{MemoryOperationsHandle, MemoryRepositoryResult, MemoryServiceSettingsRecord};

/// One authoritative memory-service settings and operations snapshot.
#[derive(Debug, Clone)]
pub struct MemoryServiceSnapshot {
    settings: MemoryServiceSettingsRecord,
    operations: Option<MemoryOperationsHandle>,
}

impl MemoryServiceSnapshot {
    /// Create a resolved service snapshot.
    #[must_use]
    pub fn new(
        settings: MemoryServiceSettingsRecord,
        operations: Option<MemoryOperationsHandle>,
    ) -> Self {
        Self {
            settings,
            operations,
        }
    }

    /// Return the persisted settings that selected this snapshot.
    #[must_use]
    pub fn settings(&self) -> &MemoryServiceSettingsRecord {
        &self.settings
    }

    /// Return fixed operations selected by the same settings read.
    #[must_use]
    pub fn operations(&self) -> Option<&MemoryOperationsHandle> {
        self.operations.as_ref()
    }

    /// Consume the snapshot into its settings and fixed operations.
    #[must_use]
    pub fn into_parts(self) -> (MemoryServiceSettingsRecord, Option<MemoryOperationsHandle>) {
        (self.settings, self.operations)
    }
}

/// Boxed future returned by object-safe memory-service access.
pub type MemoryServiceAccessFuture<'a> =
    Pin<Box<dyn Future<Output = MemoryRepositoryResult<MemoryServiceSnapshot>> + Send + 'a>>;

/// Resolve the configured memory service once for each independent request.
pub trait MemoryServiceAccess: Send + Sync + std::fmt::Debug {
    /// Return one authoritative settings and operations snapshot.
    fn resolve(&self) -> MemoryServiceAccessFuture<'_>;
}

/// Clonable memory-service access handle.
pub type MemoryServiceAccessHandle = Arc<dyn MemoryServiceAccess>;
