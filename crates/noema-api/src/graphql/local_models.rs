use std::{path::PathBuf, pin::Pin, str::FromStr};

use async_graphql::{Enum, InputObject, Result, SimpleObject};
use futures_util::{Stream, StreamExt};
use noema_providers::{ProviderKind, ReasoningEffort};

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

graphql_enum_from!(noema_providers::LocalModelBackend => GraphqlLocalModelBackend {
    Metal => Metal,
    Cuda => Cuda,
    Vulkan => Vulkan,
    Cpu => Cpu,
});

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

impl From<&noema_providers::LocalModelBuild> for GraphqlLocalModelBuild {
    fn from(value: &noema_providers::LocalModelBuild) -> Self {
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
    let manager = state.local_model_manager()?;
    let recommendations = load_catalog_views(manager).await?;
    let recommended_model = recommendations
        .into_iter()
        .find(|model| model.is_recommended);
    let installations = manager.installations().await.map_err(graphql_error)?;
    let installation = installations
        .iter()
        .find(|installation| installation.is_active)
        .or_else(|| {
            recommended_model.as_ref().and_then(|recommended| {
                installations
                    .iter()
                    .find(|installation| installation.model_id == recommended.model_id)
            })
        })
        .cloned()
        .map(installation_view);
    let runtime_status = runtime_status_view(manager.runtime_status());
    let is_ready = installation.as_ref().is_some_and(|installation| {
        installation.status == GraphqlLocalModelInstallationStatus::Installed
            && installation.is_active
            && runtime_status == GraphqlLocalModelRuntimeStatus::Running
    });

    Ok(GraphqlLocalModelSetup {
        recommended_model,
        installation,
        runtime_status,
        is_ready,
    })
}

pub(super) async fn local_model_catalog(
    state: &GraphqlState,
) -> Result<Vec<GraphqlLocalModelCatalogEntry>> {
    load_catalog_views(state.local_model_manager()?).await
}

pub(super) async fn local_model_installations(
    state: &GraphqlState,
) -> Result<Vec<GraphqlLocalModelInstallation>> {
    state
        .local_model_manager()?
        .installations()
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
    state
        .local_model_manager()?
        .install_catalog_model(input.model_id, input.file)
        .await
        .map(installation_view)
        .map_err(graphql_error)
}

pub(super) async fn import_local_model(
    state: &GraphqlState,
    input: GraphqlImportLocalModelInput,
) -> Result<GraphqlLocalModelInstallation> {
    let manager = state.local_model_manager()?;
    let backend = manager.preferred_import_backend().map_err(graphql_error)?;
    let model_id = imported_model_id(&input.name);
    let queued = match input.source_kind {
        GraphqlLocalModelSourceKind::LocalFile => {
            let import = noema_providers::LocalFileModelImport {
                name: input.name,
                model_id,
                path: PathBuf::from(required_import_value(input.local_path, "localPath")?),
                license: input.license,
                backend,
            };
            manager
                .import_local_file(import)
                .await
                .map_err(graphql_error)?
        }
        GraphqlLocalModelSourceKind::PublicGguf => {
            let import = noema_providers::HuggingFaceLocalModelImport {
                name: input.name,
                model_id,
                repo: required_import_value(input.repo, "repo")?,
                revision: required_import_value(input.revision, "revision")?,
                file: required_import_value(input.file, "file")?,
                sha256: required_import_value(input.sha256, "sha256")?,
                license: input.license,
                backend,
            };
            manager
                .import_hugging_face(import)
                .await
                .map_err(graphql_error)?
        }
        GraphqlLocalModelSourceKind::Catalog => {
            return Err(async_graphql::Error::new(
                "catalog models must use installLocalModel",
            ));
        }
    };
    Ok(installation_view(queued))
}

pub(super) async fn cancel_local_model_install(
    state: &GraphqlState,
    installation_id: String,
) -> Result<GraphqlLocalModelInstallation> {
    state
        .local_model_manager()?
        .cancel_installation(installation_id)
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
    state
        .local_model_manager()?
        .remove(installation_id)
        .await
        .map_err(graphql_error)?;
    Ok(true)
}

pub(super) async fn activate_local_model(
    state: &GraphqlState,
    installation_id: String,
) -> Result<GraphqlLocalModelInstallation> {
    state
        .local_model_manager()?
        .activate(installation_id)
        .await
        .map(installation_view)
        .map_err(graphql_error)
}

pub(super) async fn save_default_model_preference(
    state: &GraphqlState,
    input: GraphqlSaveDefaultModelPreferenceInput,
) -> Result<GraphqlDefaultModelPreference> {
    if ProviderKind::from_str(input.provider_kind.trim()) == Ok(ProviderKind::LocalModels) {
        return Err(async_graphql::Error::new(
            "Activate a specific local model installation to change the local default",
        ));
    }
    let reasoning_effort = input
        .reasoning_effort
        .as_deref()
        .map(|value| {
            ReasoningEffort::from_persistence_str(value).ok_or_else(|| {
                async_graphql::Error::new(format!(
                    "invalid default model reasoning effort: {value}"
                ))
            })
        })
        .transpose()?;
    let ready_selection = super::provider_selection::prove_ready_selection(
        state,
        &input.provider_kind,
        &input.provider_account_id,
        &input.model_profile,
        reasoning_effort,
        "graphql_default_model_preference",
    )
    .await?;
    state
        .store()?
        .save_default_model_preference_with_ready_selection(
            &input.provider_kind,
            &input.provider_account_id,
            &input.model_profile,
            input.reasoning_effort.as_deref(),
            &ready_selection,
        )
        .await
        .map(default_preference_view)
        .map_err(graphql_error)
}

pub(super) async fn retry_local_model_runtime(
    state: &GraphqlState,
) -> Result<GraphqlLocalModelRuntimeStatus> {
    state
        .local_model_manager()?
        .retry_active_installation()
        .await
        .map(runtime_status_view)
        .map_err(graphql_error)
}

pub(super) async fn local_model_events(
    state: &GraphqlState,
    after: Option<String>,
) -> Result<Pin<Box<dyn Stream<Item = Result<GraphqlLocalModelEvent>> + Send>>> {
    let stream = state
        .local_model_manager()?
        .subscribe_events(after)
        .await
        .map_err(graphql_error)?;
    Ok(Box::pin(stream.map(|event| {
        event.map(manager_event_view).map_err(graphql_error)
    })))
}

fn manager_event_view(
    record: noema_providers::LocalModelManagerEventRecord,
) -> GraphqlLocalModelEvent {
    match record.payload {
        noema_providers::LocalModelManagerEvent::Durable {
            event,
            installation,
        } => {
            let mut view = event_view(
                event,
                installation.map(|installation| installation_view(*installation)),
            );
            view.cursor = record.cursor;
            view
        }
        noema_providers::LocalModelManagerEvent::RuntimeChanged { status } => {
            GraphqlLocalModelEvent {
                cursor: record.cursor,
                kind: GraphqlLocalModelEventKind::RuntimeChanged,
                installation_id: None,
                model_id: None,
                installation: None,
                runtime_status: Some(runtime_status_view(status)),
                created_at: runtime_event_time(),
            }
        }
    }
}

fn runtime_event_time() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}
