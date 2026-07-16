//! DuckDuckGo public search provider.

use crate::search::types::{
    BEST_EFFORT_PUBLIC_CONTRACT, DUCKDUCKGO_PUBLIC_PROVIDER_ID, SearchError,
};
use futures_util::StreamExt;
use noema_capabilities::web::search::{SearchRequest, SearchResponse, SearchResult};
use reqwest::Client;
use scraper::{Html, Selector};
use std::time::Duration;
use url::Url;

const DUCKDUCKGO_HTML_ENDPOINT: &str = "https://html.duckduckgo.com/html/";
const USER_AGENT: &str = "Noema/0.1 web.search (+https://github.com/kpsuperplane/Noema)";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_BODY_BYTES: usize = 1_000_000;

pub(crate) async fn search_duckduckgo_public(
    client: &Client,
    request: &SearchRequest,
) -> Result<SearchResponse, SearchError> {
    search_duckduckgo_public_with_endpoint(client, request, DUCKDUCKGO_HTML_ENDPOINT).await
}

async fn search_duckduckgo_public_with_endpoint(
    client: &Client,
    request: &SearchRequest,
    endpoint: &str,
) -> Result<SearchResponse, SearchError> {
    let mut url = Url::parse(endpoint).map_err(|_| SearchError::Parse)?;
    url.query_pairs_mut().append_pair("q", &request.query);
    let response = client
        .get(url)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(map_reqwest_error)?;
    let status = response.status();
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS || status == reqwest::StatusCode::FORBIDDEN
    {
        return Err(SearchError::RateLimited);
    }
    if !status.is_success() {
        return Err(SearchError::Http);
    }
    let bytes = read_bounded_body(response).await?;
    let html = std::str::from_utf8(&bytes).map_err(|_| SearchError::Parse)?;
    parse_duckduckgo_html(&request.query, html, request.max_results)
}

async fn read_bounded_body(response: reqwest::Response) -> Result<Vec<u8>, SearchError> {
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(map_reqwest_error)?;
        if bytes.len().saturating_add(chunk.len()) > MAX_BODY_BYTES {
            return Err(SearchError::Parse);
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

pub(crate) fn parse_duckduckgo_html(
    query: &str,
    html: &str,
    max_results: usize,
) -> Result<SearchResponse, SearchError> {
    let document = Html::parse_document(html);
    let result_selector =
        Selector::parse(".result.results_links, .web-result").map_err(|_| SearchError::Parse)?;
    let title_selector = Selector::parse(".result__a").map_err(|_| SearchError::Parse)?;
    let snippet_selector = Selector::parse(".result__snippet").map_err(|_| SearchError::Parse)?;

    let results = document
        .select(&result_selector)
        .filter_map(|result| {
            let title_node = result.select(&title_selector).next()?;
            let title = normalized_text(title_node.text())?;
            let url = title_node
                .value()
                .attr("href")
                .and_then(normalize_result_url)?;
            let snippet = result
                .select(&snippet_selector)
                .next()
                .and_then(|node| normalized_text(node.text()))
                .unwrap_or_default();
            Some((title, url, snippet))
        })
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
        1 => "Found 1 web result".to_string(),
        count => format!("Found {count} web results"),
    };

    Ok(SearchResponse {
        provider: DUCKDUCKGO_PUBLIC_PROVIDER_ID.to_string(),
        provider_contract: BEST_EFFORT_PUBLIC_CONTRACT.to_string(),
        query: query.to_string(),
        results,
        summary,
    })
}

fn normalized_text<'a>(parts: impl Iterator<Item = &'a str>) -> Option<String> {
    let value = parts.collect::<Vec<_>>().join(" ");
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    (!normalized.is_empty()).then_some(normalized)
}

fn normalize_result_url(value: &str) -> Option<String> {
    if value.starts_with("http://") || value.starts_with("https://") {
        return Some(value.to_string());
    }
    let absolute = if value.starts_with("//") {
        format!("https:{value}")
    } else {
        value.to_string()
    };
    let url = Url::parse(&absolute).ok()?;
    if url.domain() == Some("duckduckgo.com") && url.path() == "/l/" {
        return url
            .query_pairs()
            .find_map(|(key, value)| (key == "uddg").then(|| value.into_owned()));
    }
    None
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
    use crate::search::types::{BEST_EFFORT_PUBLIC_CONTRACT, DUCKDUCKGO_PUBLIC_PROVIDER_ID};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    #[test]
    fn parses_duckduckgo_html_results() {
        let html = include_str!("fixtures/duckduckgo_results.html");

        let response = parse_duckduckgo_html("rust search", html, 10).expect("parsed");

        assert_eq!(response.provider, DUCKDUCKGO_PUBLIC_PROVIDER_ID);
        assert_eq!(response.provider_contract, BEST_EFFORT_PUBLIC_CONTRACT);
        assert_eq!(response.query, "rust search");
        assert_eq!(response.results.len(), 2);
        assert_eq!(response.results[0].rank, 1);
        assert_eq!(response.results[0].title, "Rust Search Result");
        assert_eq!(response.results[0].url, "https://example.com/rust");
        assert_eq!(
            response.results[0].snippet,
            "Rust is a language empowering everyone to build reliable software."
        );
        assert_eq!(response.results[1].url, "https://example.com/encoded");
        assert_eq!(response.summary, "Found 2 web results");
    }

    #[test]
    fn parser_respects_max_results() {
        let html = include_str!("fixtures/duckduckgo_results.html");

        let response = parse_duckduckgo_html("rust search", html, 1).expect("parsed");

        assert_eq!(response.results.len(), 1);
    }

    #[test]
    fn parser_returns_successful_empty_results() {
        let html = include_str!("fixtures/duckduckgo_no_results.html");

        let response = parse_duckduckgo_html("unlikely query", html, 10).expect("parsed");

        assert!(response.results.is_empty());
        assert_eq!(response.summary, "No web results found");
    }

    #[tokio::test]
    async fn search_rejects_body_that_exceeds_cap_while_reading() {
        let endpoint = spawn_oversized_response_server().await;
        let request = SearchRequest {
            query: "rust search".to_string(),
            reason: None,
            max_results: 5,
        };

        let error = search_duckduckgo_public_with_endpoint(&Client::new(), &request, &endpoint)
            .await
            .expect_err("oversized body rejected");

        assert!(matches!(error, SearchError::Parse));
    }

    async fn spawn_oversized_response_server() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("local addr");
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept");
            let mut buffer = [0_u8; 1024];
            loop {
                let read = socket.read(&mut buffer).await.expect("read request");
                if read == 0
                    || buffer[..read]
                        .windows(4)
                        .any(|window| window == b"\r\n\r\n")
                {
                    break;
                }
            }
            let headers = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: text/html\r\ncontent-length: {}\r\n\r\n",
                MAX_BODY_BYTES + 1
            );
            socket
                .write_all(headers.as_bytes())
                .await
                .expect("write headers");
            socket
                .write_all(&vec![b'a'; MAX_BODY_BYTES + 1])
                .await
                .expect("write body");
        });
        format!("http://{addr}/html/")
    }
}
