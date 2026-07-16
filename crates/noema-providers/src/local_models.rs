//! Durable local-model installation vocabulary shared by storage and runtime.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Stable built-in provider account used for local GGUF inference.
pub const LOCAL_MODELS_PROVIDER_ACCOUNT_ID: &str = "provider_account:local_models:default";

/// Invalid stable local-model vocabulary value.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("invalid {kind}: {value}")]
pub struct LocalModelCodecError {
    kind: &'static str,
    value: String,
}

/// A backend supported by the bundled local inference runtime.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, Hash, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LocalModelBackend {
    /// Apple Metal acceleration.
    Metal,
    /// NVIDIA CUDA acceleration.
    Cuda,
    /// Vulkan acceleration.
    Vulkan,
    /// Portable CPU inference.
    Cpu,
}

impl LocalModelBackend {
    /// Return the product-facing backend name.
    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Metal => "Metal",
            Self::Cuda => "CUDA",
            Self::Vulkan => "Vulkan",
            Self::Cpu => "CPU",
        }
    }

    /// Return the stable persistence value.
    #[must_use]
    pub const fn as_persistence_str(self) -> &'static str {
        match self {
            Self::Metal => "metal",
            Self::Cuda => "cuda",
            Self::Vulkan => "vulkan",
            Self::Cpu => "cpu",
        }
    }

    /// Parse a stable persistence value.
    #[must_use]
    pub fn from_persistence_str(value: &str) -> Option<Self> {
        match value {
            "metal" => Some(Self::Metal),
            "cuda" => Some(Self::Cuda),
            "vulkan" => Some(Self::Vulkan),
            "cpu" => Some(Self::Cpu),
            _ => None,
        }
    }
}

impl fmt::Display for LocalModelBackend {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_persistence_str())
    }
}

impl FromStr for LocalModelBackend {
    type Err = LocalModelCodecError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::from_persistence_str(value).ok_or_else(|| LocalModelCodecError {
            kind: "local_model_backend",
            value: value.to_string(),
        })
    }
}

/// Provenance class for an installed model artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalModelSourceKind {
    /// A qualified artifact from Noema's bundled catalog.
    Catalog,
    /// An advanced public Hugging Face GGUF import.
    HuggingFace,
    /// An advanced local-file import.
    LocalFile,
}

impl LocalModelSourceKind {
    /// Return the stable persistence value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Catalog => "catalog",
            Self::HuggingFace => "hugging_face",
            Self::LocalFile => "local_file",
        }
    }

    /// Parse a stable persistence value.
    #[must_use]
    pub fn from_persistence_str(value: &str) -> Option<Self> {
        match value {
            "catalog" => Some(Self::Catalog),
            "hugging_face" => Some(Self::HuggingFace),
            "local_file" => Some(Self::LocalFile),
            _ => None,
        }
    }
}

impl fmt::Display for LocalModelSourceKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for LocalModelSourceKind {
    type Err = LocalModelCodecError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::from_persistence_str(value).ok_or_else(|| LocalModelCodecError {
            kind: "local_model_source_kind",
            value: value.to_string(),
        })
    }
}

/// Durable lifecycle state for one local-model installation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalModelInstallationStatus {
    /// Installation metadata exists but transfer has not started.
    Queued,
    /// Artifact bytes are being transferred or resumed.
    Downloading,
    /// The complete artifact is being checksum-verified.
    Verifying,
    /// The verified content-addressed blob is ready for inference.
    Installed,
    /// Installation stopped because of an error.
    Failed,
    /// Installation was cancelled by the user.
    Cancelled,
}

impl LocalModelInstallationStatus {
    /// Return the stable persistence value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Downloading => "downloading",
            Self::Verifying => "verifying",
            Self::Installed => "installed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    /// Parse a stable persistence value.
    #[must_use]
    pub fn from_persistence_str(value: &str) -> Option<Self> {
        match value {
            "queued" => Some(Self::Queued),
            "downloading" => Some(Self::Downloading),
            "verifying" => Some(Self::Verifying),
            "installed" => Some(Self::Installed),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }

    /// Return whether the durable lifecycle permits the requested transition.
    #[must_use]
    pub fn can_transition_to(self, next: Self) -> bool {
        self == next
            || matches!(
                (self, next),
                (
                    Self::Queued,
                    Self::Downloading | Self::Verifying | Self::Failed | Self::Cancelled
                ) | (
                    Self::Downloading,
                    Self::Verifying | Self::Failed | Self::Cancelled
                ) | (
                    Self::Verifying,
                    Self::Installed | Self::Failed | Self::Cancelled
                )
            )
    }
}

impl fmt::Display for LocalModelInstallationStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for LocalModelInstallationStatus {
    type Err = LocalModelCodecError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::from_persistence_str(value).ok_or_else(|| LocalModelCodecError {
            kind: "local_model_installation_status",
            value: value.to_string(),
        })
    }
}

/// Cursor-bearing installation event kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalModelEventKind {
    /// A new installation was queued.
    Queued,
    /// Transfer progress changed.
    Progress,
    /// Checksum verification began.
    Verifying,
    /// Atomic installation completed.
    Installed,
    /// Installation failed.
    Failed,
    /// Installation was cancelled.
    Cancelled,
    /// Installation metadata and its unreferenced blob were removed.
    Removed,
    /// The installation became Noema's active default model.
    Activated,
}

impl LocalModelEventKind {
    /// Return the stable persistence value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Progress => "progress",
            Self::Verifying => "verifying",
            Self::Installed => "installed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Removed => "removed",
            Self::Activated => "activated",
        }
    }

    /// Parse a stable persistence value.
    #[must_use]
    pub fn from_persistence_str(value: &str) -> Option<Self> {
        match value {
            "queued" => Some(Self::Queued),
            "progress" => Some(Self::Progress),
            "verifying" => Some(Self::Verifying),
            "installed" => Some(Self::Installed),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            "removed" => Some(Self::Removed),
            "activated" => Some(Self::Activated),
            _ => None,
        }
    }
}

impl fmt::Display for LocalModelEventKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for LocalModelEventKind {
    type Err = LocalModelCodecError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::from_persistence_str(value).ok_or_else(|| LocalModelCodecError {
            kind: "local_model_event_kind",
            value: value.to_string(),
        })
    }
}

/// Input for creating or refreshing installation provenance.
#[derive(Clone, Debug, PartialEq)]
pub struct NewLocalModelInstallation {
    /// Stable installation identity.
    pub installation_id: String,
    /// Provider-facing model profile.
    pub model_id: String,
    /// Product-facing model name.
    pub display_name: String,
    /// Artifact provenance class.
    pub source_kind: LocalModelSourceKind,
    /// Source Hugging Face repository, when applicable.
    pub source_repo: Option<String>,
    /// Immutable source revision, when applicable.
    pub source_revision: Option<String>,
    /// Source artifact filename or imported local filename.
    pub source_file: Option<String>,
    /// Expected lowercase SHA-256 digest.
    pub sha256: Option<String>,
    /// Rounded decimal download size shown in the UI.
    pub download_gb: f64,
    /// Exact expected transfer size when known.
    pub expected_bytes: Option<u64>,
    /// License shown to the user, when known.
    pub license: Option<String>,
    /// Backend selected for this artifact.
    pub backend: LocalModelBackend,
}

/// Persisted installation state and provenance.
#[derive(Clone, Debug, PartialEq)]
pub struct LocalModelInstallationRecord {
    /// Stable installation identity.
    pub installation_id: String,
    /// Provider-facing model profile.
    pub model_id: String,
    /// Product-facing model name.
    pub display_name: String,
    /// Artifact provenance class.
    pub source_kind: LocalModelSourceKind,
    /// Source Hugging Face repository, when applicable.
    pub source_repo: Option<String>,
    /// Immutable source revision, when applicable.
    pub source_revision: Option<String>,
    /// Source artifact filename or imported local filename.
    pub source_file: Option<String>,
    /// Verified lowercase SHA-256 digest.
    pub sha256: Option<String>,
    /// Rounded decimal download size shown in the UI.
    pub download_gb: f64,
    /// Exact expected transfer size when known.
    pub expected_bytes: Option<u64>,
    /// Persisted transferred byte count.
    pub downloaded_bytes: u64,
    /// License shown to the user, when known.
    pub license: Option<String>,
    /// Backend selected for this artifact.
    pub backend: LocalModelBackend,
    /// Current installation lifecycle state.
    pub status: LocalModelInstallationStatus,
    /// Noema-home-relative verified blob path.
    pub blob_relative_path: Option<String>,
    /// Whether this installation is Noema's active default local model.
    pub is_active: bool,
    /// Stable failure code, when failed.
    pub error_code: Option<String>,
    /// Human-readable failure detail, when failed.
    pub error_message: Option<String>,
    /// Installation completion timestamp.
    pub installed_at: Option<String>,
    /// Creation timestamp.
    pub created_at: String,
    /// Last update timestamp.
    pub updated_at: String,
}

/// Atomic installation-state update persisted alongside an event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalModelInstallationUpdate {
    /// New lifecycle status.
    pub status: LocalModelInstallationStatus,
    /// Transferred byte count.
    pub downloaded_bytes: u64,
    /// Exact expected byte count, when learned from HTTP metadata.
    pub expected_bytes: Option<u64>,
    /// Verified digest learned while importing an advanced local file.
    pub sha256: Option<String>,
    /// Noema-home-relative blob path after successful verification.
    pub blob_relative_path: Option<String>,
    /// Stable error code for terminal failures.
    pub error_code: Option<String>,
    /// Human-readable error detail for terminal failures.
    pub error_message: Option<String>,
}

/// One cursor-bearing installation event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalModelEventRecord {
    /// Monotonic SQLite cursor.
    pub cursor: u64,
    /// Installation associated with this event.
    pub installation_id: String,
    /// Event kind.
    pub kind: LocalModelEventKind,
    /// Transferred byte count at this event.
    pub downloaded_bytes: Option<u64>,
    /// Exact expected byte count at this event.
    pub expected_bytes: Option<u64>,
    /// Optional detail for failures or runtime presentation.
    pub message: Option<String>,
    /// Event timestamp.
    pub created_at: String,
}

/// Noema-wide default model selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DefaultModelPreferenceRecord {
    /// Provider family.
    pub provider_kind: String,
    /// Provider account identity.
    pub provider_account_id: String,
    /// Provider-facing model profile.
    pub model_profile: String,
    /// Optional provider reasoning-effort string.
    pub reasoning_effort: Option<String>,
    /// Last update timestamp.
    pub updated_at: String,
}

/// Result of removing installation metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct RemovedLocalModelInstallation {
    /// Projection that was removed.
    pub installation: LocalModelInstallationRecord,
    /// Blob path that may be deleted after the transaction, when unreferenced.
    pub unreferenced_blob_relative_path: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{
        LocalModelBackend as Backend, LocalModelEventKind as EventKind,
        LocalModelInstallationStatus as Status, LocalModelSourceKind as SourceKind,
    };

    #[test]
    fn cancelled_and_installed_installations_reject_worker_state_regression() {
        assert!(Status::Downloading.can_transition_to(Status::Cancelled));
        assert!(!Status::Cancelled.can_transition_to(Status::Installed));
        assert!(!Status::Installed.can_transition_to(Status::Cancelled));
    }

    #[test]
    fn local_model_persistence_codecs_round_trip_exactly() {
        for (value, encoded) in [
            (Backend::Metal, "metal"),
            (Backend::Cuda, "cuda"),
            (Backend::Vulkan, "vulkan"),
            (Backend::Cpu, "cpu"),
        ] {
            assert_eq!(value.as_persistence_str(), encoded);
            assert_eq!(encoded.parse::<Backend>().unwrap(), value);
        }
        for (value, encoded) in [
            (SourceKind::Catalog, "catalog"),
            (SourceKind::HuggingFace, "hugging_face"),
            (SourceKind::LocalFile, "local_file"),
        ] {
            assert_eq!(value.as_str(), encoded);
            assert_eq!(encoded.parse::<SourceKind>().unwrap(), value);
        }
        for (value, encoded) in [
            (Status::Queued, "queued"),
            (Status::Downloading, "downloading"),
            (Status::Verifying, "verifying"),
            (Status::Installed, "installed"),
            (Status::Failed, "failed"),
            (Status::Cancelled, "cancelled"),
        ] {
            assert_eq!(value.as_str(), encoded);
            assert_eq!(encoded.parse::<Status>().unwrap(), value);
        }
        for (value, encoded) in [
            (EventKind::Queued, "queued"),
            (EventKind::Progress, "progress"),
            (EventKind::Verifying, "verifying"),
            (EventKind::Installed, "installed"),
            (EventKind::Failed, "failed"),
            (EventKind::Cancelled, "cancelled"),
            (EventKind::Removed, "removed"),
            (EventKind::Activated, "activated"),
        ] {
            assert_eq!(value.as_str(), encoded);
            assert_eq!(encoded.parse::<EventKind>().unwrap(), value);
        }

        assert!("unknown".parse::<Backend>().is_err());
        assert!("unknown".parse::<SourceKind>().is_err());
        assert!("unknown".parse::<Status>().is_err());
        assert!("unknown".parse::<EventKind>().is_err());
    }
}
