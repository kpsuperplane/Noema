//! Readability extraction helpers for web fetch.

use crate::web_fetch::types::FetchError;
use readabilityrs::{Article, Readability, ReadabilityOptions};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExtractedContent {
    pub title: Option<String>,
    pub markdown: String,
}

pub(crate) fn extract_readable_content(
    html: &str,
    final_url: &str,
) -> Result<ExtractedContent, FetchError> {
    let options = ReadabilityOptions::builder().output_markdown(true).build();
    let article: Article = Readability::new(html, Some(final_url), Some(options))
        .map_err(|_| FetchError::Extraction)?
        .parse()
        .ok_or(FetchError::Extraction)?;
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
        return Err(FetchError::Extraction);
    }
    Ok(ExtractedContent { title, markdown })
}

pub(crate) fn normalize_plain_text(text: &str) -> ExtractedContent {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_html_to_markdown() {
        let html = r#"
        <html>
          <head><title>Rust Learn</title></head>
          <body>
            <article><h1>Rust Learn</h1><p>Reliable systems programming.</p></article>
          </body>
        </html>
        "#;

        let extracted =
            extract_readable_content(html, "https://www.rust-lang.org/learn").expect("extract");

        assert_eq!(extracted.title.as_deref(), Some("Rust Learn"));
        assert!(extracted.markdown.contains("Rust Learn"));
        assert!(extracted.markdown.contains("Reliable systems programming."));
    }

    #[test]
    fn normalizes_plain_text_as_markdown() {
        let extracted = normalize_plain_text("  one\r\n\r\ntwo  ");

        assert_eq!(extracted.title, None);
        assert_eq!(extracted.markdown, "one\n\ntwo");
    }
}
