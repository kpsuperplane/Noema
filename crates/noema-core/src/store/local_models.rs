//! SQLite persistence for local-model installations and global activation.

use rusqlite::{OptionalExtension, Transaction, params};

use crate::local_models::{
    DefaultModelPreferenceRecord, LocalModelEventKind, LocalModelEventRecord,
    LocalModelInstallationRecord, LocalModelInstallationStatus, LocalModelInstallationUpdate,
    LocalModelSourceKind, NewLocalModelInstallation, RemovedLocalModelInstallation,
};

use super::local_model_rows::{
    INSTALLATION_SELECT, backend_str, event_from_raw, installation_from_raw, optional_u64_to_i64,
    raw_event_from_row, raw_installation_from_row, u64_to_i64,
};
use super::{NoemaStore, StoreError};

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
        self.with_connection(|conn| {
            let transaction = conn.transaction()?;
            let existing_status = transaction
                .query_row(
                    "SELECT status FROM local_model_installations WHERE installation_id = ?1",
                    [&input.installation_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            if existing_status.as_deref() == Some(LocalModelInstallationStatus::Installed.as_str()) {
                transaction.commit()?;
                return Ok(());
            }
            transaction.execute(
                r#"
                INSERT INTO local_model_installations (
                  installation_id, model_id, display_name, source_kind, source_repo,
                  source_revision, source_file, sha256, download_gb, expected_bytes,
                  downloaded_bytes, license, backend, status
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0, ?11, ?12, 'queued')
                ON CONFLICT(installation_id) DO UPDATE SET
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
            transaction.commit()?;
            Ok(())
        })
        .await?;
        self.get_local_model_installation(&input.installation_id)
            .await?
            .ok_or_else(|| StoreError::InvariantViolation {
                message: format!(
                    "local-model installation disappeared: {}",
                    input.installation_id
                ),
            })
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
        if update
            .expected_bytes
            .is_some_and(|total| update.downloaded_bytes > total)
        {
            return Err(StoreError::InvariantViolation {
                message: "local-model downloaded bytes exceed expected bytes".to_string(),
            });
        }
        if update.status == LocalModelInstallationStatus::Installed
            && update.blob_relative_path.is_none()
        {
            return Err(StoreError::InvariantViolation {
                message: "installed local model requires a blob path".to_string(),
            });
        }
        let downloaded_bytes = u64_to_i64(update.downloaded_bytes, "downloaded bytes")?;
        let expected_bytes = optional_u64_to_i64(update.expected_bytes, "expected bytes")?;
        let event_kind = event_kind_for_status(update.status);
        let message = update.error_message.clone();
        self.with_connection(|conn| {
            let transaction = conn.transaction()?;
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
                    installation_id,
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
                return Err(StoreError::InvariantViolation {
                    message: format!("local-model installation not found: {installation_id}"),
                });
            }
            append_event(
                &transaction,
                installation_id,
                event_kind,
                Some(update.downloaded_bytes),
                update.expected_bytes,
                message.as_deref(),
            )?;
            transaction.commit()?;
            Ok(())
        })
        .await?;
        self.get_local_model_installation(installation_id)
            .await?
            .ok_or_else(|| StoreError::InvariantViolation {
                message: format!("local-model installation disappeared: {installation_id}"),
            })
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
        let current = self
            .get_local_model_installation(installation_id)
            .await?
            .ok_or_else(|| StoreError::InvariantViolation {
                message: format!("local-model installation not found: {installation_id}"),
            })?;
        if current.status == LocalModelInstallationStatus::Installed {
            return Err(StoreError::InvariantViolation {
                message: "an installed local model cannot be cancelled".to_string(),
            });
        }
        if current.status == LocalModelInstallationStatus::Cancelled {
            return Ok(current);
        }
        self.update_local_model_installation(
            installation_id,
            LocalModelInstallationUpdate {
                status: LocalModelInstallationStatus::Cancelled,
                downloaded_bytes: current.downloaded_bytes,
                expected_bytes: current.expected_bytes,
                sha256: None,
                blob_relative_path: None,
                error_code: None,
                error_message: None,
            },
        )
        .await
    }

    /// Remove a non-active installation projection and retain a durable event.
    ///
    /// The caller owns deleting `unreferenced_blob_relative_path` after the
    /// transaction. Returning the path instead of deleting it while holding the
    /// SQLite lock keeps filesystem failure handling explicit.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the installation is missing, active, or
    /// SQLite fails.
    pub async fn remove_local_model_installation(
        &self,
        installation_id: &str,
    ) -> Result<RemovedLocalModelInstallation, StoreError> {
        let installation = self
            .get_local_model_installation(installation_id)
            .await?
            .ok_or_else(|| StoreError::InvariantViolation {
                message: format!("local-model installation not found: {installation_id}"),
            })?;
        if installation.is_active {
            return Err(StoreError::InvariantViolation {
                message: "the active local model cannot be removed".to_string(),
            });
        }
        let blob_path = installation.blob_relative_path.clone();
        let sha256 = installation.sha256.clone();
        let unreferenced = self
            .with_connection(|conn| {
                let transaction = conn.transaction()?;
                append_event(
                    &transaction,
                    installation_id,
                    LocalModelEventKind::Removed,
                    Some(installation.downloaded_bytes),
                    installation.expected_bytes,
                    None,
                )?;
                let removed = transaction.execute(
                    "DELETE FROM local_model_installations WHERE installation_id = ?1 AND is_active = 0",
                    [installation_id],
                )?;
                if removed != 1 {
                    return Err(StoreError::InvariantViolation {
                        message: "local-model installation became active during removal".to_string(),
                    });
                }
                let remaining: i64 = transaction.query_row(
                    "SELECT COUNT(*) FROM local_model_installations WHERE sha256 = ?1 AND blob_relative_path IS NOT NULL",
                    [&sha256],
                    |row| row.get(0),
                )?;
                transaction.commit()?;
                Ok(remaining == 0)
            })
            .await?;
        Ok(RemovedLocalModelInstallation {
            installation,
            unreferenced_blob_relative_path: unreferenced.then_some(blob_path).flatten(),
        })
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
                SELECT provider_kind, provider_account_id, model_profile,
                       reasoning_effort, updated_at
                FROM default_model_preference
                WHERE preference_id = 'default'
                "#,
                [],
                |row| {
                    Ok(DefaultModelPreferenceRecord {
                        provider_kind: row.get(0)?,
                        provider_account_id: row.get(1)?,
                        model_profile: row.get(2)?,
                        reasoning_effort: row.get(3)?,
                        updated_at: row.get(4)?,
                    })
                },
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
        let provider_kind = provider_kind.trim().to_ascii_lowercase();
        let provider_account_id = provider_account_id.trim();
        let model_profile = model_profile.trim();
        if provider_kind.is_empty() || provider_account_id.is_empty() || model_profile.is_empty() {
            return Err(StoreError::InvariantViolation {
                message: "default model provider, account, and model profile are required"
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

        self.with_connection(|conn| {
            let account_provider: Option<String> = conn
                .query_row(
                    "SELECT provider_kind FROM provider_accounts WHERE provider_account_id = ?1 AND is_active = 1",
                    [provider_account_id],
                    |row| row.get(0),
                )
                .optional()?;
            if account_provider.as_deref() != Some(provider_kind.as_str()) {
                return Err(StoreError::InvariantViolation {
                    message: "default model provider account is unavailable or mismatched"
                        .to_string(),
                });
            }
            if provider_kind == "local_models" {
                let installed: bool = conn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM local_model_installations WHERE model_id = ?1 AND status = 'installed')",
                    [model_profile],
                    |row| row.get(0),
                )?;
                if !installed {
                    return Err(StoreError::InvariantViolation {
                        message: format!("local model is not installed: {model_profile}"),
                    });
                }
            }
            conn.execute(
                r#"
                INSERT INTO default_model_preference (
                  preference_id, provider_kind, provider_account_id,
                  model_profile, reasoning_effort, updated_at
                ) VALUES ('default', ?1, ?2, ?3, ?4, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                ON CONFLICT(preference_id) DO UPDATE SET
                  provider_kind = excluded.provider_kind,
                  provider_account_id = excluded.provider_account_id,
                  model_profile = excluded.model_profile,
                  reasoning_effort = excluded.reasoning_effort,
                  updated_at = excluded.updated_at
                "#,
                params![
                    provider_kind,
                    provider_account_id,
                    model_profile,
                    reasoning_effort
                ],
            )?;
            Ok(())
        })
        .await?;
        self.get_default_model_preference()
            .await?
            .ok_or_else(|| StoreError::InvariantViolation {
                message: "default model preference disappeared after save".to_string(),
            })
    }
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
        return Err(StoreError::InvariantViolation {
            message: "invalid local-model installation metadata".to_string(),
        });
    }
    if input.source_kind == LocalModelSourceKind::Catalog
        && (input.source_repo.is_none()
            || input.source_revision.is_none()
            || input.source_file.is_none()
            || input.sha256.is_none())
    {
        return Err(StoreError::InvariantViolation {
            message: "catalog installation requires pinned source provenance".to_string(),
        });
    }
    Ok(())
}
