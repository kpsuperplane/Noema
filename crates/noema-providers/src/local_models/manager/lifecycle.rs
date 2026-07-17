//! Exact-instance reconstruction, activation, retirement, restart, and shutdown.

use std::{collections::HashMap, sync::Arc, sync::atomic::Ordering, time::Duration};

use noema_home::SystemErrorEvent;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

use crate::{
    LOCAL_MODELS_PROVIDER_ACCOUNT_ID, LocalModelInstallationRecord, LocalModelInstallationStatus,
    ProviderInstanceKey, ProviderPersistenceError, ProviderReadySelectionError,
    ProviderRetirementGuard, ProviderSelectionSnapshot, local_model_provider_instance_key,
};

use super::{
    DegradedLocalModelInstance, LIFECYCLE_RUNNING, LIFECYCLE_SHUTTING_DOWN, LIFECYCLE_STOPPED,
    LocalModelManagerError, LocalModelManagerService, LocalModelReconstructionReport,
    LocalModelRuntimeStatus, ManagedInstance,
};

const RETIREMENT_POLL_INTERVAL: Duration = Duration::from_millis(10);

impl LocalModelManagerService {
    /// Reconstruct every exact local instance referenced by durable future work.
    ///
    /// Structural corruption is validated before any process starts. Runtime
    /// launch failures are retained in the degraded report while reconstruction
    /// continues for every other valid instance.
    ///
    /// # Errors
    ///
    /// Returns a typed structural or persistence error when durable state cannot
    /// be interpreted safely.
    pub async fn reconstruct_persisted_instances(
        &self,
    ) -> Result<LocalModelReconstructionReport, LocalModelManagerError> {
        self.ensure_accepting_work()?;
        let manager = self.clone();
        tokio::spawn(async move { manager.reconstruct_persisted_instances_owned().await })
            .await
            .map_err(|_| LocalModelManagerError::Runtime {
                operation: "join_local_model_reconstruction",
                message: "local-model reconstruction task did not complete".to_string(),
            })?
    }

    async fn reconstruct_persisted_instances_owned(
        &self,
    ) -> Result<LocalModelReconstructionReport, LocalModelManagerError> {
        let _control = self.inner.control.lock().await;
        self.ensure_accepting_work()?;
        let snapshot = self
            .inner
            .lifecycle_persistence
            .local_model_reconstruction_snapshot()
            .await?;
        let required = validate_reconstruction_snapshot(&snapshot)?;

        for installation in &snapshot.installations {
            if installation.retirement_claimed_at.is_some() {
                self.inner
                    .registry
                    .block_for_retirement(installation.provider_instance_key.clone())?;
            }
        }
        if let Err(error) = self.reap_once_locked().await {
            self.record_reaper_failure(&error);
        }

        let mut report = LocalModelReconstructionReport::default();
        let mut active_process = None;
        let mut active_degraded = false;
        for installation in required {
            let key = installation.provider_instance_key.clone();
            match self.prepare_instance(installation.clone()).await {
                Ok(prepared) => {
                    self.inner
                        .degraded
                        .lock()
                        .expect("degraded lock")
                        .remove(&key);
                    if installation.is_active {
                        active_process = Some(Arc::clone(&prepared.instance.process));
                    }
                    report.ready.push(key);
                }
                Err(error @ LocalModelManagerError::Runtime { .. }) => {
                    active_degraded |= installation.is_active;
                    self.inner.registry.mark_unready(key.clone());
                    let degraded = DegradedLocalModelInstance {
                        key: key.clone(),
                        installation_id: installation.installation_id,
                        message: error.to_string(),
                    };
                    self.inner
                        .degraded
                        .lock()
                        .expect("degraded lock")
                        .insert(key, degraded.clone());
                    report.degraded.push(degraded);
                }
                Err(error) => return Err(error),
            }
        }
        report
            .ready
            .sort_by(|left, right| left.as_str().cmp(right.as_str()));
        report
            .degraded
            .sort_by(|left, right| left.key.as_str().cmp(right.key.as_str()));

        if let Some(process) = active_process {
            self.publish_active_status(process).await;
        } else if active_degraded {
            self.publish_start_failure();
        } else {
            self.inner
                .runtime_status_tx
                .send_replace(LocalModelRuntimeStatus::Stopped);
        }
        self.start_reaper_worker().await;
        Ok(report)
    }

    /// Retry the single active installation after a transient launch failure.
    ///
    /// # Errors
    ///
    /// Returns structural persistence errors and malformed installation state.
    pub async fn retry_active_installation(
        &self,
    ) -> Result<LocalModelRuntimeStatus, LocalModelManagerError> {
        self.ensure_accepting_work()?;
        let manager = self.clone();
        tokio::spawn(async move { manager.retry_active_installation_owned().await })
            .await
            .map_err(|_| LocalModelManagerError::Runtime {
                operation: "join_local_model_retry",
                message: "local-model retry task did not complete".to_string(),
            })?
    }

    async fn retry_active_installation_owned(
        &self,
    ) -> Result<LocalModelRuntimeStatus, LocalModelManagerError> {
        let _control = self.inner.control.lock().await;
        self.ensure_accepting_work()?;
        let installations = self.inner.installations.local_model_installations().await?;
        let mut active = installations
            .into_iter()
            .filter(|installation| installation.is_active);
        let Some(installation) = active.next() else {
            self.inner
                .runtime_status_tx
                .send_replace(LocalModelRuntimeStatus::Stopped);
            return Ok(LocalModelRuntimeStatus::Stopped);
        };
        if active.next().is_some() {
            return Err(ProviderPersistenceError::Invariant {
                operation: "retry_active_local_model",
            }
            .into());
        }
        ensure_unclaimed(&installation)?;
        validate_runtime_installation(&installation)?;
        let key = instance_key(&installation)?;
        let existing = self
            .inner
            .instances
            .lock()
            .expect("instances lock")
            .get(&key)
            .cloned();
        if let Some(existing) = &existing {
            let status = existing.process.status();
            if matches!(
                status,
                LocalModelRuntimeStatus::Ready { .. }
                    | LocalModelRuntimeStatus::Starting { .. }
                    | LocalModelRuntimeStatus::Retrying { .. }
            ) && self.inner.registry.lease(&key).is_ok()
            {
                self.publish_active_status(Arc::clone(&existing.process))
                    .await;
                return Ok(status);
            }
            self.stop_active_status_forwarder().await;
            self.retire_stop_and_untrack(&key, existing).await?;
        }

        match self.prepare_instance(installation).await {
            Ok(prepared) => {
                self.inner
                    .degraded
                    .lock()
                    .expect("degraded lock")
                    .remove(&key);
                self.publish_active_status(Arc::clone(&prepared.instance.process))
                    .await;
                Ok(self.runtime_status())
            }
            Err(LocalModelManagerError::Runtime { .. }) => {
                self.publish_start_failure();
                Ok(self.runtime_status())
            }
            Err(error) => Err(error),
        }
    }

    /// Start and register one installed artifact before atomically publishing
    /// every canonical selection to its exact key.
    ///
    /// # Errors
    ///
    /// Returns process, registry, validation, or activation persistence errors.
    pub async fn activate(
        &self,
        installation_id: &str,
    ) -> Result<LocalModelInstallationRecord, LocalModelManagerError> {
        self.ensure_accepting_work()?;
        let manager = self.clone();
        let installation_id = installation_id.to_string();
        tokio::spawn(async move { manager.activate_owned(&installation_id).await })
            .await
            .map_err(|_| LocalModelManagerError::Runtime {
                operation: "join_local_model_activation",
                message: "local-model activation task did not complete".to_string(),
            })?
    }

    async fn activate_owned(
        &self,
        installation_id: &str,
    ) -> Result<LocalModelInstallationRecord, LocalModelManagerError> {
        let _control = self.inner.control.lock().await;
        self.ensure_accepting_work()?;
        let installation = self.required_installation(installation_id).await?;
        ensure_unclaimed(&installation)?;
        validate_runtime_installation(&installation)?;
        let key = instance_key(&installation)?;

        let existing = self
            .inner
            .instances
            .lock()
            .expect("instances lock")
            .get(&key)
            .cloned();
        let reusable = existing.as_ref().is_some_and(|instance| {
            !matches!(
                instance.process.status(),
                LocalModelRuntimeStatus::Failed { .. } | LocalModelRuntimeStatus::Stopped
            ) && self.inner.registry.lease(&key).is_ok()
        });
        let (instance, is_new) = if reusable {
            (existing.expect("reusable instance exists"), false)
        } else {
            if let Some(existing) = existing {
                self.retire_stop_and_untrack(&key, &existing).await?;
            }
            let prepared = self.prepare_instance(installation.clone()).await?;
            (prepared.instance, true)
        };
        if self.inner.lifecycle.load(Ordering::Acquire) != LIFECYCLE_RUNNING {
            if is_new {
                self.retire_stop_and_untrack(&key, &instance).await?;
            }
            return Err(LocalModelManagerError::ShuttingDown);
        }

        let mut selection = ProviderSelectionSnapshot::explicit(
            "local_models",
            LOCAL_MODELS_PROVIDER_ACCOUNT_ID,
            installation.model_id.clone(),
            None,
            Some("local_model_activation".to_string()),
        );
        selection.provider_instance_key = Some(key.clone());
        let ready_selection = match self.inner.registry.prove_ready_selection(selection) {
            Ok(ready_selection) => ready_selection,
            Err(error) => {
                if is_new {
                    self.retire_stop_and_untrack(&key, &instance).await?;
                }
                return Err(match error {
                    ProviderReadySelectionError::Registry(error) => error.into(),
                    ProviderReadySelectionError::InvalidSelection(_) => {
                        ProviderPersistenceError::Invariant {
                            operation: "prove_local_model_activation_ready",
                        }
                        .into()
                    }
                });
            }
        };
        let activation_result = self
            .inner
            .activation
            .activate_local_model_as_system_default(installation_id, &ready_selection)
            .await;
        drop(ready_selection);
        let activated = match activation_result {
            Ok(preference) => preference,
            Err(error) => {
                if is_new {
                    self.retire_stop_and_untrack(&key, &instance).await?;
                }
                return Err(error.into());
            }
        };
        debug_assert_eq!(activated.provider_instance_key, key);
        debug_assert!(activated.is_active);
        {
            let mut instances = self.inner.instances.lock().expect("instances lock");
            for managed in instances.values_mut() {
                managed.installation.is_active = managed.installation.provider_instance_key == key;
            }
            if let Some(managed) = instances.get_mut(&key) {
                managed.installation = activated.clone();
            }
        }
        self.inner
            .degraded
            .lock()
            .expect("degraded lock")
            .remove(&key);
        self.publish_active_status(Arc::clone(&instance.process))
            .await;
        self.trigger_reaper().await;
        Ok(activated)
    }

    /// Atomically claim an inactive exact instance before retiring its runtime
    /// and deleting artifacts and persistence idempotently.
    ///
    /// # Errors
    ///
    /// Returns a typed reference conflict when durable future work still pins
    /// the instance.
    pub async fn remove(
        &self,
        installation_id: &str,
    ) -> Result<crate::RemovedLocalModelInstallation, LocalModelManagerError> {
        self.ensure_accepting_work()?;
        let manager = self.clone();
        let installation_id = installation_id.to_string();
        tokio::spawn(async move { manager.remove_owned(&installation_id).await })
            .await
            .map_err(|_| LocalModelManagerError::Runtime {
                operation: "join_local_model_removal",
                message: "local-model removal task did not complete".to_string(),
            })?
    }

    async fn remove_owned(
        &self,
        installation_id: &str,
    ) -> Result<crate::RemovedLocalModelInstallation, LocalModelManagerError> {
        loop {
            self.ensure_accepting_work()?;
            let mut completion = self.cancel_owned_worker(installation_id).await;
            if let Some(completion) = completion.as_mut() {
                if !*completion.borrow() {
                    let _ = completion.changed().await;
                }
                continue;
            }

            let control = self.inner.control.lock().await;
            self.ensure_accepting_work()?;
            let mut completion = self.cancel_owned_worker(installation_id).await;
            if let Some(completion) = completion.as_mut() {
                drop(control);
                if !*completion.borrow() {
                    let _ = completion.changed().await;
                }
                continue;
            }

            return self.remove_without_worker_locked(installation_id).await;
        }
    }

    async fn cancel_owned_worker(
        &self,
        installation_id: &str,
    ) -> Option<tokio::sync::watch::Receiver<bool>> {
        let mut workers = self.inner.workers.lock().await;
        workers.retain(|_, worker| !*worker.completion.borrow());
        workers.get(installation_id).map(|worker| {
            worker.cancellation.cancel();
            worker.completion.clone()
        })
    }

    async fn remove_without_worker_locked(
        &self,
        installation_id: &str,
    ) -> Result<crate::RemovedLocalModelInstallation, LocalModelManagerError> {
        let mut installation = self.required_installation(installation_id).await?;
        if installation.is_active {
            return Err(LocalModelManagerError::ActiveInstallation {
                installation_id: installation_id.to_string(),
            });
        }
        if installation.retirement_claimed_at.is_some()
            || installation.status == LocalModelInstallationStatus::Installed
        {
            let key = instance_key(&installation)?;
            return self.claim_and_remove_locked(&key).await;
        }
        if !matches!(
            installation.status,
            LocalModelInstallationStatus::Cancelled | LocalModelInstallationStatus::Failed
        ) {
            installation = self
                .inner
                .installations
                .cancel_local_model_installation(installation_id)
                .await?;
        }
        let removed = self
            .inner
            .installations
            .remove_terminal_local_model_installation(&installation.installation_id)
            .await?;
        if let Err(error) = self.remove_removed_artifacts(&removed).await {
            self.record_reaper_failure(&error);
        }
        Ok(removed)
    }

    /// Reject new work, stop periodic scheduling, and drain manager-owned workers.
    pub async fn begin_shutdown(&self) {
        let previous = self
            .inner
            .lifecycle
            .swap(LIFECYCLE_SHUTTING_DOWN, Ordering::AcqRel);
        if previous == LIFECYCLE_STOPPED {
            self.inner
                .lifecycle
                .store(LIFECYCLE_STOPPED, Ordering::Release);
            self.inner.lifecycle_tx.send_replace(LIFECYCLE_STOPPED);
            return;
        }
        self.inner
            .lifecycle_tx
            .send_replace(LIFECYCLE_SHUTTING_DOWN);
        for worker in self.inner.workers.lock().await.values() {
            worker.cancellation.cancel();
        }
        self.stop_reaper_worker().await;
        {
            let _control = self.inner.control.lock().await;
            for worker in self.inner.workers.lock().await.values() {
                worker.cancellation.cancel();
            }
        }
        self.drain_workers().await;
        let _control = self.inner.control.lock().await;
        self.stop_active_status_forwarder().await;
    }

    /// Drain exact leases and stop every retained process.
    ///
    /// # Errors
    ///
    /// Returns the first process shutdown error after attempting every process.
    pub async fn shutdown(&self) -> Result<(), LocalModelManagerError> {
        self.begin_shutdown().await;
        let _control = self.inner.control.lock().await;
        let instances = self
            .inner
            .instances
            .lock()
            .expect("instances lock")
            .iter()
            .map(|(key, instance)| (key.clone(), instance.clone()))
            .collect::<Vec<_>>();
        let mut retirements = Vec::with_capacity(instances.len());
        for (_, instance) in &instances {
            retirements.push(
                self.inner
                    .registry
                    .begin_retirement(&instance.registration)?,
            );
        }
        for retirement in &retirements {
            wait_until_drained(retirement).await;
        }

        let mut first_error = None;
        for (key, instance) in instances {
            match instance.process.shutdown().await {
                Ok(()) => {
                    self.inner
                        .instances
                        .lock()
                        .expect("instances lock")
                        .remove(&key);
                }
                Err(error) if first_error.is_none() => first_error = Some(error),
                Err(_) => {}
            }
        }
        if let Some(error) = first_error {
            return Err(error);
        }
        self.inner
            .runtime_status_tx
            .send_replace(LocalModelRuntimeStatus::Stopped);
        self.inner
            .lifecycle
            .store(LIFECYCLE_STOPPED, Ordering::Release);
        self.inner.lifecycle_tx.send_replace(LIFECYCLE_STOPPED);
        Ok(())
    }

    async fn prepare_instance(
        &self,
        installation: LocalModelInstallationRecord,
    ) -> Result<PreparedInstance, LocalModelManagerError> {
        ensure_unclaimed(&installation)?;
        validate_runtime_installation(&installation)?;
        let key = instance_key(&installation)?;
        let process = match self.inner.process_factory.start(installation.clone()).await {
            Ok(process) => process,
            Err(error) => {
                self.inner.registry.mark_unready(key.clone());
                self.record_start_failure(&error);
                return Err(error);
            }
        };
        let registration = match self
            .inner
            .registry
            .register(key.clone(), process.provider())
        {
            Ok(registration) => registration,
            Err(error) => {
                let _ = process.shutdown().await;
                return Err(error.into());
            }
        };
        let instance = ManagedInstance {
            installation,
            registration,
            process,
        };
        let replaced = self
            .inner
            .instances
            .lock()
            .expect("instances lock")
            .insert(key.clone(), instance.clone());
        debug_assert!(
            replaced.is_none(),
            "prepared key must not replace an instance"
        );
        Ok(PreparedInstance { key, instance })
    }

    pub(super) async fn retire_stop_and_untrack(
        &self,
        key: &ProviderInstanceKey,
        instance: &ManagedInstance,
    ) -> Result<(), LocalModelManagerError> {
        let retirement = self
            .inner
            .registry
            .begin_retirement(&instance.registration)?;
        wait_until_drained(&retirement).await;
        instance.process.shutdown().await?;
        self.inner
            .instances
            .lock()
            .expect("instances lock")
            .remove(key);
        Ok(())
    }

    async fn publish_active_status(&self, process: Arc<dyn super::LocalModelProcess>) {
        self.stop_active_status_forwarder().await;
        let cancellation = CancellationToken::new();
        self.inner.runtime_status_tx.send_replace(process.status());
        let status_tx = self.inner.runtime_status_tx.clone();
        let mut statuses = process.subscribe_status();
        let forwarder_cancellation = cancellation.clone();
        let task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    () = forwarder_cancellation.cancelled() => break,
                    changed = statuses.changed() => {
                        if changed.is_err() {
                            break;
                        }
                        status_tx.send_replace(statuses.borrow().clone());
                    }
                }
            }
        });
        self.inner
            .active_status_forwarder
            .lock()
            .await
            .replace(super::ActiveStatusForwarder { cancellation, task });
    }

    async fn stop_active_status_forwarder(&self) {
        let forwarder = self.inner.active_status_forwarder.lock().await.take();
        if let Some(forwarder) = forwarder {
            forwarder.cancellation.cancel();
            let _ = forwarder.task.await;
        }
    }

    async fn required_installation(
        &self,
        installation_id: &str,
    ) -> Result<LocalModelInstallationRecord, LocalModelManagerError> {
        self.inner
            .installations
            .local_model_installation(installation_id)
            .await?
            .ok_or_else(|| {
                ProviderPersistenceError::InstallationNotFound {
                    installation_id: installation_id.to_string(),
                }
                .into()
            })
    }

    fn publish_start_failure(&self) {
        self.inner
            .runtime_status_tx
            .send_replace(LocalModelRuntimeStatus::Failed {
                message: "local model runtime failed to start".to_string(),
            });
    }

    fn record_start_failure(&self, error: &LocalModelManagerError) {
        if let Some(system_errors) = &self.inner.system_errors {
            system_errors.try_append(
                SystemErrorEvent::new(
                    "local_model_runtime_unavailable",
                    "The local model runtime could not start",
                )
                .with_error_chain([error.to_string()]),
            );
        }
    }
}

#[derive(Clone)]
struct PreparedInstance {
    #[allow(dead_code)]
    key: ProviderInstanceKey,
    instance: ManagedInstance,
}

fn validate_reconstruction_snapshot(
    snapshot: &crate::LocalModelReconstructionSnapshot,
) -> Result<Vec<LocalModelInstallationRecord>, LocalModelManagerError> {
    let mut by_key = HashMap::new();
    for installation in &snapshot.installations {
        let key = instance_key(installation)?;
        if by_key.insert(key.clone(), installation).is_some() {
            return Err(LocalModelManagerError::DuplicateInstanceIdentity {
                provider_instance_key: key,
            });
        }
    }
    let mut required = HashMap::new();
    for reference in &snapshot.references {
        let installation = by_key
            .get(&reference.provider_instance_key)
            .ok_or_else(|| LocalModelManagerError::ReferencedInstallationMissing {
                provider_instance_key: reference.provider_instance_key.clone(),
            })?;
        if installation.retirement_claimed_at.is_some() {
            return Err(LocalModelManagerError::ReferencedInstallationClaimed {
                provider_instance_key: reference.provider_instance_key.clone(),
            });
        }
        if installation.runtime_retired_at.is_some() {
            return Err(
                LocalModelManagerError::ReferencedInstallationRuntimeRetired {
                    provider_instance_key: reference.provider_instance_key.clone(),
                },
            );
        }
        validate_runtime_installation(installation)?;
        required.insert(
            reference.provider_instance_key.clone(),
            (*installation).clone(),
        );
    }
    for installation in snapshot
        .installations
        .iter()
        .filter(|installation| installation.is_active)
    {
        ensure_unclaimed(installation)?;
        if installation.runtime_retired_at.is_some() {
            return Err(
                LocalModelManagerError::ReferencedInstallationRuntimeRetired {
                    provider_instance_key: installation.provider_instance_key.clone(),
                },
            );
        }
        validate_runtime_installation(installation)?;
        required.insert(
            installation.provider_instance_key.clone(),
            installation.clone(),
        );
    }
    let mut required = required.into_values().collect::<Vec<_>>();
    required.sort_by(|left, right| {
        left.provider_instance_key
            .as_str()
            .cmp(right.provider_instance_key.as_str())
    });
    Ok(required)
}

fn ensure_unclaimed(
    installation: &LocalModelInstallationRecord,
) -> Result<(), LocalModelManagerError> {
    if installation.retirement_claimed_at.is_some() {
        return Err(LocalModelManagerError::ReferencedInstallationClaimed {
            provider_instance_key: installation.provider_instance_key.clone(),
        });
    }
    Ok(())
}

fn validate_runtime_installation(
    installation: &LocalModelInstallationRecord,
) -> Result<(), LocalModelManagerError> {
    if installation.status != LocalModelInstallationStatus::Installed {
        return Err(LocalModelManagerError::InstallationNotReady {
            installation_id: installation.installation_id.clone(),
            reason: "installation status is not installed",
        });
    }
    if installation.sha256.is_none() || installation.blob_relative_path.is_none() {
        return Err(LocalModelManagerError::InstallationNotReady {
            installation_id: installation.installation_id.clone(),
            reason: "verified artifact metadata is incomplete",
        });
    }
    Ok(())
}

fn instance_key(
    installation: &LocalModelInstallationRecord,
) -> Result<ProviderInstanceKey, LocalModelManagerError> {
    let expected = local_model_provider_instance_key(
        LOCAL_MODELS_PROVIDER_ACCOUNT_ID,
        &installation.installation_id,
        &installation.model_id,
    )
    .map_err(|_| LocalModelManagerError::InstallationIdentityMismatch {
        installation_id: installation.installation_id.clone(),
    })?;
    if expected != installation.provider_instance_key {
        return Err(LocalModelManagerError::InstallationIdentityMismatch {
            installation_id: installation.installation_id.clone(),
        });
    }
    Ok(installation.provider_instance_key.clone())
}

pub(super) async fn wait_until_drained(retirement: &ProviderRetirementGuard) {
    while !retirement.is_drained() {
        sleep(RETIREMENT_POLL_INTERVAL).await;
        retirement.try_finalize();
    }
}
