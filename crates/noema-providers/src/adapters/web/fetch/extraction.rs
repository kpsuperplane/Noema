//! Readability extraction helpers for web fetch.

use crate::WebFetchError;
use readabilityrs::{Article, Readability, ReadabilityOptions};
use scraper::{Html, Selector};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExtractedContent {
    pub(super) title: Option<String>,
    pub(super) markdown: String,
    pub(super) links: Vec<String>,
}

pub(super) fn extract_readable_content(
    html: &str,
    final_url: &str,
) -> Result<ExtractedContent, WebFetchError> {
    let links = extract_public_links(html, final_url);
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
    Ok(ExtractedContent {
        title,
        markdown,
        links,
    })
}

#[must_use]
pub(super) fn normalize_plain_text(text: &str) -> ExtractedContent {
    ExtractedContent {
        title: None,
        markdown: normalize_newlines(text),
        links: Vec::new(),
    }
}

fn extract_public_links(html: &str, final_url: &str) -> Vec<String> {
    let Ok(base) = url::Url::parse(final_url) else {
        return Vec::new();
    };
    let selector = Selector::parse("a[href]").expect("static link selector");
    let mut links = std::collections::BTreeSet::new();
    for element in Html::parse_document(html).select(&selector) {
        let Some(href) = element.value().attr("href") else {
            continue;
        };
        let Ok(url) = base.join(href) else {
            continue;
        };
        if let Ok(url) = noema_capabilities::web::url_policy::normalize_observed_url(url.as_str()) {
            links.insert(url);
            if links.len() == 256 {
                break;
            }
        }
    }
    links.into_iter().collect()
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
