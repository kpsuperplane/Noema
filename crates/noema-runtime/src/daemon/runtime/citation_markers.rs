//! Normalization for provider-owned web citation markers.

use std::collections::{HashMap, HashSet};

use noema_home::SystemErrorEvent;
use noema_providers::{GenerateCitation, GenerateHostedWebSearch};
use serde_json::json;
use url::Url;

use super::actor::RuntimeActor;

const MARKER_START: &str = "\u{e200}cite\u{e202}";
const MARKER_SEPARATOR: char = '\u{e202}';
const MARKER_END: char = '\u{e201}';

#[derive(Debug, Clone, PartialEq, Eq)]
struct CitationSource {
    title: String,
    url: String,
}

#[derive(Debug, Default)]
pub(crate) struct CitationSourceRegistry {
    sources: HashMap<String, CitationSource>,
    search_counts: HashMap<usize, usize>,
    view_counts: HashMap<usize, usize>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct NormalizedCitationText {
    pub(crate) text: String,
    pub(crate) citations: Vec<GenerateCitation>,
    pub(crate) unresolved_references: Vec<String>,
}

impl CitationSourceRegistry {
    pub(crate) fn observe(&mut self, round_index: usize, searches: &[GenerateHostedWebSearch]) {
        for search in searches {
            let is_view = search.tool_name == "web.fetch";
            let count = if is_view {
                self.view_counts.entry(round_index).or_default()
            } else {
                self.search_counts.entry(round_index).or_default()
            };
            let kind = if is_view { "view" } else { "search" };
            for source in &search.sources {
                let reference = format!("turn{round_index}{kind}{count}");
                *count += 1;
                let Ok(url) = Url::parse(&source.url) else {
                    continue;
                };
                if !matches!(url.scheme(), "http" | "https") {
                    continue;
                }
                let title = source
                    .title
                    .as_deref()
                    .map(str::trim)
                    .filter(|title| !title.is_empty())
                    .or_else(|| url.host_str().filter(|host| !host.is_empty()))
                    .unwrap_or(source.url.as_str())
                    .to_string();
                self.sources.insert(
                    reference,
                    CitationSource {
                        title,
                        url: source.url.clone(),
                    },
                );
            }
        }
    }

    pub(crate) fn normalize(
        &self,
        text: &str,
        existing: &[GenerateCitation],
    ) -> NormalizedCitationText {
        let mut normalized = String::with_capacity(text.len());
        let mut removals = Vec::new();
        let mut citations = Vec::new();
        let mut unresolved = Vec::new();
        let mut remaining = text;
        let mut raw_utf16 = 0;
        let mut seen = HashSet::new();

        while let Some(start) = remaining.find(MARKER_START) {
            let prefix = &remaining[..start];
            normalized.push_str(prefix);
            raw_utf16 += prefix.encode_utf16().count();
            let marker_body = &remaining[start + MARKER_START.len()..];
            let Some(end) = marker_body.find(MARKER_END) else {
                normalized.push_str(&remaining[start..]);
                remaining = "";
                break;
            };
            let marker =
                &remaining[start..start + MARKER_START.len() + end + MARKER_END.len_utf8()];
            let marker_end = raw_utf16 + marker.encode_utf16().count();
            let citation_end = normalized.encode_utf16().count();
            for reference in marker_body[..end].split(MARKER_SEPARATOR) {
                if let Some(source) = self.sources.get(reference) {
                    if seen.insert((source.url.clone(), citation_end)) {
                        citations.push(GenerateCitation {
                            title: source.title.clone(),
                            url: source.url.clone(),
                            start_index: None,
                            end_index: Some(citation_end),
                        });
                    }
                } else if !reference.is_empty() {
                    unresolved.push(reference.to_string());
                }
            }
            removals.push((raw_utf16, marker_end));
            raw_utf16 = marker_end;
            remaining = &remaining[start + marker.len()..];
        }
        normalized.push_str(remaining);

        citations.extend(existing.iter().cloned().map(|mut citation| {
            citation.start_index = citation
                .start_index
                .map(|index| adjust_index(index, &removals));
            citation.end_index = citation
                .end_index
                .map(|index| adjust_index(index, &removals));
            citation
        }));
        NormalizedCitationText {
            text: normalized,
            citations,
            unresolved_references: unresolved,
        }
    }
}

impl RuntimeActor {
    pub(super) fn normalize_provider_citation_text(
        &self,
        registry: &CitationSourceRegistry,
        text: &str,
        existing: &[GenerateCitation],
        scope_kind: &'static str,
        scope_id: &str,
    ) -> NormalizedCitationText {
        let normalized = registry.normalize(text, existing);
        if !normalized.unresolved_references.is_empty() {
            self.system_errors.try_append(
                SystemErrorEvent::new(
                    "provider_citation_unresolved",
                    "Provider citation references could not be resolved",
                )
                .with_context(json!({
                    "scope_kind": scope_kind,
                    "scope_id": scope_id,
                    "reference_count": normalized.unresolved_references.len(),
                })),
            );
        }
        normalized
    }
}

fn adjust_index(index: usize, removals: &[(usize, usize)]) -> usize {
    let removed = removals
        .iter()
        .map(|(start, end)| {
            if index >= *end {
                end - start
            } else if index > *start {
                index - start
            } else {
                0
            }
        })
        .sum::<usize>();
    index.saturating_sub(removed)
}

#[cfg(test)]
mod tests {
    use noema_providers::GenerateWebSource;

    use super::*;

    #[test]
    fn resolves_markers_and_preserves_utf16_positions() {
        let mut registry = CitationSourceRegistry::default();
        registry.observe(
            0,
            &[GenerateHostedWebSearch {
                output_index: 0,
                id: None,
                tool_name: "web.search".to_string(),
                arguments: serde_json::json!({}),
                result: serde_json::json!({}),
                status: "completed".to_string(),
                sources: vec![
                    GenerateWebSource {
                        title: Some("First source".to_string()),
                        url: "https://one.example/a".to_string(),
                    },
                    GenerateWebSource {
                        title: None,
                        url: "https://two.example/b".to_string(),
                    },
                ],
            }],
        );
        let text = "😀 claim \u{e200}cite\u{e202}turn0search0\u{e202}turn0search1\u{e201}";
        let result = registry.normalize(text, &[]);

        assert_eq!(result.text, "😀 claim ");
        assert_eq!(result.citations.len(), 2);
        assert_eq!(result.citations[0].title, "First source");
        assert_eq!(result.citations[1].title, "two.example");
        assert!(
            result
                .citations
                .iter()
                .all(|citation| citation.start_index.is_none())
        );
        assert!(
            result
                .citations
                .iter()
                .all(|citation| citation.end_index == Some(9))
        );
        assert!(result.unresolved_references.is_empty());
    }

    #[test]
    fn removes_unresolved_markers_without_inventing_sources() {
        let result = CitationSourceRegistry::default()
            .normalize("claim \u{e200}cite\u{e202}turn1view1\u{e201} end", &[]);

        assert_eq!(result.text, "claim  end");
        assert!(result.citations.is_empty());
        assert_eq!(result.unresolved_references, ["turn1view1"]);
    }

    #[test]
    fn resolves_continuation_views_and_adjusts_existing_annotations() {
        let mut registry = CitationSourceRegistry::default();
        registry.observe(
            1,
            &[
                GenerateHostedWebSearch {
                    output_index: 0,
                    id: None,
                    tool_name: "web.fetch".to_string(),
                    arguments: serde_json::json!({}),
                    result: serde_json::json!({}),
                    status: "completed".to_string(),
                    sources: vec![GenerateWebSource {
                        title: None,
                        url: "https://view.example/a".to_string(),
                    }],
                },
                GenerateHostedWebSearch {
                    output_index: 1,
                    id: None,
                    tool_name: "web.fetch".to_string(),
                    arguments: serde_json::json!({}),
                    result: serde_json::json!({}),
                    status: "completed".to_string(),
                    sources: vec![GenerateWebSource {
                        title: None,
                        url: "https://find.example/b".to_string(),
                    }],
                },
            ],
        );
        let marker = "\u{e200}cite\u{e202}turn1view1\u{e202}turn1view1\u{e201}";
        let text = format!("claim{marker} tail");
        let existing = GenerateCitation {
            title: "Existing".to_string(),
            url: "https://existing.example".to_string(),
            start_index: Some(0),
            end_index: Some(text.encode_utf16().count()),
        };

        let result = registry.normalize(&text, &[existing]);

        assert_eq!(result.text, "claim tail");
        assert_eq!(result.citations.len(), 2);
        assert_eq!(result.citations[0].url, "https://find.example/b");
        assert_eq!(result.citations[0].end_index, Some(5));
        assert_eq!(result.citations[1].end_index, Some(10));
    }
}
