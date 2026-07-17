//! SQLite persistence for local-model installations and global activation.

use rusqlite::{OptionalExtension, Transaction, params, types::Type};

use noema_providers::{
    DefaultModelPreferenceRecord, LOCAL_MODELS_PROVIDER_ACCOUNT_ID, LocalModelEventKind,
    LocalModelEventRecord, LocalModelInstallationRecord, LocalModelInstallationStatus,
    LocalModelInstallationUpdate, LocalModelSourceKind, NewLocalModelInstallation,
    ProviderInstanceKey, ProviderReadySelection, ProviderSelectionSnapshot, ReasoningEffort,
    RemovedLocalModelInstallation, local_model_provider_instance_key,
};

use super::local_model_rows::{
    INSTALLATION_SELECT, backend_str, event_from_raw, installation_from_raw, optional_u64_to_i64,
    raw_event_from_row, raw_installation_from_row, u64_to_i64,
};
use super::{NoemaStore, StoreError, provider_selections::resolve_new_canonical_selection_tx};

impl NoemaStore {
    /// Create or refresh queued installation provenance.
    ///
    /// Existing completed installations are left intact so retrying setup never
    /// downgrades an installed blob.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when metadata is invalid or SQLite fails.
    pub async fn upsert_local_model_installation(
        &self,
        input: NewLocalModelInstallation,
    ) -> Result<LocalModelInstallationRecord, StoreError> {
        validate_new_installation(&input)?;
        let expected_bytes = optional_u64_to_i64(input.expected_bytes, "expected bytes")?;
        let provider_instance_key = local_model_provider_instance_key(
            LOCAL_MODELS_PROVIDER_ACCOUNT_ID,
            &input.installation_id,
            &input.model_id,
        )
        .map_err(|error| StoreError::InvariantViolation {
            message: format!("invalid local-model provider instance identity: {error}"),
        })?;
        self.with_connection(|conn| {
            let transaction = conn.transaction()?;
            let existing = transaction
                .query_row(
                    "SELECT status, model_id, provider_instance_key, retirement_claimed_at FROM local_model_installations WHERE installation_id = ?1",
                    [&input.installation_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, Option<String>>(3)?,
                        ))
                    },
                )
                .optional()?;
            if let Some((status, model_id, existing_key, retirement_claimed_at)) = existing.as_ref() {
                if retirement_claimed_at.is_some() {
                    return Err(StoreError::InvariantViolation {
                        message: format!(
                            "local-model installation is already claimed for retirement: {}",
                            input.installation_id
                        ),
                    });
                }
                if model_id != &input.model_id {
                    return Err(StoreError::InvariantViolation {
                        message: format!(
                            "local-model installation identity cannot change model: {}",
                            input.installation_id
                        ),
                    });
                }
                if existing_key != provider_instance_key.as_str() {
                    return Err(StoreError::ProviderInstanceKeyMismatch {
                        provider_instance_key: existing_key.clone(),
                    });
                }
                if status == LocalModelInstallationStatus::Installed.as_str() {
                    let installation =
                        installation_in_transaction(&transaction, &input.installation_id)?;
                    transaction.commit()?;
                    return Ok(installation);
                }
            }
            transaction.execute(
                r#"
                INSERT INTO local_model_installations (
                  installation_id, provider_instance_key, model_id, display_name,
                  source_kind, source_repo, source_revision, source_file, sha256,
                  download_gb, expected_bytes, downloaded_bytes, license, backend, status
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 0, ?12, ?13, 'queued')
                ON CONFLICT(installation_id) DO UPDATE SET
                  provider_instance_key = excluded.provider_instance_key,
                  model_id = excluded.model_id,
                  display_name = excluded.display_name,
                  source_kind = excluded.source_kind,
                  source_repo = excluded.source_repo,
                  source_revision = excluded.source_revision,
                  source_file = excluded.source_file,
                  sha256 = excluded.sha256,
                  download_gb = excluded.download_gb,
                  expected_bytes = COALESCE(local_model_installations.expected_bytes, excluded.expected_bytes),
                  license = excluded.license,
                  backend = excluded.backend,
                  downloaded_bytes = 0,
                  blob_relative_path = NULL,
                  is_active = 0,
                  installed_at = NULL,
                  status = 'queued',
                  error_code = NULL,
                  error_message = NULL,
                  updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                "#,
                params![
                    input.installation_id,
                    provider_instance_key.as_str(),
                    input.model_id,
                    input.display_name,
                    input.source_kind.as_str(),
                    input.source_repo,
                    input.source_revision,
                    input.source_file,
                    input.sha256,
                    input.download_gb,
                    expected_bytes,
                    input.license,
                    backend_str(input.backend),
                ],
            )?;
            append_event(
                &transaction,
                &input.installation_id,
                LocalModelEventKind::Queued,
                Some(0),
                input.expected_bytes,
                None,
            )?;
            let installation = installation_in_transaction(&transaction, &input.installation_id)?;
            transaction.commit()?;
            Ok(installation)
        })
        .await
    }

    /// Return one local-model installation by id.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when SQLite or stored vocabulary decoding fails.
    pub async fn get_local_model_installation(
        &self,
        installation_id: &str,
    ) -> Result<Option<LocalModelInstallationRecord>, StoreError> {
        let raw = self
            .with_connection(|conn| {
                conn.query_row(
                    &format!("{INSTALLATION_SELECT} WHERE installation_id = ?1 LIMIT 1"),
                    [installation_id],
                    raw_installation_from_row,
                )
                .optional()
                .map_err(StoreError::Sqlite)
            })
            .await?;
        raw.map(installation_from_raw).transpose()
    }

    /// Return the newest installed artifact for a provider-facing model id.
    ///
    /// The active row wins when multiple advanced imports share a model id.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when SQLite or stored vocabulary decoding fails.
    pub async fn get_installed_local_model(
        &self,
        model_id: &str,
    ) -> Result<Option<LocalModelInstallationRecord>, StoreError> {
        let raw = self
            .with_connection(|conn| {
                conn.query_row(
                    &format!(
                        "{INSTALLATION_SELECT} WHERE model_id = ?1 AND status = 'installed' \
                         AND retirement_claimed_at IS NULL \
                         ORDER BY is_active DESC, updated_at DESC, installation_id LIMIT 1"
                    ),
                    [model_id],
                    raw_installation_from_row,
                )
                .optional()
                .map_err(StoreError::Sqlite)
            })
            .await?;
        raw.map(installation_from_raw).transpose()
    }

    /// Return every installation in stable UI order.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when SQLite or stored vocabulary decoding fails.
    pub async fn list_local_model_installations(
        &self,
    ) -> Result<Vec<LocalModelInstallationRecord>, StoreError> {
        let raw = self
            .with_connection(|conn| {
                let mut statement = conn.prepare(&format!(
                    "{INSTALLATION_SELECT} ORDER BY is_active DESC, updated_at DESC, installation_id"
                ))?;
                let rows = statement.query_map([], raw_installation_from_row)?;
                rows.collect::<Result<Vec<_>, _>>()
                    .map_err(StoreError::Sqlite)
            })
            .await?;
        raw.into_iter().map(installation_from_raw).collect()
    }

    /// Persist installation state and append its subscription event atomically.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] for impossible byte counts, missing rows, or
    /// SQLite failures.
    pub async fn update_local_model_installation(
        &self,
        installation_id: &str,
        update: LocalModelInstallationUpdate,
    ) -> Result<LocalModelInstallationRecord, StoreError> {
        validate_installation_update(&update)?;
        self.with_connection(|conn| {
            let transaction = conn.transaction()?;
            let current = installation_in_transaction(&transaction, installation_id)?;
            let updated = update_installation_in_transaction(&transaction, current, update)?;
            transaction.commit()?;
            Ok(updated)
        })
        .await
    }

    /// Mark an in-flight installation cancelled and append its event atomically.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the installation is missing, already
    /// installed, or SQLite fails.
    pub async fn cancel_local_model_installation(
        &self,
        installation_id: &str,
    ) -> Result<LocalModelInstallationRecord, StoreError> {
        self.with_connection(|conn| {
            let transaction = conn.transaction()?;
            let current = installation_in_transaction(&transaction, installation_id)?;
            if current.status == LocalModelInstallationStatus::Installed {
                return Err(StoreError::InvalidLocalModelTransition {
                    from: current.status.as_str().to_string(),
                    to: LocalModelInstallationStatus::Cancelled.as_str().to_string(),
                });
            }
            if current.status == LocalModelInstallationStatus::Cancelled {
                transaction.commit()?;
                return Ok(current);
            }
            let update = LocalModelInstallationUpdate {
                status: LocalModelInstallationStatus::Cancelled,
                downloaded_bytes: current.downloaded_bytes,
                expected_bytes: current.expected_bytes,
                sha256: None,
                blob_relative_path: None,
                error_code: None,
                error_message: None,
            };
            let cancelled = update_installation_in_transaction(&transaction, current, update)?;
            transaction.commit()?;
            Ok(cancelled)
        })
        .await
    }

    /// Remove a cancelled or failed installation projection and retain a durable event.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the installation is missing, active,
    /// installed, still in progress, or SQLite fails.
    pub(crate) async fn remove_terminal_local_model_installation(
        &self,
        installation_id: &str,
    ) -> Result<RemovedLocalModelInstallation, StoreError> {
        self.with_immediate_transaction_retry(|transaction| {
            let installation = installation_in_transaction(transaction, installation_id)?;
            if installation.is_active {
                return Err(StoreError::ActiveLocalModelInstallation {
                    installation_id: installation_id.to_string(),
                });
            }
            if installation.runtime_retired_at.is_some()
                || installation.retirement_claimed_at.is_some()
            {
                return Err(StoreError::ProviderInstanceClaimed {
                    provider_instance_key: installation.provider_instance_key.to_string(),
                });
            }
            if !matches!(
                installation.status,
                LocalModelInstallationStatus::Cancelled | LocalModelInstallationStatus::Failed
            ) {
                return Err(StoreError::InvalidLocalModelRequest {
                    kind: "terminal_local_model_removal_requires_cancelled_or_failed",
                });
            }
            append_event(
                transaction,
                installation_id,
                LocalModelEventKind::Removed,
                Some(installation.downloaded_bytes),
                installation.expected_bytes,
                None,
            )?;
            let removed = transaction.execute(
                "DELETE FROM local_model_installations \
                 WHERE installation_id = ?1 AND is_active = 0 \
                   AND status IN ('cancelled', 'failed') \
                   AND runtime_retired_at IS NULL \
                   AND retirement_claimed_at IS NULL",
                [installation_id],
            )?;
            if removed != 1 {
                return Err(StoreError::LocalModelRetirementConflict {
                    operation: "remove_terminal_local_model_installation",
                });
            }
            Ok(RemovedLocalModelInstallation { installation })
        })
        .await
    }

    /// Return local-model events strictly after a cursor.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when SQLite or stored vocabulary decoding fails.
    pub async fn list_local_model_events(
        &self,
        after_cursor: Option<u64>,
        limit: u32,
    ) -> Result<Vec<LocalModelEventRecord>, StoreError> {
        let cursor = u64_to_i64(after_cursor.unwrap_or_default(), "event cursor")?;
        let limit = i64::from(limit.clamp(1, 1_000));
        let raw = self
            .with_connection(|conn| {
                let mut statement = conn.prepare(
                    r#"
                    SELECT cursor, installation_id, kind, downloaded_bytes,
                           expected_bytes, message, created_at
                    FROM local_model_events
                    WHERE cursor > ?1
                    ORDER BY cursor
                    LIMIT ?2
                    "#,
                )?;
                let rows = statement.query_map(params![cursor, limit], raw_event_from_row)?;
                rows.collect::<Result<Vec<_>, _>>()
                    .map_err(StoreError::Sqlite)
            })
            .await?;
        raw.into_iter().map(event_from_raw).collect()
    }

    /// Return Noema's current global model preference.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when SQLite fails.
    pub async fn get_default_model_preference(
        &self,
    ) -> Result<Option<DefaultModelPreferenceRecord>, StoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                r#"
                SELECT provider_kind, provider_account_id, provider_instance_key,
                       model_profile, reasoning_effort, updated_at
                FROM default_model_preference
                WHERE preference_id = 'default'
                "#,
                [],
                default_preference_from_row,
            )
            .optional()
            .map_err(StoreError::Sqlite)
        })
        .await
    }

    /// Save Noema's default for new workloads without changing existing
    /// workload-specific selections.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the account/provider pair is invalid, the
    /// model profile or reasoning effort is invalid, or SQLite fails.
    pub async fn save_default_model_preference(
        &self,
        provider_kind: &str,
        provider_account_id: &str,
        model_profile: &str,
        reasoning_effort: Option<&str>,
    ) -> Result<DefaultModelPreferenceRecord, StoreError> {
        self.save_default_model_preference_inner(
            provider_kind,
            provider_account_id,
            model_profile,
            reasoning_effort,
            None,
        )
        .await
    }

    /// Save Noema's default while retaining an exact provider readiness lease
    /// through commit.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the account/provider pair, model profile,
    /// reasoning effort, or exact readiness proof is invalid, or SQLite fails.
    pub async fn save_default_model_preference_with_ready_selection(
        &self,
        provider_kind: &str,
        provider_account_id: &str,
        model_profile: &str,
        reasoning_effort: Option<&str>,
        ready_selection: &ProviderReadySelection,
    ) -> Result<DefaultModelPreferenceRecord, StoreError> {
        self.save_default_model_preference_inner(
            provider_kind,
            provider_account_id,
            model_profile,
            reasoning_effort,
            Some(ready_selection),
        )
        .await
    }

    async fn save_default_model_preference_inner(
        &self,
        provider_kind: &str,
        provider_account_id: &str,
        model_profile: &str,
        reasoning_effort: Option<&str>,
        ready_selection: Option<&ProviderReadySelection>,
    ) -> Result<DefaultModelPreferenceRecord, StoreError> {
        let provider_kind = provider_kind.trim().to_ascii_lowercase();
        let provider_account_id = provider_account_id.trim();
        let model_profile = model_profile.trim();
        if provider_kind.is_empty() || provider_account_id.is_empty() || model_profile.is_empty() {
            return Err(StoreError::InvariantViolation {
                message: "default model provider, account, and model profile are required"
                    .to_string(),
            });
        }
        if provider_kind == "local_models" {
            return Err(StoreError::InvariantViolation {
                message: "activate a specific local-model installation to change the local default"
                    .to_string(),
            });
        }
        if reasoning_effort.is_some_and(|value| {
            !matches!(
                value,
                "none" | "minimal" | "low" | "medium" | "high" | "xhigh"
            )
        }) {
            return Err(StoreError::InvalidEnum {
                kind: "default model reasoning effort",
                value: reasoning_effort.unwrap_or_default().to_string(),
            });
        }

        let reasoning_effort = reasoning_effort.and_then(ReasoningEffort::from_persistence_str);
        let selection = ProviderSelectionSnapshot::explicit(
            provider_kind,
            provider_account_id,
            model_profile,
            reasoning_effort,
            Some("default_model_preference".to_string()),
        );
        self.with_immediate_transaction_retry(|transaction| {
            let selection =
                resolve_new_canonical_selection_tx(transaction, &selection, ready_selection)?;
            let provider_instance_key = selection
                .provider_instance_key
                .as_ref()
                .ok_or(StoreError::ProviderInstanceKeyMissing)?;
            transaction.execute(
                r#"
                INSERT INTO default_model_preference (
                  preference_id, provider_kind, provider_account_id, provider_instance_key,
                  model_profile, reasoning_effort, updated_at
                ) VALUES ('default', ?1, ?2, ?3, ?4, ?5, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                ON CONFLICT(preference_id) DO UPDATE SET
                  provider_kind = excluded.provider_kind,
                  provider_account_id = excluded.provider_account_id,
                  provider_instance_key = excluded.provider_instance_key,
                  model_profile = excluded.model_profile,
                  reasoning_effort = excluded.reasoning_effort,
                  updated_at = excluded.updated_at
                "#,
                params![
                    selection.provider_kind,
                    selection.provider_account_id,
                    provider_instance_key.as_str(),
                    selection.model_profile,
                    selection
                        .reasoning_effort
                        .map(ReasoningEffort::as_persistence_str)
                ],
            )?;
            default_preference_in_transaction(transaction)
        })
        .await
    }
}

fn default_preference_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<DefaultModelPreferenceRecord> {
    Ok(DefaultModelPreferenceRecord {
        provider_kind: row.get(0)?,
        provider_account_id: row.get(1)?,
        provider_instance_key: provider_instance_key_from_row(row, 2)?,
        model_profile: row.get(3)?,
        reasoning_effort: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

fn default_preference_in_transaction(
    transaction: &Transaction<'_>,
) -> Result<DefaultModelPreferenceRecord, StoreError> {
    transaction
        .query_row(
            r#"
            SELECT provider_kind, provider_account_id, provider_instance_key,
                   model_profile, reasoning_effort, updated_at
            FROM default_model_preference
            WHERE preference_id = 'default'
            "#,
            [],
            default_preference_from_row,
        )
        .map_err(StoreError::Sqlite)
}

fn provider_instance_key_from_row(
    row: &rusqlite::Row<'_>,
    index: usize,
) -> rusqlite::Result<ProviderInstanceKey> {
    let value = row.get::<_, String>(index)?;
    value.parse().map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(error))
    })
}

fn installation_in_transaction(
    transaction: &Transaction<'_>,
    installation_id: &str,
) -> Result<LocalModelInstallationRecord, StoreError> {
    let raw = transaction
        .query_row(
            &format!("{INSTALLATION_SELECT} WHERE installation_id = ?1 LIMIT 1"),
            [installation_id],
            raw_installation_from_row,
        )
        .optional()?
        .ok_or_else(|| StoreError::LocalModelInstallationNotFound {
            installation_id: installation_id.to_string(),
        })?;
    installation_from_raw(raw)
}

fn validate_installation_update(update: &LocalModelInstallationUpdate) -> Result<(), StoreError> {
    if update
        .expected_bytes
        .is_some_and(|total| update.downloaded_bytes > total)
    {
        return Err(StoreError::InvalidLocalModelRequest {
            kind: "downloaded_bytes_exceed_expected_bytes",
        });
    }
    if update.status == LocalModelInstallationStatus::Installed
        && update.blob_relative_path.is_none()
    {
        return Err(StoreError::InvalidLocalModelRequest {
            kind: "installed_model_requires_blob_path",
        });
    }
    Ok(())
}

fn update_installation_in_transaction(
    transaction: &Transaction<'_>,
    current: LocalModelInstallationRecord,
    update: LocalModelInstallationUpdate,
) -> Result<LocalModelInstallationRecord, StoreError> {
    if !current.status.can_transition_to(update.status) {
        return Err(StoreError::InvalidLocalModelTransition {
            from: current.status.as_str().to_string(),
            to: update.status.as_str().to_string(),
        });
    }
    let downloaded_bytes = u64_to_i64(update.downloaded_bytes, "downloaded bytes")?;
    let expected_bytes = optional_u64_to_i64(update.expected_bytes, "expected bytes")?;
    let event_kind = event_kind_for_status(update.status);
    let message = update.error_message.clone();
    let changed = transaction.execute(
        r#"
        UPDATE local_model_installations
        SET status = ?2,
            downloaded_bytes = ?3,
            expected_bytes = COALESCE(?4, expected_bytes),
            sha256 = COALESCE(?5, sha256),
            blob_relative_path = COALESCE(?6, blob_relative_path),
            error_code = ?7,
            error_message = ?8,
            installed_at = CASE WHEN ?2 = 'installed' THEN strftime('%Y-%m-%dT%H:%M:%fZ', 'now') ELSE installed_at END,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE installation_id = ?1
        "#,
        params![
            current.installation_id,
            update.status.as_str(),
            downloaded_bytes,
            expected_bytes,
            update.sha256,
            update.blob_relative_path,
            update.error_code,
            update.error_message,
        ],
    )?;
    if changed != 1 {
        return Err(StoreError::LocalModelInstallationNotFound {
            installation_id: current.installation_id,
        });
    }
    append_event(
        transaction,
        &current.installation_id,
        event_kind,
        Some(update.downloaded_bytes),
        update.expected_bytes,
        message.as_deref(),
    )?;
    installation_in_transaction(transaction, &current.installation_id)
}

pub(super) fn append_event(
    transaction: &Transaction<'_>,
    installation_id: &str,
    kind: LocalModelEventKind,
    downloaded_bytes: Option<u64>,
    expected_bytes: Option<u64>,
    message: Option<&str>,
) -> Result<(), StoreError> {
    transaction.execute(
        r#"
        INSERT INTO local_model_events (
          installation_id, kind, downloaded_bytes, expected_bytes, message
        ) VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
        params![
            installation_id,
            kind.as_str(),
            optional_u64_to_i64(downloaded_bytes, "downloaded bytes")?,
            optional_u64_to_i64(expected_bytes, "expected bytes")?,
            message,
        ],
    )?;
    Ok(())
}

fn event_kind_for_status(status: LocalModelInstallationStatus) -> LocalModelEventKind {
    match status {
        LocalModelInstallationStatus::Queued => LocalModelEventKind::Queued,
        LocalModelInstallationStatus::Downloading => LocalModelEventKind::Progress,
        LocalModelInstallationStatus::Verifying => LocalModelEventKind::Verifying,
        LocalModelInstallationStatus::Installed => LocalModelEventKind::Installed,
        LocalModelInstallationStatus::Failed => LocalModelEventKind::Failed,
        LocalModelInstallationStatus::Cancelled => LocalModelEventKind::Cancelled,
    }
}

fn validate_new_installation(input: &NewLocalModelInstallation) -> Result<(), StoreError> {
    if input.installation_id.trim().is_empty()
        || input.model_id.trim().is_empty()
        || input.display_name.trim().is_empty()
        || !input.download_gb.is_finite()
        || input.download_gb <= 0.0
        || input.sha256.as_deref().is_some_and(|digest| {
            digest.len() != 64
                || !digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        })
    {
        return Err(StoreError::InvalidLocalModelRequest {
            kind: "installation_metadata",
        });
    }
    if input.source_kind == LocalModelSourceKind::Catalog
        && (input.source_repo.is_none()
            || input.source_revision.is_none()
            || input.source_file.is_none()
            || input.sha256.is_none())
    {
        return Err(StoreError::InvalidLocalModelRequest {
            kind: "catalog_installation_provenance",
        });
    }
    Ok(())
}
