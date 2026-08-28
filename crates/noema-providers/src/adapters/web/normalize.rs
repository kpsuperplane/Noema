//! Provider-neutral normalization for hosted web adapters.

use crate::WebFetchError;
use noema_capabilities::web::{
    fetch::{FetchContentKind, FetchResponse, FetchSummaryStrategy, sanitized_display_url},
    search::{SearchResponse, SearchResult},
    url_policy::{PublicUrlError, validate_public_url},
};
use std::collections::HashSet;
pub(super) struct SearchCandidate {
    pub title: String,
    pub url: String,
    pub snippet: String,
}
pub(super) fn search_response(
    provider: &str,
    contract: &str,
    query: &str,
    max_results: usize,
    candidates: impl IntoIterator<Item = SearchCandidate>,
) -> SearchResponse {
    let results = candidates
        .into_iter()
        .filter_map(|candidate| {
            let checked = validate_public_url(candidate.url.trim()).ok()?;
            let url = sanitized_display_url(checked.as_str());
            let title = normalize_text(&candidate.title).unwrap_or_else(|| url.clone());
            Some((
                title,
                url,
                normalize_text(&candidate.snippet).unwrap_or_default(),
            ))
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
    SearchResponse {
        provider: provider.to_string(),
        provider_contract: contract.to_string(),
        query: query.to_string(),
        results,
        summary,
    }
}
pub(super) fn raw_markdown_response(
    identity: (&str, &str),
    requested_url: &str,
    final_url: Option<&str>,
    title: Option<&str>,
    links: impl IntoIterator<Item = String>,
    markdown: &str,
    max_chars: usize,
) -> Result<FetchResponse, WebFetchError> {
    let (provider, extraction) = identity;
    let url = checked_display_url(requested_url)?;
    let final_url = checked_display_url(final_url.unwrap_or(requested_url))?;
    let mut seen = HashSet::new();
    let links = links
        .into_iter()
        .filter_map(|link| checked_display_url(&link).ok())
        .filter(|link| seen.insert(link.clone()))
        .take(256)
        .collect();
    let raw_chars = markdown.chars().count();
    let content = markdown.chars().take(max_chars).collect::<String>();
    let returned_chars = content.chars().count();
    Ok(FetchResponse {
        provider: provider.to_string(),
        url,
        final_url,
        title: title.and_then(normalize_text),
        links,
        format: "markdown".to_string(),
        extraction: extraction.to_string(),
        content_kind: FetchContentKind::RawMarkdown,
        content,
        raw_excerpt: None,
        raw_chars,
        returned_chars,
        summary_model: None,
        summary_strategy: FetchSummaryStrategy::NotSummarized,
        truncated: raw_chars > returned_chars,
    })
}
pub(super) fn validate_fetch_url(url: &str) -> Result<(), WebFetchError> {
    checked_display_url(url).map(|_| ())
}
fn checked_display_url(url: &str) -> Result<String, WebFetchError> {
    validate_public_url(url)
        .map(|url| sanitized_display_url(url.as_str()))
        .map_err(map_url_error)
}
fn normalize_text(value: &str) -> Option<String> {
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    (!value.is_empty()).then_some(value)
}
fn map_url_error(error: PublicUrlError) -> WebFetchError {
    match error {
        PublicUrlError::UnsupportedScheme => WebFetchError::UnsupportedScheme,
        PublicUrlError::Malformed => WebFetchError::MalformedUrl,
        PublicUrlError::BlockedTarget => WebFetchError::BlockedTarget,
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_normalization_filters_bounds_and_preserves_ordinary_values() {
        let search = search_response(
            "test",
            "hosted_provider",
            "query",
            2,
            [
                SearchCandidate {
                    title: " blocked ".to_string(),
                    url: "http://127.0.0.1/private".to_string(),
                    snippet: String::new(),
                },
                SearchCandidate {
                    title: "  One\n title ".to_string(),
                    url: "https://example.org/path?q=ordinary#section".to_string(),
                    snippet: " one\n snippet ".to_string(),
                },
            ],
        );
        assert_eq!(search.results[0].rank, 1);
        assert_eq!(search.results[0].title, "One title");
        assert_eq!(search.results[0].snippet, "one snippet");
        assert_eq!(
            search.results[0].url,
            "https://example.org/path?q=ordinary#section"
        );

        let fetch = raw_markdown_response(
            ("test", "test_markdown"),
            "https://example.org/request?q=ordinary",
            Some("https://example.org/final#section"),
            Some("  Page\n title "),
            std::iter::once("https://user:secret@example.org/private".to_string())
                .chain(std::iter::once("https://example.org/0".to_string()))
                .chain((0..260).map(|index| format!("https://example.org/{index}"))),
            "aébc",
            3,
        )
        .expect("normalized fetch");
        assert_eq!(fetch.content, "aéb");
        assert_eq!((fetch.raw_chars, fetch.returned_chars), (4, 3));
        assert!(fetch.truncated);
        assert_eq!(fetch.links.len(), 256);
        assert_eq!(
            &fetch.links[..2],
            ["https://example.org/0", "https://example.org/1"]
        );
        assert_eq!(fetch.title.as_deref(), Some("Page title"));
    }
}
