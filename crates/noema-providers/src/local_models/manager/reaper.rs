//! Durable retirement claiming and manager-owned cleanup scheduling.

use std::{collections::HashSet, path::PathBuf, sync::Arc};

use noema_home::SystemErrorEvent;
use tokio_util::sync::CancellationToken;

use crate::{
    ClaimedLocalModelInstallation, LocalModelInstallationStatus, LocalModelRetirementClaimResult,
    LocalModelRuntimeRetirementResult, ProviderInstanceKey, ProviderPersistenceError,
    RemovedLocalModelInstallation,
};

use super::{LocalModelManager, LocalModelManagerError, ReaperWorker};

impl LocalModelManager {
    pub(super) async fn start_reaper_worker(&self) {
        let mut worker = self.inner.reaper.lock().await;
        if worker.is_some() {
            return;
        }
        let cancellation = CancellationToken::new();
        let trigger = Arc::new(tokio::sync::Notify::new());
        let manager = self.clone();
        let task_cancellation = cancellation.clone();
        let task_trigger = Arc::clone(&trigger);
        let task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    biased;
                    () = task_cancellation.cancelled() => break,
                    () = task_trigger.notified() => {}
                    () = manager.inner.reaper_clock.sleep(manager.inner.reaper_interval) => {}
                }
                if let Err(error) = manager.reap_once_owned().await {
                    manager.record_reaper_failure(&error);
                }
            }
        });
        *worker = Some(ReaperWorker {
            cancellation,
            trigger,
            task,
        });
    }

    pub(super) async fn trigger_reaper(&self) {
        if let Some(worker) = self.inner.reaper.lock().await.as_ref() {
            worker.trigger.notify_one();
        }
    }

    pub(super) async fn stop_reaper_worker(&self) {
        let worker = self.inner.reaper.lock().await.take();
        if let Some(worker) = worker {
            worker.cancellation.cancel();
            let _ = worker.task.await;
        }
    }

    async fn reap_once_owned(&self) -> Result<(), LocalModelManagerError> {
        let _control = self.inner.control.lock().await;
        self.reap_once_locked().await
    }

    pub(super) async fn reap_once_locked(&self) -> Result<(), LocalModelManagerError> {
        let snapshot = self
            .inner
            .lifecycle_persistence
            .local_model_reconstruction_snapshot()
            .await?;
        let referenced = snapshot
            .references
            .into_iter()
            .map(|reference| reference.provider_instance_key)
            .collect::<HashSet<_>>();
        let mut candidates = snapshot
            .installations
            .into_iter()
            .filter(|installation| {
                installation.retirement_claimed_at.is_some()
                    || (installation.status == LocalModelInstallationStatus::Installed
                        && !installation.is_active
                        && !referenced.contains(&installation.provider_instance_key))
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            right
                .retirement_claimed_at
                .is_some()
                .cmp(&left.retirement_claimed_at.is_some())
                .then_with(|| left.installation_id.cmp(&right.installation_id))
        });

        let mut first_error = None;
        for installation in candidates.into_iter().take(self.inner.reaper_batch_size) {
            let result = if installation.retirement_claimed_at.is_some() {
                self.reconcile_claimed_installation_locked(&installation.provider_instance_key)
                    .await
            } else {
                self.retire_unreferenced_runtime_locked(&installation).await
            };
            if let Err(error) = result
                && first_error.is_none()
            {
                first_error = Some(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    async fn reconcile_claimed_installation_locked(
        &self,
        key: &ProviderInstanceKey,
    ) -> Result<(), LocalModelManagerError> {
        match self
            .inner
            .lifecycle_persistence
            .claim_unreferenced_instance_for_retirement(key)
            .await?
        {
            LocalModelRetirementClaimResult::Claimed(claim)
            | LocalModelRetirementClaimResult::AlreadyClaimed(claim) => {
                self.retire_and_remove_claimed_locked(claim).await?;
            }
            LocalModelRetirementClaimResult::Referenced { .. }
            | LocalModelRetirementClaimResult::Missing => {}
        }
        Ok(())
    }

    async fn retire_unreferenced_runtime_locked(
        &self,
        installation: &crate::LocalModelInstallationRecord,
    ) -> Result<(), LocalModelManagerError> {
        let key = &installation.provider_instance_key;
        match self
            .inner
            .lifecycle_persistence
            .retire_unreferenced_instance_runtime(key)
            .await?
        {
            LocalModelRuntimeRetirementResult::Retired(_)
            | LocalModelRuntimeRetirementResult::AlreadyRetired(_) => {}
            LocalModelRuntimeRetirementResult::Referenced { .. }
            | LocalModelRuntimeRetirementResult::Missing => return Ok(()),
        }
        let instance = self
            .inner
            .instances
            .lock()
            .expect("instances lock")
            .get(key)
            .cloned();
        let Some(instance) = instance else {
            return Ok(());
        };
        let retirement = self
            .inner
            .registry
            .begin_retirement(&instance.registration)?;
        super::lifecycle::wait_until_drained(&retirement).await;
        instance.process.shutdown().await?;
        self.inner
            .instances
            .lock()
            .expect("instances lock")
            .remove(key);
        Ok(())
    }

    pub(super) async fn claim_and_remove_locked(
        &self,
        key: &ProviderInstanceKey,
    ) -> Result<RemovedLocalModelInstallation, LocalModelManagerError> {
        match self
            .inner
            .lifecycle_persistence
            .claim_unreferenced_instance_for_retirement(key)
            .await?
        {
            LocalModelRetirementClaimResult::Claimed(claim)
            | LocalModelRetirementClaimResult::AlreadyClaimed(claim) => {
                self.retire_and_remove_claimed_locked(claim).await
            }
            LocalModelRetirementClaimResult::Referenced { .. } => {
                Err(ProviderPersistenceError::ProviderInstanceReferenced {
                    provider_instance_key: key.clone(),
                }
                .into())
            }
            LocalModelRetirementClaimResult::Missing => {
                Err(ProviderPersistenceError::InstallationNotFound {
                    installation_id: key.to_string(),
                }
                .into())
            }
        }
    }

    async fn retire_and_remove_claimed_locked(
        &self,
        claim: ClaimedLocalModelInstallation,
    ) -> Result<RemovedLocalModelInstallation, LocalModelManagerError> {
        let key = claim.installation.provider_instance_key.clone();
        let blocked_retirement = self.inner.registry.block_for_retirement(key.clone())?;
        let instance = self
            .inner
            .instances
            .lock()
            .expect("instances lock")
            .get(&key)
            .cloned();
        let retirement = match (blocked_retirement, instance.as_ref()) {
            (Some(retirement), _) => Some(retirement),
            (None, Some(instance)) => Some(
                self.inner
                    .registry
                    .begin_retirement(&instance.registration)?,
            ),
            (None, None) => None,
        };
        if let Some(retirement) = retirement {
            super::lifecycle::wait_until_drained(&retirement).await;
        }
        if let Some(instance) = instance {
            instance.process.shutdown().await?;
            self.inner
                .instances
                .lock()
                .expect("instances lock")
                .remove(&key);
        }

        let removed = self
            .inner
            .lifecycle_persistence
            .complete_claimed_local_model_removal(&key)
            .await
            .map_err(LocalModelManagerError::from)?;
        self.inner.registry.unblock_after_completed_retirement(&key);
        if let Err(error) = self.remove_removed_artifacts(&removed).await {
            self.record_reaper_failure(&error);
        }
        Ok(removed)
    }

    pub(super) async fn remove_removed_artifacts(
        &self,
        removed: &RemovedLocalModelInstallation,
    ) -> Result<(), LocalModelManagerError> {
        let installation = &removed.installation;
        if let Some(sha256) = installation.sha256.as_deref() {
            let path = self
                .inner
                .paths
                .local_model_partial_path(sha256)
                .map_err(|error| LocalModelManagerError::Installation {
                    message: error.to_string(),
                })?;
            remove_file_if_present(path).await?;
        }
        remove_file_if_present(
            self.inner
                .paths
                .local_model_import_partial_path(&installation.installation_id),
        )
        .await?;
        Ok(())
    }

    pub(super) fn record_reaper_failure(&self, error: &LocalModelManagerError) {
        if let Some(system_errors) = &self.inner.system_errors {
            system_errors.try_append(
                SystemErrorEvent::new(
                    "local_model_retirement_cleanup_failed",
                    "A claimed local model could not be cleaned up",
                )
                .with_error_chain([error.to_string()]),
            );
        }
    }
}

async fn remove_file_if_present(path: PathBuf) -> Result<(), LocalModelManagerError> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(LocalModelManagerError::Installation {
            message: error.to_string(),
        }),
    }
}
