use std::{fs, fs::File};

use super::*;

impl NativeMemory {
    pub(super) fn apply_staged(&self, staged: &PendingPayload) -> Result<(), NativeMemoryError> {
        for path in &staged.deletes {
            let path = normalize_page_path(path)?;
            if path == ROOT_PAGE_PATH {
                return Err(NativeMemoryError::InvalidChangeSet(
                    "root.md cannot be deleted".to_string(),
                ));
            }
            let file = self.root().join(&path);
            if file.exists() {
                fs::remove_file(file)?;
            }
        }
        for page in &staged.pages {
            let path = normalize_page_path(&page.path)?;
            let absolute = self.root().join(&path);
            if let Some(parent) = absolute.parent() {
                fs::create_dir_all(parent)?;
            }
            let temporary = absolute.with_extension("md.tmp");
            fs::write(&temporary, &page.bytes)?;
            File::open(&temporary)?.sync_all()?;
            fs::rename(temporary, absolute)?;
        }
        Ok(())
    }

    pub(super) fn stage_changes(
        &self,
        changes: &MemoryChangeSet,
    ) -> Result<Vec<StagedPage>, NativeMemoryError> {
        changes
            .upserts
            .iter()
            .map(|change| {
                let path = normalize_page_path(&change.path)?;
                Ok(StagedPage {
                    path: path.clone(),
                    bytes: self.render_page(change, &path)?.into_bytes(),
                })
            })
            .collect()
    }

    pub(super) fn resolved_deletes(
        &self,
        changes: &MemoryChangeSet,
    ) -> Result<Vec<String>, NativeMemoryError> {
        let mut deletes = changes
            .deletes
            .iter()
            .map(|path| normalize_page_path(path))
            .collect::<Result<std::collections::BTreeSet<_>, _>>()?;
        for change in &changes.upserts {
            let Some(id) = change.id.as_deref() else {
                continue;
            };
            if let Some(current_path) = self.find_page_path_by_id(id)? {
                let target = normalize_page_path(&change.path)?;
                if current_path != target {
                    if current_path == ROOT_PAGE_PATH {
                        return Err(NativeMemoryError::InvalidChangeSet(
                            "root.md cannot be moved".to_string(),
                        ));
                    }
                    deletes.insert(current_path);
                }
            }
        }
        Ok(deletes.into_iter().collect())
    }

    pub(super) fn validate_final_hierarchy(
        &self,
        changes: &MemoryChangeSet,
        deletes: &[String],
    ) -> Result<(), NativeMemoryError> {
        let mut paths = self
            .all_page_paths()?
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>();
        for path in deletes {
            paths.remove(path);
        }
        for change in &changes.upserts {
            paths.insert(normalize_page_path(&change.path)?);
        }
        if !paths.contains(ROOT_PAGE_PATH) {
            return Err(NativeMemoryError::InvalidChangeSet(
                "the final tree must contain root.md".to_string(),
            ));
        }
        for path in &paths {
            if let Some(parent) = parent_page_path(path)
                && !paths.contains(&parent)
            {
                return Err(NativeMemoryError::InvalidChangeSet(format!(
                    "page {path} requires parent page {parent}"
                )));
            }
        }
        let upsert_paths = changes
            .upserts
            .iter()
            .map(|change| normalize_page_path(&change.path))
            .collect::<Result<std::collections::BTreeSet<_>, _>>()?;
        let mut final_ids = std::collections::BTreeMap::new();
        for path in self.all_page_paths()? {
            if !deletes.contains(&path) && !upsert_paths.contains(&path) {
                let page = self.parse_page(&path)?;
                validate_article_structure(&path, &page.body)?;
                let id = page.id;
                if let Some(other_path) = final_ids.insert(id.clone(), path.clone()) {
                    return Err(NativeMemoryError::InvalidChangeSet(format!(
                        "stable id {id} is shared by {other_path} and {path}"
                    )));
                }
            }
        }
        for change in &changes.upserts {
            let path = normalize_page_path(&change.path)?;
            let id = change.id.clone().unwrap_or_else(|| page_id(&path));
            if let Some(other_path) = final_ids.insert(id.clone(), path.clone()) {
                return Err(NativeMemoryError::InvalidChangeSet(format!(
                    "stable id {id} is shared by {other_path} and {path}"
                )));
            }
        }
        Ok(())
    }

    fn find_page_path_by_id(&self, id: &str) -> Result<Option<String>, NativeMemoryError> {
        for path in self.all_page_paths()? {
            if self.parse_page(&path)?.id == id {
                return Ok(Some(path));
            }
        }
        Ok(None)
    }

    pub(super) fn write_page(&self, change: &MemoryPageChange) -> Result<(), NativeMemoryError> {
        let path = normalize_page_path(&change.path)?;
        let absolute = self.root().join(&path);
        if let Some(parent) = absolute.parent() {
            fs::create_dir_all(parent)?;
        }
        let markdown = self.render_page(change, &path)?;
        let temporary = absolute.with_extension("md.tmp");
        fs::write(&temporary, markdown.as_bytes())?;
        File::open(&temporary)?.sync_all()?;
        fs::rename(temporary, absolute)?;
        Ok(())
    }

    pub(super) fn render_page(
        &self,
        change: &MemoryPageChange,
        path: &str,
    ) -> Result<String, NativeMemoryError> {
        let title = change.title.trim();
        let body = change.body.trim();
        let rendered_body = format!("# {title}\n\n{body}");
        validate_page_content(&rendered_body)?;
        let now = timestamp_now();
        let id = change.id.clone().unwrap_or_else(|| page_id(path));
        let current_path = if self.root().join(path).exists() {
            Some(path.to_string())
        } else if let Some(id) = change.id.as_deref() {
            self.find_page_path_by_id(id)?
        } else {
            None
        };
        let current = current_path
            .as_deref()
            .map(|current_path| self.parse_page(current_path))
            .transpose()?;
        if current.is_none() && change.id.is_some() {
            return Err(NativeMemoryError::InvalidChangeSet(format!(
                "stable id {} does not identify an existing page",
                change.id.as_deref().unwrap_or_default()
            )));
        }
        let created_at = current
            .as_ref()
            .map(|page| page.created_at.clone())
            .unwrap_or_else(|| now.clone());
        if let Some(current) = current {
            if change.id.as_deref() != Some(current.id.as_str()) {
                return Err(NativeMemoryError::InvalidChangeSet(format!(
                    "stable id mismatch for {path}"
                )));
            }
            if change.expected_hash.as_deref() != Some(current.hash.as_str()) {
                return Err(NativeMemoryError::InvalidChangeSet(format!(
                    "expected hash is required for updating {path}"
                )));
            }
        } else if change.expected_hash.is_some() {
            return Err(NativeMemoryError::InvalidChangeSet(format!(
                "expected hash provided for new page {path}"
            )));
        }
        let source_lines = if change.sources.is_empty() {
            String::new()
        } else {
            format!(
                "sources:\n{}",
                change
                    .sources
                    .iter()
                    .map(|source| format!("  - {source}\n"))
                    .collect::<String>()
            )
        };
        Ok(format!(
            "---\nschema: noema.memory.page/v1\nid: {id}\nowner: {MEMORY_OWNER}\nscope: {MEMORY_SCOPE}\ntitle: {}\nicon: {}\ncreated_at: {created_at}\nupdated_at: {now}\n{source_lines}---\n\n# {}\n\n{}\n",
            title, change.icon, title, body,
        ))
    }
}
