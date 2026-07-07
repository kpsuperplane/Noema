//! Exa hosted web search provider.

use crate::search::types::{SearchError, SearchRequest, SearchResponse, SearchResult};
use serde::Serialize;
use serde_json::Value;

pub(crate) const EXA_SEARCH_PROVIDER_ID: &str = "exa";
pub(crate) const EXA_SEARCH_CONTRACT: &str = "hosted_provider";

#[derive(Debug, Clone)]
pub struct ExaSearchClient {
    pub base_url: String,
    pub api_key: String,
    pub http: reqwest::Client,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExaSearchRequest<'a> {
    query: &'a str,
    num_results: usize,
}

pub(crate) async fn search_exa(
    client: &ExaSearchClient,
    request: &SearchRequest,
) -> Result<SearchResponse, SearchError> {
    let response = client
        .http
        .post(format!("{}/search", client.base_url.trim_end_matches('/')))
        .header("x-api-key", &client.api_key)
        .json(&ExaSearchRequest {
            query: &request.query,
            num_results: request.max_results,
        })
        .send()
        .await
        .map_err(map_reqwest_error)?;
    match response.status() {
        reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN => {
            return Err(SearchError::AuthFailed);
        }
        reqwest::StatusCode::TOO_MANY_REQUESTS => return Err(SearchError::RateLimited),
        status if !status.is_success() => return Err(SearchError::Http),
        _ => {}
    }
    let value: Value = response.json().await.map_err(|_| SearchError::Parse)?;
    normalize_exa_search_response(&request.query, request.max_results, &value)
}

pub(crate) fn normalize_exa_search_response(
    query: &str,
    max_results: usize,
    value: &Value,
) -> Result<SearchResponse, SearchError> {
    let results = value
        .get("results")
        .and_then(Value::as_array)
        .ok_or(SearchError::Parse)?
        .iter()
        .filter_map(exa_result)
        .take(max_results)
        .enumerate()
        .map(|(index, (title, url, snippet))| SearchResult {
            rank: index + 1,
            title,
            url,
            snippet,
        })
        .collect::<Vec<_>>();
    let summary = match results.len() {
        0 => "No web results found".to_string(),
        1 => "Found 1 Exa web result".to_string(),
        count => format!("Found {count} Exa web results"),
    };
    Ok(SearchResponse {
        provider: EXA_SEARCH_PROVIDER_ID.to_string(),
        provider_contract: EXA_SEARCH_CONTRACT.to_string(),
        query: query.to_string(),
        results,
        summary,
    })
}

fn exa_result(value: &Value) -> Option<(String, String, String)> {
    let url = value.get("url")?.as_str()?.trim().to_string();
    if url.is_empty() {
        return None;
    }
    let title = value
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&url)
        .to_string();
    let snippet = value
        .get("highlights")
        .and_then(Value::as_array)
        .and_then(|items| items.iter().find_map(Value::as_str))
        .or_else(|| value.get("summary").and_then(Value::as_str))
        .or_else(|| value.get("text").and_then(Value::as_str))
        .map(|text| text.split_whitespace().collect::<Vec<_>>().join(" "))
        .unwrap_or_default();
    Some((title, url, snippet))
}

fn map_reqwest_error(error: reqwest::Error) -> SearchError {
    if error.is_timeout() {
        SearchError::Timeout
    } else {
        SearchError::Http
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::types::SearchRequest;
    use serde_json::json;
    use tokio::sync::mpsc;

    #[test]
    fn normalizes_exa_search_results() {
        let value = json!({
            "results": [
                {
                    "title": "Rust",
                    "url": "https://www.rust-lang.org/",
                    "highlights": ["A language empowering everyone"],
                    "text": "Full text",
                    "summary": "Summary"
                },
                {
                    "title": "Cargo",
                    "url": "https://doc.rust-lang.org/cargo/",
                    "text": "Cargo is Rust's package manager."
                }
            ]
        });

        let response = normalize_exa_search_response("rust", 1, &value).expect("response");

        assert_eq!(response.provider, "exa");
        assert_eq!(response.provider_contract, "hosted_provider");
        assert_eq!(response.results.len(), 1);
        assert_eq!(response.results[0].rank, 1);
        assert_eq!(response.results[0].title, "Rust");
        assert_eq!(
            response.results[0].snippet,
            "A language empowering everyone"
        );
    }

    #[tokio::test]
    async fn sends_search_request_with_api_key() {
        let (base_url, mut request_rx) = spawn_server(200, r#"{"results":[]}"#).await;
        let client = ExaSearchClient {
            base_url,
            api_key: "secret".to_string(),
            http: reqwest::Client::new(),
        };

        let response = search_exa(
            &client,
            &SearchRequest {
                query: "rust".to_string(),
                reason: None,
                max_results: 3,
            },
        )
        .await
        .expect("search");

        let request = request_rx.recv().await.expect("request");
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/search");
        assert_eq!(request.x_api_key.as_deref(), Some("secret"));
        assert!(request.body.contains("\"query\":\"rust\""));
        assert!(request.body.contains("\"numResults\":3"));
        assert!(response.results.is_empty());
    }

    #[derive(Debug)]
    struct CapturedRequest {
        method: String,
        path: String,
        x_api_key: Option<String>,
        body: String,
    }

    async fn spawn_server(
        status: u16,
        body: &'static str,
    ) -> (String, mpsc::Receiver<CapturedRequest>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let address = listener.local_addr().expect("local addr");
        let (request_tx, request_rx) = mpsc::channel(1);

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept");
            let mut buffer = vec![0_u8; 8192];
            let read = tokio::io::AsyncReadExt::read(&mut stream, &mut buffer)
                .await
                .expect("read request");
            let request_text = String::from_utf8(buffer[..read].to_vec()).expect("utf8 request");
            let (head, body_text) = request_text
                .split_once("\r\n\r\n")
                .expect("request separator");
            let mut lines = head.lines();
            let request_line = lines.next().expect("request line");
            let mut request_parts = request_line.split_whitespace();
            let method = request_parts.next().unwrap_or_default().to_string();
            let path = request_parts.next().unwrap_or_default().to_string();
            let x_api_key = lines.find_map(|line| {
                line.split_once(':').and_then(|(name, value)| {
                    name.eq_ignore_ascii_case("x-api-key")
                        .then(|| value.trim().to_string())
                })
            });
            request_tx
                .send(CapturedRequest {
                    method,
                    path,
                    x_api_key,
                    body: body_text.to_string(),
                })
                .await
                .expect("send request");

            let response = format!(
                "HTTP/1.1 {status} OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            tokio::io::AsyncWriteExt::write_all(&mut stream, response.as_bytes())
                .await
                .expect("write response");
        });

        (format!("http://{}", address), request_rx)
    }
}
