//! Shared production transport policy for Exa web adapters.

use reqwest::{Client, RequestBuilder};
use std::{fmt, time::Duration};

pub(super) const EXA_API_BASE_URL: &str = "https://api.exa.ai";
const EXA_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct ExaClient {
    base_url: String,
    api_key: String,
    http: Client,
}

impl ExaClient {
    pub fn new(api_key: String) -> Result<Self, reqwest::Error> {
        Ok(Self::injected(
            EXA_API_BASE_URL.to_string(),
            api_key,
            http_client_with_timeout(EXA_REQUEST_TIMEOUT)?,
        ))
    }

    pub fn with_client(base_url: String, api_key: String, http: Client) -> Self {
        Self::injected(base_url, api_key, http)
    }

    fn injected(base_url: String, api_key: String, http: Client) -> Self {
        Self {
            base_url,
            api_key,
            http,
        }
    }

    pub(super) fn post(&self, path: &str) -> RequestBuilder {
        self.http
            .post(format!("{}{path}", self.base_url.trim_end_matches('/')))
            .header("x-api-key", &self.api_key)
    }

    #[cfg(test)]
    pub(super) fn base_url(&self) -> &str {
        &self.base_url
    }
}

impl fmt::Debug for ExaClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExaClient")
            .field("base_url", &self.base_url)
            .field("api_key", &"[REDACTED]")
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
        let client = ExaClient::new("exa-secret".to_string()).expect("client");
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
