//! Normalization for provider-owned web citation markers.

use std::collections::{BTreeMap, HashMap, HashSet};

use noema_home::SystemErrorEvent;
use noema_providers::GenerateCitation;
use serde_json::json;
use url::Url;

use super::actor::RuntimeActor;

const MARKER_START: &str = "\u{e200}cite\u{e202}";
const MARKER_SEPARATOR: char = '\u{e202}';
const MARKER_END: char = '\u{e201}';
const TASK_SOURCE_PREFIX: &str = "[^noema-source-";

#[derive(Debug, Clone, PartialEq, Eq)]
struct CitationSource {
    title: String,
    url: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct NormalizedCitationText {
    pub(crate) text: String,
    pub(crate) citations: Vec<GenerateCitation>,
    pub(crate) unresolved_references: Vec<String>,
}

/// Project one stored Task result into the provider citation contract.
#[must_use]
pub fn project_task_result(text: &str) -> (String, Vec<GenerateCitation>) {
    let decoded = decode_task_sources(text);
    (decoded.text, decoded.citations)
}

pub(crate) fn normalize(text: &str, existing: &[GenerateCitation]) -> NormalizedCitationText {
    let (text, existing) = strip_links(text, existing);
    let mut normalized = String::with_capacity(text.len());
    let mut removals = Vec::new();
    let mut citations = Vec::new();
    let mut unresolved = Vec::new();
    let mut remaining = text.as_str();
    let mut raw_utf16 = 0;
    let mut seen = HashSet::new();

    while let Some(start) = remaining.find(MARKER_START) {
        let prefix = &remaining[..start];
        normalized.push_str(prefix);
        raw_utf16 += prefix.encode_utf16().count();
        let marker_body = &remaining[start + MARKER_START.len()..];
        let Some(end) = marker_body.find(MARKER_END) else {
            let end = marker_body
                .find(char::is_whitespace)
                .unwrap_or(marker_body.len());
            let malformed = &remaining[start..start + MARKER_START.len() + end];
            normalized.push_str(malformed);
            let length = malformed.encode_utf16().count();
            unresolved.push(marker_body[..end].to_string());
            raw_utf16 += length;
            remaining = &marker_body[end..];
            continue;
        };
        let marker = &remaining[start..start + MARKER_START.len() + end + MARKER_END.len_utf8()];
        let marker_end = raw_utf16 + marker.encode_utf16().count();
        let citation_end = normalized.encode_utf16().count();
        let references = marker_body[..end]
            .split(MARKER_SEPARATOR)
            .filter(|reference| !reference.is_empty())
            .collect::<Vec<_>>();
        let annotation_resolves_marker = existing.iter().any(|citation| {
            citation
                .start_index
                .zip(citation.end_index)
                .is_some_and(|(start, end)| start < marker_end && end > raw_utf16)
        });
        let resolved_sources = references
            .iter()
            .filter_map(|reference| citation_source(reference))
            .collect::<Vec<_>>();
        if annotation_resolves_marker || resolved_sources.len() == references.len() {
            for source in resolved_sources {
                if seen.insert((source.url.clone(), citation_end)) {
                    citations.push(GenerateCitation {
                        title: source.title,
                        url: source.url,
                        start_index: None,
                        end_index: Some(citation_end),
                    });
                }
            }
            removals.push((raw_utf16, marker_end));
        } else {
            normalized.push_str(marker);
            unresolved.extend(
                references
                    .into_iter()
                    .filter(|reference| citation_source(reference).is_none())
                    .map(ToString::to_string),
            );
        }
        raw_utf16 = marker_end;
        remaining = &remaining[start + marker.len()..];
    }
    normalized.push_str(remaining);

    citations.extend(existing.into_iter().map(|mut citation| {
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

fn citation_source(reference: &str) -> Option<CitationSource> {
    let url = Url::parse(reference).ok()?;
    matches!(url.scheme(), "http" | "https").then(|| CitationSource {
        title: url
            .host_str()
            .filter(|host| !host.is_empty())
            .unwrap_or(reference)
            .to_string(),
        url: url.to_string(),
    })
}

pub(crate) fn normalize_task_result(text: &str) -> NormalizedCitationText {
    let decoded = decode_task_sources(text);
    let mut normalized = normalize(&decoded.text, &decoded.citations);
    normalized
        .unresolved_references
        .extend(decoded.unresolved_references);
    normalized.text = encode_task_sources(&normalized.text, &normalized.citations);
    normalized
}

fn decode_task_sources(text: &str) -> NormalizedCitationText {
    let mut body = String::with_capacity(text.len());
    let mut sources = HashMap::new();
    let mut unresolved = Vec::new();
    for line in text.split_inclusive('\n') {
        let value = line.trim_end_matches(['\r', '\n']);
        if value.starts_with(TASK_SOURCE_PREFIX) && value.contains("]:") {
            match parse_task_source_definition(value) {
                Some((number, source)) => {
                    sources.entry(number).or_insert(source);
                }
                None => unresolved.push(value.to_string()),
            }
        } else {
            body.push_str(line);
        }
    }

    let mut clean = String::with_capacity(body.len());
    let mut citations = Vec::new();
    let mut remaining = body.as_str();
    while let Some(start) = remaining.find(TASK_SOURCE_PREFIX) {
        clean.push_str(&remaining[..start]);
        let marker = &remaining[start..];
        let Some(close) = marker.find(']') else {
            let end = marker.find(char::is_whitespace).unwrap_or(marker.len());
            unresolved.push(marker[..end].to_string());
            remaining = &remaining[start + end..];
            continue;
        };
        let marker = &marker[..=close];
        let number = marker
            .strip_prefix(TASK_SOURCE_PREFIX)
            .and_then(|value| value.strip_suffix(']'))
            .and_then(|value| value.parse::<usize>().ok());
        if let Some(source) = number.and_then(|number| sources.get(&number)) {
            citations.push(GenerateCitation {
                title: source.title.clone(),
                url: source.url.clone(),
                start_index: None,
                end_index: Some(clean.encode_utf16().count()),
            });
        } else {
            unresolved.push(marker.to_string());
        }
        remaining = &remaining[start + marker.len()..];
    }
    clean.push_str(remaining);
    NormalizedCitationText {
        text: clean,
        citations,
        unresolved_references: unresolved,
    }
}

fn parse_task_source_definition(value: &str) -> Option<(usize, CitationSource)> {
    let (marker, definition) = value.split_once(": [")?;
    let number = marker
        .strip_prefix(TASK_SOURCE_PREFIX)?
        .strip_suffix(']')?
        .parse::<usize>()
        .ok()?;
    let (title, url_and_locator) = definition.rsplit_once("](<")?;
    let (url, locator) = url_and_locator.split_once(">)")?;
    let url = Url::parse(url).ok()?;
    let mut title = title
        .replace("\\]", "]")
        .replace("\\[", "[")
        .replace("\\\\", "\\");
    if !locator.trim().is_empty() {
        title.push(' ');
        title.push_str(locator.trim());
    }
    (!title.trim().is_empty() && is_task_source_url(&url)).then(|| {
        (
            number,
            CitationSource {
                title,
                url: url.to_string(),
            },
        )
    })
}

fn encode_task_sources(text: &str, citations: &[GenerateCitation]) -> String {
    let total = text.encode_utf16().count();
    let mut ordered = citations.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|citation| citation.end_index.unwrap_or(total));
    let mut number_by_url = HashMap::new();
    let mut sources = Vec::new();
    let mut markers = BTreeMap::<usize, Vec<usize>>::new();
    for citation in ordered {
        let Ok(url) = Url::parse(&citation.url) else {
            continue;
        };
        let end = citation.end_index.unwrap_or(total);
        if citation.title.trim().is_empty()
            || !is_task_source_url(&url)
            || citation.start_index.is_some_and(|start| start >= end)
            || utf16_slice(text, 0, end).is_none()
        {
            continue;
        }
        let number = *number_by_url.entry(url.to_string()).or_insert_with(|| {
            sources.push(CitationSource {
                title: citation.title.trim().to_string(),
                url: url.to_string(),
            });
            sources.len()
        });
        if !markers.entry(end).or_default().contains(&number) {
            markers.entry(end).or_default().push(number);
        }
    }
    if sources.is_empty() {
        return text.to_string();
    }

    let mut output = String::with_capacity(text.len() + sources.len() * 64);
    let mut prior = 0;
    for (end, numbers) in markers {
        output.push_str(utf16_slice(text, prior, end).expect("validated citation offset"));
        for number in numbers {
            output.push_str(&format!("[^noema-source-{number}]"));
        }
        prior = end;
    }
    output.push_str(utf16_slice(text, prior, total).expect("validated citation tail"));
    if !output.ends_with("\n\n") {
        if !output.ends_with('\n') {
            output.push('\n');
        }
        output.push('\n');
    }
    for (index, source) in sources.into_iter().enumerate() {
        let title = escape_task_source_title(&source.title);
        output.push_str(&format!(
            "[^noema-source-{}]: [{title}](<{}>)\n",
            index + 1,
            source.url
        ));
    }
    output
}

fn is_task_source_url(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https" | "artifact")
}

fn escape_task_source_title(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace(['\r', '\n'], " ")
}

fn strip_links(text: &str, citations: &[GenerateCitation]) -> (String, Vec<GenerateCitation>) {
    let annotated_ranges = citations
        .iter()
        .filter_map(|citation| {
            let (start, end) = (citation.start_index?, citation.end_index?);
            let value = utf16_slice(text, start, end)?;
            is_parenthesized_domain_link(value, &citation.url).then_some((start, end))
        })
        .collect::<HashSet<_>>();
    if annotated_ranges.is_empty() {
        return (text.to_string(), citations.to_vec());
    }

    let citation_start = |start| {
        (start > 0 && utf16_slice(text, start - 1, start) == Some(" "))
            .then(|| start - 1)
            .unwrap_or(start)
    };
    let mut removals = annotated_ranges
        .iter()
        .map(|&(start, end)| (citation_start(start), end))
        .collect::<Vec<_>>();
    removals.sort_unstable();

    let mut cleaned = String::with_capacity(text.len());
    let mut prior_end = 0;
    for &(start, end) in &removals {
        cleaned.push_str(utf16_slice(text, prior_end, start).expect("validated citation range"));
        prior_end = end;
    }
    cleaned.push_str(
        utf16_slice(text, prior_end, text.encode_utf16().count())
            .expect("validated citation range"),
    );

    let citations = citations
        .iter()
        .cloned()
        .map(|mut citation| {
            let annotated_range = citation.start_index.zip(citation.end_index);
            if annotated_range.is_some_and(|range| annotated_ranges.contains(&range)) {
                citation.start_index = None;
                citation.end_index = annotated_range
                    .map(|(start, _)| adjust_index(citation_start(start), &removals));
            } else {
                citation.start_index = citation
                    .start_index
                    .map(|index| adjust_index(index, &removals));
                citation.end_index = citation
                    .end_index
                    .map(|index| adjust_index(index, &removals));
            }
            citation
        })
        .collect();
    (cleaned, citations)
}

fn is_parenthesized_domain_link(value: &str, expected_url: &str) -> bool {
    let Some(link) = value
        .strip_prefix("([")
        .and_then(|value| value.strip_suffix("))"))
    else {
        return false;
    };
    let Some((label, url)) = link.rsplit_once("](") else {
        return false;
    };
    let Ok(expected) = Url::parse(expected_url) else {
        return false;
    };
    let Some(host) = expected.host_str() else {
        return false;
    };
    url == expected_url
        && label
            .trim_start_matches("www.")
            .eq_ignore_ascii_case(host.trim_start_matches("www."))
}

fn utf16_slice(text: &str, start: usize, end: usize) -> Option<&str> {
    (start <= end).then_some(())?;
    let mut start_byte = None;
    let mut units = 0;
    for (byte, character) in text.char_indices() {
        if units == start {
            start_byte = Some(byte);
        }
        if units == end {
            return Some(&text[start_byte?..byte]);
        }
        units += character.len_utf16();
        if units > end || (start_byte.is_none() && units > start) {
            return None;
        }
    }
    (units == end).then(|| &text[start_byte.unwrap_or(text.len())..])
}

impl RuntimeActor {
    pub(super) fn normalize_provider_citation_text(
        &self,
        text: &str,
        existing: &[GenerateCitation],
        scope_kind: &'static str,
        scope_id: &str,
    ) -> NormalizedCitationText {
        let normalized = normalize(text, existing);
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
                    "references": &normalized.unresolved_references,
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
    use super::*;

    #[test]
    fn structured_annotation_resolves_private_marker() {
        let prefix = "😀 claim ";
        let text = format!("{prefix}\u{e200}cite\u{e202}turn5search4\u{e201}");
        let citation = GenerateCitation {
            title: "Official source".to_string(),
            url: "https://one.example/a".to_string(),
            start_index: Some(prefix.encode_utf16().count()),
            end_index: Some(text.encode_utf16().count()),
        };
        let result = normalize(&text, &[citation]);

        assert_eq!(result.text, prefix);
        assert_eq!(result.citations.len(), 1);
        assert_eq!(result.citations[0].title, "Official source");
        assert_eq!(result.citations[0].end_index, Some(9));
        assert!(result.unresolved_references.is_empty());
    }

    #[test]
    fn preserves_unresolved_markers_without_inventing_sources() {
        let result = normalize("claim \u{e200}cite\u{e202}turn1view1\u{e201} end", &[]);

        assert_eq!(
            result.text,
            "claim \u{e200}cite\u{e202}turn1view1\u{e201} end"
        );
        assert!(result.citations.is_empty());
        assert_eq!(result.unresolved_references, ["turn1view1"]);
    }

    #[test]
    fn resolves_direct_https_markers_without_fetching() {
        let result = normalize(
            "claim \u{e200}cite\u{e202}https://example.com/news?id=1\u{e201}",
            &[],
        );

        assert_eq!(result.text, "claim ");
        assert_eq!(result.citations.len(), 1);
        assert_eq!(result.citations[0].title, "example.com");
        assert_eq!(result.citations[0].url, "https://example.com/news?id=1");
        assert_eq!(result.citations[0].end_index, Some(6));
        assert!(result.unresolved_references.is_empty());
    }

    #[test]
    fn annotation_range_adjusts_after_private_marker_removal() {
        let marker = "\u{e200}cite\u{e202}turn1view1\u{e201}";
        let text = format!("claim{marker} tail");
        let existing = GenerateCitation {
            title: "Existing".to_string(),
            url: "https://existing.example".to_string(),
            start_index: Some(0),
            end_index: Some(text.encode_utf16().count()),
        };

        let result = normalize(&text, &[existing]);

        assert_eq!(result.text, "claim tail");
        assert_eq!(result.citations.len(), 1);
        assert_eq!(result.citations[0].url, "https://existing.example");
        assert_eq!(result.citations[0].end_index, Some(10));
    }

    #[test]
    fn removes_annotated_domain_suffix_but_preserves_answer_links() {
        let url = "https://one.example/menu";
        let prefix = "😀 Claim.";
        let suffix = format!("{prefix} ([one.example]({url}))");
        let suffix_start = prefix.encode_utf16().count() + 1;
        let requested = format!("Open the [menu]({url}).");
        let requested_start = requested.find("[menu]").unwrap();
        let citation = |start, end| GenerateCitation {
            title: "Menu".to_string(),
            url: url.to_string(),
            start_index: Some(start),
            end_index: Some(end),
        };

        let stripped = normalize(
            &suffix,
            &[citation(suffix_start, suffix.encode_utf16().count())],
        );
        assert_eq!(stripped.text, prefix);
        assert_eq!(stripped.citations[0].start_index, None);
        assert_eq!(stripped.citations[0].end_index, Some(9));

        let leading = format!("([one.example]({url}))");
        let stripped = normalize(&leading, &[citation(0, leading.encode_utf16().count())]);
        assert_eq!(stripped.text, "");
        assert_eq!(stripped.citations[0].end_index, Some(0));

        let preserved = normalize(
            &requested,
            &[citation(requested_start, requested.len() - 1)],
        );
        assert_eq!(preserved.text, requested);
    }
    #[test]
    fn task_result_merges_durable_and_provider_sources() {
        let marker =
            "\u{e200}cite\u{e202}https://old.example/a\u{e202}https://new.example/b\u{e201}";
        let input = format!(
            "Old[^noema-source-1]. New{marker}\n\n[^note]: keep\n[^noema-source-1]: [Old](<https://old.example/a>)\n"
        );
        let result = normalize_task_result(&input);
        assert_eq!(
            result.text,
            "Old[^noema-source-1]. New[^noema-source-1][^noema-source-2]\n\n[^note]: keep\n\n[^noema-source-1]: [Old](<https://old.example/a>)\n[^noema-source-2]: [new.example](<https://new.example/b>)\n"
        );
        assert_eq!(normalize_task_result(&result.text).text, result.text);
    }

    #[test]
    fn task_result_preserves_local_artifact_source_and_locator() {
        let input = "Desk[^noema-source-1].\n\n[^noema-source-1]: [Desk photo](<artifact:123>) — OCR text block; owner: Kevin; disclosure: private.\n";
        let result = normalize_task_result(input);
        assert!(result.unresolved_references.is_empty());
        assert!(result.text.contains("Desk[^noema-source-1]."));
        assert!(result.text.contains("artifact:123"));
        assert!(
            result
                .text
                .contains("OCR text block; owner: Kevin; disclosure: private.")
        );
        assert_eq!(normalize_task_result(&result.text).text, result.text);
    }
}
