//! Rebuildable SQLite projection of filesystem-canonical adapter definitions.

use noema_capability_adapters::{ConnectionProjection, DefinitionProjection};
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

/// One rebuildable adapter-connection projection record without secret bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterConnectionRecord {
    /// Stable filesystem connection identity.
    pub connection_id: String,
    /// Stable tool namespace slug.
    pub connection_slug: Option<String>,
    /// Exact definition content address.
    pub semantic_digest: Option<String>,
    /// Stable external account identity when known.
    pub account_id: Option<String>,
    /// Recognizable account label when discovered.
    pub account_label: Option<String>,
    /// Reviewed account surface.
    pub account_kind: Option<String>,
    /// Current lifecycle or blocked status.
    pub status: String,
    /// Descriptor/lifecycle revision.
    pub connection_revision: Option<u64>,
    /// Credential generation revision.
    pub credential_revision: Option<u64>,
    /// Provider grant revision.
    pub grant_revision: Option<u64>,
    /// Reviewed policy revision.
    pub policy_revision: Option<u64>,
    /// Current secret generation identity without its bytes.
    pub credential_generation: Option<String>,
    /// Exact non-secret granted scope subset.
    pub granted_scopes: Vec<String>,
    /// Reviewed operation identities.
    pub allowed_operations: Vec<String>,
    /// Canonical descriptor path relative to `NOEMA_HOME`.
    pub descriptor_relative_path: String,
    /// Canonical secret path reference relative to `NOEMA_HOME`.
    pub credential_relative_path: Option<String>,
    /// Safe diagnostic category for blocked objects.
    pub diagnostic_code: Option<String>,
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

    /// Replace the disposable adapter-connection projection in one transaction.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the filesystem-derived snapshot is invalid,
    /// cannot be serialized, or cannot be committed atomically.
    pub async fn reconcile_adapter_connections(
        &self,
        connections: &[ConnectionProjection],
    ) -> Result<(), StoreError> {
        validate_connection_snapshot(connections)?;
        let rows = connections
            .iter()
            .map(|connection| {
                Ok((
                    connection,
                    serde_json::to_string(&connection.granted_scopes)?,
                    serde_json::to_string(&connection.allowed_operations)?,
                ))
            })
            .collect::<Result<Vec<_>, serde_json::Error>>()?;
        self.with_connection(|connection| {
            let transaction =
                connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            transaction.execute("DELETE FROM adapter_connections", [])?;
            for (adapter, granted_scopes, allowed_operations) in &rows {
                transaction.execute(
                    r#"
                    INSERT INTO adapter_connections (
                      connection_id, connection_slug, semantic_digest, account_id, account_label, account_kind,
                      status, connection_revision, credential_revision, grant_revision,
                      policy_revision, credential_generation, granted_scopes_json,
                      allowed_operations_json, descriptor_relative_path,
                      credential_relative_path, diagnostic_code
                    ) VALUES (
                      ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17
                    )
                    "#,
                    params![
                        adapter.connection_id,
                        adapter.connection_slug,
                        adapter.semantic_digest,
                        adapter.account_id,
                        adapter.account_label,
                        adapter.account_kind,
                        adapter.status,
                        adapter.connection_revision,
                        adapter.credential_revision,
                        adapter.grant_revision,
                        adapter.policy_revision,
                        adapter.credential_generation,
                        granted_scopes,
                        allowed_operations,
                        adapter.descriptor_relative_path,
                        adapter.credential_relative_path,
                        adapter.diagnostic_code,
                    ],
                )?;
            }
            transaction.commit()?;
            Ok(())
        })
        .await
    }

    /// Load the complete deterministic adapter-connection projection.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when SQLite or canonical projection JSON cannot be read.
    pub async fn adapter_connections(&self) -> Result<Vec<AdapterConnectionRecord>, StoreError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                r#"
                SELECT connection_id, connection_slug, semantic_digest, account_id, account_label, account_kind,
                       status, connection_revision, credential_revision, grant_revision,
                       policy_revision, credential_generation, granted_scopes_json,
                       allowed_operations_json, descriptor_relative_path,
                       credential_relative_path, diagnostic_code
                FROM adapter_connections
                ORDER BY connection_id
                "#,
            )?;
            let rows = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, Option<u64>>(7)?,
                        row.get::<_, Option<u64>>(8)?,
                        row.get::<_, Option<u64>>(9)?,
                        row.get::<_, Option<u64>>(10)?,
                        row.get::<_, Option<String>>(11)?,
                        row.get::<_, String>(12)?,
                        row.get::<_, String>(13)?,
                        row.get::<_, String>(14)?,
                        row.get::<_, Option<String>>(15)?,
                        row.get::<_, Option<String>>(16)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            rows.into_iter()
                .map(|row| {
                    Ok(AdapterConnectionRecord {
                        connection_id: row.0,
                        connection_slug: row.1,
                        semantic_digest: row.2,
                        account_id: row.3,
                        account_label: row.4,
                        account_kind: row.5,
                        status: row.6,
                        connection_revision: row.7,
                        credential_revision: row.8,
                        grant_revision: row.9,
                        policy_revision: row.10,
                        credential_generation: row.11,
                        granted_scopes: serde_json::from_str(&row.12)?,
                        allowed_operations: serde_json::from_str(&row.13)?,
                        descriptor_relative_path: row.14,
                        credential_relative_path: row.15,
                        diagnostic_code: row.16,
                    })
                })
                .collect::<Result<Vec<_>, serde_json::Error>>()
                .map_err(StoreError::from)
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

fn validate_connection_snapshot(connections: &[ConnectionProjection]) -> Result<(), StoreError> {
    if connections.len() > MAX_DEFINITIONS {
        return Err(invariant("adapter connection snapshot exceeds its bound"));
    }
    let mut ids = BTreeSet::new();
    let mut slugs = BTreeSet::new();
    for connection in connections {
        let expected_descriptor = format!(
            "adapters/connections/{}/connection.json",
            connection.connection_id
        );
        let valid_active = connection.status != "blocked"
            && connection
                .connection_slug
                .as_deref()
                .is_some_and(|slug| valid_component(slug, 96))
            && connection
                .semantic_digest
                .as_deref()
                .is_some_and(valid_digest)
            && connection
                .account_kind
                .as_deref()
                .is_some_and(|kind| valid_component(kind, 96))
            && connection
                .connection_revision
                .is_some_and(|revision| revision > 0)
            && connection.credential_revision.is_some()
            && connection
                .grant_revision
                .is_some_and(|revision| revision > 0)
            && connection
                .policy_revision
                .is_some_and(|revision| revision > 0)
            && connection.diagnostic_code.is_none();
        let valid_blocked = connection.status == "blocked"
            && connection.connection_slug.is_none()
            && connection.semantic_digest.is_none()
            && connection.account_label.is_none()
            && connection.account_kind.is_none()
            && connection.connection_revision.is_none()
            && connection.credential_revision.is_none()
            && connection.grant_revision.is_none()
            && connection.policy_revision.is_none()
            && connection.credential_generation.is_none()
            && connection.credential_relative_path.is_none()
            && connection
                .diagnostic_code
                .is_some_and(|code| valid_component(code, 64));
        if !valid_connection_id(&connection.connection_id)
            || !ids.insert(connection.connection_id.as_str())
            || connection.descriptor_relative_path != expected_descriptor
            || !matches!(
                connection.status,
                "active" | "suspended" | "authentication_required" | "blocked"
            )
            || !(valid_active || valid_blocked)
            || !sorted_unique_text(&connection.granted_scopes, 256)
            || !sorted_unique_components(&connection.allowed_operations)
            || connection
                .account_label
                .as_deref()
                .is_some_and(|label| !valid_label(label))
            || connection
                .connection_slug
                .as_deref()
                .is_some_and(|slug| !slugs.insert(slug))
            || !valid_credential_reference(connection)
        {
            return Err(invariant("adapter connection projection is invalid"));
        }
    }
    Ok(())
}

fn valid_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn valid_credential_reference(connection: &ConnectionProjection) -> bool {
    match (
        &connection.credential_generation,
        &connection.credential_relative_path,
    ) {
        (None, None) => connection.credential_revision == Some(0) || connection.status == "blocked",
        (Some(generation), Some(path)) => {
            valid_connection_id(generation)
                && connection
                    .credential_revision
                    .is_some_and(|revision| revision > 0)
                && path
                    == &format!(
                        "adapters/connections/{}/credentials/{generation}.json",
                        connection.connection_id
                    )
        }
        _ => false,
    }
}

fn valid_connection_id(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn sorted_unique_components(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
        && values.iter().all(|value| valid_component(value, 256))
}

fn sorted_unique_text(values: &[String], max: usize) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
        && values.iter().all(|value| {
            !value.is_empty()
                && value.len() <= max
                && value.trim() == value
                && !value.bytes().any(|byte| byte.is_ascii_control())
        })
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
#[path = "adapters/tests.rs"]
mod tests;
