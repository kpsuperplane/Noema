use std::{path::PathBuf, pin::Pin};

use async_graphql::{Enum, InputObject, Result, SimpleObject};
use futures_util::Stream;

use super::{errors::graphql_error, schema::GraphqlState};
use views::{
    default_preference_view, event_view, installation_view, load_catalog_views, runtime_status_view,
};

mod views;

/// A backend available to the local llama.cpp runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "LocalModelBackend")]
pub enum GraphqlLocalModelBackend {
    /// Apple Metal acceleration.
    Metal,
    /// NVIDIA CUDA acceleration.
    Cuda,
    /// Vulkan acceleration.
    Vulkan,
    /// Portable CPU inference.
    Cpu,
}

impl From<crate::LocalModelBackend> for GraphqlLocalModelBackend {
    fn from(value: crate::LocalModelBackend) -> Self {
        match value {
            crate::LocalModelBackend::Metal => Self::Metal,
            crate::LocalModelBackend::Cuda => Self::Cuda,
            crate::LocalModelBackend::Vulkan => Self::Vulkan,
            crate::LocalModelBackend::Cpu => Self::Cpu,
        }
    }
}

/// One GGUF artifact offered for a curated local model.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "LocalModelBuild")]
pub struct GraphqlLocalModelBuild {
    /// Artifact filename in the pinned source repository.
    pub file: String,
    /// Verified SHA-256 digest.
    pub sha256: String,
    /// Rounded UI-facing download size in decimal gigabytes.
    pub download_gb: f64,
    /// Backends compatible with this artifact.
    pub backends: Vec<GraphqlLocalModelBackend>,
    /// Minimum system or unified memory in whole gigabytes.
    pub min_ram_gb: i64,
    /// Minimum discrete or unified accelerator memory in whole gigabytes.
    pub min_vram_gb: Option<i64>,
}

impl From<&crate::LocalModelBuild> for GraphqlLocalModelBuild {
    fn from(value: &crate::LocalModelBuild) -> Self {
        Self {
            file: value.file.clone(),
            sha256: value.sha256.clone(),
            download_gb: value.download_gb,
            backends: value.backends.iter().copied().map(Into::into).collect(),
            min_ram_gb: value.min_ram_gb.cast_signed(),
            min_vram_gb: value.min_vram_gb.map(u64::cast_signed),
        }
    }
}

/// Hardware values that caused a curated model build to match.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "LocalModelHardwareFit")]
pub struct GraphqlLocalModelHardwareFit {
    /// Backend selected for the build.
    pub backend: GraphqlLocalModelBackend,
    /// Detected system or unified memory in whole gigabytes.
    pub ram_gb: i64,
    /// Detected discrete accelerator memory in whole gigabytes.
    pub vram_gb: Option<i64>,
    /// Whether the accelerator shares system memory.
    pub unified_memory: bool,
    /// Product-facing explanation generated from the match.
    pub explanation: String,
}

/// One curated local model and the build selected for this machine.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "LocalModelCatalogEntry")]
pub struct GraphqlLocalModelCatalogEntry {
    /// Stable curated model identifier.
    pub model_id: String,
    /// Product-facing model name.
    pub name: String,
    /// License shown before download.
    pub license: String,
    /// Recommendation priority from the bundled catalog.
    pub priority: i64,
    /// Pinned Hugging Face repository.
    pub repo: String,
    /// Immutable source revision.
    pub revision: String,
    /// Every curated artifact for advanced inspection.
    pub builds: Vec<GraphqlLocalModelBuild>,
    /// Best compatible artifact for this machine, when one fits.
    pub selected_build: Option<GraphqlLocalModelBuild>,
    /// Backend used by the selected artifact.
    pub compatible_backend: Option<GraphqlLocalModelBackend>,
    /// Detected hardware values that satisfy the artifact thresholds.
    pub hardware_fit: Option<GraphqlLocalModelHardwareFit>,
    /// Whether this is the top recommendation for the current machine.
    pub is_recommended: bool,
}

/// Current llama.cpp process state.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "LocalModelRuntimeStatus")]
pub enum GraphqlLocalModelRuntimeStatus {
    /// No installed model is selected.
    Inactive,
    /// The runtime is starting or loading weights.
    Starting,
    /// The runtime is ready for inference.
    Running,
    /// The runtime is stopping.
    Stopping,
    /// The runtime failed and may be retried.
    Failed,
}

/// Current local-model installation state.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "LocalModelInstallationStatus")]
pub enum GraphqlLocalModelInstallationStatus {
    /// The installation is queued.
    Queued,
    /// Model bytes are being downloaded or copied.
    Downloading,
    /// The artifact checksum is being verified.
    Verifying,
    /// The model was installed atomically.
    Installed,
    /// The operation was cancelled.
    Cancelled,
    /// The operation failed and may be retried.
    Failed,
}

/// Provenance for an installed local model.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "LocalModelSourceKind")]
pub enum GraphqlLocalModelSourceKind {
    /// Artifact from Noema's bundled curated catalog.
    Catalog,
    /// User-supplied public Hugging Face GGUF.
    PublicGguf,
    /// GGUF imported from the local filesystem.
    LocalFile,
}

/// One durable local-model installation projection.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "LocalModelInstallation")]
pub struct GraphqlLocalModelInstallation {
    /// Stable installation identifier.
    pub installation_id: String,
    /// Catalog model id, or an import-specific stable id.
    pub model_id: String,
    /// Product-facing model name.
    pub name: String,
    /// Artifact filename.
    pub file: String,
    /// Model provenance.
    pub source_kind: GraphqlLocalModelSourceKind,
    /// Current operation state.
    pub status: GraphqlLocalModelInstallationStatus,
    /// Verified content digest when known.
    pub sha256: Option<String>,
    /// Downloaded or copied bytes.
    pub completed_bytes: i64,
    /// Expected total bytes when known.
    pub total_bytes: Option<i64>,
    /// Disk bytes owned by the installation after deduplication.
    pub disk_bytes: i64,
    /// Runtime backend selected for this installation.
    pub backend: Option<GraphqlLocalModelBackend>,
    /// Whether this installation is Noema's active local model.
    pub is_active: bool,
    /// Stable non-secret error code.
    pub error_code: Option<String>,
    /// UI-safe operation failure message.
    pub error_message: Option<String>,
    /// Durable creation timestamp.
    pub created_at: String,
    /// Durable update timestamp.
    pub updated_at: String,
}

/// Noema-wide default model selection.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "DefaultModelPreference")]
pub struct GraphqlDefaultModelPreference {
    /// Provider family used for new workloads.
    pub provider_kind: String,
    /// Provider account used for new workloads.
    pub provider_account_id: String,
    /// Provider-specific model profile.
    pub model_profile: String,
    /// Optional provider-specific reasoning effort.
    pub reasoning_effort: Option<String>,
}

/// First-run local-model recommendation and current readiness.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "LocalModelSetup")]
pub struct GraphqlLocalModelSetup {
    /// Top curated recommendation for this machine.
    pub recommended_model: Option<GraphqlLocalModelCatalogEntry>,
    /// Installation currently satisfying local setup, when present.
    pub installation: Option<GraphqlLocalModelInstallation>,
    /// Current supervised llama.cpp process state.
    pub runtime_status: GraphqlLocalModelRuntimeStatus,
    /// Whether setup is complete and local inference is usable.
    pub is_ready: bool,
}

/// Install one curated model/build.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "InstallLocalModelInput")]
pub struct GraphqlInstallLocalModelInput {
    /// Stable model id from the bundled catalog.
    pub model_id: String,
    /// Optional artifact filename; the machine-selected build is used when omitted.
    pub file: Option<String>,
}

/// Import a public GGUF or a GGUF already on this machine.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "ImportLocalModelInput")]
pub struct GraphqlImportLocalModelInput {
    /// Product-facing name for the imported model.
    pub name: String,
    /// Import provenance.
    pub source_kind: GraphqlLocalModelSourceKind,
    /// Local file path for a local-file import.
    pub local_path: Option<String>,
    /// Public Hugging Face repository for a public-GGUF import.
    pub repo: Option<String>,
    /// Immutable Hugging Face revision for a public-GGUF import.
    pub revision: Option<String>,
    /// GGUF filename in the public repository.
    pub file: Option<String>,
    /// Required verified SHA-256 digest for a public-GGUF import.
    pub sha256: Option<String>,
    /// License label shown in Settings when supplied by the user.
    pub license: Option<String>,
}

/// Save Noema's system model default without changing existing explicit selections.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "SaveDefaultModelPreferenceInput")]
pub struct GraphqlSaveDefaultModelPreferenceInput {
    /// Provider family used for new workloads.
    pub provider_kind: String,
    /// Provider account used for new workloads.
    pub provider_account_id: String,
    /// Provider-specific model profile.
    pub model_profile: String,
    /// Optional provider-specific reasoning effort.
    pub reasoning_effort: Option<String>,
}

/// Category for one cursor-bearing local-model event.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "LocalModelEventKind")]
pub enum GraphqlLocalModelEventKind {
    /// An installation projection changed.
    InstallationUpdated,
    /// Download or copy progress advanced.
    TransferProgress,
    /// The active local model changed.
    ActiveModelChanged,
    /// The supervised runtime state changed.
    RuntimeChanged,
    /// The system default model preference changed.
    DefaultPreferenceChanged,
}

/// One durable local-model event, suitable for reconnect backfill.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "LocalModelEvent")]
pub struct GraphqlLocalModelEvent {
    /// Exclusive cursor for reconnect/backfill.
    pub cursor: String,
    /// Typed event category.
    pub kind: GraphqlLocalModelEventKind,
    /// Installation affected by the event, when applicable.
    pub installation_id: Option<String>,
    /// Model affected by the event, when applicable.
    pub model_id: Option<String>,
    /// Current installation projection, when applicable.
    pub installation: Option<GraphqlLocalModelInstallation>,
    /// Current runtime state, when applicable.
    pub runtime_status: Option<GraphqlLocalModelRuntimeStatus>,
    /// Durable event creation timestamp.
    pub created_at: String,
}

pub(super) async fn local_model_setup(state: &GraphqlState) -> Result<GraphqlLocalModelSetup> {
    let recommendations = load_catalog_views().await?;
    let installations = state
        .store()?
        .list_local_model_installations()
        .await
        .map_err(graphql_error)?;
    let installation = installations
        .into_iter()
        .find(|installation| installation.is_active)
        .map(installation_view);
    let is_ready = installation.as_ref().is_some_and(|installation| {
        installation.status == GraphqlLocalModelInstallationStatus::Installed
    });
    let runtime_status = match state.runtime() {
        Ok(runtime) => runtime.local_model_runtime_status().await.map_or(
            GraphqlLocalModelRuntimeStatus::Inactive,
            runtime_status_view,
        ),
        Err(_) => GraphqlLocalModelRuntimeStatus::Inactive,
    };

    Ok(GraphqlLocalModelSetup {
        recommended_model: recommendations
            .into_iter()
            .find(|model| model.is_recommended),
        installation,
        runtime_status,
        is_ready,
    })
}

pub(super) async fn local_model_catalog() -> Result<Vec<GraphqlLocalModelCatalogEntry>> {
    load_catalog_views().await
}

pub(super) async fn local_model_installations(
    state: &GraphqlState,
) -> Result<Vec<GraphqlLocalModelInstallation>> {
    state
        .store()?
        .list_local_model_installations()
        .await
        .map(|installations| installations.into_iter().map(installation_view).collect())
        .map_err(graphql_error)
}

pub(super) async fn default_model_preference(
    state: &GraphqlState,
) -> Result<Option<GraphqlDefaultModelPreference>> {
    state
        .store()?
        .get_default_model_preference()
        .await
        .map(|preference| preference.map(default_preference_view))
        .map_err(graphql_error)
}

pub(super) async fn install_local_model(
    state: &GraphqlState,
    input: GraphqlInstallLocalModelInput,
) -> Result<GraphqlLocalModelInstallation> {
    let hardware = crate::detect_local_hardware_profiles().map_err(graphql_error)?;
    let catalog = crate::LocalModelCatalog::bundled().map_err(graphql_error)?;
    let selection = match input.file.as_deref() {
        Some(file) => catalog.select_named_build(&input.model_id, file, &hardware),
        None => catalog.select_build(&input.model_id, &hardware),
    }
    .ok_or_else(|| {
        async_graphql::Error::new("the selected local model build does not fit this machine")
    })?;
    let model = selection.model.clone();
    let build = selection.build.clone();
    let backend = selection.hardware.backend;
    let installer = crate::LocalModelInstaller::new(state.store()?.clone(), state.paths()?.clone())
        .map_err(graphql_error)?;
    let queued = installer
        .queue_catalog_model(&model, &build, backend)
        .await
        .map_err(graphql_error)?;
    if queued.status == crate::LocalModelInstallationStatus::Installed {
        return activate_local_model(state, queued.installation_id).await;
    }

    let cancellation = tokio_util::sync::CancellationToken::new();
    state.retain_local_model_cancellation(queued.installation_id.clone(), cancellation.clone());
    let task_state = state.clone();
    let installation_id = queued.installation_id.clone();
    tokio::spawn(async move {
        let result = installer
            .install_catalog_model(&model, &build, backend, false, cancellation)
            .await;
        if let Ok(installed) = result
            && let Ok(runtime) = task_state.runtime()
            && let Ok(paths) = task_state.paths()
            && runtime
                .register_installed_local_model(&installed, paths)
                .await
                .is_ok()
            && let Ok(store) = task_state.store()
        {
            let _ = store
                .activate_local_model_as_system_default(&installation_id)
                .await;
        }
        task_state.release_local_model_cancellation(&installation_id);
    });
    Ok(installation_view(queued))
}

pub(super) async fn import_local_model(
    state: &GraphqlState,
    input: GraphqlImportLocalModelInput,
) -> Result<GraphqlLocalModelInstallation> {
    let backend = crate::detect_local_hardware_profiles()
        .map_err(graphql_error)?
        .first()
        .map_or(crate::LocalModelBackend::Cpu, |profile| profile.backend);
    let installer = crate::LocalModelInstaller::new(state.store()?.clone(), state.paths()?.clone())
        .map_err(graphql_error)?;
    let cancellation = tokio_util::sync::CancellationToken::new();
    let model_id = imported_model_id(&input.name);

    let (queued, operation) = match input.source_kind {
        GraphqlLocalModelSourceKind::LocalFile => {
            let import = crate::LocalFileModelImport {
                name: input.name,
                model_id,
                path: PathBuf::from(required_import_value(input.local_path, "localPath")?),
                license: input.license,
                backend,
            };
            let queued = installer
                .queue_local_file(&import)
                .await
                .map_err(graphql_error)?;
            let installer = installer.clone();
            let cancellation = cancellation.clone();
            let operation =
                tokio::spawn(
                    async move { installer.import_local_file(import, cancellation).await },
                );
            (queued, operation)
        }
        GraphqlLocalModelSourceKind::PublicGguf => {
            let import = crate::HuggingFaceLocalModelImport {
                name: input.name,
                model_id,
                repo: required_import_value(input.repo, "repo")?,
                revision: required_import_value(input.revision, "revision")?,
                file: required_import_value(input.file, "file")?,
                sha256: required_import_value(input.sha256, "sha256")?,
                license: input.license,
                backend,
            };
            let queued = installer
                .queue_hugging_face(&import)
                .await
                .map_err(graphql_error)?;
            let installer = installer.clone();
            let cancellation = cancellation.clone();
            let operation =
                tokio::spawn(
                    async move { installer.import_hugging_face(import, cancellation).await },
                );
            (queued, operation)
        }
        GraphqlLocalModelSourceKind::Catalog => {
            return Err(async_graphql::Error::new(
                "catalog models must use installLocalModel",
            ));
        }
    };
    state.retain_local_model_cancellation(queued.installation_id.clone(), cancellation);
    let task_state = state.clone();
    let installation_id = queued.installation_id.clone();
    tokio::spawn(async move {
        let _ = operation.await;
        task_state.release_local_model_cancellation(&installation_id);
    });
    Ok(installation_view(queued))
}

pub(super) async fn cancel_local_model_install(
    state: &GraphqlState,
    installation_id: String,
) -> Result<GraphqlLocalModelInstallation> {
    let _ = state.cancel_local_model_operation(&installation_id);
    state
        .store()?
        .cancel_local_model_installation(&installation_id)
        .await
        .map(installation_view)
        .map_err(graphql_error)
}

fn imported_model_id(name: &str) -> String {
    let slug = name
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "imported-model".to_string()
    } else {
        slug.to_string()
    }
}

fn required_import_value(value: Option<String>, field: &str) -> Result<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| async_graphql::Error::new(format!("{field} is required for this import")))
}

pub(super) async fn remove_local_model(
    state: &GraphqlState,
    installation_id: String,
) -> Result<bool> {
    crate::LocalModelInstaller::new(state.store()?.clone(), state.paths()?.clone())
        .map_err(graphql_error)?
        .remove(&installation_id)
        .await
        .map_err(graphql_error)?;
    Ok(true)
}

pub(super) async fn activate_local_model(
    state: &GraphqlState,
    installation_id: String,
) -> Result<GraphqlLocalModelInstallation> {
    let installation = state
        .store()?
        .get_local_model_installation(&installation_id)
        .await
        .map_err(graphql_error)?
        .ok_or_else(|| async_graphql::Error::new("local-model installation is unavailable"))?;
    state
        .runtime()?
        .register_installed_local_model(&installation, state.paths()?)
        .await
        .map_err(graphql_error)?;
    state
        .store()?
        .activate_local_model_as_system_default(&installation_id)
        .await
        .map_err(graphql_error)?;
    let installation = state
        .store()?
        .get_local_model_installation(&installation_id)
        .await
        .map_err(graphql_error)?
        .ok_or_else(|| async_graphql::Error::new("local-model installation is unavailable"))?;
    Ok(installation_view(installation))
}

pub(super) async fn save_default_model_preference(
    state: &GraphqlState,
    input: GraphqlSaveDefaultModelPreferenceInput,
) -> Result<GraphqlDefaultModelPreference> {
    state
        .store()?
        .save_default_model_preference(
            &input.provider_kind,
            &input.provider_account_id,
            &input.model_profile,
            input.reasoning_effort.as_deref(),
        )
        .await
        .map(default_preference_view)
        .map_err(graphql_error)
}

pub(super) async fn retry_local_model_runtime(
    state: &GraphqlState,
) -> Result<GraphqlLocalModelRuntimeStatus> {
    state
        .runtime()?
        .retry_local_model_runtime()
        .await
        .map(runtime_status_view)
        .map_err(graphql_error)
}

pub(super) async fn local_model_events(
    state: &GraphqlState,
    after: Option<String>,
) -> Result<Pin<Box<dyn Stream<Item = Result<GraphqlLocalModelEvent>> + Send>>> {
    let store = state.store()?.clone();
    let runtime = state.runtime().ok().cloned();
    let mut runtime_status = match &runtime {
        Some(runtime) => runtime.subscribe_local_model_runtime_status().await,
        None => None,
    };
    let mut cursor = match after {
        Some(cursor) => parse_event_cursor(&cursor)?,
        None => store
            .list_local_model_events(None, 1_000)
            .await
            .map_err(graphql_error)?
            .last()
            .map_or(0, |event| event.cursor),
    };
    let mut runtime_sequence = 0_u64;

    Ok(Box::pin(async_stream::stream! {
        loop {
            let mut runtime_closed = false;
            if let Some(receiver) = runtime_status.as_mut() {
                tokio::select! {
                    changed = receiver.changed() => {
                        if changed.is_ok() {
                            runtime_sequence = runtime_sequence.saturating_add(1);
                            let status = receiver.borrow().clone();
                            yield Ok(GraphqlLocalModelEvent {
                                cursor: format!("{cursor}:runtime:{runtime_sequence}"),
                                kind: GraphqlLocalModelEventKind::RuntimeChanged,
                                installation_id: None,
                                model_id: None,
                                installation: None,
                                runtime_status: Some(runtime_status_view(status)),
                                created_at: runtime_event_time(),
                            });
                        } else {
                            runtime_closed = true;
                        }
                    }
                    () = tokio::time::sleep(std::time::Duration::from_millis(250)) => {}
                }
            } else {
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            }
            if runtime_closed {
                runtime_status = None;
            }
            let events = match store.list_local_model_events(Some(cursor), 256).await {
                Ok(events) => events,
                Err(error) => {
                    yield Err(graphql_error(error));
                    break;
                }
            };
            for event in events {
                cursor = event.cursor;
                let activated = event.kind == crate::local_models::LocalModelEventKind::Activated;
                let installation = match store
                    .get_local_model_installation(&event.installation_id)
                    .await
                {
                    Ok(installation) => installation.map(installation_view),
                    Err(error) => {
                        yield Err(graphql_error(error));
                        return;
                    }
                };
                yield Ok(event_view(event, installation));
                if activated {
                    runtime_status = match &runtime {
                        Some(runtime) => runtime.subscribe_local_model_runtime_status().await,
                        None => None,
                    };
                }
            }
        }
    }))
}

fn parse_event_cursor(cursor: &str) -> Result<u64> {
    cursor
        .trim()
        .split(':')
        .next()
        .unwrap_or_default()
        .parse::<u64>()
        .map_err(|_| async_graphql::Error::new("invalid local-model event cursor"))
}

fn runtime_event_time() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}
