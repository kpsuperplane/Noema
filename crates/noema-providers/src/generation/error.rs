use thiserror::Error;

use super::GenerateActionItem;

/// Transport-neutral classification for provider I/O failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderTransportKind {
    /// A client or request could not be constructed.
    Configuration,
    /// A connection could not be established or was interrupted.
    Connection,
    /// A transport operation exceeded its deadline.
    Timeout,
    /// A request could not be sent completely.
    Request,
    /// A response could not be received or decoded completely.
    Response,
    /// The remote transport violated its protocol.
    Protocol,
    /// The adapter could not classify the transport failure more narrowly.
    Unknown,
}

/// Sanitized context attached to a provider transport failure.
///
/// This record deliberately excludes URLs, headers, query strings, bodies,
/// filesystem paths, and raw transport errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderTransportContext {
    /// Safe operation name, such as `build_client` or `send_generation`.
    pub operation: String,
    /// Numeric response status when it is safe and available.
    pub status_code: Option<u16>,
    /// Provider request id when it is safe and available.
    pub request_id: Option<String>,
}

/// Errors produced by model providers.
#[derive(Debug, Error)]
pub enum ProviderError {
    /// Required provider credentials were missing.
    #[error("missing credentials for {provider}: {credential}")]
    MissingCredentials {
        /// Provider name.
        provider: String,
        /// Missing credential name.
        credential: String,
    },

    /// The caller supplied an invalid request.
    #[error("invalid request: {message}")]
    InvalidRequest {
        /// Human-readable validation failure.
        message: String,
    },

    /// A provider transport failed before a valid response was available.
    #[error("{provider} provider transport failed ({kind:?}): {message}")]
    TransportFailure {
        /// Provider name.
        provider: String,
        /// Transport-neutral failure classification.
        kind: ProviderTransportKind,
        /// Sanitized failure detail.
        message: String,
        /// Sanitized operation and response metadata.
        context: ProviderTransportContext,
    },

    /// The provider returned a non-success API response.
    #[error("provider API error ({status}): {message}")]
    ApiError {
        /// HTTP or provider status code.
        status: u16,
        /// Provider error message.
        message: String,
        /// Provider request id when available.
        request_id: Option<String>,
    },

    /// The provider rejected the request because of rate limits.
    #[error("provider rate limit: {message}")]
    RateLimit {
        /// Provider rate-limit message.
        message: String,
        /// Provider request id when available.
        request_id: Option<String>,
    },

    /// The provider rejected credentials or authorization.
    #[error("provider authentication failed: {message}")]
    AuthenticationFailure {
        /// Provider authentication message.
        message: String,
        /// Provider request id when available.
        request_id: Option<String>,
    },

    /// The provider returned an invalid or unsupported response shape.
    #[error("malformed provider response: {message}")]
    MalformedResponse {
        /// Parse or validation failure.
        message: String,
    },

    /// The provider failed after completing some durable action items.
    #[error("{provider} provider returned partial output: {message}")]
    PartialResponse {
        /// Provider name.
        provider: String,
        /// Model identifier used by the provider.
        model: String,
        /// Failure message.
        message: String,
        /// Completed action items that should still be persisted for audit.
        output: Vec<GenerateActionItem>,
    },

    /// A provider-specific protocol failed.
    #[error("{provider} provider protocol error: {message}")]
    ProtocolError {
        /// Provider name.
        provider: String,
        /// Protocol failure message.
        message: String,
    },

    /// A provider operation timed out.
    #[error("{provider} provider timed out during {operation} after {seconds} seconds")]
    Timeout {
        /// Provider name.
        provider: String,
        /// Operation that timed out.
        operation: String,
        /// Timeout in seconds.
        seconds: u64,
    },

    /// The provider cannot currently be used.
    #[error("{provider} provider is unavailable: {message}")]
    ProviderUnavailable {
        /// Provider name.
        provider: String,
        /// Availability failure message.
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_error_formatting_has_no_raw_source_slot() {
        let raw_source = "request to https://example.test?api_key=SECRET failed";
        let error = ProviderError::TransportFailure {
            provider: "openai".to_string(),
            kind: ProviderTransportKind::Connection,
            message: "connection failed".to_string(),
            context: ProviderTransportContext {
                operation: "send_generation".to_string(),
                status_code: None,
                request_id: None,
            },
        };

        let formatted = format!("{error:?} {error}");
        assert!(!formatted.contains(raw_source));
        assert!(!formatted.contains("SECRET"));
        assert!(formatted.contains("send_generation"));
    }
}
