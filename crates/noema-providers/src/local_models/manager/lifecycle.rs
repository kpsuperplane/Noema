//! Exact-instance activation, retirement, restart, and shutdown.

use std::{sync::Arc, sync::atomic::Ordering, time::Duration};

use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

use noema_home::SystemErrorEvent;

use crate::{
    LOCAL_MODELS_PROVIDER_ACCOUNT_ID, LocalModelInstallationRecord, LocalModelInstallationStatus,
    ProviderInstanceKey, ProviderRetirementGuard, local_model_provider_instance_key,
};

use super::{
    LIFECYCLE_RUNNING, LIFECYCLE_SHUTTING_DOWN, LIFECYCLE_STOPPED, LocalModelManager,
    LocalModelManagerError, LocalModelRuntimeStatus, ManagedInstance,
};
use crate::local_models::routes::ActiveLocalModelRoute;

const RETIREMENT_POLL_INTERVAL: Duration = Duration::from_millis(10);

impl LocalModelManager {
    /// Reconstructs only the Phase 9 active installation.
    ///
    /// A transient process launch failure is published as a failed runtime
    /// status while the manager remains available for retry.
    ///
    /// # Errors
    ///
    /// Returns [`LocalModelManagerError`] for invalid durable state or
    /// repository failures.
    pub async fn start_active_installation(&self) -> Result<(), LocalModelManagerError> {
        self.retry_active_installation().await.map(|_| ())
    }

    /// Retries the durable active installation when its retained process is absent or failed.
    ///
    /// A healthy retained process is preserved. A transient retry failure is
    /// returned as a published [`LocalModelRuntimeStatus::Failed`] value while
    /// structural persistence and installation errors remain typed failures.
    ///
    /// # Errors
    ///
    /// Returns [`LocalModelManagerError`] for invalid durable state,
    /// repository failures, or malformed installed-artifact metadata.
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
            return Err(LocalModelManagerError::Persistence(
                crate::ProviderPersistenceError::Invariant {
                    operation: "retry_active_local_model",
                },
            ));
        }
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
                self.ensure_accepting_work()?;
                let mut publication = self.inner.routes.begin_publication().await;
                publication.publish(ActiveLocalModelRoute {
                    key,
                    model_id: installation.model_id,
                });
                drop(publication);
                self.publish_active_status(Arc::clone(&existing.process))
                    .await;
                return Ok(status);
            }
        }

        if let Some(existing) = &existing {
            let mut publication = self.inner.routes.begin_publication().await;
            publication.clear();
            drop(publication);
            self.stop_active_status_forwarder().await;
            let retirement = self
                .inner
                .registry
                .begin_retirement(&existing.registration)?;
            wait_until_drained(&retirement).await;
            existing.process.shutdown().await?;
            self.inner
                .instances
                .lock()
                .expect("instances lock")
                .remove(&key);
        }

        let prepared = match self.prepare_instance(installation).await {
            Ok(prepared) => prepared,
            Err(LocalModelManagerError::Runtime { .. }) => {
                self.publish_start_failure();
                return Ok(self.runtime_status());
            }
            Err(error) => return Err(error),
        };
        if self.inner.lifecycle.load(Ordering::Acquire) != LIFECYCLE_RUNNING {
            self.retire_stop_and_untrack(&prepared.key, &prepared.instance)
                .await?;
            return Err(LocalModelManagerError::ShuttingDown);
        }
        let process = Arc::clone(&prepared.instance.process);
        let mut publication = self.inner.routes.begin_publication().await;
        publication.publish(ActiveLocalModelRoute {
            key: prepared.key.clone(),
            model_id: prepared.instance.installation.model_id.clone(),
        });
        drop(publication);
        self.publish_active_status(process).await;
        Ok(self.runtime_status())
    }

    /// Starts, health-checks, registers, and activates one installed artifact.
    ///
    /// # Errors
    ///
    /// Returns [`LocalModelManagerError`] when the installation is unavailable,
    /// process startup fails, registration fails, or activation cannot commit.
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
        validate_runtime_installation(&installation)?;
        let key = instance_key(&installation)?;

        let existing = self
            .inner
            .instances
            .lock()
            .expect("instances lock")
            .get(&key)
            .cloned();
        let reusable = existing.as_ref().is_some_and(|existing| {
            !matches!(
                existing.process.status(),
                LocalModelRuntimeStatus::Failed { .. } | LocalModelRuntimeStatus::Stopped
            ) && self.inner.registry.lease(&key).is_ok()
        });
        let (instance, is_new) = if reusable {
            (existing.expect("reusable instance exists"), false)
        } else {
            if let Some(existing) = existing {
                if self.inner.routes.active_key().await.as_ref() == Some(&key) {
                    let mut publication = self.inner.routes.begin_publication().await;
                    publication.clear();
                    drop(publication);
                    self.stop_active_status_forwarder().await;
                }
                let retirement = self
                    .inner
                    .registry
                    .begin_retirement(&existing.registration)?;
                wait_until_drained(&retirement).await;
                existing.process.shutdown().await?;
                self.inner
                    .instances
                    .lock()
                    .expect("instances lock")
                    .remove(&key);
            }
            match self.prepare_instance(installation.clone()).await {
                Ok(prepared) => (prepared.instance, true),
                Err(error) => {
                    if self.inner.routes.active_key().await.is_none() {
                        self.publish_start_failure();
                    }
                    return Err(error);
                }
            }
        };
        if self.inner.lifecycle.load(Ordering::Acquire) != LIFECYCLE_RUNNING {
            if is_new {
                self.retire_stop_and_untrack(&key, &instance).await?;
            }
            return Err(LocalModelManagerError::ShuttingDown);
        }

        let mut publication = self.inner.routes.begin_publication().await;
        if let Err(error) = self
            .inner
            .activation
            .activate_local_model_as_system_default(installation_id)
            .await
        {
            drop(publication);
            if is_new {
                self.retire_stop_and_untrack(&key, &instance).await?;
            }
            return Err(error.into());
        }
        publication.publish(ActiveLocalModelRoute {
            key: key.clone(),
            model_id: installation.model_id.clone(),
        });
        drop(publication);

        self.publish_active_status(Arc::clone(&instance.process))
            .await;
        self.required_installation(installation_id).await
    }

    /// Retires an inactive process before deleting its row and artifact files.
    ///
    /// # Errors
    ///
    /// Returns [`LocalModelManagerError::ActiveInstallation`] for the active
    /// installation. Drain, stop, persistence, and filesystem failures are
    /// returned without deleting persistence before a successful process stop.
    pub async fn remove(
        &self,
        installation_id: &str,
    ) -> Result<crate::RemovedLocalModelInstallation, LocalModelManagerError> {
        self.ensure_accepting_work()?;
        let _control = self.inner.control.lock().await;
        self.ensure_accepting_work()?;
        let installation = self.required_installation(installation_id).await?;
        let key = instance_key(&installation)?;
        if installation.is_active || self.inner.routes.active_key().await.as_ref() == Some(&key) {
            return Err(LocalModelManagerError::ActiveInstallation {
                installation_id: installation_id.to_string(),
            });
        }

        let instance = self
            .inner
            .instances
            .lock()
            .expect("instances lock")
            .get(&key)
            .cloned();
        if let Some(instance) = &instance {
            let retirement = self
                .inner
                .registry
                .begin_retirement(&instance.registration)?;
            wait_until_drained(&retirement).await;
            instance.process.shutdown().await?;
        }
        let removed = self.inner.installer.remove(installation_id).await?;
        if instance.is_some() {
            self.inner
                .instances
                .lock()
                .expect("instances lock")
                .remove(&key);
        }
        Ok(removed)
    }

    /// Rejects new work and cancels every active installation/import worker.
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
        {
            let _control = self.inner.control.lock().await;
            for worker in self.inner.workers.lock().await.values() {
                worker.cancellation.cancel();
            }
        }
        self.drain_workers().await;
        let _control = self.inner.control.lock().await;
        let mut publication = self.inner.routes.begin_publication().await;
        publication.clear();
        drop(publication);
        self.stop_active_status_forwarder().await;
    }

    /// Drains installation workers and leases, then stops every managed process.
    ///
    /// # Errors
    ///
    /// Returns [`LocalModelManagerError`] when any process fails to stop. Every
    /// other process is still given a stop attempt, and a later call may retry.
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
        validate_runtime_installation(&installation)?;
        let key = instance_key(&installation)?;
        let process = match self.inner.process_factory.start(installation.clone()).await {
            Ok(process) => process,
            Err(error) => {
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

    async fn retire_stop_and_untrack(
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
                LocalModelManagerError::Persistence(
                    crate::ProviderPersistenceError::InstallationNotFound {
                        installation_id: installation_id.to_string(),
                    },
                )
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
    key: ProviderInstanceKey,
    instance: ManagedInstance,
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
    local_model_provider_instance_key(
        LOCAL_MODELS_PROVIDER_ACCOUNT_ID,
        &installation.installation_id,
        &installation.model_id,
    )
    .map_err(|_| LocalModelManagerError::InstallationNotReady {
        installation_id: installation.installation_id.clone(),
        reason: "provider instance identity is invalid",
    })
}

async fn wait_until_drained(retirement: &ProviderRetirementGuard) {
    while !retirement.is_drained() {
        sleep(RETIREMENT_POLL_INTERVAL).await;
        retirement.try_finalize();
    }
}
