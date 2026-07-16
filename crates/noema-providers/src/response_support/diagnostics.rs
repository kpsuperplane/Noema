use noema_home::{SystemErrorEvent, SystemErrorLogger};
use serde_json::Value;

use crate::ProviderError;

/// Diagnostic category for malformed provider responses.
pub const SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE: &str = "provider_malformed_response";

/// Redacted diagnostic context for structured provider responses.
#[derive(Debug, Clone)]
pub struct StructuredResponseDiagnosticContext {
    /// Developer diagnostic logger.
    pub logger: Option<SystemErrorLogger>,
    /// Provider kind.
    pub provider_kind: String,
    /// Model requested by Noema.
    pub model: String,
    /// Noema conversation id, when available.
    pub conversation_id: Option<String>,
}

impl StructuredResponseDiagnosticContext {
    /// Build a provider diagnostic context.
    #[must_use]
    pub fn new(
        logger: Option<SystemErrorLogger>,
        provider_kind: impl Into<String>,
        model: impl Into<String>,
        conversation_id: Option<String>,
    ) -> Self {
        Self {
            logger,
            provider_kind: provider_kind.into(),
            model: model.into(),
            conversation_id,
        }
    }

    fn context_json(&self, request_id: Option<&str>) -> Value {
        serde_json::json!({
            "provider_kind": self.provider_kind,
            "model": self.model,
            "conversation_id": self.conversation_id,
            "request_id": request_id,
        })
    }

    /// Record a malformed structured response without exposing credentials.
    pub fn log_malformed(&self, message: impl Into<String>, raw: Value) {
        self.log_malformed_with_request_id(message, None, raw);
    }

    /// Record a malformed structured response with an upstream request id.
    pub fn log_malformed_with_request_id(
        &self,
        message: impl Into<String>,
        request_id: Option<&str>,
        raw: Value,
    ) {
        if let Some(logger) = &self.logger {
            let message = message.into();
            logger.try_append(
                SystemErrorEvent::new(SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE, message.clone())
                    .with_context(self.context_json(request_id))
                    .with_error_chain([message])
                    .with_raw(raw),
            );
        }
    }

    /// Record a malformed provider error with request context.
    pub fn log_malformed_error(&self, error: &ProviderError, request_id: Option<&str>, raw: Value) {
        if let Some(logger) = &self.logger {
            logger.try_append(
                SystemErrorEvent::new(SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE, error.to_string())
                    .with_context(self.context_json(request_id))
                    .with_error_chain([error.to_string()])
                    .with_raw(raw),
            );
        }
    }
}
