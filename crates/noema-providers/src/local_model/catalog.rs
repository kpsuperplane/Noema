//! Provider-facing local-model catalog projections.

use super::LocalModelBackend;

/// Hardware values used to decide whether a catalog build fits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalHardwareProfile {
    /// Backend usable on this machine.
    pub backend: LocalModelBackend,
    /// Available system or unified memory, in whole gigabytes.
    pub ram_gb: u64,
    /// Available discrete accelerator memory, in whole gigabytes.
    pub vram_gb: Option<u64>,
    /// Whether accelerator memory shares the system-memory pool.
    pub unified_memory: bool,
}

impl LocalHardwareProfile {
    /// Creates a hardware profile for one usable backend.
    #[must_use]
    pub const fn new(
        backend: LocalModelBackend,
        ram_gb: u64,
        vram_gb: Option<u64>,
        unified_memory: bool,
    ) -> Self {
        Self {
            backend,
            ram_gb,
            vram_gb,
            unified_memory,
        }
    }

    #[cfg(feature = "local-models")]
    pub(crate) const fn accelerator_memory_gb(self) -> Option<u64> {
        if self.unified_memory {
            Some(self.ram_gb)
        } else {
            self.vram_gb
        }
    }
}

/// One downloadable GGUF build for a curated model.
#[derive(Clone, Debug, PartialEq)]
pub struct LocalModelBuild {
    /// Artifact filename in the pinned Hugging Face repository.
    pub file: String,
    /// Expected lowercase hexadecimal SHA-256 digest.
    pub sha256: String,
    /// Rounded download size displayed to the user, in decimal gigabytes.
    pub download_gb: f64,
    /// Runtime backends compatible with this artifact.
    pub backends: Vec<LocalModelBackend>,
    /// Minimum system or unified memory, in whole gigabytes.
    pub min_ram_gb: u64,
    /// Minimum accelerator memory, in whole gigabytes.
    pub min_vram_gb: Option<u64>,
}

impl LocalModelBuild {
    #[cfg(feature = "local-models")]
    pub(crate) fn fits(&self, hardware: LocalHardwareProfile) -> bool {
        if !self.backends.contains(&hardware.backend) || hardware.ram_gb < self.min_ram_gb {
            return false;
        }

        self.min_vram_gb.is_none_or(|minimum| {
            hardware
                .accelerator_memory_gb()
                .is_some_and(|available| available >= minimum)
        })
    }
}

/// A curated model and its pinned downloadable builds.
#[derive(Clone, Debug, PartialEq)]
pub struct LocalModelCatalogEntry {
    /// Stable Noema model identifier.
    pub id: String,
    /// Product-facing model name.
    pub name: String,
    /// SPDX-style license identifier shown before download.
    pub license: String,
    /// Recommendation priority; larger values win.
    pub priority: u32,
    /// Hugging Face repository in `owner/repository` form.
    pub repo: String,
    /// Pinned immutable repository commit.
    pub revision: String,
    /// Curated GGUF artifacts for this model.
    pub builds: Vec<LocalModelBuild>,
}

/// Bundled catalog entries together with the machine profiles used for selection.
#[derive(Clone, Debug, PartialEq)]
pub struct LocalModelCatalogSnapshot {
    /// Catalog projections in maintainer-defined order.
    pub entries: Vec<LocalModelCatalogSnapshotEntry>,
}

/// One catalog entry with manager-owned compatibility and recommendation results.
#[derive(Clone, Debug, PartialEq)]
pub struct LocalModelCatalogSnapshotEntry {
    /// Validated bundled model and all downloadable builds.
    pub model: LocalModelCatalogEntry,
    /// Best compatible build for this machine, when one fits.
    pub selected_build: Option<LocalModelBuild>,
    /// Hardware values that selected the compatible build.
    pub compatible_hardware: Option<LocalHardwareProfile>,
    /// Explanation derived from the matched model, build, and hardware.
    pub compatibility_explanation: Option<String>,
    /// Whether this model is the top compatible recommendation.
    pub is_recommended: bool,
}
