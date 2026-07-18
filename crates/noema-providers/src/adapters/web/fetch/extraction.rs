//! Readability extraction helpers for web fetch.

use crate::WebFetchError;
use readabilityrs::{Article, Readability, ReadabilityOptions};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExtractedContent {
    pub(super) title: Option<String>,
    pub(super) markdown: String,
}

pub(super) fn extract_readable_content(
    html: &str,
    final_url: &str,
) -> Result<ExtractedContent, WebFetchError> {
    let options = ReadabilityOptions::builder().output_markdown(true).build();
    let article: Article = Readability::new(html, Some(final_url), Some(options))
        .map_err(|_| WebFetchError::Extraction)?
        .parse()
        .ok_or(WebFetchError::Extraction)?;
    let mut markdown = normalize_newlines(
        article
            .markdown_content
            .or(article.text_content)
            .or(article.content)
            .unwrap_or_default()
            .as_str(),
    );
    let title = article
        .title
        .map(|title| title.trim().to_string())
        .filter(|title| !title.is_empty());
    if let Some(title) = title.as_deref()
        && !markdown.contains(title)
    {
        markdown = format!("# {title}\n\n{markdown}");
    }
    if markdown.chars().count() < 20 {
        return Err(WebFetchError::Extraction);
    }
    Ok(ExtractedContent { title, markdown })
}

#[must_use]
pub(super) fn normalize_plain_text(text: &str) -> ExtractedContent {
    ExtractedContent {
        title: None,
        markdown: normalize_newlines(text),
    }
}

fn normalize_newlines(text: &str) -> String {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}
