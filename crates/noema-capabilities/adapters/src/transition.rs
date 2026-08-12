//! Deterministic impact planning for one adapter definition revision.

use crate::{
    CompiledAdapterDefinition, CompiledOperation,
    private_fs::{
        create_private_dir, random_hex, read_bounded_regular_file, sync_directory, write_new_file,
    },
};
use noema_home::NoemaPaths;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

const TRANSITIONS_DIR: &str = "transitions";
const MAX_JOURNAL_BYTES: u64 = 16 * 1024;

/// Safe, public impact of replacing one reviewed adapter definition.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AdapterDefinitionTransition {
    /// Operations introduced by the replacement.
    pub added_operations: Vec<String>,
    /// Operations whose executable contract changed.
    pub changed_operations: Vec<String>,
    /// Operations absent from the replacement.
    pub removed_operations: Vec<String>,
    /// Whether existing authentication authority cannot move to the replacement.
    pub authentication_changed: bool,
    /// Connections bound to the replacement lineage.
    pub affected_connections: usize,
    /// Polling schedules bound to the replacement lineage.
    pub affected_schedules: usize,
    /// Connections that must receive new authentication.
    pub authentication_required_connections: usize,
    /// Redundant connection records that the migration will consolidate.
    pub consolidated_connections: usize,
}

impl AdapterDefinitionTransition {
    pub(crate) fn between(
        current: &CompiledAdapterDefinition,
        replacement: &CompiledAdapterDefinition,
    ) -> Self {
        let mut transition = Self {
            authentication_changed: !super::service::compatible_authentication_replacement(
                &current.authentication,
                &replacement.authentication,
            ),
            ..Self::default()
        };
        for operation in &replacement.operations {
            match find_operation(&current.operations, &operation.operation_id) {
                None => transition
                    .added_operations
                    .push(operation.operation_id.clone()),
                Some(existing) if existing.operation_digest != operation.operation_digest => {
                    transition
                        .changed_operations
                        .push(operation.operation_id.clone());
                }
                Some(_) => {}
            }
        }
        for operation in &current.operations {
            if find_operation(&replacement.operations, &operation.operation_id).is_none() {
                transition
                    .removed_operations
                    .push(operation.operation_id.clone());
            }
        }
        transition.added_operations.sort();
        transition.changed_operations.sort();
        transition.removed_operations.sort();
        transition
    }
}

fn find_operation<'a>(
    operations: &'a [CompiledOperation],
    operation_id: &str,
) -> Option<&'a CompiledOperation> {
    operations
        .iter()
        .find(|operation| operation.operation_id == operation_id)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DefinitionTransitionJournal {
    pub(crate) schema_version: u16,
    pub(crate) definition_id: String,
    pub(crate) requested_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) reviewed_digest: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct DefinitionTransitionJournalStore {
    paths: NoemaPaths,
}

impl DefinitionTransitionJournalStore {
    pub(crate) fn new(paths: NoemaPaths) -> Self {
        Self { paths }
    }

    pub(crate) fn save(
        &self,
        journal: &DefinitionTransitionJournal,
    ) -> Result<(), crate::DefinitionStoreError> {
        let root = self.root()?;
        let target = root.join(&journal.requested_digest);
        let temporary = root.join(format!(".replace-{}", random_hex(16)?));
        let bytes = crate::digest::canonical_json_bytes(&serde_json::to_value(journal)?)?;
        if bytes.len() as u64 > MAX_JOURNAL_BYTES {
            return Err(crate::DefinitionStoreError::Integrity(
                "transition_journal_oversized",
            ));
        }
        write_new_file(&temporary, &bytes)?;
        if let Err(error) = fs::rename(&temporary, &target) {
            let _ = fs::remove_file(&temporary);
            return Err(error.into());
        }
        sync_directory(&root)?;
        Ok(())
    }

    pub(crate) fn scan(
        &self,
    ) -> Result<Vec<DefinitionTransitionJournal>, crate::DefinitionStoreError> {
        let root = self.root()?;
        let mut entries = fs::read_dir(&root)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(fs::DirEntry::file_name);
        let mut journals = Vec::new();
        for entry in entries {
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if name.starts_with('.') {
                let _ = fs::remove_file(entry.path());
                continue;
            }
            let bytes = read_bounded_regular_file(&entry.path(), MAX_JOURNAL_BYTES)?;
            let journal: DefinitionTransitionJournal = serde_json::from_slice(&bytes)?;
            if journal.schema_version != 1 || journal.requested_digest != name {
                return Err(crate::DefinitionStoreError::Integrity("transition_journal"));
            }
            journals.push(journal);
        }
        Ok(journals)
    }

    pub(crate) fn remove(&self, requested_digest: &str) -> Result<(), crate::DefinitionStoreError> {
        let root = self.root()?;
        let target = root.join(requested_digest);
        if target.exists() {
            fs::remove_file(target)?;
            sync_directory(&root)?;
        }
        Ok(())
    }

    fn root(&self) -> Result<PathBuf, crate::DefinitionStoreError> {
        let adapters = self.paths.adapters_dir();
        create_private_dir(&adapters)?;
        let root = adapters.join(TRANSITIONS_DIR);
        create_private_dir(&root)?;
        Ok(root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transition_journal_survives_restart_and_removal() {
        let directory = tempfile::tempdir().expect("temporary home");
        let paths = NoemaPaths::from_noema_home(directory.path()).expect("Noema paths");
        let journal = DefinitionTransitionJournal {
            schema_version: 1,
            definition_id: "definition:calendar".to_string(),
            requested_digest: "a".repeat(64),
            reviewed_digest: Some("b".repeat(64)),
        };
        DefinitionTransitionJournalStore::new(paths.clone())
            .save(&journal)
            .expect("save journal");

        let restarted = DefinitionTransitionJournalStore::new(paths);
        let saved = restarted.scan().expect("scan journal");
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0], journal);
        restarted
            .remove(&journal.requested_digest)
            .expect("remove journal");
        assert!(restarted.scan().expect("scan empty journal").is_empty());
    }
}
