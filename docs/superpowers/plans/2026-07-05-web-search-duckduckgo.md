# Rust-Only Web Search Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a zero-setup `web.search` tool backed by a Rust-only DuckDuckGo public search adapter.

**Architecture:** Add `web.search` as a first-party native Noema tool with a dedicated `NoemaToolExecution::WebSearch` execution kind. Keep the provider contract stable while the initial adapter uses bounded Rust HTTP plus fixture-tested DuckDuckGo HTML parsing. Runtime dispatch follows the existing local tool lifecycle so search calls produce visible transcript markers and feed normalized results back to the provider in the same turn.

**Tech Stack:** Rust 2024, `reqwest`, `url`, `scraper` for HTML parsing, existing Noema native tool plane, existing daemon transcript activity model, existing web transcript marker model, Bun lint/build for frontend validation.

---

## File Structure

- Modify `Cargo.toml`: add `scraper = "0.27"` to workspace dependencies.
- Modify `crates/noema-core/Cargo.toml`: add `scraper.workspace = true`.
- Modify `crates/noema-core/src/lib.rs`: expose the new `search` module internally.
- Modify `crates/noema-core/src/provider/tools.rs`: add `NoemaToolExecution::WebSearch`.
- Create `crates/noema-core/src/search.rs`: module facade for the first-party web search subsystem.
- Create `crates/noema-core/src/search/types.rs`: normalized search request/result/error/provider metadata types.
- Create `crates/noema-core/src/search/tool.rs`: `web.search` tool spec, argument parser, executor, and unit tests.
- Create `crates/noema-core/src/search/duckduckgo.rs`: DuckDuckGo public provider, bounded HTTP request, fixture-backed parser, and unit tests.
- Create `crates/noema-core/src/search/fixtures/duckduckgo_results.html`: saved successful result fixture.
- Create `crates/noema-core/src/search/fixtures/duckduckgo_no_results.html`: saved empty-result fixture.
- Modify `crates/noema-core/src/daemon/runtime/model_tools.rs`: include `web.search` in native-capable model tools and prompt rows.
- Modify `crates/noema-core/src/daemon/runtime/local_tools.rs`: dispatch `web.search` through the search executor.
- Modify `crates/noema-core/src/daemon/runtime/transcript_persistence.rs`: render web-search-specific display metadata.
- Modify `crates/noema-core/web/src/components/transcript/markerModel.ts`: label `web.search` as web search in chat markers.
- Modify `crates/noema-core/src/daemon/tests.rs`: add runtime coverage for native exposure, execution, transcript activity, and continuation.
- Modify `crates/noema-core/web/src/components/transcript/markerModel.test.ts`: add marker label/detail coverage for `web.search`.

---

### Task 1: Canonical Tool Kind And Tool Spec

**Files:**
- Modify: `crates/noema-core/src/provider/tools.rs`
- Create: `crates/noema-core/src/search.rs`
- Create: `crates/noema-core/src/search/tool.rs`
- Create: `crates/noema-core/src/search/types.rs`
- Modify: `crates/noema-core/src/lib.rs`

- [ ] **Step 1: Add failing provider tool-kind test**

Add this test to the existing `#[cfg(test)] mod tests` in `crates/noema-core/src/provider/tools.rs`:

```rust
#[test]
fn canonical_tool_spec_accepts_web_search_execution_kind() {
    let spec = NoemaToolSpec::new(
        "web.search",
        "Search the public web using Noema's configured search provider.",
        json!({
            "type": "object",
            "properties": {
                "query": {"type": "string"}
            },
            "required": ["query"],
            "additionalProperties": false
        }),
        NoemaToolExecution::WebSearch,
    )
    .expect("web search tool spec");

    assert_eq!(spec.name.as_str(), "web.search");
    assert!(matches!(spec.execution, NoemaToolExecution::WebSearch));
}
```

- [ ] **Step 2: Run the failing tool-kind test**

Run:

```bash
cargo test -p noema-core provider::tools::tests::canonical_tool_spec_accepts_web_search_execution_kind
```

Expected: fail to compile because `NoemaToolExecution::WebSearch` does not exist.

- [ ] **Step 3: Add the canonical execution variant**

In `crates/noema-core/src/provider/tools.rs`, update `NoemaToolExecution`:

```rust
pub enum NoemaToolExecution {
    /// Builtin tool executed by the local Noema runtime.
    LocalBuiltin,
    /// First-party public web search executed by the Noema runtime.
    WebSearch,
    /// Tool executed through a calibrated MCP server.
    Mcp {
        /// Persisted MCP server id.
        server_id: String,
        /// Provider-visible MCP tool name.
        tool_name: String,
        /// Persisted MCP tool id.
        tool_id: String,
    },
}
```

- [ ] **Step 4: Add search module facade and normalized types**

Create `crates/noema-core/src/search.rs`:

```rust
//! First-party governed web search capability.

pub(crate) mod duckduckgo;
pub(crate) mod tool;
pub(crate) mod types;
```

Create `crates/noema-core/src/search/types.rs`:

```rust
//! Provider-neutral web search types.

use serde::{Deserialize, Serialize};
use reqwest::Client;

/// Stable id for Noema's default public DuckDuckGo provider.
pub const DUCKDUCKGO_PUBLIC_PROVIDER_ID: &str = "duckduckgo_public";
/// Reliability label for providers that do not have a formal API contract.
pub const BEST_EFFORT_PUBLIC_CONTRACT: &str = "best_effort_public";

/// Normalized search request after tool argument validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SearchRequest {
    pub query: String,
    pub reason: Option<String>,
    pub max_results: usize,
}

/// Normalized successful search response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SearchResponse {
    pub provider: String,
    pub provider_contract: String,
    pub query: String,
    pub results: Vec<SearchResult>,
    pub summary: String,
}

/// One normalized search result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SearchResult {
    pub rank: usize,
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// Safe error categories returned by the first-party search tool.
#[derive(Debug, thiserror::Error)]
pub(crate) enum SearchError {
    #[error("{0}")]
    InvalidArguments(String),
    #[error("search request timed out")]
    Timeout,
    #[error("search provider rate limited or blocked the request")]
    RateLimited,
    #[error("search provider request failed")]
    Http,
    #[error("search provider response could not be parsed")]
    Parse,
}

/// Runtime provider selected for the first-party `web.search` tool.
#[derive(Debug, Clone)]
pub(crate) enum SearchRuntimeProvider {
    DuckDuckGoPublic { client: Client },
    #[cfg(test)]
    Static { response: SearchResponse },
}

impl Default for SearchRuntimeProvider {
    fn default() -> Self {
        Self::DuckDuckGoPublic {
            client: Client::new(),
        }
    }
}

impl SearchRuntimeProvider {
    pub(crate) async fn search(
        &self,
        request: &SearchRequest,
    ) -> Result<SearchResponse, SearchError> {
        match self {
            Self::DuckDuckGoPublic { client } => {
                crate::search::duckduckgo::search_duckduckgo_public(client, request).await
            }
            #[cfg(test)]
            Self::Static { response } => {
                let mut response = response.clone();
                response.query = request.query.clone();
                response.results.truncate(request.max_results);
                response.summary = match response.results.len() {
                    0 => "No web results found".to_string(),
                    1 => "Found 1 web result".to_string(),
                    count => format!("Found {count} web results"),
                };
                Ok(response)
            }
        }
    }
}
```

Modify `crates/noema-core/src/lib.rs`:

```rust
/// First-party governed web search capability.
pub mod search;
```

- [ ] **Step 5: Add failing `web.search` tool spec and parser tests**

Create `crates/noema-core/src/search/tool.rs` with the tests first:

```rust
//! Model-visible `web.search` tool contract and runtime executor.

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn web_search_tool_spec_matches_runtime_arguments() {
        let spec = web_search_tool_spec().expect("tool spec");

        assert_eq!(spec.name.as_str(), WEB_SEARCH_TOOL);
        assert!(matches!(spec.execution, NoemaToolExecution::WebSearch));
        assert_eq!(spec.input_schema.as_value()["required"], json!(["query"]));
        assert_eq!(
            spec.input_schema.as_value()["properties"]["query"]["maxLength"],
            MAX_QUERY_CHARS
        );
        assert_eq!(
            spec.input_schema.as_value()["properties"]["max_results"]["maximum"],
            MAX_RESULTS
        );
    }

    #[test]
    fn parses_nested_arguments_and_trims_query() {
        let arguments = parse_web_search_arguments(&json!({
            "arguments": {
                "query": "  rust reqwest current version  ",
                "reason": "  answer a version question  ",
                "max_results": 7
            }
        }))
        .expect("arguments");

        assert_eq!(arguments.query, "rust reqwest current version");
        assert_eq!(arguments.reason.as_deref(), Some("answer a version question"));
        assert_eq!(arguments.max_results, 7);
    }

    #[test]
    fn defaults_and_clamps_result_count() {
        let arguments = parse_web_search_arguments(&json!({
            "query": "duckduckgo html endpoint",
            "max_results": 50
        }))
        .expect("arguments");

        assert_eq!(arguments.max_results, MAX_RESULTS);
    }

    #[test]
    fn rejects_empty_query() {
        let error = parse_web_search_arguments(&json!({"query": "   "}))
            .expect_err("empty query rejected");

        assert_eq!(safe_error_message(&error), "query is required");
    }

    #[test]
    fn rejects_nested_outer_fields() {
        let error = parse_web_search_arguments(&json!({
            "arguments": {"query": "trains"},
            "provider": "brave_search"
        }))
        .expect_err("outer fields rejected");

        assert_eq!(
            safe_error_message(&error),
            "nested arguments payload cannot include outer fields"
        );
    }
}
```

- [ ] **Step 6: Run the failing search tool tests**

Run:

```bash
cargo test -p noema-core search::tool::tests
```

Expected: fail to compile because `web_search_tool_spec`, constants, parser, and imports do not exist.

- [ ] **Step 7: Implement the tool spec and parser**

Replace `crates/noema-core/src/search/tool.rs` with:

```rust
//! Model-visible `web.search` tool contract and runtime executor.

use crate::{
    provider::{NoemaToolExecution, NoemaToolSpec, ToolContractError},
    search::types::{SearchError, SearchRequest},
};
use serde::Deserialize;
use serde_json::{Value, json};

pub(crate) const WEB_SEARCH_TOOL: &str = "web.search";
pub(crate) const DEFAULT_RESULTS: usize = 5;
pub(crate) const MAX_RESULTS: usize = 10;
pub(crate) const MAX_QUERY_CHARS: usize = 500;
const MAX_REASON_CHARS: usize = 500;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WebSearchArguments {
    query: String,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    max_results: Option<usize>,
}

pub(crate) fn is_web_search_tool(name: &str) -> bool {
    name == WEB_SEARCH_TOOL
}

pub(crate) fn web_search_tool_spec() -> Result<NoemaToolSpec, ToolContractError> {
    NoemaToolSpec::new(
        WEB_SEARCH_TOOL,
        "Search the public web using Noema's configured search provider.",
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": MAX_QUERY_CHARS,
                    "description": "The exact internet search query to send to the configured search provider."
                },
                "reason": {
                    "type": "string",
                    "maxLength": MAX_REASON_CHARS,
                    "description": "Brief reason this search is useful for the current response."
                },
                "max_results": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": MAX_RESULTS,
                    "description": "Maximum number of search results to return."
                }
            },
            "required": ["query"],
            "additionalProperties": false
        }),
        NoemaToolExecution::WebSearch,
    )
}

pub(crate) fn parse_web_search_arguments(payload: &Value) -> Result<SearchRequest, SearchError> {
    let argument_value = if let Some(arguments) = payload.get("arguments") {
        reject_nested_outer_fields(payload)?;
        arguments.clone()
    } else {
        payload.clone()
    };
    let mut arguments: WebSearchArguments =
        serde_json::from_value(argument_value).map_err(|error| {
            SearchError::InvalidArguments(format!("invalid arguments: {error}"))
        })?;

    arguments.query = arguments.query.trim().to_string();
    if arguments.query.is_empty() {
        return Err(SearchError::InvalidArguments("query is required".to_string()));
    }
    if arguments.query.chars().count() > MAX_QUERY_CHARS {
        return Err(SearchError::InvalidArguments(format!(
            "query must be {MAX_QUERY_CHARS} characters or fewer"
        )));
    }
    let reason = arguments
        .reason
        .map(|reason| reason.trim().to_string())
        .filter(|reason| !reason.is_empty());
    if reason
        .as_deref()
        .is_some_and(|value| value.chars().count() > MAX_REASON_CHARS)
    {
        return Err(SearchError::InvalidArguments(format!(
            "reason must be {MAX_REASON_CHARS} characters or fewer"
        )));
    }

    Ok(SearchRequest {
        query: arguments.query,
        reason,
        max_results: arguments.max_results.unwrap_or(DEFAULT_RESULTS).clamp(1, MAX_RESULTS),
    })
}

fn reject_nested_outer_fields(payload: &Value) -> Result<(), SearchError> {
    let Some(object) = payload.as_object() else {
        return Ok(());
    };
    if object.keys().all(|key| key == "arguments") {
        Ok(())
    } else {
        Err(SearchError::InvalidArguments(
            "nested arguments payload cannot include outer fields".to_string(),
        ))
    }
}

pub(crate) fn safe_error_message(error: &SearchError) -> String {
    match error {
        SearchError::InvalidArguments(message) => message.clone(),
        SearchError::Timeout => "search request timed out".to_string(),
        SearchError::RateLimited => "search provider rate limited or blocked the request".to_string(),
        SearchError::Http => "search provider request failed".to_string(),
        SearchError::Parse => "search provider response could not be parsed".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn web_search_tool_spec_matches_runtime_arguments() {
        let spec = web_search_tool_spec().expect("tool spec");

        assert_eq!(spec.name.as_str(), WEB_SEARCH_TOOL);
        assert!(matches!(spec.execution, NoemaToolExecution::WebSearch));
        assert_eq!(spec.input_schema.as_value()["required"], json!(["query"]));
        assert_eq!(
            spec.input_schema.as_value()["properties"]["query"]["maxLength"],
            MAX_QUERY_CHARS
        );
        assert_eq!(
            spec.input_schema.as_value()["properties"]["max_results"]["maximum"],
            MAX_RESULTS
        );
    }

    #[test]
    fn parses_nested_arguments_and_trims_query() {
        let arguments = parse_web_search_arguments(&json!({
            "arguments": {
                "query": "  rust reqwest current version  ",
                "reason": "  answer a version question  ",
                "max_results": 7
            }
        }))
        .expect("arguments");

        assert_eq!(arguments.query, "rust reqwest current version");
        assert_eq!(arguments.reason.as_deref(), Some("answer a version question"));
        assert_eq!(arguments.max_results, 7);
    }

    #[test]
    fn defaults_and_clamps_result_count() {
        let arguments = parse_web_search_arguments(&json!({
            "query": "duckduckgo html endpoint",
            "max_results": 50
        }))
        .expect("arguments");

        assert_eq!(arguments.max_results, MAX_RESULTS);
    }

    #[test]
    fn rejects_empty_query() {
        let error = parse_web_search_arguments(&json!({"query": "   "}))
            .expect_err("empty query rejected");

        assert_eq!(safe_error_message(&error), "query is required");
    }

    #[test]
    fn rejects_nested_outer_fields() {
        let error = parse_web_search_arguments(&json!({
            "arguments": {"query": "trains"},
            "provider": "brave_search"
        }))
        .expect_err("outer fields rejected");

        assert_eq!(
            safe_error_message(&error),
            "nested arguments payload cannot include outer fields"
        );
    }
}
```

- [ ] **Step 8: Run Task 1 tests**

Run:

```bash
cargo test -p noema-core provider::tools::tests::canonical_tool_spec_accepts_web_search_execution_kind search::tool::tests
```

Expected: all Task 1 tests pass.

- [ ] **Step 9: Commit Task 1**

Run:

```bash
git add crates/noema-core/src/provider/tools.rs crates/noema-core/src/search.rs crates/noema-core/src/search/tool.rs crates/noema-core/src/search/types.rs crates/noema-core/src/lib.rs
git commit -m "feat(search): define web search tool contract"
```

---

### Task 2: DuckDuckGo Parser And Provider

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/noema-core/Cargo.toml`
- Create: `crates/noema-core/src/search/duckduckgo.rs`
- Create: `crates/noema-core/src/search/fixtures/duckduckgo_results.html`
- Create: `crates/noema-core/src/search/fixtures/duckduckgo_no_results.html`
- Modify: `crates/noema-core/src/search/tool.rs`
- Modify: `crates/noema-core/src/search/types.rs`

- [ ] **Step 1: Add `scraper` dependency**

In root `Cargo.toml`, add this line to `[workspace.dependencies]`:

```toml
scraper = "0.27"
```

In `crates/noema-core/Cargo.toml`, add this line to `[dependencies]`:

```toml
scraper.workspace = true
```

- [ ] **Step 2: Add parser fixtures**

Create `crates/noema-core/src/search/fixtures/duckduckgo_results.html`:

```html
<!doctype html>
<html>
  <body>
    <div class="result results_links">
      <h2 class="result__title">
        <a class="result__a" href="https://example.com/rust">Rust Search Result</a>
      </h2>
      <a class="result__snippet" href="https://example.com/rust">
        Rust is a language empowering everyone to build reliable software.
      </a>
    </div>
    <div class="result results_links">
      <h2 class="result__title">
        <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2Fencoded&amp;rut=abc">Encoded Result</a>
      </h2>
      <a class="result__snippet" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2Fencoded">
        Encoded result snippet.
      </a>
    </div>
    <script>ignore me</script>
    <form><input name="q" value="not a result"></form>
  </body>
</html>
```

Create `crates/noema-core/src/search/fixtures/duckduckgo_no_results.html`:

```html
<!doctype html>
<html>
  <body>
    <div class="no-results">No results.</div>
  </body>
</html>
```

- [ ] **Step 3: Write failing DuckDuckGo parser tests**

Create `crates/noema-core/src/search/duckduckgo.rs` with tests first:

```rust
//! DuckDuckGo public search provider.

#[cfg(test)]
mod tests {
    use super::*;

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
}
```

- [ ] **Step 4: Run the failing parser tests**

Run:

```bash
cargo test -p noema-core search::duckduckgo::tests
```

Expected: fail to compile because the parser and imports do not exist.

- [ ] **Step 5: Implement parser and public provider**

Replace `crates/noema-core/src/search/duckduckgo.rs` with:

```rust
//! DuckDuckGo public search provider.

use crate::search::types::{
    BEST_EFFORT_PUBLIC_CONTRACT, DUCKDUCKGO_PUBLIC_PROVIDER_ID, SearchError, SearchRequest,
    SearchResponse, SearchResult,
};
use reqwest::Client;
use scraper::{Html, Selector};
use serde_json::json;
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
    let response = client
        .get(DUCKDUCKGO_HTML_ENDPOINT)
        .query(&[("q", request.query.as_str())])
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(map_reqwest_error)?;
    let status = response.status();
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS || status == reqwest::StatusCode::FORBIDDEN {
        return Err(SearchError::RateLimited);
    }
    if !status.is_success() {
        return Err(SearchError::Http);
    }
    let bytes = response.bytes().await.map_err(map_reqwest_error)?;
    if bytes.len() > MAX_BODY_BYTES {
        return Err(SearchError::Parse);
    }
    let html = std::str::from_utf8(&bytes).map_err(|_| SearchError::Parse)?;
    parse_duckduckgo_html(&request.query, html, request.max_results)
}

pub(crate) fn parse_duckduckgo_html(
    query: &str,
    html: &str,
    max_results: usize,
) -> Result<SearchResponse, SearchError> {
    let document = Html::parse_document(html);
    let result_selector = Selector::parse(".result.results_links, .web-result")
        .map_err(|_| SearchError::Parse)?;
    let title_selector = Selector::parse(".result__a").map_err(|_| SearchError::Parse)?;
    let snippet_selector = Selector::parse(".result__snippet").map_err(|_| SearchError::Parse)?;

    let results = document
        .select(&result_selector)
        .filter_map(|result| {
            let title_node = result.select(&title_selector).next()?;
            let title = normalized_text(title_node.text())?;
            let url = title_node.value().attr("href").and_then(normalize_result_url)?;
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

pub(crate) fn provider_diagnostics() -> serde_json::Value {
    json!({
        "provider": DUCKDUCKGO_PUBLIC_PROVIDER_ID,
        "provider_contract": BEST_EFFORT_PUBLIC_CONTRACT,
        "endpoint": DUCKDUCKGO_HTML_ENDPOINT,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
```

- [ ] **Step 6: Add runtime executor to `search/tool.rs`**

Add these imports to `crates/noema-core/src/search/tool.rs`:

```rust
use crate::search::types::{SearchResponse, SearchRuntimeProvider};
```

Add the result type and executor below `WebSearchArguments`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WebSearchToolResult {
    pub call_id: Option<String>,
    pub name: String,
    pub success: bool,
    pub payload: Value,
}

pub(crate) async fn execute_web_search(
    provider: &SearchRuntimeProvider,
    call_id: Option<String>,
    payload: &Value,
) -> WebSearchToolResult {
    match execute_web_search_inner(provider, payload).await {
        Ok(response) => WebSearchToolResult {
            call_id,
            name: WEB_SEARCH_TOOL.to_string(),
            success: true,
            payload: serde_json::to_value(response).unwrap_or_else(|_| {
                json!({"error": "search response serialization failed"})
            }),
        },
        Err(error) => WebSearchToolResult {
            call_id,
            name: WEB_SEARCH_TOOL.to_string(),
            success: false,
            payload: json!({
                "error": safe_error_message(&error),
            }),
        },
    }
}

async fn execute_web_search_inner(
    provider: &SearchRuntimeProvider,
    payload: &Value,
) -> Result<SearchResponse, SearchError> {
    let request = parse_web_search_arguments(payload)?;
    provider.search(&request).await
}
```

- [ ] **Step 7: Run Task 2 tests**

Run:

```bash
cargo test -p noema-core search::duckduckgo::tests search::tool::tests
```

Expected: all Task 2 tests pass and `Cargo.lock` updates with `scraper`.

- [ ] **Step 8: Commit Task 2**

Run:

```bash
git add Cargo.toml crates/noema-core/Cargo.toml Cargo.lock crates/noema-core/src/search
git commit -m "feat(search): add duckduckgo public provider"
```

---

### Task 3: Runtime Tool Exposure And Dispatch

**Files:**
- Modify: `crates/noema-core/src/daemon/runtime/model_tools.rs`
- Modify: `crates/noema-core/src/daemon/runtime/local_tools.rs`
- Modify: `crates/noema-core/src/daemon/tests.rs`

- [ ] **Step 1: Add failing model-tools exposure test**

In `crates/noema-core/src/daemon/runtime/model_tools.rs`, update `native_provider_gets_builtin_and_calibrated_mcp_tools` expected names:

```rust
assert_eq!(
    names,
    vec![
        "search_memory",
        "update_own_name",
        "web.search",
        "mcp.mcp:docs.read",
    ]
);
```

Add this assertion in the same test:

```rust
assert!(tools.native.iter().any(|tool| {
    tool.name.as_str() == "web.search" && matches!(tool.execution, NoemaToolExecution::WebSearch)
}));
```

In `non_native_provider_gets_only_builtin_envelope_fallback`, keep the existing expected legacy builtins unchanged:

```rust
assert_eq!(
    tools.legacy_builtin_envelope_tools,
    vec!["search_memory".to_string(), "update_own_name".to_string()]
);
```

- [ ] **Step 2: Run the failing exposure test**

Run:

```bash
cargo test -p noema-core daemon::runtime::model_tools::tests::native_provider_gets_builtin_and_calibrated_mcp_tools daemon::runtime::model_tools::tests::non_native_provider_gets_only_builtin_envelope_fallback
```

Expected: native provider test fails because `web.search` is not exposed.

- [ ] **Step 3: Expose `web.search` for native-capable providers only**

In `crates/noema-core/src/daemon/runtime/model_tools.rs`, add import:

```rust
search::tool::web_search_tool_spec,
```

Update `build_model_tools` after `let builtin_tools = builtin_tool_specs(include_agent_name_tool)?;`:

```rust
let web_search_tool = web_search_tool_spec()?;
```

Inside the `if capabilities.native_tools` branch, after `let mut native = builtin_tools.clone();`, add:

```rust
native.push(web_search_tool);
```

Do not add `web.search` to `legacy_builtin_envelope_tools`.

Update `prompt_rows` to render `WebSearch` distinctly:

```rust
NoemaToolExecution::WebSearch => {
    format!("- web\t{}\t{}", tool.name, tool.description)
}
```

- [ ] **Step 4: Run exposure tests again**

Run:

```bash
cargo test -p noema-core daemon::runtime::model_tools::tests::native_provider_gets_builtin_and_calibrated_mcp_tools daemon::runtime::model_tools::tests::non_native_provider_gets_only_builtin_envelope_fallback
```

Expected: both tests pass.

- [ ] **Step 5: Add failing dispatch test with a static search provider**

In `crates/noema-core/src/daemon/tests.rs`, add a helper next to `test_runtime_handle_with_store`:

```rust
async fn test_runtime_handle_with_search_provider(
    provider: Arc<dyn super::runtime::RuntimeModelProvider>,
    search_provider: crate::search::types::SearchRuntimeProvider,
) -> (CodexRuntimeHandle, crate::NoemaStore) {
    let home = tempfile::tempdir().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let store = crate::NoemaStore::open(&crate::StoreConfig::from_paths(&paths))
        .await
        .expect("store");
    std::mem::forget(home);
    let handle = CodexRuntimeHandle::spawn_with_provider_and_search_provider(
        provider,
        store.clone(),
        search_provider,
    )
    .await
    .expect("runtime");
    (handle, store)
}
```

Add a test near the existing local tool tests:

```rust
#[tokio::test]
async fn runtime_actor_executes_web_search_as_local_tool_result() {
    let search_provider = crate::search::types::SearchRuntimeProvider::Static {
        response: crate::search::types::SearchResponse {
            provider: "duckduckgo_public".to_string(),
            provider_contract: "best_effort_public".to_string(),
            query: String::new(),
            summary: "Found 1 web result".to_string(),
            results: vec![crate::search::types::SearchResult {
                rank: 1,
                title: "Rust Programming Language".to_string(),
                url: "https://www.rust-lang.org/".to_string(),
                snippet: "A language empowering everyone to build reliable software.".to_string(),
            }],
        },
    };
    let (handle, store) =
        test_runtime_handle_with_search_provider(Arc::new(fake_provider(FakeCodexScenario::NativeWebSearch)), search_provider).await;
    let conversation = handle
        .start_primary_conversation(None)
        .await
        .expect("conversation");

    collect_turn(
        &handle,
        conversation.conversation_id.clone(),
        "Search the web for Rust.".to_string(),
    )
    .await
    .expect("turn");

    let items = store
        .list_conversation_items(&conversation.conversation_id, ReplayMode::Audit)
        .await
        .expect("items");

    assert!(items.iter().any(|item| {
        matches!(item.kind, ConversationItemKind::ToolCall)
            && item.payload_json["metadata"]["action"]["name"] == "web.search"
            && item.payload_json["metadata"]["display"]["target"] == "Web search: rust language"
    }));
    assert!(items.iter().any(|item| {
        matches!(item.kind, ConversationItemKind::ToolResult)
            && item.payload_json["metadata"]["action"]["name"] == "web.search"
            && item.payload_json["metadata"]["action"]["success"] == true
            && item.payload_json["metadata"]["action"]["payload"]["provider"] == "duckduckgo_public"
    }));
    handle.shutdown().await;
}
```

Also add `NativeWebSearch` to `FakeCodexScenario` and make the fake provider return a native tool call named `web.search` with payload:

```rust
json!({
    "query": "rust language",
    "reason": "answer the user's request",
    "max_results": 3
})
```

- [ ] **Step 6: Run the failing dispatch test**

Run:

```bash
cargo test -p noema-core daemon::tests::runtime_actor_executes_web_search_as_local_tool_result
```

Expected: fail to compile because `spawn_with_provider_and_search_provider` and the `web.search` dispatch path do not exist.

- [ ] **Step 7: Add runtime search provider injection and dispatch web search**

In `crates/noema-core/src/daemon/runtime/actor.rs`, add an actor field:

```rust
pub(in crate::daemon) search_provider: crate::search::types::SearchRuntimeProvider,
```

Initialize it in `CodexRuntimeActor::new`:

```rust
search_provider: crate::search::types::SearchRuntimeProvider::default(),
```

Add a test-only constructor in `impl CodexRuntimeActor`:

```rust
#[cfg(test)]
pub(in crate::daemon) async fn new_with_search_provider(
    default_provider_kind: String,
    providers: HashMap<String, Arc<dyn RuntimeModelProvider>>,
    store: NoemaStore,
    system_errors: SystemErrorLogger,
    search_provider: crate::search::types::SearchRuntimeProvider,
) -> Result<Self, DaemonError> {
    let mut actor = Self::new(default_provider_kind, providers, store, system_errors).await?;
    actor.search_provider = search_provider;
    Ok(actor)
}
```

In `crates/noema-core/src/daemon/runtime/handle.rs`, add a test-only spawn helper:

```rust
#[cfg(test)]
pub(crate) async fn spawn_with_provider_and_search_provider(
    provider: Arc<dyn RuntimeModelProvider>,
    store: NoemaStore,
    search_provider: crate::search::types::SearchRuntimeProvider,
) -> Result<Self, DaemonError> {
    let provider_kind = "codex".to_string();
    let system_errors = store.system_error_logger();
    let providers = HashMap::from([(provider_kind.clone(), provider)]);
    let Some(default_provider) = providers.get(&provider_kind) else {
        return Err(DaemonError::Provider(ProviderError::ProviderUnavailable {
            provider: provider_kind,
            message: "default provider is not available in this daemon".to_string(),
        }));
    };
    let tool_classification_model = default_provider.default_tool_classification_model();
    let (sender, receiver) = mpsc::channel(16);
    let actor = CodexRuntimeActor::new_with_search_provider(
        provider_kind.clone(),
        providers,
        store,
        system_errors,
        search_provider,
    )
    .await?;
    tokio::spawn(actor.run(receiver));
    Ok(Self {
        sender,
        default_provider_kind: provider_kind,
        tool_classification_model,
    })
}
```

In `crates/noema-core/src/daemon/runtime/local_tools.rs`, add imports:

```rust
use crate::search::tool::{WebSearchToolResult, execute_web_search, is_web_search_tool};
```

Add a new enum variant:

```rust
WebSearch {
    call_id: Option<String>,
    provider_call_id: Option<String>,
    provider_name: Option<String>,
    arguments: Value,
    result: WebSearchToolResult,
},
```

In `execute_local_tool`, before the MCP gateway fallback:

```rust
} else if is_web_search_tool(&call.name) {
    LocalToolResult::WebSearch {
        call_id: call.call_id.clone(),
        provider_call_id: call.provider_call_id.clone(),
        provider_name: call.provider_name.clone(),
        arguments: call.payload.clone(),
        result: execute_web_search(&self.search_provider, call.call_id.clone(), &call.payload).await,
    }
} else {
```

Update `LocalToolResult` helper methods so `WebSearch` behaves like `Memory` for `name`, `success`, `payload`, `arguments`, `call_id`, `provider_call_id`, `provider_name`, and `requires_provider_continuation`.

- [ ] **Step 8: Run dispatch test again**

Run:

```bash
cargo test -p noema-core daemon::tests::runtime_actor_executes_web_search_as_local_tool_result
```

Expected: pass without live network access because the test runtime uses `SearchRuntimeProvider::Static`.

- [ ] **Step 9: Commit Task 3**

Run:

```bash
git add crates/noema-core/src/daemon/runtime/model_tools.rs crates/noema-core/src/daemon/runtime/local_tools.rs crates/noema-core/src/daemon/runtime/actor.rs crates/noema-core/src/daemon/tests.rs
git commit -m "feat(search): expose web search to native tools"
```

---

### Task 4: Transcript Display And Frontend Markers

**Files:**
- Modify: `crates/noema-core/src/daemon/runtime/transcript_persistence.rs`
- Modify: `crates/noema-core/web/src/components/transcript/markerModel.ts`
- Modify: `crates/noema-core/web/src/components/transcript/markerModel.test.ts`

- [ ] **Step 1: Add backend display tests**

In `crates/noema-core/src/daemon/runtime/transcript_persistence.rs`, add or extend tests for `tool_call_display` and `tool_result_display`:

```rust
#[test]
fn web_search_tool_call_display_shows_visible_query() {
    let display = tool_call_display(
        "web.search",
        &json!({
            "query": "rust language",
            "reason": "answer current question",
            "max_results": 3
        }),
    );

    assert_eq!(display["name"], "Search web");
    assert_eq!(display["access"], "Searches public web");
    assert_eq!(display["target"], "Web search: rust language");
    assert_eq!(display["purpose"], "answer current question");
}

#[test]
fn web_search_tool_result_display_shows_provider_and_count() {
    let display = tool_result_display(
        Some("web.search"),
        Some(true),
        &json!({
            "provider": "duckduckgo_public",
            "provider_contract": "best_effort_public",
            "summary": "Found 2 web results",
            "results": [{}, {}]
        }),
    );

    assert_eq!(display["name"], "Search web");
    assert_eq!(display["result"], "Found 2 web results");
    assert_eq!(display["provider"], "DuckDuckGo public search");
    assert_eq!(display["reliability"], "Best effort");
}
```

- [ ] **Step 2: Run failing backend display tests**

Run:

```bash
cargo test -p noema-core daemon::runtime::transcript_persistence::tests::web_search_tool_call_display_shows_visible_query daemon::runtime::transcript_persistence::tests::web_search_tool_result_display_shows_provider_and_count
```

Expected: fail because `web.search` currently uses generic connected-tool display.

- [ ] **Step 3: Implement backend display metadata**

In `readable_tool_name`, add:

```rust
"web.search" => "Search web".to_string(),
```

In `tool_access_label`, add:

```rust
"web.search" => "Searches public web",
```

In `tool_call_display`, add an `else if name == "web.search"` branch:

```rust
} else if name == "web.search" {
    insert_display_value(
        &mut display,
        "purpose",
        arguments
            .get("reason")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string),
    );
    let query = arguments
        .get("query")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    insert_display_value(
        &mut display,
        "target",
        query.map(|query| format!("Web search: {query}")),
    );
```

In `tool_result_display`, add an `else if name == "web.search"` branch:

```rust
} else if name == "web.search" {
    insert_display_value(
        &mut display,
        "result",
        Some(web_search_result_label(success, payload)),
    );
    insert_display_value(
        &mut display,
        "provider",
        payload
            .get("provider")
            .and_then(Value::as_str)
            .map(web_search_provider_label),
    );
    insert_display_value(
        &mut display,
        "reliability",
        payload
            .get("provider_contract")
            .and_then(Value::as_str)
            .map(web_search_contract_label),
    );
```

Add helper functions:

```rust
fn web_search_result_label(success: Option<bool>, payload: &Value) -> String {
    if let Some(error) = payload.get("error").and_then(Value::as_str) {
        return format!("Failed: {error}");
    }
    payload
        .get("summary")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| success_result_label(success, payload))
}

fn web_search_provider_label(provider: &str) -> String {
    match provider {
        "duckduckgo_public" => "DuckDuckGo public search".to_string(),
        other => other.replace('_', " "),
    }
}

fn web_search_contract_label(contract: &str) -> String {
    match contract {
        "best_effort_public" => "Best effort".to_string(),
        other => other.replace('_', " "),
    }
}
```

- [ ] **Step 4: Run backend display tests again**

Run:

```bash
cargo test -p noema-core daemon::runtime::transcript_persistence::tests::web_search_tool_call_display_shows_visible_query daemon::runtime::transcript_persistence::tests::web_search_tool_result_display_shows_provider_and_count
```

Expected: pass.

- [ ] **Step 5: Add frontend marker tests**

In `crates/noema-core/web/src/components/transcript/markerModel.test.ts`, add:

Add `toolMarkerLabel` to the existing import from `./markerModel`:

```ts
import {
  formatToolDetail,
  memoryCardsFromClaimOutcomes,
  memoryDetailItems,
  toolDetailRows,
  toolMarkerExpandable,
  toolMarkerLabel,
  toolMarkerName,
  toolMarkerTarget
} from "./markerModel";
```

```ts
test("labels web search markers with visible query", () => {
  const label = formatToolDetail("Tool call: web.search", {
    action: {
      name: "web.search",
      payload: {
        query: "rust language",
        reason: "answer current question"
      }
    },
    display: {
      name: "Search web",
      access: "Searches public web",
      target: "Web search: rust language"
    }
  });

  assert.match(label, /Search web/);
  assert.match(label, /Web search: rust language/);
});
```

Add a companion test with a full `ToolMarkerGroup` fixture:

```ts
test("web search marker target prefers result summary", () => {
  const marker: ToolMarkerGroup = {
    id: "tool_call:web",
    call: {
      id: "call-entry",
      type: "activity",
      item: {
        kind: "activity",
        id: "activity:web-call",
        activity_kind: "tool_call",
        status: "COMPLETED",
        title: "Tool call: web.search",
        summary: "Web search: rust language",
        metadata: {
          action: {
            name: "web.search",
            payload: { query: "rust language" }
          },
          display: {
            name: "Search web",
            target: "Web search: rust language"
          }
        }
      }
    },
    result: {
      id: "result-entry",
      type: "activity",
      item: {
        kind: "activity",
        id: "activity:web-result",
        activity_kind: "tool_result",
        status: "COMPLETED",
        title: "Tool result: web.search",
        summary: "Found 1 web result",
        metadata: {
          action: {
            name: "web.search",
            success: true,
            payload: {
              provider: "duckduckgo_public",
              provider_contract: "best_effort_public",
              summary: "Found 1 web result",
              results: [{ rank: 1, title: "Rust", url: "https://www.rust-lang.org/", snippet: "Rust language." }]
            }
          },
          display: {
            name: "Search web",
            result: "Found 1 web result",
            provider: "DuckDuckGo public search",
            reliability: "Best effort"
          }
        }
      }
    }
  };

  assert.equal(toolMarkerLabel(marker), "Used Search web");
  assert.equal(toolMarkerTarget(marker), "Found 1 web result");
});
```

- [ ] **Step 6: Run frontend marker tests**

Run:

```bash
cd crates/noema-core/web
bun test src/components/transcript/markerModel.test.ts
```

Expected: fail if `readableToolName("web.search")` still returns `"Search"`.

- [ ] **Step 7: Implement frontend marker naming**

In `crates/noema-core/web/src/components/transcript/markerModel.ts`, update `readableToolName`:

```ts
if (trimmed === "web.search") {
  return "Search web";
}
```

In `displayToolMetadataPreview`, append provider/reliability display rows:

```ts
appendDisplayRow(rows, "Provider", stringValue(display.provider));
appendDisplayRow(rows, "Reliability", stringValue(display.reliability));
```

- [ ] **Step 8: Run frontend marker tests again**

Run:

```bash
cd crates/noema-core/web
bun test src/components/transcript/markerModel.test.ts
```

Expected: pass.

- [ ] **Step 9: Commit Task 4**

Run:

```bash
git add crates/noema-core/src/daemon/runtime/transcript_persistence.rs crates/noema-core/web/src/components/transcript/markerModel.ts crates/noema-core/web/src/components/transcript/markerModel.test.ts
git commit -m "feat(search): show web search activity in chat"
```

---

### Task 5: End-To-End Runtime Coverage And Validation

**Files:**
- Modify: `crates/noema-core/src/daemon/tests.rs`
- Modify: `docs/context/current.md`

- [ ] **Step 1: Add continuation regression coverage**

In `crates/noema-core/src/daemon/tests.rs`, add a test near `native_capable_provider_continuation_uses_native_tool_result_input`:

```rust
#[tokio::test]
async fn web_search_result_is_sent_as_native_tool_result_input() {
    let search_provider = crate::search::types::SearchRuntimeProvider::Static {
        response: crate::search::types::SearchResponse {
            provider: "duckduckgo_public".to_string(),
            provider_contract: "best_effort_public".to_string(),
            query: String::new(),
            summary: "Found 1 web result".to_string(),
            results: vec![crate::search::types::SearchResult {
                rank: 1,
                title: "Rust Programming Language".to_string(),
                url: "https://www.rust-lang.org/".to_string(),
                snippet: "A language empowering everyone to build reliable software.".to_string(),
            }],
        },
    };
    let provider = Arc::new(
        RecordingFakeProvider::new("codex", FakeCodexScenario::NativeWebSearchContinuation)
            .with_tool_capabilities(ProviderToolCapabilities {
                native_tools: true,
                parallel_tool_calls: true,
                native_tool_results: true,
                schema_dialect: ProviderToolSchemaDialect::OpenAiResponses,
                fallback_mode: ProviderToolFallbackMode::NativeRequired,
                ..ProviderToolCapabilities::default()
            }),
    );
    let (handle, _store) =
        test_runtime_handle_with_search_provider(provider.clone(), search_provider).await;
    let conversation = handle
        .start_primary_conversation(None)
        .await
        .expect("conversation");

    collect_turn(
        &handle,
        conversation.conversation_id.clone(),
        "Search for Rust.".to_string(),
    )
    .await
    .expect("turn");

    let requests = provider.requests();
    assert!(requests.iter().any(|request| {
        let GenerateInput::NativeToolResults(results) = &request.input else {
            return false;
        };
        results.iter().any(|result| {
            result.name == "web.search"
                && result.success
                && result.payload["provider"] == "duckduckgo_public"
                && result
                    .payload
                    .get("results")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|results| !results.is_empty())
        })
    }));
    handle.shutdown().await;
}
```

Extend the fake provider scenario so it returns a final answer only after it receives a `web.search` native tool result:

```rust
if results.iter().any(|result| result.name == "web.search") {
    assistant_with_no_memories("I found a current web result.")
} else {
    web_search_tool_call("call_web_1", json!({
        "query": "rust language",
        "reason": "answer the current question",
        "max_results": 3
    }))
}
```

- [ ] **Step 2: Run the focused runtime tests**

Run:

```bash
cargo test -p noema-core daemon::tests::runtime_actor_executes_web_search_as_local_tool_result daemon::tests::web_search_result_is_sent_as_native_tool_result_input
```

Expected: pass without live network access.

- [ ] **Step 3: Update durable context**

In `docs/context/current.md`, add one concise settled-decision bullet near the native tool plane entries:

```markdown
- The first web-search slice is planned as a first-party `web.search` native
  tool, not an MCP: the only initial provider is a Rust-only best-effort
  DuckDuckGo public adapter, model-proposed queries are visible in normal chat
  markers, and results return as normalized search metadata without fetching
  pages.
```

- [ ] **Step 4: Run full Rust validation**

Run:

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

Expected: all pass.

- [ ] **Step 5: Run frontend validation**

Run:

```bash
cd crates/noema-core/web
bun run gen:types
bun run lint
bun run build
```

Expected: all pass. No browser inspection is required because this is not a frontend UI layout change.

- [ ] **Step 6: Verify against the running agent on port 3737**

Run this against the already-running Noema web daemon:

```bash
CONVERSATION_ID="$(
  curl -sS \
    -H 'content-type: application/json' \
    -H 'origin: http://127.0.0.1:3737' \
    --data '{"query":"mutation { ensurePrimaryConversation { conversationId provider } }"}' \
    http://127.0.0.1:3737/graphql \
  | jq -r '.data.ensurePrimaryConversation.conversationId'
)"

CLIENT_MESSAGE_ID="web-search-e2e-$(date +%s)"

curl -sS \
  -H 'content-type: application/json' \
  -H 'origin: http://127.0.0.1:3737' \
  --data "$(jq -nc --arg conversationId "$CONVERSATION_ID" --arg clientMessageId "$CLIENT_MESSAGE_ID" '{
    query: "mutation SendConversationTurn($input: SendConversationTurnInput!) { sendConversationTurn(input: $input) { conversationId clientMessageId } }",
    variables: {
      input: {
        conversationId: $conversationId,
        input: "Use web.search to search the web for the official Rust programming language website and summarize the top result.",
        clientMessageId: $clientMessageId
      }
    }
  }')" \
  http://127.0.0.1:3737/graphql

for _ in $(seq 1 60); do
  TRANSCRIPT="$(
    curl -sS \
      -H 'content-type: application/json' \
      -H 'origin: http://127.0.0.1:3737' \
      --data "$(jq -nc --arg conversationId "$CONVERSATION_ID" '{
        query: "query Transcript($input: ConversationTranscriptPageInput!) { conversationTranscriptPage(input: $input) { items { item { __typename ... on ActivityTranscriptItem { activityKind status title metadata } ... on AssistantTextTranscriptItem { text } } } } }",
        variables: { input: { conversationId: $conversationId, limit: 80 } }
      }')" \
      http://127.0.0.1:3737/graphql
  )"
  echo "$TRANSCRIPT" | jq -e '
    any(.data.conversationTranscriptPage.items[].item; .title? == "Tool call: web.search")
    and any(.data.conversationTranscriptPage.items[].item; .title? == "Tool result: web.search")
    and any(.data.conversationTranscriptPage.items[].item; .metadata?.display?.target? | startswith("Web search:"))
  ' >/dev/null && break
  sleep 2
done

echo "$TRANSCRIPT" | jq -e '
  any(.data.conversationTranscriptPage.items[].item; .title? == "Tool call: web.search")
  and any(.data.conversationTranscriptPage.items[].item; .title? == "Tool result: web.search")
  and any(.data.conversationTranscriptPage.items[].item; .metadata?.display?.target? | startswith("Web search:"))
'
```

Expected: final `jq` exits 0. Add this exact note to the implementation summary:

```text
verified web.search working end2end by calling the running agent at :3737
```

- [ ] **Step 7: Run ship checklist before final commit**

Run:

```bash
git status --short --branch
git diff --check
git diff --cached --stat
git diff --cached --name-status
```

Expected: no whitespace errors. Staged files should be limited to web-search implementation files, generated frontend GraphQL types only if changed by `bun run gen:types`, and `docs/context/current.md`.

- [ ] **Step 8: Commit Task 5**

Run:

```bash
git add docs/context/current.md crates/noema-core/src crates/noema-core/Cargo.toml Cargo.toml Cargo.lock crates/noema-core/web/src/components/transcript/markerModel.ts crates/noema-core/web/src/components/transcript/markerModel.test.ts crates/noema-core/web/src/generated/graphql.ts
git commit -m "feat(search): add rust duckduckgo web search"
```

---

## Final Acceptance Criteria

- `web.search` is model-visible only through native tool-capable providers.
- `web.search` is not exposed through legacy builtin envelope fallback.
- Initial provider is only `duckduckgo_public`.
- No Python, MCP, browser, page fetch, cookies, or API keys are required.
- DuckDuckGo parsing is covered by local fixtures and ordinary tests do not hit the live internet.
- Tool call marker shows the exact query in normal chat.
- Tool result marker shows provider, reliability, and result count/summary.
- Search results are normalized metadata only: rank, title, URL, snippet.
- Failed searches persist as failed tool results and can be explained by the provider.
- The final implementation summary includes: `verified web.search working end2end by calling the running agent at :3737`.
- Rust and frontend validation commands pass.
