use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    path::{Component, Path},
};

use ring::digest::{Context, SHA256};
use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use unicode_segmentation::UnicodeSegmentation;

use super::{
    MEMORY_MAX_WORDS, MEMORY_PAGE_ICON_KEYS, MemoryChangeSet, MemoryState, NativeMemoryError,
    ROOT_PAGE_PATH,
};

#[derive(Debug)]
pub(super) struct ParsedPage {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) icon: String,
    pub(super) body: String,
    pub(super) hash: String,
    pub(super) created_at: String,
    pub(super) sources: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct PendingPayload {
    pub(super) pages: Vec<StagedPage>,
    pub(super) deletes: Vec<String>,
    pub(super) state: MemoryState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct StagedPage {
    pub(super) path: String,
    pub(super) bytes: Vec<u8>,
}

pub(super) fn walk_pages(root: &Path, current: &Path, output: &mut Vec<String>) -> io::Result<()> {
    for entry in std::fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        if name == ".pending" || name == ".state.md" || name == "memory.sqlite3" {
            continue;
        }
        if path.is_dir() {
            walk_pages(root, &path, output)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("md")
            && let Ok(relative) = path.strip_prefix(root)
            && let Some(relative) = relative.to_str()
        {
            output.push(relative.replace(std::path::MAIN_SEPARATOR, "/"));
        }
    }
    Ok(())
}

pub(super) fn split_frontmatter(
    content: &str,
) -> Result<(BTreeMap<String, String>, String), NativeMemoryError> {
    let mut lines = content.lines();
    if lines.next() != Some("---") {
        return Err(NativeMemoryError::InvalidPage(
            "missing frontmatter".to_string(),
        ));
    }
    let mut frontmatter = BTreeMap::new();
    for line in &mut lines {
        if line == "---" {
            return Ok((frontmatter, lines.collect::<Vec<_>>().join("\n")));
        }
        if let Some((key, value)) = line.split_once(':') {
            frontmatter.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    Err(NativeMemoryError::InvalidPage(
        "unterminated frontmatter".to_string(),
    ))
}

pub(super) fn source_manifest(content: &str) -> Vec<String> {
    let mut in_frontmatter = false;
    let mut in_sources = false;
    let mut sources = Vec::new();
    for line in content.lines() {
        if line == "---" {
            if in_frontmatter {
                break;
            }
            in_frontmatter = true;
            continue;
        }
        if !in_frontmatter {
            continue;
        }
        if line == "sources:" {
            in_sources = true;
            continue;
        }
        if in_sources {
            if let Some(source) = line.strip_prefix("  - ") {
                sources.push(source.to_string());
            } else {
                in_sources = false;
            }
        }
    }
    sources
}

pub(super) fn validate_change_set(changes: &MemoryChangeSet) -> Result<(), NativeMemoryError> {
    let mut seen = std::collections::HashSet::new();
    let mut changed_ids = std::collections::HashSet::new();
    for change in &changes.upserts {
        let path = normalize_page_path(&change.path)?;
        if !seen.insert(path.clone()) {
            return Err(NativeMemoryError::InvalidChangeSet(format!(
                "duplicate path {path}"
            )));
        }
        if change.title.trim().is_empty() {
            return Err(NativeMemoryError::InvalidChangeSet(format!(
                "empty title for {path}"
            )));
        }
        validate_frontmatter_value("title", &change.title)?;
        validate_frontmatter_value("icon", &change.icon)?;
        if !MEMORY_PAGE_ICON_KEYS.contains(&change.icon.as_str()) {
            return Err(NativeMemoryError::InvalidChangeSet(format!(
                "unsupported icon {} for {path}",
                change.icon
            )));
        }
        if let Some(id) = &change.id {
            validate_frontmatter_value("id", id)?;
            if !changed_ids.insert(id.clone()) {
                return Err(NativeMemoryError::InvalidChangeSet(format!(
                    "stable id appears more than once in change set: {id}"
                )));
            }
        }
        for source in &change.sources {
            validate_frontmatter_value("source", source)?;
        }
        if change.body.lines().any(|line| line.starts_with("# ")) {
            return Err(NativeMemoryError::InvalidChangeSet(format!(
                "body for {path} must not repeat the generated title heading"
            )));
        }
        validate_page_content(&change.body)?;
        validate_article_structure(&path, &change.body)?;
        validate_citations(&change.body, &change.sources)?;
    }
    for path in &changes.deletes {
        let path = normalize_page_path(path)?;
        if path == ROOT_PAGE_PATH {
            return Err(NativeMemoryError::InvalidChangeSet(
                "root.md cannot be deleted".to_string(),
            ));
        }
        if !seen.insert(path.clone()) {
            return Err(NativeMemoryError::InvalidChangeSet(format!(
                "path appears more than once in change set: {path}"
            )));
        }
    }
    Ok(())
}

pub(super) fn validate_article_structure(path: &str, body: &str) -> Result<(), NativeMemoryError> {
    let words = UnicodeSegmentation::unicode_words(body).count();
    if path == ROOT_PAGE_PATH && words >= 120 {
        let sections = body.lines().filter(|line| line.starts_with("## ")).count();
        if sections < 2 {
            return Err(NativeMemoryError::InvalidChangeSet(
                "a developed root.md article must organize its prose under at least two ## sections"
                    .to_string(),
            ));
        }
    }
    Ok(())
}

fn validate_frontmatter_value(label: &str, value: &str) -> Result<(), NativeMemoryError> {
    if value.trim().is_empty() || value.trim() != value || value.chars().any(char::is_control) {
        return Err(NativeMemoryError::InvalidChangeSet(format!(
            "{label} must be a non-empty single-line value"
        )));
    }
    Ok(())
}

fn validate_citations(body: &str, sources: &[String]) -> Result<(), NativeMemoryError> {
    let mut references = BTreeSet::new();
    let mut definitions = BTreeMap::new();
    let mut remainder = body;
    while let Some(start) = remainder.find("[^") {
        remainder = &remainder[start + 2..];
        let Some(end) = remainder.find(']') else {
            return Err(NativeMemoryError::InvalidChangeSet(
                "unterminated footnote citation".to_string(),
            ));
        };
        let label = &remainder[..end];
        if label.is_empty() || label.chars().any(char::is_whitespace) {
            return Err(NativeMemoryError::InvalidChangeSet(
                "footnote labels must be non-empty and contain no whitespace".to_string(),
            ));
        }
        remainder = &remainder[end + 1..];
        if let Some(definition) = remainder.strip_prefix(':') {
            let source = definition
                .lines()
                .next()
                .unwrap_or_default()
                .trim()
                .trim_matches('`')
                .trim();
            if source.is_empty()
                || definitions
                    .insert(label.to_string(), source.to_string())
                    .is_some()
            {
                return Err(NativeMemoryError::InvalidChangeSet(format!(
                    "invalid or duplicate footnote definition {label}"
                )));
            }
        } else {
            references.insert(label.to_string());
        }
    }

    let definition_labels = definitions.keys().cloned().collect::<BTreeSet<_>>();
    if references != definition_labels {
        return Err(NativeMemoryError::InvalidChangeSet(
            "every footnote citation must have exactly one definition, and every definition must be cited"
                .to_string(),
        ));
    }
    let manifest = sources.iter().cloned().collect::<BTreeSet<_>>();
    if manifest.len() != sources.len()
        || definitions.values().cloned().collect::<BTreeSet<_>>() != manifest
    {
        return Err(NativeMemoryError::InvalidChangeSet(
            "the source manifest must exactly match footnote definition targets".to_string(),
        ));
    }
    Ok(())
}

pub(super) fn validate_page_content(body: &str) -> Result<(), NativeMemoryError> {
    let words = UnicodeSegmentation::unicode_words(body).count();
    if words > MEMORY_MAX_WORDS {
        return Err(NativeMemoryError::InvalidPage(format!(
            "body exceeds {MEMORY_MAX_WORDS} words"
        )));
    }
    Ok(())
}

pub(super) fn normalize_page_path(path: &str) -> Result<String, NativeMemoryError> {
    let candidate = Path::new(path);
    if candidate.is_absolute() || candidate.extension().and_then(|ext| ext.to_str()) != Some("md") {
        return Err(NativeMemoryError::InvalidPage(format!(
            "path must be relative .md: {path}"
        )));
    }
    let mut components = Vec::new();
    for component in candidate.components() {
        match component {
            Component::Normal(value) => {
                let value = value.to_string_lossy().to_string();
                if value.starts_with('.') {
                    return Err(NativeMemoryError::InvalidPage(format!(
                        "hidden and reserved paths are not allowed: {path}"
                    )));
                }
                components.push(value);
            }
            _ => {
                return Err(NativeMemoryError::InvalidPage(format!(
                    "path is not normalized: {path}"
                )));
            }
        }
    }
    if components.is_empty() {
        return Err(NativeMemoryError::InvalidPage(format!(
            "empty path: {path}"
        )));
    }
    Ok(components.join("/"))
}

pub(super) fn lexical_fts_query(query: &str) -> Option<String> {
    let terms = UnicodeSegmentation::unicode_words(query)
        .map(|term| format!("\"{}\"*", term.replace('"', "\"\"")))
        .collect::<Vec<_>>();
    (!terms.is_empty()).then(|| terms.join(" AND "))
}

pub(super) fn lexical_fts_any_query(query: &str) -> Option<String> {
    let mut seen = BTreeSet::new();
    let mut terms = UnicodeSegmentation::unicode_words(query)
        .map(str::to_lowercase)
        .filter(|term| seen.insert(term.clone()))
        .collect::<Vec<_>>();
    if terms.len() > 128 {
        terms.drain(64..terms.len() - 64);
    }
    let terms = terms
        .into_iter()
        .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
        .collect::<Vec<_>>();
    (!terms.is_empty()).then(|| terms.join(" OR "))
}

pub(super) fn page_id(path: &str) -> String {
    format!("memory:human:{path}")
}

pub(super) fn parent_page_path(path: &str) -> Option<String> {
    let path = Path::new(path);
    let parent = path.parent()?.to_str()?;
    if parent.is_empty() || parent == "." {
        None
    } else {
        Some(format!("{parent}.md"))
    }
}

pub(super) fn hash_content(content: &[u8]) -> String {
    let mut context = Context::new(&SHA256);
    context.update(content);
    let digest = context.finish();
    digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(super) fn unix_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

pub(super) fn timestamp_now() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}
