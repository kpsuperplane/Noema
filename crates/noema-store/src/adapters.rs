//! Rebuildable SQLite projection of filesystem-canonical adapter definitions.

use noema_capability_adapters::DefinitionProjection;
use rusqlite::params;
use std::collections::BTreeSet;

use crate::{NoemaStore, StoreError};

const MAX_DEFINITIONS: usize = 1_024;

/// One rebuildable adapter-definition projection record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterDefinitionRecord {
    /// Filesystem semantic content address.
    pub semantic_digest: String,
    /// Stable definition identity when compilation succeeded.
    pub definition_id: Option<String>,
    /// Stable adapter family when compilation succeeded.
    pub adapter_id: Option<String>,
    /// Exact retained-source digest.
    pub source_digest: Option<String>,
    /// Safe manifest path relative to `NOEMA_HOME`.
    pub manifest_relative_path: String,
    /// Safe provenance path relative to `NOEMA_HOME`.
    pub provenance_relative_path: String,
    /// `compiled` or `blocked`.
    pub compile_status: String,
    /// `reviewed`, `pending`, or `unknown`.
    pub review_status: String,
    /// Safe blocked-object diagnostic category.
    pub diagnostic_code: Option<String>,
    /// Number of immutable operation plans.
    pub operation_count: usize,
    /// Stable compiler implementation version.
    pub compiler_version: String,
}

impl NoemaStore {
    /// Replace the rebuildable definition projection in one transaction.
    ///
    /// The provided rows must come from a completed filesystem scan. SQLite
    /// stores no manifest or source body and can be deleted without losing
    /// definition authority.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when rows are duplicated/malformed or SQLite
    /// cannot commit the exact snapshot.
    pub async fn reconcile_adapter_definitions(
        &self,
        definitions: &[DefinitionProjection],
    ) -> Result<(), StoreError> {
        validate_snapshot(definitions)?;
        self.with_connection(|connection| {
            let transaction =
                connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            transaction.execute("DELETE FROM adapter_definitions", [])?;
            for definition in definitions {
                transaction.execute(
                    r#"
                    INSERT INTO adapter_definitions (
                      semantic_digest, definition_id, adapter_id, source_digest,
                      manifest_relative_path, provenance_relative_path,
                      compile_status, review_status, diagnostic_code,
                      operation_count, compiler_version
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                    "#,
                    params![
                        definition.semantic_digest,
                        definition.definition_id,
                        definition.adapter_id,
                        definition.source_digest,
                        definition.manifest_relative_path,
                        definition.provenance_relative_path,
                        definition.compile_status,
                        definition.review_status,
                        definition.diagnostic_code,
                        definition.operation_count,
                        definition.compiler_version,
                    ],
                )?;
            }
            transaction.commit()?;
            Ok(())
        })
        .await
    }

    /// Load the complete deterministic adapter-definition projection.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when SQLite cannot read a row.
    pub async fn adapter_definitions(&self) -> Result<Vec<AdapterDefinitionRecord>, StoreError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                r#"
                SELECT semantic_digest, definition_id, adapter_id, source_digest,
                       manifest_relative_path, provenance_relative_path,
                       compile_status, review_status, diagnostic_code,
                       operation_count, compiler_version
                FROM adapter_definitions
                ORDER BY semantic_digest
                "#,
            )?;
            statement
                .query_map([], |row| {
                    Ok(AdapterDefinitionRecord {
                        semantic_digest: row.get(0)?,
                        definition_id: row.get(1)?,
                        adapter_id: row.get(2)?,
                        source_digest: row.get(3)?,
                        manifest_relative_path: row.get(4)?,
                        provenance_relative_path: row.get(5)?,
                        compile_status: row.get(6)?,
                        review_status: row.get(7)?,
                        diagnostic_code: row.get(8)?,
                        operation_count: row.get(9)?,
                        compiler_version: row.get(10)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()
                .map_err(StoreError::Sqlite)
        })
        .await
    }
}

fn validate_snapshot(definitions: &[DefinitionProjection]) -> Result<(), StoreError> {
    if definitions.len() > MAX_DEFINITIONS {
        return Err(invariant("adapter definition snapshot exceeds its bound"));
    }
    let mut digests = BTreeSet::new();
    for definition in definitions {
        let expected_manifest = format!(
            "adapters/definitions/{}/manifest.json",
            definition.semantic_digest
        );
        let expected_provenance = format!(
            "adapters/definitions/{}/provenance.json",
            definition.semantic_digest
        );
        if !valid_digest(&definition.semantic_digest)
            || definition
                .source_digest
                .as_deref()
                .is_some_and(|digest| !valid_digest(digest))
            || !digests.insert(definition.semantic_digest.as_str())
            || definition.manifest_relative_path != expected_manifest
            || definition.provenance_relative_path != expected_provenance
            || definition
                .diagnostic_code
                .is_some_and(|code| !valid_component(code, 64))
            || definition.operation_count > 256
            || !valid_component(definition.compiler_version, 64)
            || !valid_projection_state(definition)
        {
            return Err(invariant("adapter definition projection is invalid"));
        }
    }
    Ok(())
}

fn valid_projection_state(definition: &DefinitionProjection) -> bool {
    match definition.compile_status {
        "compiled" => {
            definition
                .definition_id
                .as_deref()
                .is_some_and(|id| valid_component(id, 96))
                && definition
                    .adapter_id
                    .as_deref()
                    .is_some_and(|id| valid_component(id, 96))
                && matches!(definition.review_status, "reviewed" | "pending")
                && definition.diagnostic_code.is_none()
                && definition.operation_count > 0
        }
        "blocked" => {
            definition.definition_id.is_none()
                && definition.adapter_id.is_none()
                && definition.review_status == "unknown"
                && definition.diagnostic_code.is_some()
                && definition.operation_count == 0
        }
        _ => false,
    }
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn valid_component(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value.trim() == value
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
}

fn invariant(message: &'static str) -> StoreError {
    StoreError::InvariantViolation {
        message: message.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NoemaStore, StoreConfig};
    use noema_capability_adapters::{AdapterDefinitionStore, AdapterManifestV1};
    use noema_home::NoemaPaths;

    fn fixture_manifest() -> AdapterManifestV1 {
        serde_json::from_value(serde_json::json!({
            "schema_version": 1,
            "definition_id": "definition:offline_fixture",
            "adapter_id": "offline_fixture",
            "definition_revision": "v1",
            "reviewed": true,
            "origin": "https://api.example.test/",
            "authentication": {"mode":"none"},
            "provider_data_policy": {"retention_allowed":true,"deletion_supported":true},
            "quota": {"cost_class":"free","request_units":1},
            "operations": [{
                "operation_id":"list",
                "method":"GET",
                "path":"/v1/items",
                "effect":"read_only",
                "admission":"direct",
                "result": {
                    "classification":"public",
                    "model_route":"any_known_route",
                    "model_payload":"full",
                    "provider_retention":"allow",
                    "persistence":"redacted"
                },
                "retry":"transport_safe_read",
                "pagination":{"kind":"none"}
            }]
        }))
        .expect("fixture manifest")
    }

    #[tokio::test]
    async fn fresh_sqlite_rebuilds_exact_definition_projection_from_files() {
        let home = tempfile::tempdir().expect("home");
        let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
        let definitions = AdapterDefinitionStore::new(paths.clone());
        definitions
            .install(
                &fixture_manifest(),
                "fixture://independent-company-a/openapi.json",
                None,
                Some((br#"{"openapi":"3.0.3"}"#, "json")),
            )
            .expect("install");
        let scan = definitions.scan().expect("scan");
        let store = NoemaStore::open(&StoreConfig::new(paths.sqlite_db_path()))
            .await
            .expect("store");
        store
            .reconcile_adapter_definitions(&scan.projections())
            .await
            .expect("reconcile");
        let before = store.adapter_definitions().await.expect("rows");
        drop(store);
        for suffix in ["", "-wal", "-shm"] {
            let path =
                std::path::PathBuf::from(format!("{}{suffix}", paths.sqlite_db_path().display()));
            if path.exists() {
                std::fs::remove_file(path).expect("remove database family");
            }
        }
        let recreated = NoemaStore::open(&StoreConfig::new(paths.sqlite_db_path()))
            .await
            .expect("recreated store");
        let rediscovered = definitions.scan().expect("rescan");
        recreated
            .reconcile_adapter_definitions(&rediscovered.projections())
            .await
            .expect("reconcile recreated");
        assert_eq!(recreated.adapter_definitions().await.expect("rows"), before);
        let schema = std::fs::read_to_string(paths.root().join(&before[0].manifest_relative_path))
            .expect("canonical manifest");
        assert!(schema.contains("offline_fixture"));
    }

    #[test]
    fn projection_allows_revisions_but_rejects_duplicate_content_authority() {
        let row = DefinitionProjection {
            semantic_digest: "a".repeat(64),
            definition_id: Some("definition:duplicate".to_string()),
            adapter_id: Some("fixture".to_string()),
            source_digest: None,
            manifest_relative_path: format!(
                "adapters/definitions/{}/manifest.json",
                "a".repeat(64)
            ),
            provenance_relative_path: format!(
                "adapters/definitions/{}/provenance.json",
                "a".repeat(64)
            ),
            compile_status: "compiled",
            review_status: "reviewed",
            diagnostic_code: None,
            operation_count: 1,
            compiler_version: "test",
        };
        let mut other = row.clone();
        other.semantic_digest = "b".repeat(64);
        other.manifest_relative_path = format!(
            "adapters/definitions/{}/manifest.json",
            other.semantic_digest
        );
        other.provenance_relative_path = format!(
            "adapters/definitions/{}/provenance.json",
            other.semantic_digest
        );
        assert!(validate_snapshot(&[row.clone(), other]).is_ok());
        assert!(validate_snapshot(&[row.clone(), row]).is_err());
    }
}
