#![cfg_attr(not(test), allow(dead_code))]

use crate::search::types::{SearchError, SearchResponse, SearchResult};
use crate::{provider::adapters::responses::HostedWebSearchRequest, search::types::SearchRequest};
use serde_json::Value;

pub(crate) const OPENAI_HOSTED_SEARCH_PROVIDER_ID: &str = "openai";
pub(crate) const OPENAI_HOSTED_SEARCH_CONTRACT: &str = "hosted_search";

#[derive(Debug, Clone)]
pub struct OpenAiHostedSearchClient {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub http: reqwest::Client,
}

pub(crate) async fn search_openai_hosted(
    client: &OpenAiHostedSearchClient,
    request: &SearchRequest,
) -> Result<SearchResponse, SearchError> {
    let response = client
        .http
        .post(format!(
            "{}/responses",
            client.base_url.trim_end_matches('/')
        ))
        .bearer_auth(&client.api_key)
        .json(&HostedWebSearchRequest::new(
            client.model.clone(),
            &request.query,
        ))
        .send()
        .await
        .map_err(|error| {
            if error.is_timeout() {
                SearchError::Timeout
            } else {
                SearchError::Http
            }
        })?;
    if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(SearchError::RateLimited);
    }
    if !response.status().is_success() {
        return Err(SearchError::Http);
    }
    let value: Value = response.json().await.map_err(|_| SearchError::Parse)?;
    normalize_openai_hosted_search_response(&request.query, request.max_results, &value)
}

pub(crate) fn normalize_openai_hosted_search_response(
    query: &str,
    max_results: usize,
    value: &Value,
) -> Result<SearchResponse, SearchError> {
    let mut results = Vec::new();
    for annotation in value
        .get("output")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(output_content)
        .flat_map(output_text_annotations)
    {
        if annotation.get("type").and_then(Value::as_str) != Some("url_citation") {
            continue;
        }

        if results.len() >= max_results {
            break;
        }

        let Some(url) = annotation.get("url").and_then(Value::as_str) else {
            continue;
        };
        if results
            .iter()
            .any(|existing: &SearchResult| existing.url == url)
        {
            continue;
        }

        let title = annotation
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or(url)
            .to_string();
        results.push(SearchResult {
            rank: results.len() + 1,
            title,
            url: url.to_string(),
            snippet: String::new(),
        });
    }

    let summary = match results.len() {
        0 => "OpenAI hosted web search returned no citations".to_string(),
        1 => "OpenAI hosted web search returned 1 citation".to_string(),
        count => format!("OpenAI hosted web search returned {count} citations"),
    };

    Ok(SearchResponse {
        provider: OPENAI_HOSTED_SEARCH_PROVIDER_ID.to_string(),
        provider_contract: OPENAI_HOSTED_SEARCH_CONTRACT.to_string(),
        query: query.to_string(),
        results,
        summary,
    })
}

fn output_content(output: &Value) -> Vec<&Value> {
    output
        .get("content")
        .and_then(Value::as_array)
        .map(|items| items.iter().collect())
        .unwrap_or_default()
}

fn output_text_annotations(content: &Value) -> Vec<&Value> {
    content
        .get("annotations")
        .and_then(Value::as_array)
        .map(|items| items.iter().collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::types::SearchRequest;
    use serde_json::json;
    use tokio::sync::mpsc;

    #[test]
    fn normalizes_openai_hosted_search_response_with_citations() {
        let value = json!({
            "id": "resp_1",
            "output": [
                {
                    "type": "message",
                    "content": [
                        {
                            "type": "output_text",
                            "text": "Rust current docs mention ownership.",
                            "annotations": [
                                {
                                    "type": "url_citation",
                                    "url": "https://www.rust-lang.org/learn",
                                    "title": "Learn Rust"
                                }
                            ]
                        }
                    ]
                }
            ]
        });

        let response =
            normalize_openai_hosted_search_response("rust learn", 5, &value).expect("normalized");

        assert_eq!(response.provider, "openai");
        assert_eq!(response.provider_contract, "hosted_search");
        assert_eq!(response.query, "rust learn");
        assert_eq!(response.results.len(), 1);
        assert_eq!(response.results[0].url, "https://www.rust-lang.org/learn");
        assert_eq!(response.results[0].title, "Learn Rust");
        assert_eq!(
            response.summary,
            "OpenAI hosted web search returned 1 citation"
        );
    }

    #[tokio::test]
    async fn executes_hosted_search_request_and_normalizes_citations() {
        let (base_url, mut request_rx) = spawn_server(
            200,
            json!({
                "id": "resp_1",
                "output": [
                    {
                        "type": "message",
                        "content": [
                            {
                                "type": "output_text",
                                "text": "Rust docs",
                                "annotations": [
                                    {
                                        "type": "url_citation",
                                        "url": "https://www.rust-lang.org/learn",
                                        "title": "Learn Rust"
                                    }
                                ]
                            }
                        ]
                    }
                ]
            })
            .to_string(),
        )
        .await;
        let client = OpenAiHostedSearchClient {
            base_url,
            api_key: "secret".to_string(),
            model: "gpt-test".to_string(),
            http: reqwest::Client::new(),
        };

        let response = search_openai_hosted(
            &client,
            &SearchRequest {
                query: "rust learn".to_string(),
                reason: None,
                max_results: 5,
            },
        )
        .await
        .expect("search response");

        let request = request_rx.recv().await.expect("request");
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/responses");
        assert_eq!(request.authorization.as_deref(), Some("Bearer secret"));
        assert!(request.body.contains("\"type\":\"web_search\""));
        assert!(request.body.contains("Search the web for this query"));
        assert_eq!(response.provider, "openai");
        assert_eq!(response.results.len(), 1);
        assert_eq!(response.results[0].title, "Learn Rust");
    }

    #[tokio::test]
    async fn hosted_search_truncates_to_max_results_with_stable_ranks() {
        let (base_url, _request_rx) = spawn_server(
            200,
            json!({
                "id": "resp_2",
                "output": [
                    {
                        "type": "message",
                        "content": [
                            {
                                "type": "output_text",
                                "text": "Rust docs",
                                "annotations": [
                                    {
                                        "type": "url_citation",
                                        "url": "https://example.com/one",
                                        "title": "One"
                                    },
                                    {
                                        "type": "url_citation",
                                        "url": "https://example.com/two",
                                        "title": "Two"
                                    },
                                    {
                                        "type": "url_citation",
                                        "url": "https://example.com/one",
                                        "title": "One duplicate"
                                    },
                                    {
                                        "type": "url_citation",
                                        "url": "https://example.com/three",
                                        "title": "Three"
                                    }
                                ]
                            }
                        ]
                    }
                ]
            })
            .to_string(),
        )
        .await;
        let client = OpenAiHostedSearchClient {
            base_url,
            api_key: "secret".to_string(),
            model: "gpt-test".to_string(),
            http: reqwest::Client::new(),
        };

        let response = search_openai_hosted(
            &client,
            &SearchRequest {
                query: "rust learn".to_string(),
                reason: None,
                max_results: 2,
            },
        )
        .await
        .expect("search response");

        assert_eq!(response.results.len(), 2);
        assert_eq!(response.results[0].rank, 1);
        assert_eq!(response.results[0].url, "https://example.com/one");
        assert_eq!(response.results[1].rank, 2);
        assert_eq!(response.results[1].url, "https://example.com/two");
        assert_eq!(
            response.summary,
            "OpenAI hosted web search returned 2 citations"
        );
    }

    #[derive(Debug)]
    struct CapturedRequest {
        method: String,
        path: String,
        authorization: Option<String>,
        body: String,
    }

    async fn spawn_server(status: u16, body: String) -> (String, mpsc::Receiver<CapturedRequest>) {
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
            let authorization = lines.find_map(|line| {
                line.split_once(':').and_then(|(name, value)| {
                    name.eq_ignore_ascii_case("authorization")
                        .then(|| value.trim().to_string())
                })
            });
            request_tx
                .send(CapturedRequest {
                    method,
                    path,
                    authorization,
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
