//! Rebuildable SQLite projection of filesystem-canonical adapter definitions.

use noema_capability_adapters::{
    AuthorizationGrantStatus, AuthorizationGrantV1, ConnectionProjection, DefinitionProjection,
    OauthApplicationStatus, OauthAuthoritySnapshot,
};
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
    /// Recognizable connection label when discovered.
    pub connection_label: Option<String>,
    /// Current lifecycle or blocked status.
    pub status: String,
    /// Descriptor/lifecycle revision.
    pub connection_revision: Option<u64>,
    /// Credential generation revision.
    pub credential_revision: Option<u64>,
    /// Reviewed policy revision.
    pub policy_revision: Option<u64>,
    /// Current secret generation identity without its bytes.
    pub credential_generation: Option<String>,
    /// Reusable OAuth grant identity when selected.
    pub grant_id: Option<String>,
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
    /// Replace all rebuildable OAuth authority projections in one transaction.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the snapshot is invalid or SQLite fails.
    #[cfg(test)]
    pub(crate) async fn reconcile_adapter_oauth_authorities(
        &self,
        snapshot: &OauthAuthoritySnapshot,
    ) -> Result<(), StoreError> {
        self.reconcile_adapter_state(Some(snapshot), None).await
    }

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
    #[cfg(test)]
    pub(crate) async fn reconcile_adapter_connections(
        &self,
        connections: &[ConnectionProjection],
    ) -> Result<(), StoreError> {
        self.reconcile_adapter_state(None, Some(connections)).await
    }

    /// Replace OAuth and connection projections in one transaction.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when either snapshot is invalid or SQLite fails.
    pub async fn reconcile_complete_adapter_state(
        &self,
        oauth: &OauthAuthoritySnapshot,
        connections: &[ConnectionProjection],
    ) -> Result<(), StoreError> {
        self.reconcile_adapter_state(Some(oauth), Some(connections))
            .await
    }

    async fn reconcile_adapter_state(
        &self,
        oauth: Option<&OauthAuthoritySnapshot>,
        connections: Option<&[ConnectionProjection]>,
    ) -> Result<(), StoreError> {
        if let Some(snapshot) = oauth {
            validate_oauth_snapshot(snapshot)?;
        }
        if let Some(snapshot) = connections {
            validate_connection_snapshot(snapshot)?;
        }
        let grant_scopes = oauth
            .into_iter()
            .flat_map(|snapshot| &snapshot.grants)
            .map(|grant| {
                Ok((
                    grant,
                    serde_json::to_string(&grant.desired_scopes)?,
                    serde_json::to_string(&grant.granted_scopes)?,
                ))
            })
            .collect::<Result<Vec<_>, serde_json::Error>>()?;
        let connection_rows = connections
            .into_iter()
            .flatten()
            .map(|connection| {
                Ok((
                    connection,
                    serde_json::to_string(&connection.allowed_operations)?,
                ))
            })
            .collect::<Result<Vec<_>, serde_json::Error>>()?;
        self.with_connection(|connection| {
            let transaction =
                connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            if connections.is_some() {
                transaction.execute("DELETE FROM adapter_connections", [])?;
            }
            if let Some(snapshot) = oauth {
                replace_adapter_oauth_authorities(&transaction, snapshot, &grant_scopes)?;
            }
            for (adapter, allowed_operations) in &connection_rows {
                transaction.execute(
                    r#"
                    INSERT INTO adapter_connections (
                      connection_id, connection_slug, semantic_digest, connection_label,
                      status, connection_revision, credential_revision, policy_revision,
                      credential_generation, grant_id, allowed_operations_json,
                      descriptor_relative_path, credential_relative_path, diagnostic_code
                    ) VALUES (
                      ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14
                    )
                    "#,
                    params![
                        adapter.connection_id,
                        adapter.connection_slug,
                        adapter.semantic_digest,
                        adapter.connection_label,
                        adapter.status,
                        adapter.connection_revision,
                        adapter.credential_revision,
                        adapter.policy_revision,
                        adapter.credential_generation,
                        adapter.grant_id,
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
                SELECT connection_id, connection_slug, semantic_digest, connection_label,
                       status, connection_revision, credential_revision, policy_revision,
                       credential_generation, grant_id, allowed_operations_json,
                       descriptor_relative_path, credential_relative_path, diagnostic_code
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
                        row.get::<_, String>(4)?,
                        row.get::<_, Option<u64>>(5)?,
                        row.get::<_, Option<u64>>(6)?,
                        row.get::<_, Option<u64>>(7)?,
                        row.get::<_, Option<String>>(8)?,
                        row.get::<_, Option<String>>(9)?,
                        row.get::<_, String>(10)?,
                        row.get::<_, String>(11)?,
                        row.get::<_, Option<String>>(12)?,
                        row.get::<_, Option<String>>(13)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            rows.into_iter()
                .map(|row| {
                    Ok(AdapterConnectionRecord {
                        connection_id: row.0,
                        connection_slug: row.1,
                        semantic_digest: row.2,
                        connection_label: row.3,
                        status: row.4,
                        connection_revision: row.5,
                        credential_revision: row.6,
                        policy_revision: row.7,
                        credential_generation: row.8,
                        grant_id: row.9,
                        allowed_operations: serde_json::from_str(&row.10)?,
                        descriptor_relative_path: row.11,
                        credential_relative_path: row.12,
                        diagnostic_code: row.13,
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
                .connection_revision
                .is_some_and(|revision| revision > 0)
            && connection
                .policy_revision
                .is_some_and(|revision| revision > 0)
            && connection.diagnostic_code.is_none();
        let valid_blocked = connection.status == "blocked"
            && connection.connection_slug.is_none()
            && connection.semantic_digest.is_none()
            && connection.connection_label.is_none()
            && connection.connection_revision.is_none()
            && connection.credential_revision.is_none()
            && connection.policy_revision.is_none()
            && connection.credential_generation.is_none()
            && connection.grant_id.is_none()
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
            || !sorted_unique_components(&connection.allowed_operations)
            || connection
                .connection_label
                .as_deref()
                .is_some_and(|label| !valid_label(label))
            || connection
                .connection_slug
                .as_deref()
                .is_some_and(|slug| !slugs.insert(slug))
            || connection
                .grant_id
                .as_deref()
                .is_some_and(|grant_id| !valid_connection_id(grant_id))
            || connection.grant_id.is_some() && connection.credential_generation.is_some()
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
        (None, None) => connection.credential_revision.is_none(),
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

fn replace_adapter_oauth_authorities(
    transaction: &rusqlite::Transaction<'_>,
    snapshot: &OauthAuthoritySnapshot,
    grant_scopes: &[(&AuthorizationGrantV1, String, String)],
) -> Result<(), StoreError> {
    transaction.execute("DELETE FROM adapter_oauth_grants", [])?;
    transaction.execute("DELETE FROM adapter_external_accounts", [])?;
    transaction.execute("DELETE FROM adapter_oauth_applications", [])?;
    transaction.execute("DELETE FROM adapter_oauth_profiles", [])?;
    for profile in &snapshot.profiles {
        transaction.execute(
            r#"INSERT INTO adapter_oauth_profiles (
              profile_digest, profile_id, display_name, grant_audience,
              descriptor_relative_path
            ) VALUES (?1, ?2, ?3, ?4, ?5)"#,
            params![
                profile.profile_digest,
                profile.profile.profile_id,
                profile.profile.display_name,
                profile.profile.grant_audience,
                format!(
                    "adapters/oauth-profiles/{}/profile.json",
                    profile.profile_digest
                ),
            ],
        )?;
    }
    for application in &snapshot.applications {
        transaction.execute(
            r#"INSERT INTO adapter_oauth_applications (
              application_id, profile_digest, callback_mode, client_id,
              project_label, status, revision, credential_generation,
              descriptor_relative_path
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"#,
            params![
                application.application_id,
                application.profile_digest,
                callback_mode(application.callback_mode),
                application.client_id,
                application.project_label,
                application_status(application.status),
                application.revision,
                application.credential_generation,
                format!(
                    "adapters/oauth-applications/{}/application.json",
                    application.application_id
                ),
            ],
        )?;
    }
    for account in &snapshot.accounts {
        transaction.execute(
            r#"INSERT INTO adapter_external_accounts (
              account_id, profile_digest, provider_subject, account_label,
              revision, descriptor_relative_path
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)"#,
            params![
                account.account_id,
                account.profile_digest,
                account.provider_subject,
                account.account_label,
                account.revision,
                format!(
                    "adapters/external-accounts/{}/account.json",
                    account.account_id
                ),
            ],
        )?;
    }
    for (grant, desired_scopes, granted_scopes) in grant_scopes {
        transaction.execute(
            r#"INSERT INTO adapter_oauth_grants (
              grant_id, application_id, account_id, account_label, audience,
              desired_scopes_json, granted_scopes_json, authority_revision,
              token_revision, status, descriptor_relative_path
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"#,
            params![
                grant.grant_id,
                grant.application_id,
                grant.account_id,
                grant.account_label,
                grant.audience,
                desired_scopes,
                granted_scopes,
                grant.authority_revision,
                grant.token_revision,
                grant_status(grant.status),
                format!("adapters/oauth-grants/{}/grant.json", grant.grant_id),
            ],
        )?;
    }
    Ok(())
}

fn validate_oauth_snapshot(snapshot: &OauthAuthoritySnapshot) -> Result<(), StoreError> {
    if [
        snapshot.profiles.len(),
        snapshot.applications.len(),
        snapshot.accounts.len(),
        snapshot.grants.len(),
    ]
    .into_iter()
    .any(|count| count > MAX_DEFINITIONS)
    {
        return Err(invariant("adapter OAuth snapshot exceeds its bound"));
    }
    let profiles = snapshot
        .profiles
        .iter()
        .map(|profile| profile.profile_digest.as_str())
        .collect::<BTreeSet<_>>();
    let applications = snapshot
        .applications
        .iter()
        .map(|application| application.application_id.as_str())
        .collect::<BTreeSet<_>>();
    let accounts = snapshot
        .accounts
        .iter()
        .map(|account| account.account_id.as_str())
        .collect::<BTreeSet<_>>();
    if profiles.len() != snapshot.profiles.len()
        || applications.len() != snapshot.applications.len()
        || accounts.len() != snapshot.accounts.len()
        || snapshot
            .applications
            .iter()
            .any(|application| !profiles.contains(application.profile_digest.as_str()))
        || snapshot
            .accounts
            .iter()
            .any(|account| !profiles.contains(account.profile_digest.as_str()))
    {
        return Err(invariant("adapter OAuth projection is invalid"));
    }
    let mut grants = BTreeSet::new();
    for grant in &snapshot.grants {
        if !grants.insert(grant.grant_id.as_str())
            || !applications.contains(grant.application_id.as_str())
            || grant
                .account_id
                .as_deref()
                .is_some_and(|account| !accounts.contains(account))
            || !sorted_unique_text(&grant.desired_scopes, 1_024)
            || !sorted_unique_text(&grant.granted_scopes, 1_024)
        {
            return Err(invariant("adapter OAuth projection is invalid"));
        }
    }
    Ok(())
}

const fn callback_mode(mode: noema_capability_adapters::Oauth2CallbackMode) -> &'static str {
    match mode {
        noema_capability_adapters::Oauth2CallbackMode::Loopback => "loopback",
        noema_capability_adapters::Oauth2CallbackMode::Hosted => "hosted",
    }
}

const fn application_status(status: OauthApplicationStatus) -> &'static str {
    match status {
        OauthApplicationStatus::Active => "active",
        OauthApplicationStatus::Suspended => "suspended",
    }
}

const fn grant_status(status: AuthorizationGrantStatus) -> &'static str {
    match status {
        AuthorizationGrantStatus::Active => "active",
        AuthorizationGrantStatus::AuthenticationRequired => "authentication_required",
        AuthorizationGrantStatus::Revoked => "revoked",
        AuthorizationGrantStatus::Blocked => "blocked",
    }
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
