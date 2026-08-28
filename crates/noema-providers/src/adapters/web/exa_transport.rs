//! Shared production transport policy for Exa web adapters.

use crate::ProviderCredential;
use reqwest::{Client, RequestBuilder};
use std::{fmt, time::Duration};
pub(super) const EXA_API_BASE_URL: &str = "https://api.exa.ai";
const EXA_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Exa client shared by its search and fetch implementations.
#[derive(Clone)]
pub struct ExaWebClient {
    base_url: String,
    credential: ProviderCredential,
    http: Client,
}
impl ExaWebClient {
    /// Build an Exa client for one credential.
    ///
    /// # Errors
    ///
    /// Returns an error when the HTTP client cannot be configured.
    pub fn new(credential: ProviderCredential) -> Result<Self, reqwest::Error> {
        Ok(Self::injected(
            EXA_API_BASE_URL.to_string(),
            credential,
            http_client_with_timeout(EXA_REQUEST_TIMEOUT)?,
        ))
    }
    #[cfg(test)]
    pub(super) fn with_client(
        base_url: String,
        credential: ProviderCredential,
        http: Client,
    ) -> Self {
        Self::injected(base_url, credential, http)
    }
    fn injected(base_url: String, credential: ProviderCredential, http: Client) -> Self {
        Self {
            base_url,
            credential,
            http,
        }
    }
    pub(super) fn post(&self, path: &str) -> RequestBuilder {
        self.http
            .post(format!("{}{path}", self.base_url.trim_end_matches('/')))
            .header("x-api-key", self.credential.expose_secret())
    }
    #[cfg(test)]
    pub(super) fn base_url(&self) -> &str {
        &self.base_url
    }
}
impl fmt::Debug for ExaWebClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExaWebClient")
            .field("base_url", &self.base_url)
            .field("credential", &"[REDACTED]")
            .field("http", &"[CONFIGURED]")
            .finish()
    }
}
fn http_client_with_timeout(timeout: Duration) -> Result<Client, reqwest::Error> {
    Client::builder().timeout(timeout).build()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn production_transport_owns_endpoint_redaction_and_timeout() {
        let client = ExaWebClient::new("exa-secret".to_string().into()).expect("client");
        assert_eq!(client.base_url(), EXA_API_BASE_URL);
        let debug = format!("{client:?}");
        assert!(!debug.contains("exa-secret"));
        assert!(debug.contains("[REDACTED]"));

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("local addr");
        tokio::spawn(async move {
            let (_socket, _) = listener.accept().await.expect("accept");
            tokio::time::sleep(Duration::from_millis(250)).await;
        });
        let client = http_client_with_timeout(Duration::from_millis(25)).expect("client");
        let error = client
            .get(format!("http://{addr}"))
            .send()
            .await
            .expect_err("configured request timeout");
        assert!(error.is_timeout());
    }
}
