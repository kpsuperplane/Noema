use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use tokio::sync::{Notify, watch};

use crate::{
    ClaimedLocalModelInstallation, GenerateRequest, GenerateResponse, GenerateStreamEvent,
    LocalModelActivationPersistence, LocalModelBackend, LocalModelEventKind, LocalModelEventRecord,
    LocalModelInstallationPersistence, LocalModelInstallationRecord, LocalModelInstallationStatus,
    LocalModelInstallationUpdate, LocalModelInstanceReference, LocalModelInstanceReferenceSource,
    LocalModelLifecyclePersistence, LocalModelReconstructionSnapshot,
    LocalModelRetirementClaimResult, LocalModelRuntimeRetirementResult, LocalModelSourceKind,
    NewLocalModelInstallation, ProviderError, ProviderHandle, ProviderInstanceKey,
    ProviderOperationFuture, ProviderOperations, ProviderPersistenceError,
    ProviderPersistenceFuture, ProviderReadySelection, RemovedLocalModelInstallation,
    local_model_provider_instance_key,
};

use super::super::{LocalModelManagerError, LocalModelRuntimeStatus};
use crate::local_models::manager::process::{
    LocalModelProcess, LocalModelProcessFactory, LocalModelProcessFuture,
};

#[derive(Default)]
pub(in crate::local_models) struct FakeRepository {
    state: Mutex<RepositoryState>,
    fail_activation: AtomicBool,
    installation_reads: AtomicUsize,
    allowed_installation_reads: AtomicUsize,
    pause_activation: AtomicBool,
    activation_entered: Notify,
    activation_release: Notify,
    pause_upsert: AtomicBool,
    upsert_entered: Notify,
    upsert_release: Notify,
    delay_mutations: AtomicBool,
    active_mutations: AtomicUsize,
    max_active_mutations: AtomicUsize,
    copying_transitions: AtomicUsize,
    log: Arc<Mutex<Vec<String>>>,
}

#[derive(Default)]
struct RepositoryState {
    installations: BTreeMap<String, LocalModelInstallationRecord>,
    references: Vec<LocalModelInstanceReference>,
    events: Vec<LocalModelEventRecord>,
}

impl RepositoryState {
    fn installation_id(&self, key: &ProviderInstanceKey) -> Option<String> {
        self.installations
            .values()
            .find(|installation| installation.provider_instance_key == *key)
            .map(|installation| installation.installation_id.clone())
    }

    fn reference_sources(
        &self,
        key: &ProviderInstanceKey,
    ) -> Vec<LocalModelInstanceReferenceSource> {
        self.references
            .iter()
            .filter(|reference| reference.provider_instance_key == *key)
            .map(|reference| reference.source.clone())
            .collect()
    }
}

impl FakeRepository {
    fn lock_state(&self) -> std::sync::MutexGuard<'_, RepositoryState> {
        self.state.lock().expect("repository lock")
    }

    pub(super) fn insert(&self, record: LocalModelInstallationRecord) {
        self.lock_state()
            .installations
            .insert(record.installation_id.clone(), record);
    }

    pub(in crate::local_models) fn record(
        &self,
        installation_id: &str,
    ) -> Option<LocalModelInstallationRecord> {
        self.lock_state()
            .installations
            .get(installation_id)
            .cloned()
    }

    pub(super) fn record_key(&self, installation_id: &str) -> ProviderInstanceKey {
        self.record(installation_id)
            .unwrap_or_else(|| panic!("missing installation {installation_id}"))
            .provider_instance_key
    }

    pub(in crate::local_models) fn events(&self) -> Vec<LocalModelEventRecord> {
        self.lock_state().events.clone()
    }

    pub(super) fn reference(
        &self,
        provider_instance_key: ProviderInstanceKey,
        source: LocalModelInstanceReferenceSource,
    ) {
        self.lock_state()
            .references
            .push(LocalModelInstanceReference {
                provider_instance_key,
                source,
            });
    }

    pub(super) fn deactivate_all_and_clear_references(&self) {
        let mut state = self.lock_state();
        for installation in state.installations.values_mut() {
            installation.is_active = false;
        }
        state.references.clear();
    }

    pub(super) fn set_active(&self, installation_id: &str) {
        let mut state = self.lock_state();
        for installation in state.installations.values_mut() {
            installation.is_active = installation.installation_id == installation_id;
        }
    }

    pub(super) fn fail_next_activation(&self) {
        self.fail_activation.store(true, Ordering::Release);
    }

    pub(super) fn allow_installation_reads(&self, count: usize) {
        self.allowed_installation_reads
            .store(count, Ordering::Release);
    }

    pub(super) fn pause_next_activation(&self) {
        self.pause_activation.store(true, Ordering::Release);
    }

    pub(super) async fn wait_for_activation(&self) {
        self.activation_entered.notified().await;
    }

    pub(super) fn release_activation(&self) {
        self.activation_release.notify_one();
    }

    pub(super) fn pause_next_upsert(&self) {
        self.pause_upsert.store(true, Ordering::Release);
    }

    pub(super) async fn wait_for_upsert(&self) {
        self.upsert_entered.notified().await;
    }

    pub(super) fn release_upsert(&self) {
        self.upsert_release.notify_one();
    }

    pub(super) fn delay_mutations(&self) {
        self.delay_mutations.store(true, Ordering::Release);
    }

    pub(super) fn max_active_mutations(&self) -> usize {
        self.max_active_mutations.load(Ordering::Acquire)
    }

    pub(super) fn active_mutations(&self) -> usize {
        self.active_mutations.load(Ordering::Acquire)
    }

    pub(super) fn copying_transitions(&self) -> usize {
        self.copying_transitions.load(Ordering::Acquire)
    }

    pub(super) fn append_event(&self, installation_id: &str, kind: LocalModelEventKind) -> u64 {
        let mut state = self.lock_state();
        let record = state
            .installations
            .get(installation_id)
            .cloned()
            .expect("event installation");
        append_event(&mut state, &record, kind, None);
        state.events.last().expect("appended event").cursor
    }

    pub(super) fn log(&self) -> Arc<Mutex<Vec<String>>> {
        Arc::clone(&self.log)
    }

    async fn mutation_delay(&self) -> MutationProbe<'_> {
        let active = self.active_mutations.fetch_add(1, Ordering::AcqRel) + 1;
        self.max_active_mutations
            .fetch_max(active, Ordering::AcqRel);
        if self.delay_mutations.load(Ordering::Acquire) {
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        MutationProbe { repository: self }
    }
}

struct MutationProbe<'a> {
    repository: &'a FakeRepository,
}

impl Drop for MutationProbe<'_> {
    fn drop(&mut self) {
        self.repository
            .active_mutations
            .fetch_sub(1, Ordering::AcqRel);
    }
}

impl LocalModelInstallationPersistence for FakeRepository {
    fn upsert_local_model_installation(
        &self,
        input: NewLocalModelInstallation,
    ) -> ProviderPersistenceFuture<'_, LocalModelInstallationRecord> {
        Box::pin(async move {
            if self.pause_upsert.swap(false, Ordering::AcqRel) {
                self.upsert_entered.notify_one();
                self.upsert_release.notified().await;
            }
            let _probe = self.mutation_delay().await;
            let mut state = self.lock_state();
            if let Some(record) = state
                .installations
                .get(&input.installation_id)
                .filter(|record| record.status == LocalModelInstallationStatus::Installed)
                .cloned()
            {
                return Ok(record);
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
        Box::pin(async move {
            let read = self.installation_reads.fetch_add(1, Ordering::AcqRel);
            let allowed = self.allowed_installation_reads.load(Ordering::Acquire);
            assert!(
                allowed == 0 || read < allowed,
                "unexpected installation read after {read} successful reads"
            );
            Ok(self.record(installation_id))
        })
    }

    fn local_model_installations(
        &self,
    ) -> ProviderPersistenceFuture<'_, Vec<LocalModelInstallationRecord>> {
        Box::pin(async move { Ok(self.lock_state().installations.values().cloned().collect()) })
    }

    fn update_local_model_installation<'a>(
        &'a self,
        installation_id: &'a str,
        update: LocalModelInstallationUpdate,
    ) -> ProviderPersistenceFuture<'a, LocalModelInstallationRecord> {
        Box::pin(async move {
            if update.status == LocalModelInstallationStatus::Downloading
                && update.downloaded_bytes == 0
            {
                self.copying_transitions.fetch_add(1, Ordering::AcqRel);
            }
            let _probe = self.mutation_delay().await;
            let mut state = self.lock_state();
            let mut record = state
                .installations
                .get(installation_id)
                .cloned()
                .ok_or_else(|| missing(installation_id))?;
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
            state
                .installations
                .insert(installation_id.to_string(), record.clone());
            append_event(
                &mut state,
                &record,
                event_kind(record.status),
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
            let _probe = self.mutation_delay().await;
            let mut state = self.lock_state();
            let mut record = state
                .installations
                .get(installation_id)
                .cloned()
                .ok_or_else(|| missing(installation_id))?;
            record.status = LocalModelInstallationStatus::Cancelled;
            state
                .installations
                .insert(installation_id.to_string(), record.clone());
            append_event(&mut state, &record, LocalModelEventKind::Cancelled, None);
            Ok(record)
        })
    }

    fn remove_terminal_local_model_installation<'a>(
        &'a self,
        installation_id: &'a str,
    ) -> ProviderPersistenceFuture<'a, RemovedLocalModelInstallation> {
        Box::pin(async move {
            let _probe = self.mutation_delay().await;
            self.log
                .lock()
                .expect("log lock")
                .push(format!("remove:{installation_id}"));
            let mut state = self.lock_state();
            let record = state
                .installations
                .remove(installation_id)
                .ok_or_else(|| missing(installation_id))?;
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
            let after = after_cursor.unwrap_or_default();
            Ok(self
                .lock_state()
                .events
                .iter()
                .filter(|event| event.cursor > after)
                .take(limit as usize)
                .cloned()
                .collect())
        })
    }
}

impl LocalModelActivationPersistence for FakeRepository {
    fn activate_local_model_as_system_default<'a>(
        &'a self,
        installation_id: &'a str,
        ready_selection: &'a ProviderReadySelection,
    ) -> ProviderPersistenceFuture<'a, LocalModelInstallationRecord> {
        Box::pin(async move {
            if self.pause_activation.swap(false, Ordering::AcqRel) {
                self.activation_entered.notify_one();
                self.activation_release.notified().await;
            }
            if self.fail_activation.swap(false, Ordering::AcqRel) {
                return Err(ProviderPersistenceError::Persistence {
                    operation: "activate_local_model",
                });
            }
            let mut state = self.lock_state();
            if !state.installations.contains_key(installation_id) {
                return Err(missing(installation_id));
            }
            for record in state.installations.values_mut() {
                record.is_active = record.installation_id == installation_id;
                if record.installation_id == installation_id {
                    record.runtime_retired_at = None;
                }
            }
            state.references.retain(|reference| {
                reference.source != LocalModelInstanceReferenceSource::DefaultModelPreference
            });
            state.references.push(LocalModelInstanceReference {
                provider_instance_key: ready_selection.key().clone(),
                source: LocalModelInstanceReferenceSource::DefaultModelPreference,
            });
            let activated = state
                .installations
                .get(installation_id)
                .expect("activated installation")
                .clone();
            Ok(activated)
        })
    }
}

impl LocalModelLifecyclePersistence for FakeRepository {
    fn local_model_reconstruction_snapshot(
        &self,
    ) -> ProviderPersistenceFuture<'_, LocalModelReconstructionSnapshot> {
        Box::pin(async move {
            let state = self.lock_state();
            let mut references = state.references.clone();
            references.extend(
                state
                    .installations
                    .values()
                    .filter(|installation| installation.is_active)
                    .map(|installation| LocalModelInstanceReference {
                        provider_instance_key: installation.provider_instance_key.clone(),
                        source: LocalModelInstanceReferenceSource::ActiveInstallation,
                    }),
            );
            Ok(LocalModelReconstructionSnapshot {
                installations: state.installations.values().cloned().collect(),
                references,
            })
        })
    }

    fn retire_unreferenced_instance_runtime<'a>(
        &'a self,
        provider_instance_key: &'a ProviderInstanceKey,
    ) -> ProviderPersistenceFuture<'a, LocalModelRuntimeRetirementResult> {
        Box::pin(async move {
            let mut state = self.lock_state();
            let installation_id = state.installation_id(provider_instance_key);
            let Some(installation_id) = installation_id else {
                return Ok(LocalModelRuntimeRetirementResult::Missing);
            };
            let references = state.reference_sources(provider_instance_key);
            let installation = state
                .installations
                .get_mut(&installation_id)
                .expect("located installation");
            if installation.is_active {
                return Ok(LocalModelRuntimeRetirementResult::Referenced {
                    installation: installation.clone(),
                    references: vec![LocalModelInstanceReferenceSource::ActiveInstallation],
                });
            }
            if !references.is_empty() {
                return Ok(LocalModelRuntimeRetirementResult::Referenced {
                    installation: installation.clone(),
                    references,
                });
            }
            if installation.runtime_retired_at.is_some() {
                return Ok(LocalModelRuntimeRetirementResult::AlreadyRetired(
                    installation.clone(),
                ));
            }
            installation.runtime_retired_at = Some(timestamp().to_string());
            Ok(LocalModelRuntimeRetirementResult::Retired(
                installation.clone(),
            ))
        })
    }

    fn claim_unreferenced_instance_for_retirement<'a>(
        &'a self,
        provider_instance_key: &'a ProviderInstanceKey,
    ) -> ProviderPersistenceFuture<'a, LocalModelRetirementClaimResult> {
        Box::pin(async move {
            let mut state = self.lock_state();
            let installation_id = state.installation_id(provider_instance_key);
            let Some(installation_id) = installation_id else {
                return Ok(LocalModelRetirementClaimResult::Missing);
            };
            let references = state.reference_sources(provider_instance_key);
            let installation = state
                .installations
                .get(&installation_id)
                .expect("located installation");
            if installation.is_active {
                return Ok(LocalModelRetirementClaimResult::Referenced {
                    installation: installation.clone(),
                    references: vec![LocalModelInstanceReferenceSource::ActiveInstallation],
                });
            }
            if !references.is_empty() {
                return Ok(LocalModelRetirementClaimResult::Referenced {
                    installation: installation.clone(),
                    references,
                });
            }
            let already_claimed = installation.retirement_claimed_at.is_some();
            let installation = state
                .installations
                .get_mut(&installation_id)
                .expect("located installation");
            if !already_claimed {
                installation.retirement_claimed_at = Some(timestamp().to_string());
            }
            let claim = ClaimedLocalModelInstallation {
                installation: installation.clone(),
            };
            Ok(if already_claimed {
                LocalModelRetirementClaimResult::AlreadyClaimed(claim)
            } else {
                LocalModelRetirementClaimResult::Claimed(claim)
            })
        })
    }

    fn complete_claimed_local_model_removal<'a>(
        &'a self,
        provider_instance_key: &'a ProviderInstanceKey,
    ) -> ProviderPersistenceFuture<'a, RemovedLocalModelInstallation> {
        Box::pin(async move {
            let mut state = self.lock_state();
            let installation_id = state.installation_id(provider_instance_key).ok_or(
                ProviderPersistenceError::InstallationNotFound {
                    installation_id: provider_instance_key.to_string(),
                },
            )?;
            let installation = state
                .installations
                .remove(&installation_id)
                .expect("located installation");
            self.log
                .lock()
                .expect("log lock")
                .push(format!("remove:{}", installation.installation_id));
            append_event(
                &mut state,
                &installation,
                LocalModelEventKind::Removed,
                None,
            );
            Ok(RemovedLocalModelInstallation { installation })
        })
    }
}

#[derive(Debug)]
pub(super) struct FakeProcessFactory {
    processes: Mutex<HashMap<String, Arc<FakeProcess>>>,
    fail_starts: Mutex<HashSet<String>>,
    log: Arc<Mutex<Vec<String>>>,
}

impl FakeProcessFactory {
    pub(super) fn new(log: Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            processes: Mutex::new(HashMap::new()),
            fail_starts: Mutex::new(HashSet::new()),
            log,
        }
    }

    pub(super) fn process(&self, installation_id: &str) -> Arc<FakeProcess> {
        Arc::clone(
            self.processes
                .lock()
                .expect("processes lock")
                .get(installation_id)
                .expect("managed fake process"),
        )
    }

    pub(super) fn started_ids(&self) -> Vec<String> {
        let mut ids = self
            .processes
            .lock()
            .expect("processes lock")
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        ids.sort();
        ids
    }

    pub(super) fn fail_start(&self, installation_id: &str) {
        self.fail_starts
            .lock()
            .expect("fail starts lock")
            .insert(installation_id.to_string());
    }

    pub(super) fn allow_start(&self, installation_id: &str) {
        self.fail_starts
            .lock()
            .expect("fail starts lock")
            .remove(installation_id);
    }
}

impl LocalModelProcessFactory for FakeProcessFactory {
    fn start(
        &self,
        installation: LocalModelInstallationRecord,
    ) -> LocalModelProcessFuture<'_, Arc<dyn LocalModelProcess>> {
        Box::pin(async move {
            if self
                .fail_starts
                .lock()
                .expect("fail starts lock")
                .contains(&installation.installation_id)
            {
                return Err(LocalModelManagerError::Runtime {
                    operation: "start_fake_process",
                    message: "injected start failure".to_string(),
                });
            }
            let process = Arc::new(FakeProcess::new(
                installation.installation_id.clone(),
                installation.model_id,
                Arc::clone(&self.log),
            ));
            self.processes
                .lock()
                .expect("processes lock")
                .insert(installation.installation_id, Arc::clone(&process));
            Ok(process as Arc<dyn LocalModelProcess>)
        })
    }
}

#[derive(Debug)]
pub(super) struct FakeProcess {
    installation_id: String,
    provider: ProviderHandle,
    status_tx: watch::Sender<LocalModelRuntimeStatus>,
    shutdowns: AtomicUsize,
    fail_shutdown: AtomicBool,
    log: Arc<Mutex<Vec<String>>>,
}

impl FakeProcess {
    fn new(installation_id: String, model_id: String, log: Arc<Mutex<Vec<String>>>) -> Self {
        let (status_tx, _) = watch::channel(LocalModelRuntimeStatus::Ready {
            backend: LocalModelBackend::Cpu,
            endpoint: format!("http://{installation_id}.invalid/"),
            model_id,
        });
        Self {
            installation_id,
            provider: Arc::new(FakeProvider),
            status_tx,
            shutdowns: AtomicUsize::new(0),
            fail_shutdown: AtomicBool::new(false),
            log,
        }
    }

    pub(super) fn shutdowns(&self) -> usize {
        self.shutdowns.load(Ordering::Acquire)
    }

    pub(super) fn fail_shutdown(&self) {
        self.fail_shutdown.store(true, Ordering::Release);
    }

    pub(super) fn set_status(&self, status: LocalModelRuntimeStatus) {
        self.status_tx.send_replace(status);
    }
}

impl LocalModelProcess for FakeProcess {
    fn provider(&self) -> ProviderHandle {
        Arc::clone(&self.provider)
    }

    fn status(&self) -> LocalModelRuntimeStatus {
        self.status_tx.borrow().clone()
    }

    fn subscribe_status(&self) -> watch::Receiver<LocalModelRuntimeStatus> {
        self.status_tx.subscribe()
    }

    fn shutdown(&self) -> LocalModelProcessFuture<'_, ()> {
        Box::pin(async move {
            self.log
                .lock()
                .expect("log lock")
                .push(format!("stop:{}", self.installation_id));
            if self.fail_shutdown.load(Ordering::Acquire) {
                return Err(LocalModelManagerError::Runtime {
                    operation: "stop_fake_process",
                    message: "injected stop failure".to_string(),
                });
            }
            self.shutdowns.fetch_add(1, Ordering::AcqRel);
            self.status_tx
                .send_replace(LocalModelRuntimeStatus::Stopped);
            Ok(())
        })
    }
}

#[derive(Debug)]
struct FakeProvider;

impl ProviderOperations for FakeProvider {
    fn generate_streaming<'a>(
        &'a self,
        _request: GenerateRequest,
        _on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> ProviderOperationFuture<'a, GenerateResponse> {
        Box::pin(async {
            Err(ProviderError::ProviderUnavailable {
                provider: "fake_local".to_string(),
                message: "fake provider does not generate".to_string(),
            })
        })
    }
}

pub(super) fn installed_record(
    installation_id: &str,
    model_id: &str,
    digest_byte: char,
    active: bool,
) -> LocalModelInstallationRecord {
    let sha256 = digest_byte.to_string().repeat(64);
    LocalModelInstallationRecord {
        installation_id: installation_id.to_string(),
        provider_instance_key: local_model_provider_instance_key(
            crate::LOCAL_MODELS_PROVIDER_ACCOUNT_ID,
            installation_id,
            model_id,
        )
        .expect("fake exact key"),
        model_id: model_id.to_string(),
        display_name: installation_id.to_string(),
        source_kind: LocalModelSourceKind::LocalFile,
        source_repo: None,
        source_revision: None,
        source_file: Some(format!("{installation_id}.gguf")),
        sha256: Some(sha256.clone()),
        download_gb: 0.001,
        expected_bytes: Some(4),
        downloaded_bytes: 4,
        license: None,
        backend: LocalModelBackend::Cpu,
        status: LocalModelInstallationStatus::Installed,
        blob_relative_path: Some(format!("models/blobs/{sha256}.gguf")),
        is_active: active,
        runtime_retired_at: None,
        retirement_claimed_at: None,
        error_code: None,
        error_message: None,
        installed_at: Some(timestamp().to_string()),
        created_at: timestamp().to_string(),
        updated_at: timestamp().to_string(),
    }
}

fn queued_record(input: NewLocalModelInstallation) -> LocalModelInstallationRecord {
    let mut record = installed_record(&input.installation_id, &input.model_id, '0', false);
    record.display_name = input.display_name;
    record.source_kind = input.source_kind;
    record.source_repo = input.source_repo;
    record.source_revision = input.source_revision;
    record.source_file = input.source_file;
    record.sha256 = input.sha256;
    record.download_gb = input.download_gb;
    record.expected_bytes = input.expected_bytes;
    record.downloaded_bytes = 0;
    record.license = input.license;
    record.backend = input.backend;
    record.status = LocalModelInstallationStatus::Queued;
    record.blob_relative_path = None;
    record.installed_at = None;
    record
}

fn append_event(
    state: &mut RepositoryState,
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

fn event_kind(status: LocalModelInstallationStatus) -> LocalModelEventKind {
    match status {
        LocalModelInstallationStatus::Queued => LocalModelEventKind::Queued,
        LocalModelInstallationStatus::Downloading => LocalModelEventKind::Progress,
        LocalModelInstallationStatus::Verifying => LocalModelEventKind::Verifying,
        LocalModelInstallationStatus::Installed => LocalModelEventKind::Installed,
        LocalModelInstallationStatus::Failed => LocalModelEventKind::Failed,
        LocalModelInstallationStatus::Cancelled => LocalModelEventKind::Cancelled,
    }
}

fn missing(installation_id: &str) -> ProviderPersistenceError {
    ProviderPersistenceError::InstallationNotFound {
        installation_id: installation_id.to_string(),
    }
}

const fn timestamp() -> &'static str {
    "2026-07-16T00:00:00.000Z"
}
