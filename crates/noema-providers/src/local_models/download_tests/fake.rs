use std::{collections::BTreeMap, sync::Mutex};

use crate::{
    LocalModelEventKind, LocalModelEventRecord, LocalModelInstallationPersistence,
    LocalModelInstallationRecord, LocalModelInstallationStatus, LocalModelInstallationUpdate,
    NewLocalModelInstallation, ProviderPersistenceError, ProviderPersistenceFuture,
    RemovedLocalModelInstallation,
};

#[derive(Default)]
pub(super) struct FakeInstallationPersistence {
    state: Mutex<FakeState>,
}

#[derive(Default)]
struct FakeState {
    installations: BTreeMap<String, LocalModelInstallationRecord>,
    events: Vec<LocalModelEventRecord>,
}

impl FakeInstallationPersistence {
    pub(super) fn installation(
        &self,
        installation_id: &str,
    ) -> Option<LocalModelInstallationRecord> {
        self.state
            .lock()
            .expect("fake persistence lock")
            .installations
            .get(installation_id)
            .cloned()
    }

    pub(super) fn events(&self) -> Vec<LocalModelEventRecord> {
        self.state
            .lock()
            .expect("fake persistence lock")
            .events
            .clone()
    }
}

impl LocalModelInstallationPersistence for FakeInstallationPersistence {
    fn upsert_local_model_installation(
        &self,
        input: NewLocalModelInstallation,
    ) -> ProviderPersistenceFuture<'_, LocalModelInstallationRecord> {
        Box::pin(async move {
            let mut state = self.state.lock().expect("fake persistence lock");
            if let Some(installed) = state
                .installations
                .get(&input.installation_id)
                .filter(|record| record.status == LocalModelInstallationStatus::Installed)
                .cloned()
            {
                return Ok(installed);
            }
            let record = queued_record(input);
            state
                .installations
                .insert(record.installation_id.clone(), record.clone());
            append_event(&mut state, &record, LocalModelEventKind::Queued, None);
            Ok(record)
        })
    }

    fn local_model_installation<'a>(
        &'a self,
        installation_id: &'a str,
    ) -> ProviderPersistenceFuture<'a, Option<LocalModelInstallationRecord>> {
        Box::pin(async move { Ok(self.installation(installation_id)) })
    }

    fn installed_local_model<'a>(
        &'a self,
        model_id: &'a str,
    ) -> ProviderPersistenceFuture<'a, Option<LocalModelInstallationRecord>> {
        Box::pin(async move {
            Ok(self
                .state
                .lock()
                .expect("fake persistence lock")
                .installations
                .values()
                .find(|record| {
                    record.model_id == model_id
                        && record.status == LocalModelInstallationStatus::Installed
                })
                .cloned())
        })
    }

    fn local_model_installations(
        &self,
    ) -> ProviderPersistenceFuture<'_, Vec<LocalModelInstallationRecord>> {
        Box::pin(async move {
            Ok(self
                .state
                .lock()
                .expect("fake persistence lock")
                .installations
                .values()
                .cloned()
                .collect())
        })
    }

    fn update_local_model_installation<'a>(
        &'a self,
        installation_id: &'a str,
        update: LocalModelInstallationUpdate,
    ) -> ProviderPersistenceFuture<'a, LocalModelInstallationRecord> {
        Box::pin(async move {
            let mut state = self.state.lock().expect("fake persistence lock");
            let current = state
                .installations
                .get(installation_id)
                .cloned()
                .ok_or_else(|| ProviderPersistenceError::InstallationNotFound {
                    installation_id: installation_id.to_string(),
                })?;
            if !current.status.can_transition_to(update.status) {
                return Err(ProviderPersistenceError::InvalidInstallationTransition {
                    from: current.status.to_string(),
                    to: update.status.to_string(),
                });
            }
            if update
                .expected_bytes
                .is_some_and(|expected| update.downloaded_bytes > expected)
            {
                return Err(ProviderPersistenceError::InvalidRequest {
                    kind: "downloaded_bytes_exceed_expected_bytes",
                });
            }
            if update.status == LocalModelInstallationStatus::Installed
                && update.blob_relative_path.is_none()
            {
                return Err(ProviderPersistenceError::InvalidRequest {
                    kind: "installed_model_requires_blob_path",
                });
            }

            let mut record = current;
            record.status = update.status;
            record.downloaded_bytes = update.downloaded_bytes;
            record.expected_bytes = update.expected_bytes.or(record.expected_bytes);
            record.sha256 = update.sha256.or(record.sha256);
            record.blob_relative_path = update.blob_relative_path.or(record.blob_relative_path);
            record.error_code = update.error_code;
            record.error_message = update.error_message;
            if record.status == LocalModelInstallationStatus::Installed {
                record.installed_at = Some(timestamp().to_string());
            }
            record.updated_at = timestamp().to_string();
            state
                .installations
                .insert(installation_id.to_string(), record.clone());
            append_event(
                &mut state,
                &record,
                event_kind_for_status(record.status),
                record.error_message.clone(),
            );
            Ok(record)
        })
    }

    fn cancel_local_model_installation<'a>(
        &'a self,
        installation_id: &'a str,
    ) -> ProviderPersistenceFuture<'a, LocalModelInstallationRecord> {
        Box::pin(async move {
            let mut state = self.state.lock().expect("fake persistence lock");
            let record = state
                .installations
                .get_mut(installation_id)
                .ok_or_else(|| ProviderPersistenceError::InstallationNotFound {
                    installation_id: installation_id.to_string(),
                })?;
            if !record
                .status
                .can_transition_to(LocalModelInstallationStatus::Cancelled)
            {
                return Err(ProviderPersistenceError::InvalidInstallationTransition {
                    from: record.status.to_string(),
                    to: LocalModelInstallationStatus::Cancelled.to_string(),
                });
            }
            record.status = LocalModelInstallationStatus::Cancelled;
            let record = record.clone();
            append_event(&mut state, &record, LocalModelEventKind::Cancelled, None);
            Ok(record)
        })
    }

    fn remove_terminal_local_model_installation<'a>(
        &'a self,
        installation_id: &'a str,
    ) -> ProviderPersistenceFuture<'a, RemovedLocalModelInstallation> {
        Box::pin(async move {
            let mut state = self.state.lock().expect("fake persistence lock");
            let record = state
                .installations
                .get(installation_id)
                .cloned()
                .ok_or_else(|| ProviderPersistenceError::InstallationNotFound {
                    installation_id: installation_id.to_string(),
                })?;
            if record.is_active
                || !matches!(
                    record.status,
                    LocalModelInstallationStatus::Cancelled | LocalModelInstallationStatus::Failed
                )
                || record.retirement_claimed_at.is_some()
            {
                return Err(ProviderPersistenceError::Conflict {
                    operation: "remove_terminal_local_model_installation",
                });
            }
            state.installations.remove(installation_id);
            append_event(&mut state, &record, LocalModelEventKind::Removed, None);
            Ok(RemovedLocalModelInstallation {
                installation: record,
            })
        })
    }

    fn local_model_events(
        &self,
        after_cursor: Option<u64>,
        limit: u32,
    ) -> ProviderPersistenceFuture<'_, Vec<LocalModelEventRecord>> {
        Box::pin(async move {
            let after_cursor = after_cursor.unwrap_or_default();
            Ok(self
                .events()
                .into_iter()
                .filter(|event| event.cursor > after_cursor)
                .take(limit.clamp(1, 1_000) as usize)
                .collect())
        })
    }
}

fn queued_record(input: NewLocalModelInstallation) -> LocalModelInstallationRecord {
    let provider_instance_key = crate::local_model_provider_instance_key(
        crate::LOCAL_MODELS_PROVIDER_ACCOUNT_ID,
        &input.installation_id,
        &input.model_id,
    )
    .expect("fake exact key");
    LocalModelInstallationRecord {
        installation_id: input.installation_id,
        provider_instance_key,
        model_id: input.model_id,
        display_name: input.display_name,
        source_kind: input.source_kind,
        source_repo: input.source_repo,
        source_revision: input.source_revision,
        source_file: input.source_file,
        sha256: input.sha256,
        download_gb: input.download_gb,
        expected_bytes: input.expected_bytes,
        downloaded_bytes: 0,
        license: input.license,
        backend: input.backend,
        status: LocalModelInstallationStatus::Queued,
        blob_relative_path: None,
        is_active: false,
        runtime_retired_at: None,
        retirement_claimed_at: None,
        error_code: None,
        error_message: None,
        installed_at: None,
        created_at: timestamp().to_string(),
        updated_at: timestamp().to_string(),
    }
}

fn append_event(
    state: &mut FakeState,
    record: &LocalModelInstallationRecord,
    kind: LocalModelEventKind,
    message: Option<String>,
) {
    state.events.push(LocalModelEventRecord {
        cursor: state.events.len() as u64 + 1,
        installation_id: record.installation_id.clone(),
        kind,
        downloaded_bytes: Some(record.downloaded_bytes),
        expected_bytes: record.expected_bytes,
        message,
        created_at: timestamp().to_string(),
    });
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

const fn timestamp() -> &'static str {
    "2026-07-16T00:00:00.000Z"
}
