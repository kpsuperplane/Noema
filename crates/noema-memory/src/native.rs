//! Native Markdown memory storage.
//!
//! The filesystem is the authority for the local-human tree.  SQLite is only a
//! disposable lexical index and is rebuilt from Markdown after each publish.

use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[path = "native_helpers.rs"]
mod native_helpers;
use native_helpers::*;
#[path = "native_publish.rs"]
mod native_publish;

/// Fixed owner for the first local-human tree.
pub const MEMORY_OWNER: &str = "human:local";
/// Fixed scope for the first local-human tree.
pub const MEMORY_SCOPE: &str = "human:local";
/// Canonical root page path.
pub const ROOT_PAGE_PATH: &str = "root.md";
/// Maximum number of Unicode words in one page body.
pub const MEMORY_MAX_WORDS: usize = 750;
/// Lucide icon keys accepted for generated memory pages.
pub const MEMORY_PAGE_ICON_KEYS: &[&str] = &[
    "brain",
    "file-text",
    "user",
    "users",
    "heart",
    "house",
    "briefcase-business",
    "graduation-cap",
    "book-open",
    "lightbulb",
    "target",
    "calendar-days",
    "map-pin",
    "plane",
    "heart-pulse",
    "dumbbell",
    "utensils",
    "music",
    "palette",
    "camera",
    "gamepad-2",
    "mountain",
    "paw-print",
    "code-2",
    "wallet-cards",
    "sparkles",
    "compass",
    "notebook-pen",
];

/// Model-visible page read tool name.
pub const READ_MEMORY_PAGE_TOOL_NAME: &str = "read_memory_page";
/// Model-visible native lexical search tool name.
pub const NATIVE_SEARCH_MEMORY_TOOL_NAME: &str = "search_memory";

/// Build the page read tool contract.
///
/// # Errors
///
/// Returns an error if the tool schema cannot be constructed.
pub fn read_memory_page_tool_spec()
-> Result<noema_capabilities::ToolSpec, noema_capabilities::ToolContractError> {
    noema_capabilities::ToolSpec::new(
        READ_MEMORY_PAGE_TOOL_NAME,
        "Read a canonical memory page by an exact path or id listed in the root or page hierarchy. Prefer this when a listed page plausibly covers the question.",
        serde_json::json!({"type":"object","properties":{"page":{"type":"string"}},"required":["page"],"additionalProperties":false}),
    )
}

/// Build the native lexical search tool contract.
///
/// # Errors
///
/// Returns an error if the tool schema cannot be constructed.
pub fn native_search_memory_tool_spec()
-> Result<noema_capabilities::ToolSpec, noema_capabilities::ToolContractError> {
    noema_capabilities::ToolSpec::new(
        NATIVE_SEARCH_MEMORY_TOOL_NAME,
        "Search canonical memory when the hierarchy has no clear page or the question spans pages. Returns page references with lexical-match snippets.",
        serde_json::json!({"type":"object","properties":{"query":{"type":"string"},"limit":{"type":"integer","minimum":1,"maximum":16}},"required":["query"],"additionalProperties":false}),
    )
}

/// Machine-only consolidation checkpoint committed with a publish.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MemoryState {
    /// Primary conversation captured by the update.
    pub conversation_id: Option<String>,
    /// Last sequence index included in the committed update.
    pub last_consolidated_sequence: i64,
    /// Last durable item id included in the committed update.
    pub last_consolidated_item: Option<String>,
    /// State update timestamp.
    pub updated_at: String,
}

/// A page change emitted by the model and validated before publication.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryPageChange {
    /// Existing stable page id, when updating or moving a page.
    #[serde(default)]
    pub id: Option<String>,
    /// Expected current hash for compare-and-publish updates.
    #[serde(default)]
    pub expected_hash: Option<String>,
    /// Normalized relative Markdown path.
    pub path: String,
    /// Human-facing page title.
    pub title: String,
    /// Lucide icon key representing the page subject.
    pub icon: String,
    /// Markdown page body, excluding frontmatter.
    pub body: String,
    /// Evidence groups cited by numeric markers in the page body.
    #[serde(default)]
    pub citations: Vec<MemoryCitation>,
}

/// Exact evidence sources shown under one inline citation marker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryCitation {
    /// Durable source identifiers supporting the nearby claim.
    pub sources: Vec<String>,
}

/// A complete structured update. Deletes are relative Markdown paths.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MemoryChangeSet {
    /// Pages to create or replace.
    #[serde(default)]
    pub upserts: Vec<MemoryPageChange>,
    /// Pages to delete.
    #[serde(default)]
    pub deletes: Vec<String>,
}

/// A canonical page read model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryPage {
    /// Stable page id from frontmatter.
    pub id: String,
    /// Normalized relative Markdown path.
    pub path: String,
    /// Human-facing page title.
    pub title: String,
    /// Lucide icon key representing the page subject.
    pub icon: String,
    /// Markdown body excluding frontmatter and the generated title heading.
    pub body: String,
    /// SHA-256 hash of the canonical page bytes.
    pub hash: String,
    /// Ordered evidence groups derived from generated footnote definitions.
    pub citations: Vec<MemoryCitation>,
    /// Parent page path, when this page is nested below another page.
    pub parent: Option<String>,
    /// Ordered ancestor references from the nearest root-level page downward.
    pub ancestors: Vec<MemoryPageRef>,
    /// Deterministic direct child references.
    pub children: Vec<MemoryPageRef>,
}

/// A filesystem-derived child/page reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryPageRef {
    /// Stable page id from frontmatter.
    pub id: String,
    /// Normalized relative Markdown path.
    pub path: String,
    /// Human-facing page title.
    pub title: String,
    /// Lucide icon key representing the page subject.
    pub icon: String,
    /// Bounded plain-text lead for navigation surfaces.
    pub excerpt: String,
    /// SHA-256 hash of the canonical page bytes.
    pub hash: String,
}

impl From<&MemoryPage> for MemoryPageRef {
    fn from(page: &MemoryPage) -> Self {
        Self {
            id: page.id.clone(),
            path: page.path.clone(),
            title: page.title.clone(),
            icon: page.icon.clone(),
            excerpt: page_lead_excerpt(&page.body),
            hash: page.hash.clone(),
        }
    }
}

/// A lexical search result.  Search never returns a complete page body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemorySearchResult {
    /// Stable page id from frontmatter.
    pub id: String,
    /// Normalized relative Markdown path.
    pub path: String,
    /// Human-facing page title.
    pub title: String,
    /// Short lexical-match snippet.
    pub snippet: String,
    /// SHA-256 hash of the canonical page bytes.
    pub hash: String,
}

/// Native memory storage failure.
#[derive(Debug, Error)]
pub enum NativeMemoryError {
    /// Filesystem operation failed.
    #[error("memory filesystem error: {0}")]
    Io(#[from] io::Error),
    /// Derived SQLite index operation failed.
    #[error("memory index error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// A page failed schema or canonical-content validation.
    #[error("memory page is invalid: {0}")]
    InvalidPage(String),
    /// A model-proposed change set failed validation.
    #[error("memory update is invalid: {0}")]
    InvalidChangeSet(String),
}

/// One process-local native memory handle.
#[derive(Debug, Clone)]
pub struct NativeMemory {
    inner: Arc<NativeMemoryInner>,
}

#[derive(Debug)]
struct NativeMemoryInner {
    root: PathBuf,
    index_path: PathBuf,
    gate: std::sync::RwLock<()>,
}

impl NativeMemory {
    /// Construct a handle from the canonical human root and derived FTS path.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>, index_path: impl Into<PathBuf>) -> Self {
        Self {
            inner: Arc::new(NativeMemoryInner {
                root: root.into(),
                index_path: index_path.into(),
                gate: std::sync::RwLock::new(()),
            }),
        }
    }

    #[must_use]
    /// Return the canonical human-memory root directory.
    pub(crate) fn root(&self) -> &Path {
        &self.inner.root
    }

    #[must_use]
    /// Return the disposable SQLite FTS index path.
    pub(crate) fn index_path(&self) -> &Path {
        &self.inner.index_path
    }

    /// Initialize the tree and recover the most recent pending publish.
    ///
    /// # Errors
    ///
    /// Returns an error when the memory directory, recovery log, or derived
    /// index cannot be read or written.
    pub fn initialize(&self) -> Result<(), NativeMemoryError> {
        fs::create_dir_all(self.root())?;
        fs::create_dir_all(self.root().join(".pending"))?;
        if !self.root().join(ROOT_PAGE_PATH).exists() {
            let change = MemoryPageChange {
                id: None,
                expected_hash: None,
                path: ROOT_PAGE_PATH.to_string(),
                title: "Human memory".to_string(),
                icon: "user".to_string(),
                body: String::new(),
                citations: Vec::new(),
            };
            self.write_page(&change)?;
        }
        self.recover_pending()?;
        self.rebuild_index()
    }

    /// Read the root page and deterministic direct children.
    ///
    /// # Errors
    ///
    /// Returns an error when the root page or its children are invalid or
    /// cannot be read.
    pub fn read_root(&self) -> Result<MemoryPage, NativeMemoryError> {
        self.read_page(ROOT_PAGE_PATH)
    }

    /// Read every canonical page for a reconciliation prompt.
    ///
    /// # Errors
    ///
    /// Returns an error when the memory directory or any page is invalid.
    pub fn list_pages(&self) -> Result<Vec<MemoryPage>, NativeMemoryError> {
        let _guard = self
            .inner
            .gate
            .read()
            .map_err(|_| NativeMemoryError::InvalidPage("memory lock poisoned".to_string()))?;
        self.all_page_paths()?
            .into_iter()
            .map(|path| self.read_page_unlocked(&path))
            .collect()
    }

    /// Read one page by normalized relative path or stable id.
    ///
    /// # Errors
    ///
    /// Returns an error when the selector is unknown or the page is invalid.
    pub fn read_page(&self, page: &str) -> Result<MemoryPage, NativeMemoryError> {
        let _guard = self
            .inner
            .gate
            .read()
            .map_err(|_| NativeMemoryError::InvalidPage("memory lock poisoned".to_string()))?;
        self.read_page_unlocked(page)
    }

    fn read_page_unlocked(&self, page: &str) -> Result<MemoryPage, NativeMemoryError> {
        let path = match normalize_page_path(page) {
            Ok(candidate) if self.root().join(&candidate).is_file() => candidate,
            _ => self
                .all_page_paths()?
                .into_iter()
                .find(|candidate| {
                    self.parse_page(candidate)
                        .map(|parsed| parsed.id == page)
                        .unwrap_or(false)
                })
                .ok_or_else(|| NativeMemoryError::InvalidPage(format!("unknown page {page}")))?,
        };
        let parsed = self.parse_page(&path)?;
        let parent = parent_page_path(&path);
        let mut ancestors = Vec::new();
        let mut ancestor_path = parent.clone();
        while let Some(current) = ancestor_path {
            ancestors.push(page_reference(current.clone(), self.parse_page(&current)?));
            ancestor_path = parent_page_path(&current);
        }
        ancestors.reverse();
        let mut children = Vec::new();
        let child_dir = if path == ROOT_PAGE_PATH {
            self.root().to_path_buf()
        } else {
            self.root().join(Path::new(&path).with_extension(""))
        };
        if child_dir.is_dir() {
            let mut paths = fs::read_dir(child_dir)?
                .filter_map(Result::ok)
                .filter_map(|entry| entry.file_name().into_string().ok())
                .filter(|name| name.ends_with(".md") && name != ".state.md")
                .collect::<Vec<_>>();
            paths.sort();
            for name in paths {
                let child_path = if path == ROOT_PAGE_PATH {
                    name.clone()
                } else {
                    format!("{}/{}", path.trim_end_matches(".md"), name)
                };
                if child_path == ROOT_PAGE_PATH {
                    continue;
                }
                children.push(page_reference(
                    child_path.clone(),
                    self.parse_page(&child_path)?,
                ));
            }
        }
        Ok(MemoryPage {
            id: parsed.id,
            path,
            title: parsed.title,
            icon: parsed.icon,
            body: parsed.body,
            hash: parsed.hash,
            citations: parsed.citations,
            parent,
            ancestors,
            children,
        })
    }

    /// Search the derived lexical index and return snippets/page references.
    ///
    /// # Errors
    ///
    /// Returns an error when the derived index cannot be opened or a matching
    /// page is invalid.
    pub fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemorySearchResult>, NativeMemoryError> {
        let _guard = self
            .inner
            .gate
            .read()
            .map_err(|_| NativeMemoryError::InvalidPage("memory lock poisoned".to_string()))?;
        let Some(query) = lexical_fts_query(query) else {
            return Ok(Vec::new());
        };
        self.search_unlocked(&query, limit)
    }

    /// Rank pages matching any terms in a long evidence query.
    ///
    /// # Errors
    ///
    /// Returns an error when the derived index cannot be opened or a matching
    /// page is invalid.
    pub fn search_relevant(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemorySearchResult>, NativeMemoryError> {
        let _guard = self
            .inner
            .gate
            .read()
            .map_err(|_| NativeMemoryError::InvalidPage("memory lock poisoned".to_string()))?;
        let Some(query) = lexical_fts_any_query(query) else {
            return Ok(Vec::new());
        };
        self.search_unlocked(&query, limit)
    }

    fn search_unlocked(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemorySearchResult>, NativeMemoryError> {
        let connection = self.open_index()?;
        let mut statement = connection.prepare(
            "SELECT path, title, snippet(memory_fts, 2, '', '', ' … ', 18)\n             FROM memory_fts WHERE memory_fts MATCH ?1 ORDER BY rank LIMIT ?2",
        )?;
        let rows = statement.query_map(params![query, limit as i64], |row| {
            let path: String = row.get(0)?;
            let title: String = row.get(1)?;
            let snippet: String = row.get(2)?;
            Ok((path, title, snippet))
        })?;
        let mut results = Vec::new();
        for row in rows {
            let (path, title, snippet) = row?;
            let parsed = self.parse_page(&path)?;
            results.push(MemorySearchResult {
                id: parsed.id,
                path,
                title,
                snippet,
                hash: parsed.hash,
            });
        }
        Ok(results)
    }

    /// Validate, stage, publish, and atomically index a change set.
    ///
    /// # Errors
    ///
    /// Returns an error when validation, staging, filesystem publication, or
    /// index rebuild fails.
    pub fn publish(&self, changes: &MemoryChangeSet) -> Result<(), NativeMemoryError> {
        self.publish_with_state(
            changes,
            &MemoryState {
                updated_at: timestamp_now(),
                ..MemoryState::default()
            },
        )
    }

    /// Publish a validated change set and atomically advance its checkpoint.
    ///
    /// # Errors
    ///
    /// Returns an error when validation, staging, filesystem publication, or
    /// index/state persistence fails.
    pub fn publish_with_state(
        &self,
        changes: &MemoryChangeSet,
        state: &MemoryState,
    ) -> Result<(), NativeMemoryError> {
        let _guard = self
            .inner
            .gate
            .write()
            .map_err(|_| NativeMemoryError::InvalidPage("memory lock poisoned".to_string()))?;
        validate_change_set(changes)?;
        let deletes = self.resolved_deletes(changes)?;
        self.validate_final_hierarchy(changes, &deletes)?;
        let operation_id = format!("{}-{}", std::process::id(), unix_seconds());
        let pending = self.root().join(".pending").join(&operation_id);
        fs::create_dir_all(&pending)?;
        let staged_pages = self.stage_changes(changes)?;
        let staged = PendingPayload {
            pages: staged_pages,
            deletes,
            state: state.clone(),
        };
        let payload = serde_json::to_vec(&staged)
            .map_err(|error| NativeMemoryError::InvalidChangeSet(error.to_string()))?;
        let mut staged_file = File::create(pending.join("changes.json"))?;
        staged_file.write_all(&payload)?;
        staged_file.sync_all()?;
        self.apply_staged(&staged)?;
        self.rebuild_index()?;
        self.advance_state(state)?;
        fs::remove_dir_all(&pending)?;
        Ok(())
    }

    /// Return the machine-only checkpoint document, if present.
    ///
    /// # Errors
    ///
    /// Returns an error when the checkpoint cannot be read.
    fn state_body(&self) -> Result<Option<String>, NativeMemoryError> {
        let path = self.root().join(".state.md");
        Ok(path
            .exists()
            .then(|| fs::read_to_string(path))
            .transpose()?)
    }

    /// Read the last committed consolidation checkpoint.
    ///
    /// # Errors
    ///
    /// Returns an error when the checkpoint is malformed or cannot be read.
    pub fn state(&self) -> Result<MemoryState, NativeMemoryError> {
        let Some(body) = self.state_body()? else {
            return Ok(MemoryState::default());
        };
        let (frontmatter, _) = split_frontmatter(&body)?;
        Ok(MemoryState {
            conversation_id: frontmatter
                .get("conversation_id")
                .cloned()
                .filter(|value| !value.is_empty()),
            last_consolidated_sequence: frontmatter
                .get("last_consolidated_sequence")
                .and_then(|value| value.parse().ok())
                .unwrap_or_default(),
            last_consolidated_item: frontmatter
                .get("last_consolidated_item")
                .cloned()
                .filter(|value| !value.is_empty()),
            updated_at: frontmatter.get("updated_at").cloned().unwrap_or_default(),
        })
    }

    fn parse_page(&self, path: &str) -> Result<ParsedPage, NativeMemoryError> {
        let content = fs::read_to_string(self.root().join(path))?;
        let (frontmatter, body) = split_frontmatter(&content)?;
        if frontmatter.get("schema").map(String::as_str) != Some("noema.memory.page/v2")
            || frontmatter.get("owner").map(String::as_str) != Some(MEMORY_OWNER)
            || frontmatter.get("scope").map(String::as_str) != Some(MEMORY_SCOPE)
        {
            return Err(NativeMemoryError::InvalidPage(format!(
                "{path} has invalid owner or scope"
            )));
        }
        let title = frontmatter
            .get("title")
            .cloned()
            .ok_or_else(|| NativeMemoryError::InvalidPage(format!("{path} has no title")))?;
        let icon = frontmatter
            .get("icon")
            .cloned()
            .ok_or_else(|| NativeMemoryError::InvalidPage(format!("{path} has no icon")))?;
        if !MEMORY_PAGE_ICON_KEYS.contains(&icon.as_str()) {
            return Err(NativeMemoryError::InvalidPage(format!(
                "{path} has unsupported icon {icon}"
            )));
        }
        let body = body.trim();
        let generated_heading = format!("# {title}");
        let body = body
            .strip_prefix(&generated_heading)
            .filter(|rest| rest.is_empty() || rest.starts_with('\n'))
            .unwrap_or(body)
            .trim();
        let (body, citations) = split_article_citations(body)?;
        let id = frontmatter
            .get("id")
            .cloned()
            .ok_or_else(|| NativeMemoryError::InvalidPage(format!("{path} has no id")))?;
        let created_at = frontmatter.get("created_at").cloned().unwrap_or_default();
        Ok(ParsedPage {
            id,
            title,
            icon,
            body,
            hash: hash_content(content.as_bytes()),
            created_at,
            citations,
        })
    }

    fn all_page_paths(&self) -> Result<Vec<String>, NativeMemoryError> {
        let mut paths = Vec::new();
        walk_pages(self.root(), self.root(), &mut paths)?;
        paths.sort();
        Ok(paths)
    }

    fn open_index(&self) -> Result<Connection, NativeMemoryError> {
        if let Some(parent) = self.index_path().parent() {
            fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(self.index_path())?;
        connection.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts USING fts5(path UNINDEXED, title, body);",
        )?;
        Ok(connection)
    }

    fn rebuild_index(&self) -> Result<(), NativeMemoryError> {
        let connection = self.open_index()?;
        let transaction = connection.unchecked_transaction()?;
        transaction.execute("DELETE FROM memory_fts", [])?;
        let mut ids = std::collections::HashSet::new();
        for path in self.all_page_paths()? {
            let parsed = self.parse_page(&path)?;
            if !ids.insert(parsed.id.clone()) {
                return Err(NativeMemoryError::InvalidPage(format!(
                    "duplicate stable id {}",
                    parsed.id
                )));
            }
            transaction.execute(
                "INSERT INTO memory_fts(path,title,body) VALUES (?1,?2,?3)",
                params![path, parsed.title, parsed.body],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    fn recover_pending(&self) -> Result<(), NativeMemoryError> {
        let pending_root = self.root().join(".pending");
        let mut entries = fs::read_dir(&pending_root)?
            .filter_map(Result::ok)
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            if !entry.path().is_dir() {
                continue;
            }
            let payload = entry.path().join("changes.json");
            if !payload.exists() {
                fs::remove_dir_all(entry.path())?;
                continue;
            }
            let pending: PendingPayload = serde_json::from_slice(&fs::read(payload)?)
                .map_err(|error| NativeMemoryError::InvalidChangeSet(error.to_string()))?;
            self.apply_staged(&pending)?;
            self.rebuild_index()?;
            self.advance_state(&pending.state)?;
            fs::remove_dir_all(entry.path())?;
        }
        Ok(())
    }

    fn advance_state(&self, checkpoint: &MemoryState) -> Result<(), NativeMemoryError> {
        let updated_at = if checkpoint.updated_at.trim().is_empty() {
            timestamp_now()
        } else {
            checkpoint.updated_at.clone()
        };
        let state = format!(
            "---\nschema: noema.memory.state/v1\nowner: {MEMORY_OWNER}\nscope: {MEMORY_SCOPE}\nconversation_id: {}\nlast_consolidated_sequence: {}\nlast_consolidated_item: {}\nupdated_at: {updated_at}\n---\n",
            checkpoint.conversation_id.as_deref().unwrap_or_default(),
            checkpoint.last_consolidated_sequence,
            checkpoint
                .last_consolidated_item
                .as_deref()
                .unwrap_or_default()
        );
        let temporary = self.root().join(".state.md.tmp");
        fs::write(&temporary, state.as_bytes())?;
        File::open(&temporary)?.sync_all()?;
        fs::rename(temporary, self.root().join(".state.md"))?;
        Ok(())
    }
}

fn page_reference(path: String, page: ParsedPage) -> MemoryPageRef {
    MemoryPageRef {
        id: page.id,
        path,
        title: page.title,
        icon: page.icon,
        excerpt: page_lead_excerpt(&page.body),
        hash: page.hash,
    }
}

fn page_lead_excerpt(body: &str) -> String {
    const LIMIT: usize = 180;
    let lead = body
        .split("\n\n")
        .map(str::trim)
        .find(|paragraph| !paragraph.is_empty() && !paragraph.starts_with('#'))
        .unwrap_or_default()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let mut plain = String::with_capacity(lead.len());
    let mut remainder = lead.as_str();
    while let Some(start) = remainder.find("[^") {
        plain.push_str(&remainder[..start]);
        let Some(end) = remainder[start + 2..].find(']') else {
            plain.push_str(&remainder[start..]);
            remainder = "";
            break;
        };
        remainder = &remainder[start + end + 3..];
    }
    plain.push_str(remainder);
    let Some((boundary, _)) = plain.char_indices().nth(LIMIT) else {
        return plain;
    };
    let boundary = plain[..boundary].rfind(' ').unwrap_or(boundary);
    format!("{}…", plain[..boundary].trim_end())
}

#[cfg(test)]
mod tests {
    include!("native_tests.rs");
}
