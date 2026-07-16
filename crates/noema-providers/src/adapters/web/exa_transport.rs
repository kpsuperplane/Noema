//! Shared production transport policy for Exa web adapters.

use reqwest::Client;
use std::time::Duration;

pub(super) const EXA_API_BASE_URL: &str = "https://api.exa.ai";
const EXA_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

pub(super) fn production_http_client() -> Result<Client, reqwest::Error> {
    http_client_with_timeout(EXA_REQUEST_TIMEOUT)
}

fn http_client_with_timeout(timeout: Duration) -> Result<Client, reqwest::Error> {
    Client::builder().timeout(timeout).build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn configured_client_enforces_request_timeout() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
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
            .expect_err("request should time out");

        assert!(error.is_timeout());
    }
}
