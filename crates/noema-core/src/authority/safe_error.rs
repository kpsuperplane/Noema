//! Serializable transport-safe API errors and internal diagnostic boundary.

use serde::Serialize;
use std::error::Error;

use super::CorrelationId;

/// Stable machine-readable API error code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ApiErrorCode {
    /// Authentication or request authority is absent or invalid.
    Unauthenticated,
    /// The principal is authenticated but not authorized.
    Forbidden,
    /// The requested object is absent or intentionally hidden.
    NotFound,
    /// The request is structurally invalid.
    InvalidInput,
    /// The requested typed scope is not currently supported.
    UnsupportedScope,
    /// A bounded resource limit was reached.
    ResourceExhausted,
    /// An unexpected internal failure occurred.
    Internal,
}

impl ApiErrorCode {
    /// Return the stable transport spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unauthenticated => "UNAUTHENTICATED",
            Self::Forbidden => "FORBIDDEN",
            Self::NotFound => "NOT_FOUND",
            Self::InvalidInput => "INVALID_INPUT",
            Self::UnsupportedScope => "UNSUPPORTED_SCOPE",
            Self::ResourceExhausted => "RESOURCE_EXHAUSTED",
            Self::Internal => "INTERNAL",
        }
    }
}

/// The only error shape exposed through application transports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SafeApiError {
    /// Stable machine-readable code.
    pub code: ApiErrorCode,
    /// Bounded server-owned public message.
    pub public_message: &'static str,
    /// Correlation id shared with the internal diagnostic.
    pub correlation_id: CorrelationId,
}

impl SafeApiError {
    /// Construct an expected error from a server-owned static public message.
    #[must_use]
    pub const fn expected(
        code: ApiErrorCode,
        public_message: &'static str,
        correlation_id: CorrelationId,
    ) -> Self {
        Self {
            code,
            public_message,
            correlation_id,
        }
    }

    /// Construct a generic unexpected error without including its cause.
    #[must_use]
    pub const fn internal(correlation_id: CorrelationId) -> Self {
        Self::expected(
            ApiErrorCode::Internal,
            "An internal error occurred.",
            correlation_id,
        )
    }

    pub(super) fn missing_authority(correlation_id: CorrelationId) -> Self {
        Self::expected(
            ApiErrorCode::Unauthenticated,
            "Request authority is unavailable.",
            correlation_id,
        )
    }

    pub(super) fn forbidden(correlation_id: CorrelationId) -> Self {
        Self::expected(
            ApiErrorCode::Forbidden,
            "This action is not allowed.",
            correlation_id,
        )
    }
}

/// Stable category for one unexpected internal diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InternalErrorEventKind {
    /// GraphQL resolver failure.
    GraphqlResolver,
    /// HTTP request boundary failure.
    HttpRequest,
    /// Desktop command boundary failure.
    DesktopCommand,
}

/// Safe metadata paired with an internal error cause at the diagnostic boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InternalErrorDiagnostic {
    /// Stable event category.
    pub event_kind: InternalErrorEventKind,
    /// Correlation id also returned to the caller.
    pub correlation_id: CorrelationId,
}

/// Sink for unexpected internal failures.
pub trait InternalErrorReporter: Send + Sync {
    /// Report one internal failure. Implementations must keep causes out of transports.
    fn report(&self, diagnostic: &InternalErrorDiagnostic, error: &(dyn Error + 'static));
}

/// Report one unexpected failure and return its transport-safe representation.
#[must_use]
pub fn report_internal(
    error: &(dyn Error + 'static),
    correlation_id: CorrelationId,
    event_kind: InternalErrorEventKind,
    reporter: &dyn InternalErrorReporter,
) -> SafeApiError {
    reporter.report(
        &InternalErrorDiagnostic {
            event_kind,
            correlation_id: correlation_id.clone(),
        },
        error,
    );
    SafeApiError::internal(correlation_id)
}
