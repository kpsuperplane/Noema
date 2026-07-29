//! Idempotent installation/import worker ownership and cancellation.

use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use crate::{
    HuggingFaceLocalModelImport, LocalFileModelImport, LocalModelBackend,
    LocalModelCatalogSnapshot, LocalModelCatalogSnapshotEntry, LocalModelInstallationRecord,
    LocalModelInstallationStatus,
};

use super::{InstallationWorker, LocalModelManagerError, LocalModelManagerService};
use crate::local_models::{LocalModelCatalog, detect_local_hardware_profiles};

impl LocalModelManagerService {
    /// Returns the platform-preferred backend for arbitrary imported GGUFs.
    ///
    /// Unlike catalog selection, this does not assume that an imported file
    /// has any curated build compatibility metadata.
    ///
    /// # Errors
    ///
    /// Returns [`LocalModelManagerError::Discovery`] when hardware detection fails.
    pub fn preferred_import_backend() -> Result<LocalModelBackend, LocalModelManagerError> {
        let hardware = detect_hardware()?;
        Ok(hardware
            .first()
            .map_or(LocalModelBackend::Cpu, |profile| profile.backend))
    }

    /// Returns the bundled catalog and the machine profiles used for selection.
    ///
    /// # Errors
    ///
    /// Returns [`LocalModelManagerError::Discovery`] when bundled catalog
    /// validation or hardware detection fails.
    pub fn catalog_snapshot() -> Result<LocalModelCatalogSnapshot, LocalModelManagerError> {
        let (catalog, hardware) = load_catalog_and_hardware()?;
        let recommended_model_id = catalog
            .recommend_with_fallback(&hardware)
            .map(|recommendation| recommendation.model.id.as_str());
        Ok(LocalModelCatalogSnapshot {
            entries: catalog
                .models()
                .iter()
                .map(|model| {
                    let selection = catalog.select_build(&model.id, &hardware);
                    LocalModelCatalogSnapshotEntry {
                        model: model.clone(),
                        selected_build: selection.map(|selection| selection.build.clone()),
                        compatible_hardware: selection.map(|selection| selection.hardware),
                        compatibility_explanation: selection
                            .map(|selection| selection.explanation()),
                        is_recommended: recommended_model_id == Some(model.id.as_str()),
                    }
                })
                .collect(),
        })
    }

    /// Queues one compatible catalog artifact and activates it after verification.
    ///
    /// Concurrent duplicate calls return the current projection without
    /// starting a second worker.
    ///
    /// # Errors
    ///
    /// Returns [`LocalModelManagerError`] for discovery, selection, queueing,
    /// or immediate activation failures.
    pub async fn install_catalog_model(
        &self,
        model_id: &str,
        file: Option<&str>,
    ) -> Result<LocalModelInstallationRecord, LocalModelManagerError> {
        self.ensure_accepting_work()?;
        let (catalog, hardware) = load_catalog_and_hardware()?;
        let selection = match file {
            Some(file) => catalog.select_named_build(model_id, file, &hardware),
            None => catalog.select_build(model_id, &hardware),
        }
        .ok_or_else(|| LocalModelManagerError::Discovery {
            operation: "select_local_model_build",
            message: "the requested local model build does not fit this machine".to_string(),
        })?;
        let model = selection.model.clone();
        let build = selection.build.clone();
        let backend = selection.hardware.backend;

        let control = self.inner.control.lock().await;
        self.ensure_accepting_work()?;
        let queued = self
            .inner
            .installer
            .queue_catalog_model(&model, &build, backend)
            .await?;
        if self.ensure_accepting_work().is_err() {
            let _ = self
                .inner
                .installations
                .cancel_local_model_installation(&queued.installation_id)
                .await;
            return Err(LocalModelManagerError::ShuttingDown);
        }
        if queued.status == LocalModelInstallationStatus::Installed {
            drop(control);
            return self
                .prepare_first_installation_for_setup(&queued.installation_id)
                .await;
        }
        if self.worker_is_running(&queued.installation_id).await {
            return Ok(queued);
        }

        let cancellation = CancellationToken::new();
        let manager = self.clone();
        let worker_cancellation = cancellation.clone();
        let installation_id = queued.installation_id.clone();
        let operation_id = installation_id.clone();
        self.spawn_worker(installation_id, cancellation, async move {
            let installed = {
                let _mutation_gate = manager.inner.control.lock().await;
                if manager
                    .inner
                    .lifecycle
                    .load(std::sync::atomic::Ordering::Acquire)
                    != super::LIFECYCLE_RUNNING
                {
                    worker_cancellation.cancel();
                }
                manager
                    .inner
                    .installer
                    .install_catalog_model(&model, &build, backend, worker_cancellation)
                    .await
            };
            if installed.is_ok() {
                let _ = manager
                    .prepare_first_installation_for_setup(&operation_id)
                    .await;
            }
        })
        .await;
        Ok(queued)
    }

    async fn prepare_first_installation_for_setup(
        &self,
        installation_id: &str,
    ) -> Result<LocalModelInstallationRecord, LocalModelManagerError> {
        if self
            .installations()
            .await?
            .iter()
            .any(|installation| installation.is_active)
        {
            return self.required_installation(installation_id).await;
        }
        self.prepare_for_setup(installation_id).await
    }

    /// Queues a pinned public GGUF import and starts one owned worker.
    ///
    /// # Errors
    ///
    /// Returns [`LocalModelManagerError`] when validation or queue persistence fails.
    pub async fn import_hugging_face(
        &self,
        input: HuggingFaceLocalModelImport,
    ) -> Result<LocalModelInstallationRecord, LocalModelManagerError> {
        self.ensure_accepting_work()?;
        let control = self.inner.control.lock().await;
        self.ensure_accepting_work()?;
        let queued = self.inner.installer.queue_hugging_face(&input).await?;
        if self.ensure_accepting_work().is_err() {
            let _ = self
                .inner
                .installations
                .cancel_local_model_installation(&queued.installation_id)
                .await;
            return Err(LocalModelManagerError::ShuttingDown);
        }
        if queued.status == LocalModelInstallationStatus::Installed
            || self.worker_is_running(&queued.installation_id).await
        {
            return Ok(queued);
        }

        let cancellation = CancellationToken::new();
        let worker_cancellation = cancellation.clone();
        let manager = self.clone();
        self.spawn_worker(queued.installation_id.clone(), cancellation, async move {
            let _mutation_gate = manager.inner.control.lock().await;
            if manager
                .inner
                .lifecycle
                .load(std::sync::atomic::Ordering::Acquire)
                != super::LIFECYCLE_RUNNING
            {
                worker_cancellation.cancel();
            }
            let _ = manager
                .inner
                .installer
                .import_hugging_face(input, worker_cancellation)
                .await;
        })
        .await;
        drop(control);
        Ok(queued)
    }

    /// Queues a local GGUF copy and starts one owned worker.
    ///
    /// # Errors
    ///
    /// Returns [`LocalModelManagerError`] when validation or queue persistence fails.
    pub async fn import_local_file(
        &self,
        input: LocalFileModelImport,
    ) -> Result<LocalModelInstallationRecord, LocalModelManagerError> {
        self.ensure_accepting_work()?;
        let control = self.inner.control.lock().await;
        self.ensure_accepting_work()?;
        let queued = self.inner.installer.queue_local_file(&input).await?;
        if self.ensure_accepting_work().is_err() {
            let _ = self
                .inner
                .installations
                .cancel_local_model_installation(&queued.installation_id)
                .await;
            return Err(LocalModelManagerError::ShuttingDown);
        }
        if queued.status == LocalModelInstallationStatus::Installed
            || self.worker_is_running(&queued.installation_id).await
        {
            return Ok(queued);
        }

        let cancellation = CancellationToken::new();
        let worker_cancellation = cancellation.clone();
        let manager = self.clone();
        self.spawn_worker(queued.installation_id.clone(), cancellation, async move {
            let _mutation_gate = manager.inner.control.lock().await;
            if manager
                .inner
                .lifecycle
                .load(std::sync::atomic::Ordering::Acquire)
                != super::LIFECYCLE_RUNNING
            {
                worker_cancellation.cancel();
            }
            let _ = manager
                .inner
                .installer
                .import_local_file(input, worker_cancellation)
                .await;
        })
        .await;
        drop(control);
        Ok(queued)
    }

    /// Cancels an owned worker and durably marks its installation cancelled.
    ///
    /// # Errors
    ///
    /// Returns [`LocalModelManagerError::Persistence`] when durable cancellation fails.
    pub async fn cancel_installation(
        &self,
        installation_id: &str,
    ) -> Result<LocalModelInstallationRecord, LocalModelManagerError> {
        self.ensure_accepting_work()?;
        if let Some(worker) = self.inner.workers.lock().await.get(installation_id) {
            worker.cancellation.cancel();
        }
        let _control = self.inner.control.lock().await;
        self.ensure_accepting_work()?;
        self.inner
            .installations
            .cancel_local_model_installation(installation_id)
            .await
            .map_err(Into::into)
    }

    async fn worker_is_running(&self, installation_id: &str) -> bool {
        let mut workers = self.inner.workers.lock().await;
        workers.retain(|_, worker| !*worker.completion.borrow());
        workers.contains_key(installation_id)
    }

    async fn spawn_worker(
        &self,
        installation_id: String,
        cancellation: CancellationToken,
        operation: impl std::future::Future<Output = ()> + Send + 'static,
    ) {
        let (completion_tx, completion) = watch::channel(false);
        self.inner.workers.lock().await.insert(
            installation_id,
            InstallationWorker {
                cancellation,
                completion,
            },
        );
        tokio::spawn(async move {
            operation.await;
            completion_tx.send_replace(true);
        });
    }

    pub(super) async fn drain_workers(&self) {
        let mut completions = self
            .inner
            .workers
            .lock()
            .await
            .values()
            .map(|worker| worker.completion.clone())
            .collect::<Vec<_>>();
        for completion in &mut completions {
            if !*completion.borrow() {
                let _ = completion.changed().await;
            }
        }
        self.inner.workers.lock().await.clear();
    }
}

fn load_catalog_and_hardware()
-> Result<(LocalModelCatalog, Vec<crate::LocalHardwareProfile>), LocalModelManagerError> {
    let catalog =
        LocalModelCatalog::bundled().map_err(|error| LocalModelManagerError::Discovery {
            operation: "load_local_model_catalog",
            message: error.to_string(),
        })?;
    let hardware = detect_hardware()?;
    Ok((catalog, hardware))
}

fn detect_hardware() -> Result<Vec<crate::LocalHardwareProfile>, LocalModelManagerError> {
    detect_local_hardware_profiles().map_err(|error| LocalModelManagerError::Discovery {
        operation: "detect_local_model_hardware",
        message: error.to_string(),
    })
}
