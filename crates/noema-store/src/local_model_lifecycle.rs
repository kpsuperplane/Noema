//! Transactional reconstruction and retirement coordination for local models.

use std::io;

use noema_providers::{
    ClaimedLocalModelInstallation, LocalModelEventKind, LocalModelInstallationStatus,
    LocalModelInstanceReference, LocalModelInstanceReferenceSource,
    LocalModelReconstructionSnapshot, LocalModelRetirementClaimResult,
    LocalModelRuntimeRetirementResult, ProviderInstanceKey, RemovedLocalModelInstallation,
};
use rusqlite::{OptionalExtension, Transaction};

use crate::{
    NoemaStore, StoreError,
    local_model_rows::{INSTALLATION_SELECT, installation_from_raw, raw_installation_from_row},
    local_models::append_event,
    sqlite::{conversion_failure, parse_column},
};

/// One authoritative projection of every row that can lawfully create a future
/// local-provider lease. Claim guards and reconstruction both use this query.
const REFERENCE_ROWS_SQL: &str = r#"
SELECT provider_instance_key, 'active_installation' AS source_kind,
       installation_id AS source_id
FROM local_model_installations
WHERE is_active = 1
UNION ALL
SELECT provider_instance_key, 'default_model_preference', NULL
FROM default_model_preference
WHERE provider_kind = 'local_models'
UNION ALL
SELECT provider_instance_key, 'agent_runtime_preference', agent_id
FROM agent_runtime_preferences
WHERE provider_kind = 'local_models'
UNION ALL
SELECT provider_instance_key, 'auxiliary_model_preference', task_id
FROM auxiliary_model_preferences
WHERE provider_kind = 'local_models'
UNION ALL
SELECT provider_instance_key, 'memory_service', NULL
FROM memory_service_settings
WHERE provider_kind = 'local_models'
UNION ALL
SELECT provider_instance_key, 'task_model_pool', pool_entry_id
FROM task_model_pool_entries
WHERE provider_kind = 'local_models' AND enabled = 1
UNION ALL
SELECT executor_provider_instance_key, 'task_snapshot', task_id
FROM tasks
WHERE executor_provider_kind = 'local_models'
  AND status NOT IN ('completed', 'failed', 'cancelled')
UNION ALL
SELECT reviewer_provider_instance_key, 'task_snapshot', task_id
FROM tasks
WHERE reviewer_provider_kind = 'local_models'
  AND status NOT IN ('completed', 'failed', 'cancelled')
UNION ALL
SELECT agent_runs.provider_instance_key, 'agent_run_snapshot', agent_runs.run_id
FROM agent_runs
LEFT JOIN tasks ON tasks.task_id = agent_runs.task_id
WHERE agent_runs.provider_kind = 'local_models'
  AND (
    agent_runs.status IN ('queued', 'leased', 'running', 'waiting_for_approval')
    OR (
      agent_runs.status = 'interrupted'
      AND agent_runs.cancellation_requested = 0
      AND tasks.status NOT IN ('completed', 'failed', 'cancelled')
    )
  )
"#;

impl NoemaStore {
    /// Read installations and all future-reference owners from one SQLite snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when persisted installation/reference rows cannot
    /// be read as one consistent snapshot.
    pub async fn local_model_reconstruction_snapshot(
        &self,
    ) -> Result<LocalModelReconstructionSnapshot, StoreError> {
        self.with_connection(|conn| {
            let transaction = conn.transaction()?;
            let installations = installations_tx(&transaction)?;
            let references = references_tx(&transaction, None)?;
            transaction.commit()?;
            Ok(LocalModelReconstructionSnapshot {
                installations,
                references,
            })
        })
        .await
    }

    /// Atomically mark one installed, inactive, unreferenced runtime as stopped.
    ///
    /// The durable marker closes the race with future-reference writers: those
    /// writers reject the marker in their own transactions, while activation
    /// may clear it only as part of publishing a newly ready runtime.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when SQLite cannot establish or classify the
    /// reversible runtime-retirement intent.
    pub async fn retire_unreferenced_instance_runtime(
        &self,
        provider_instance_key: &ProviderInstanceKey,
    ) -> Result<LocalModelRuntimeRetirementResult, StoreError> {
        self.with_immediate_transaction_retry(|transaction| {
            let Some(installation) = installation_by_key_tx(transaction, provider_instance_key)?
            else {
                return Ok(LocalModelRuntimeRetirementResult::Missing);
            };
            let references = reference_sources_tx(transaction, provider_instance_key)?;
            if installation.is_active || !references.is_empty() {
                return Ok(LocalModelRuntimeRetirementResult::Referenced {
                    installation,
                    references,
                });
            }
            if installation.retirement_claimed_at.is_some() {
                return Err(StoreError::LocalModelRetirementConflict {
                    operation: "runtime_retire_removal_claimed_local_model",
                });
            }
            if installation.status != LocalModelInstallationStatus::Installed {
                return Err(StoreError::LocalModelRetirementConflict {
                    operation: "runtime_retire_non_installed_local_model",
                });
            }
            if installation.runtime_retired_at.is_some() {
                return Ok(LocalModelRuntimeRetirementResult::AlreadyRetired(
                    installation,
                ));
            }

            let retire_sql = format!(
                r#"
                UPDATE local_model_installations
                SET runtime_retired_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE provider_instance_key = ?1
                  AND status = 'installed'
                  AND is_active = 0
                  AND runtime_retired_at IS NULL
                  AND retirement_claimed_at IS NULL
                  AND NOT EXISTS (
                    SELECT 1 FROM ({REFERENCE_ROWS_SQL}) AS reference_rows
                    WHERE reference_rows.provider_instance_key = ?1
                  )
                "#,
            );
            if transaction.execute(&retire_sql, [provider_instance_key.as_str()])? != 1 {
                return Err(StoreError::LocalModelRetirementConflict {
                    operation: "retire_unreferenced_instance_runtime",
                });
            }
            let retired = installation_by_key_tx(transaction, provider_instance_key)?.ok_or(
                StoreError::LocalModelRetirementConflict {
                    operation: "read_runtime_retired_local_model_installation",
                },
            )?;
            Ok(LocalModelRuntimeRetirementResult::Retired(retired))
        })
        .await
    }

    /// Atomically claim an inactive exact instance for removal after proving zero references.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when SQLite cannot establish or classify the
    /// monotonic retirement claim.
    pub async fn claim_unreferenced_instance_for_retirement(
        &self,
        provider_instance_key: &ProviderInstanceKey,
    ) -> Result<LocalModelRetirementClaimResult, StoreError> {
        self.with_immediate_transaction_retry(|transaction| {
            let Some(installation) = installation_by_key_tx(transaction, provider_instance_key)?
            else {
                return Ok(LocalModelRetirementClaimResult::Missing);
            };
            let references = reference_sources_tx(transaction, provider_instance_key)?;
            if installation.is_active || !references.is_empty() {
                return Ok(LocalModelRetirementClaimResult::Referenced {
                    installation,
                    references,
                });
            }

            let already_claimed = installation.retirement_claimed_at.is_some();
            if !already_claimed {
                if installation.status != LocalModelInstallationStatus::Installed {
                    return Err(StoreError::LocalModelRetirementConflict {
                        operation: "claim_non_installed_local_model",
                    });
                }
                let claim_sql = format!(
                    r#"
                    UPDATE local_model_installations
                    SET retirement_claimed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                        updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                    WHERE provider_instance_key = ?1
                      AND status = 'installed'
                      AND is_active = 0
                      AND retirement_claimed_at IS NULL
                      AND NOT EXISTS (
                        SELECT 1 FROM ({REFERENCE_ROWS_SQL}) AS reference_rows
                        WHERE reference_rows.provider_instance_key = ?1
                      )
                    "#,
                );
                if transaction.execute(&claim_sql, [provider_instance_key.as_str()])? != 1 {
                    return Err(StoreError::LocalModelRetirementConflict {
                        operation: "claim_unreferenced_instance_for_retirement",
                    });
                }
            }

            let claimed = installation_by_key_tx(transaction, provider_instance_key)?.ok_or(
                StoreError::LocalModelRetirementConflict {
                    operation: "read_claimed_local_model_installation",
                },
            )?;
            let claim = ClaimedLocalModelInstallation {
                installation: claimed,
            };
            Ok(if already_claimed {
                LocalModelRetirementClaimResult::AlreadyClaimed(claim)
            } else {
                LocalModelRetirementClaimResult::Claimed(claim)
            })
        })
        .await
    }

    /// Delete one claimed, inactive, still-unreferenced exact installation.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the claim is absent, a future reference
    /// exists, or the guarded removal cannot commit.
    pub async fn complete_claimed_local_model_removal(
        &self,
        provider_instance_key: &ProviderInstanceKey,
    ) -> Result<RemovedLocalModelInstallation, StoreError> {
        self.with_immediate_transaction_retry(|transaction| {
            let installation = installation_by_key_tx(transaction, provider_instance_key)?
                .ok_or_else(|| StoreError::LocalModelInstallationNotFound {
                    installation_id: provider_instance_key.to_string(),
                })?;
            if installation.is_active {
                return Err(StoreError::ActiveLocalModelInstallation {
                    installation_id: installation.installation_id,
                });
            }
            if !reference_sources_tx(transaction, provider_instance_key)?.is_empty() {
                return Err(StoreError::ProviderInstanceReferenced {
                    provider_instance_key: provider_instance_key.to_string(),
                });
            }
            if installation.retirement_claimed_at.is_none() {
                return Err(StoreError::LocalModelRetirementConflict {
                    operation: "complete_unclaimed_local_model_removal",
                });
            }

            append_event(
                transaction,
                &installation.installation_id,
                LocalModelEventKind::Removed,
                Some(installation.downloaded_bytes),
                installation.expected_bytes,
                None,
            )?;
            let delete_sql = format!(
                r#"
                DELETE FROM local_model_installations
                WHERE provider_instance_key = ?1
                  AND is_active = 0
                  AND retirement_claimed_at IS NOT NULL
                  AND NOT EXISTS (
                    SELECT 1 FROM ({REFERENCE_ROWS_SQL}) AS reference_rows
                    WHERE reference_rows.provider_instance_key = ?1
                  )
                "#,
            );
            if transaction.execute(&delete_sql, [provider_instance_key.as_str()])? != 1 {
                return Err(StoreError::LocalModelRetirementConflict {
                    operation: "complete_claimed_local_model_removal",
                });
            }
            // Verified content-addressed blobs are reclaimed only by a future
            // digest-aware GC. A row deletion never authorizes filesystem
            // deletion after this commit.
            Ok(RemovedLocalModelInstallation { installation })
        })
        .await
    }
}

fn installations_tx(
    transaction: &Transaction<'_>,
) -> Result<Vec<noema_providers::LocalModelInstallationRecord>, StoreError> {
    let mut statement =
        transaction.prepare(&format!("{INSTALLATION_SELECT} ORDER BY installation_id"))?;
    let rows = statement
        .query_map([], raw_installation_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter().map(installation_from_raw).collect()
}

fn installation_by_key_tx(
    transaction: &Transaction<'_>,
    provider_instance_key: &ProviderInstanceKey,
) -> Result<Option<noema_providers::LocalModelInstallationRecord>, StoreError> {
    let raw = transaction
        .query_row(
            &format!("{INSTALLATION_SELECT} WHERE provider_instance_key = ?1 LIMIT 1"),
            [provider_instance_key.as_str()],
            raw_installation_from_row,
        )
        .optional()?;
    raw.map(installation_from_raw).transpose()
}

fn references_tx(
    transaction: &Transaction<'_>,
    provider_instance_key: Option<&ProviderInstanceKey>,
) -> Result<Vec<LocalModelInstanceReference>, StoreError> {
    let (sql, exact_key) = match provider_instance_key {
        Some(key) => (
            format!(
                "SELECT DISTINCT provider_instance_key, source_kind, source_id \
                 FROM ({REFERENCE_ROWS_SQL}) AS reference_rows \
                 WHERE provider_instance_key = ?1 \
                 ORDER BY source_kind, COALESCE(source_id, '')"
            ),
            Some(key.as_str()),
        ),
        None => (
            format!(
                "SELECT DISTINCT provider_instance_key, source_kind, source_id \
                 FROM ({REFERENCE_ROWS_SQL}) AS reference_rows \
                 ORDER BY provider_instance_key, source_kind, COALESCE(source_id, '')"
            ),
            None,
        ),
    };
    let mut statement = transaction.prepare(&sql)?;
    let rows = if let Some(key) = exact_key {
        statement.query_map([key], reference_from_row)?
    } else {
        statement.query_map([], reference_from_row)?
    };
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::Sqlite)
}

fn reference_sources_tx(
    transaction: &Transaction<'_>,
    provider_instance_key: &ProviderInstanceKey,
) -> Result<Vec<LocalModelInstanceReferenceSource>, StoreError> {
    references_tx(transaction, Some(provider_instance_key)).map(|references| {
        references
            .into_iter()
            .map(|reference| reference.source)
            .collect()
    })
}

fn reference_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<LocalModelInstanceReference> {
    let provider_instance_key = parse_column(row, 0)?;
    let source_kind = row.get::<_, String>(1)?;
    let source_id = row.get::<_, Option<String>>(2)?;
    let source = match source_kind.as_str() {
        "active_installation" => LocalModelInstanceReferenceSource::ActiveInstallation,
        "default_model_preference" => LocalModelInstanceReferenceSource::DefaultModelPreference,
        "agent_runtime_preference" => LocalModelInstanceReferenceSource::AgentRuntimePreference {
            agent_id: required_source_id(source_id, &source_kind)?,
        },
        "auxiliary_model_preference" => {
            LocalModelInstanceReferenceSource::AuxiliaryModelPreference {
                workload: required_source_id(source_id, &source_kind)?,
            }
        }
        "memory_service" => LocalModelInstanceReferenceSource::MemoryService,
        "task_model_pool" => LocalModelInstanceReferenceSource::TaskModelPool {
            pool_entry_id: required_source_id(source_id, &source_kind)?,
        },
        "task_snapshot" => LocalModelInstanceReferenceSource::TaskSnapshot {
            task_id: required_source_id(source_id, &source_kind)?,
        },
        "agent_run_snapshot" => LocalModelInstanceReferenceSource::AgentRunSnapshot {
            run_id: required_source_id(source_id, &source_kind)?,
        },
        _ => {
            return Err(conversion_failure(
                1,
                rusqlite::types::Type::Text,
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "unknown local-model reference source",
                ),
            ));
        }
    };
    Ok(LocalModelInstanceReference {
        provider_instance_key,
        source,
    })
}

fn required_source_id(source_id: Option<String>, source_kind: &str) -> rusqlite::Result<String> {
    source_id.ok_or_else(|| {
        conversion_failure(
            2,
            rusqlite::types::Type::Null,
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{source_kind} reference has no owner id"),
            ),
        )
    })
}
