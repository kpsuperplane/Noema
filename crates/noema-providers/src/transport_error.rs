use crate::{ProviderError, ProviderTransportContext, ProviderTransportKind};

pub(crate) fn reqwest_transport_error(
    provider: &str,
    operation: &'static str,
    source: &reqwest::Error,
) -> ProviderError {
    let kind = if source.is_timeout() {
        ProviderTransportKind::Timeout
    } else if source.is_connect() {
        ProviderTransportKind::Connection
    } else if source.is_builder() {
        ProviderTransportKind::Configuration
    } else if source.is_request() || source.is_body() {
        ProviderTransportKind::Request
    } else if source.is_decode() {
        ProviderTransportKind::Response
    } else {
        ProviderTransportKind::Unknown
    };
    let message = match kind {
        ProviderTransportKind::Configuration => "transport configuration failed",
        ProviderTransportKind::Connection => "connection failed",
        ProviderTransportKind::Timeout => "transport operation timed out",
        ProviderTransportKind::Request => "request transport failed",
        ProviderTransportKind::Response => "response transport failed",
        ProviderTransportKind::Protocol => "transport protocol failed",
        ProviderTransportKind::Unknown => "transport operation failed",
    };

    ProviderError::TransportFailure {
        provider: provider.to_string(),
        kind,
        message: message.to_string(),
        context: ProviderTransportContext {
            operation: operation.to_string(),
            status_code: source.status().map(|status| status.as_u16()),
            request_id: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapped_error_never_formats_the_source_url() {
        let source = reqwest::Client::new()
            .get("http://[::1?api_key=SECRET")
            .build()
            .expect_err("invalid URL");
        let error = reqwest_transport_error("openai", "build_request", &source);
        let formatted = format!("{error:?} {error}");

        assert!(!formatted.contains("SECRET"));
        assert!(!formatted.contains("api_key"));
        assert!(formatted.contains("build_request"));
    }
}
