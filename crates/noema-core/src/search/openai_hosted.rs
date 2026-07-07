#![cfg_attr(not(test), allow(dead_code))]

use crate::search::types::{SearchError, SearchResponse, SearchResult};
use serde_json::Value;

pub(crate) const OPENAI_HOSTED_SEARCH_PROVIDER_ID: &str = "openai";
pub(crate) const OPENAI_HOSTED_SEARCH_CONTRACT: &str = "hosted_search";

pub(crate) fn normalize_openai_hosted_search_response(
    query: &str,
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
    use serde_json::json;

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
            normalize_openai_hosted_search_response("rust learn", &value).expect("normalized");

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
}
