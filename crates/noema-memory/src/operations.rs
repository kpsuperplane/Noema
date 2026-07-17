//! Service-neutral memory operations consumed by the runtime and API.

use std::{future::Future, pin::Pin, sync::Arc};

use thiserror::Error;

use crate::model::{
    AddMemoryRequest, ListMemoriesRequest, ListMemoriesResponse, SearchMemoriesRequest,
    SearchMemoriesResponse,
};

/// Boxed future returned by object-safe memory operations.
pub type MemoryOperationFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, MemoryOperationError>> + Send + 'a>>;

/// Successful readiness result from the selected memory service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryServiceReadiness {
    /// Whether the service reports itself ready for memory operations.
    pub ready: bool,
}

/// Stable memory service lifecycle event kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryServiceEventKind {
    /// Service startup began.
    Starting,
    /// Service became ready.
    Ready,
    /// Service shutdown began.
    Stopping,
    /// Service stopped.
    Stopped,
    /// Service startup or readiness failed.
    Failed,
}

/// Service lifecycle event safe to publish across runtime/API boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryServiceEvent {
    /// Stable lifecycle transition.
    pub kind: MemoryServiceEventKind,
}

/// Fixed service-level failures safe for runtime and API decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum MemoryOperationError {
    /// No memory service is currently available.
    #[error("memory service is unavailable")]
    ServiceUnavailable,
    /// The service request timed out.
    #[error("memory service request timed out")]
    TimedOut,
    /// The service returned a response that could not be decoded.
    #[error("memory service returned an unreadable response")]
    UnreadableResponse,
    /// The request failed before the service returned a response.
    #[error("memory service request failed")]
    RequestFailed,
    /// The service rejected authentication.
    #[error("memory service rejected authentication")]
    AuthenticationRejected,
    /// The service returned a non-success status.
    #[error("memory service returned an unsuccessful status")]
    UnsuccessfulStatus(u16),
}

impl MemoryOperationError {
    /// Return the stable model/API-visible error code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::ServiceUnavailable => "service_unavailable",
            Self::TimedOut => "timeout",
            Self::UnreadableResponse => "decode_failed",
            Self::RequestFailed => "request_failed",
            Self::AuthenticationRejected => "auth_error",
            Self::UnsuccessfulStatus(_) => "http_error",
        }
    }
}

/// Operations implemented by the selected memory service backend.
pub trait MemoryOperations: Send + Sync + std::fmt::Debug {
    /// Check whether the selected memory service is ready.
    fn check_readiness(&self) -> MemoryOperationFuture<'_, MemoryServiceReadiness>;

    /// Submit one memory observation.
    fn add_memory(&self, request: AddMemoryRequest) -> MemoryOperationFuture<'_, ()>;

    /// Search memory records.
    fn search_memories(
        &self,
        request: SearchMemoriesRequest,
    ) -> MemoryOperationFuture<'_, SearchMemoriesResponse>;

    /// List memory records for one user scope.
    fn list_memories(
        &self,
        request: ListMemoriesRequest,
    ) -> MemoryOperationFuture<'_, ListMemoriesResponse>;
}

/// Clonable memory operations handle.
pub type MemoryOperationsHandle = Arc<dyn MemoryOperations>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operations_trait_remains_dyn_compatible() {
        fn accepts_operations(_operations: Option<&dyn MemoryOperations>) {}

        accepts_operations(None);
    }

    #[test]
    fn readiness_and_lifecycle_events_stay_backend_neutral() {
        assert!(MemoryServiceReadiness { ready: true }.ready);
        assert_eq!(
            MemoryServiceEvent {
                kind: MemoryServiceEventKind::Ready,
            }
            .kind,
            MemoryServiceEventKind::Ready
        );
    }

    #[test]
    fn operation_errors_preserve_existing_safe_codes_and_messages() {
        let cases = [
            (
                MemoryOperationError::ServiceUnavailable,
                "service_unavailable",
                "memory service is unavailable",
            ),
            (
                MemoryOperationError::TimedOut,
                "timeout",
                "memory service request timed out",
            ),
            (
                MemoryOperationError::UnreadableResponse,
                "decode_failed",
                "memory service returned an unreadable response",
            ),
            (
                MemoryOperationError::RequestFailed,
                "request_failed",
                "memory service request failed",
            ),
            (
                MemoryOperationError::AuthenticationRejected,
                "auth_error",
                "memory service rejected authentication",
            ),
            (
                MemoryOperationError::UnsuccessfulStatus(500),
                "http_error",
                "memory service returned an unsuccessful status",
            ),
        ];

        for (error, code, message) in cases {
            assert_eq!(error.code(), code);
            assert_eq!(error.to_string(), message);
        }
    }
}
