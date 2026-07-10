use async_graphql::ErrorExtensions;

use crate::authority::SafeApiError;

impl From<SafeApiError> for async_graphql::Error {
    fn from(error: SafeApiError) -> Self {
        async_graphql::Error::new(error.public_message).extend_with(|_, extensions| {
            extensions.set("code", error.code.as_str());
            extensions.set("correlationId", error.correlation_id.as_str());
        })
    }
}

pub(super) fn graphql_error(error: impl std::fmt::Display) -> async_graphql::Error {
    async_graphql::Error::new(error.to_string())
}

#[cfg(test)]
mod tests {
    use std::{
        error::Error,
        fmt,
        sync::atomic::{AtomicUsize, Ordering},
    };

    use async_graphql::Value;

    use crate::authority::{
        ApiErrorCode, CorrelationId, InternalErrorDiagnostic, InternalErrorEventKind,
        InternalErrorReporter, SafeApiError, report_internal,
    };

    #[derive(Debug)]
    struct CanaryError;

    impl fmt::Display for CanaryError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("SECRET_CANARY authorization=Bearer-token")
        }
    }

    impl Error for CanaryError {}

    struct CountingReporter(AtomicUsize);

    impl InternalErrorReporter for CountingReporter {
        fn report(&self, diagnostic: &InternalErrorDiagnostic, _error: &(dyn Error + 'static)) {
            assert_eq!(
                diagnostic.event_kind,
                InternalErrorEventKind::GraphqlResolver
            );
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn safe_graphql_extensions_match_reusable_serialization() {
        let correlation_id = CorrelationId::generate().expect("correlation id");
        let safe = SafeApiError::expected(
            ApiErrorCode::InvalidInput,
            "The request is invalid.",
            correlation_id.clone(),
        );
        let body = serde_json::to_value(&safe).expect("serialize body");
        let graphql: async_graphql::Error = safe.into();
        let extensions = graphql.extensions.expect("extensions");

        assert_eq!(graphql.message, body["publicMessage"]);
        assert_eq!(extensions.get("code"), Some(&Value::from("INVALID_INPUT")));
        assert_eq!(
            extensions.get("correlationId"),
            Some(&Value::from(correlation_id.as_str()))
        );
        assert_eq!(body["code"], "INVALID_INPUT");
        assert_eq!(body["correlationId"], correlation_id.as_str());
    }

    #[test]
    fn internal_errors_are_generic_do_not_leak_and_report_once() {
        let reporter = CountingReporter(AtomicUsize::new(0));
        let correlation_id = CorrelationId::generate().expect("correlation id");
        let safe = report_internal(
            &CanaryError,
            correlation_id.clone(),
            InternalErrorEventKind::GraphqlResolver,
            &reporter,
        );
        let body = serde_json::to_string(&safe).expect("serialize body");
        let graphql: async_graphql::Error = safe.into();
        let transport = format!("{body} {}", graphql.message);

        assert_eq!(reporter.0.load(Ordering::SeqCst), 1);
        assert_eq!(graphql.message, "An internal error occurred.");
        assert!(!transport.contains("SECRET_CANARY"));
        assert!(!transport.contains("Bearer-token"));
        assert!(transport.contains(correlation_id.as_str()));
    }
}
