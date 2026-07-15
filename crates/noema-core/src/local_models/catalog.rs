use std::collections::HashSet;

use serde::Deserialize;
use thiserror::Error;

const BUNDLED_CATALOG: &str = include_str!("../../resources/local-models/catalog.toml");

/// A backend supported by the bundled local inference runtime.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq)]
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
    /// Returns the product-facing backend name.
    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Metal => "Metal",
            Self::Cuda => "CUDA",
            Self::Vulkan => "Vulkan",
            Self::Cpu => "CPU",
        }
    }
}

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

    const fn accelerator_memory_gb(self) -> Option<u64> {
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
    fn fits(&self, hardware: LocalHardwareProfile) -> bool {
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

/// The validated bundled local-model catalog.
#[derive(Clone, Debug, PartialEq)]
pub struct LocalModelCatalog {
    models: Vec<LocalModelCatalogEntry>,
}

impl LocalModelCatalog {
    /// Parses and validates a catalog from TOML.
    ///
    /// # Errors
    ///
    /// Returns an error when TOML cannot be decoded or any catalog invariant is
    /// violated.
    pub fn parse(source: &str) -> Result<Self, LocalModelCatalogError> {
        let raw = toml::from_str::<RawCatalog>(source)?;
        Self::try_from(raw)
    }

    /// Loads the catalog bundled with this exact Noema runtime.
    ///
    /// # Errors
    ///
    /// Returns an error if the bundled resource and parser disagree.
    pub fn bundled() -> Result<Self, LocalModelCatalogError> {
        Self::parse(BUNDLED_CATALOG)
    }

    /// Returns catalog entries in their maintainer-defined order.
    #[must_use]
    pub fn models(&self) -> &[LocalModelCatalogEntry] {
        &self.models
    }

    /// Recommends the highest-priority model fitting one backend profile.
    #[must_use]
    pub fn recommend(
        &self,
        hardware: LocalHardwareProfile,
    ) -> Option<LocalModelRecommendation<'_>> {
        self.models
            .iter()
            .enumerate()
            .filter_map(|(catalog_index, model)| {
                model
                    .builds
                    .iter()
                    .find(|build| build.fits(hardware))
                    .map(|build| (catalog_index, model, build))
            })
            .min_by_key(|(catalog_index, model, _)| (u32::MAX - model.priority, *catalog_index))
            .map(|(_, model, build)| LocalModelRecommendation {
                model,
                build,
                hardware,
            })
    }

    /// Tries usable backends in preference order and falls back when no build
    /// fits an earlier backend.
    #[must_use]
    pub fn recommend_with_fallback(
        &self,
        hardware: &[LocalHardwareProfile],
    ) -> Option<LocalModelRecommendation<'_>> {
        self.models
            .iter()
            .enumerate()
            .filter_map(|(catalog_index, model)| {
                select_model_build(model, hardware, None)
                    .map(|recommendation| (catalog_index, model.priority, recommendation))
            })
            .min_by_key(|(catalog_index, priority, _)| (u32::MAX - priority, *catalog_index))
            .map(|(_, _, recommendation)| recommendation)
    }

    /// Selects the first compatible build for one catalog model while trying
    /// usable backends in preference order.
    #[must_use]
    pub fn select_build(
        &self,
        model_id: &str,
        hardware: &[LocalHardwareProfile],
    ) -> Option<LocalModelRecommendation<'_>> {
        let model = self.models.iter().find(|model| model.id == model_id)?;
        select_model_build(model, hardware, None)
    }

    /// Selects a named artifact when it fits one of the usable backends.
    #[must_use]
    pub fn select_named_build(
        &self,
        model_id: &str,
        file: &str,
        hardware: &[LocalHardwareProfile],
    ) -> Option<LocalModelRecommendation<'_>> {
        let model = self.models.iter().find(|model| model.id == model_id)?;
        select_model_build(model, hardware, Some(file))
    }
}

fn select_model_build<'a>(
    model: &'a LocalModelCatalogEntry,
    hardware: &[LocalHardwareProfile],
    file: Option<&str>,
) -> Option<LocalModelRecommendation<'a>> {
    hardware.iter().find_map(|profile| {
        model
            .builds
            .iter()
            .find(|build| file.is_none_or(|file| build.file == file) && build.fits(*profile))
            .map(|build| LocalModelRecommendation {
                model,
                build,
                hardware: *profile,
            })
    })
}

/// A model/build recommendation together with the matched hardware values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LocalModelRecommendation<'a> {
    /// Selected curated model.
    pub model: &'a LocalModelCatalogEntry,
    /// Selected build for the matched backend.
    pub build: &'a LocalModelBuild,
    /// Hardware values that satisfied the build thresholds.
    pub hardware: LocalHardwareProfile,
}

impl LocalModelRecommendation<'_> {
    /// Generates the recommendation explanation from matched catalog and
    /// hardware data.
    #[must_use]
    pub fn explanation(&self) -> String {
        let memory = if self.hardware.unified_memory {
            format!("{} GB of unified memory", self.hardware.ram_gb)
        } else if self.hardware.backend == LocalModelBackend::Cpu {
            format!("{} GB of system memory", self.hardware.ram_gb)
        } else {
            format!(
                "{} GB of accelerator memory",
                self.hardware.vram_gb.unwrap_or_default()
            )
        };

        format!(
            "Recommended because {} fits your {} backend and {memory}.",
            self.model.name,
            self.hardware.backend.display_name()
        )
    }
}

/// Catalog parsing and invariant errors.
#[derive(Debug, Error)]
pub enum LocalModelCatalogError {
    /// TOML decoding failed, including unknown fields and invalid backends.
    #[error("invalid local-model catalog TOML: {0}")]
    Toml(#[from] toml::de::Error),
    /// Two catalog entries use the same stable model identifier.
    #[error("duplicate local-model id `{0}`")]
    DuplicateModelId(String),
    /// Two builds under one model use the same artifact filename.
    #[error("model `{model_id}` has duplicate build `{file}`")]
    DuplicateBuild {
        /// Model containing the duplicate build.
        model_id: String,
        /// Duplicated artifact filename.
        file: String,
    },
    /// A catalog string field is empty or malformed.
    #[error("model `{model_id}` has invalid {field} `{value}`")]
    InvalidModelField {
        /// Model containing the invalid value.
        model_id: String,
        /// Invalid field name.
        field: &'static str,
        /// Invalid field value.
        value: String,
    },
    /// A model has no downloadable artifacts.
    #[error("model `{0}` must contain at least one build")]
    MissingBuilds(String),
    /// A build field is empty or malformed.
    #[error("model `{model_id}` build `{file}` has invalid {field} `{value}`")]
    InvalidBuildField {
        /// Model containing the invalid build.
        model_id: String,
        /// Build filename, or a placeholder when the filename itself is invalid.
        file: String,
        /// Invalid field name.
        field: &'static str,
        /// Invalid field value.
        value: String,
    },
    /// A build declares memory constraints that cannot be interpreted.
    #[error("model `{model_id}` build `{file}` has impossible memory requirements: {reason}")]
    ImpossibleMemory {
        /// Model containing the invalid build.
        model_id: String,
        /// Invalid build filename.
        file: String,
        /// Human-readable invariant violation.
        reason: &'static str,
    },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCatalog {
    models: Vec<RawModel>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawModel {
    id: String,
    name: String,
    license: String,
    priority: u32,
    repo: String,
    revision: String,
    #[serde(default)]
    builds: Vec<RawBuild>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBuild {
    file: String,
    sha256: String,
    download_gb: f64,
    backends: Vec<LocalModelBackend>,
    min_ram_gb: u64,
    min_vram_gb: Option<u64>,
}

impl TryFrom<RawCatalog> for LocalModelCatalog {
    type Error = LocalModelCatalogError;

    fn try_from(raw: RawCatalog) -> Result<Self, Self::Error> {
        let mut ids = HashSet::with_capacity(raw.models.len());
        let mut models = Vec::with_capacity(raw.models.len());

        for model in raw.models {
            validate_model(&model)?;
            if !ids.insert(model.id.clone()) {
                return Err(LocalModelCatalogError::DuplicateModelId(model.id));
            }

            let mut filenames = HashSet::with_capacity(model.builds.len());
            let mut builds = Vec::with_capacity(model.builds.len());
            for build in model.builds {
                validate_build(&model.id, &build)?;
                if !filenames.insert(build.file.clone()) {
                    return Err(LocalModelCatalogError::DuplicateBuild {
                        model_id: model.id,
                        file: build.file,
                    });
                }
                builds.push(LocalModelBuild {
                    file: build.file,
                    sha256: build.sha256,
                    download_gb: build.download_gb,
                    backends: build.backends,
                    min_ram_gb: build.min_ram_gb,
                    min_vram_gb: build.min_vram_gb,
                });
            }

            models.push(LocalModelCatalogEntry {
                id: model.id,
                name: model.name,
                license: model.license,
                priority: model.priority,
                repo: model.repo,
                revision: model.revision,
                builds,
            });
        }

        Ok(Self { models })
    }
}

fn validate_model(model: &RawModel) -> Result<(), LocalModelCatalogError> {
    if model.id.is_empty()
        || !model
            .id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(invalid_model_field(model, "id", &model.id));
    }
    if model.name.trim().is_empty() {
        return Err(invalid_model_field(model, "name", &model.name));
    }
    if model.license.trim().is_empty() {
        return Err(invalid_model_field(model, "license", &model.license));
    }
    if !valid_repository(&model.repo) {
        return Err(invalid_model_field(model, "repo", &model.repo));
    }
    if model.revision.len() != 40 || !model.revision.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(invalid_model_field(model, "revision", &model.revision));
    }
    if model.builds.is_empty() {
        return Err(LocalModelCatalogError::MissingBuilds(model.id.clone()));
    }
    Ok(())
}

fn validate_build(model_id: &str, build: &RawBuild) -> Result<(), LocalModelCatalogError> {
    let file_label = if build.file.is_empty() {
        "<empty>"
    } else {
        build.file.as_str()
    };
    if build.file.contains(['/', '\\'])
        || !build.file.to_ascii_lowercase().ends_with(".gguf")
        || build.file.len() <= ".gguf".len()
    {
        return Err(invalid_build_field(
            model_id,
            file_label,
            "file",
            &build.file,
        ));
    }
    if build.sha256.len() != 64
        || !build
            .sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(invalid_build_field(
            model_id,
            file_label,
            "sha256",
            &build.sha256,
        ));
    }
    if !build.download_gb.is_finite() || build.download_gb <= 0.0 {
        return Err(invalid_build_field(
            model_id,
            file_label,
            "download_gb",
            &build.download_gb.to_string(),
        ));
    }
    if build.backends.is_empty() {
        return Err(invalid_build_field(model_id, file_label, "backends", "[]"));
    }
    let unique_backends = build.backends.iter().copied().collect::<HashSet<_>>();
    if unique_backends.len() != build.backends.len() {
        return Err(invalid_build_field(
            model_id,
            file_label,
            "backends",
            "duplicate backend",
        ));
    }
    if build.min_ram_gb == 0 {
        return Err(LocalModelCatalogError::ImpossibleMemory {
            model_id: model_id.to_owned(),
            file: build.file.clone(),
            reason: "min_ram_gb must be greater than zero",
        });
    }
    if build.min_vram_gb == Some(0) {
        return Err(LocalModelCatalogError::ImpossibleMemory {
            model_id: model_id.to_owned(),
            file: build.file.clone(),
            reason: "min_vram_gb must be greater than zero when present",
        });
    }
    if build.backends.contains(&LocalModelBackend::Cpu) && build.min_vram_gb.is_some() {
        return Err(LocalModelCatalogError::ImpossibleMemory {
            model_id: model_id.to_owned(),
            file: build.file.clone(),
            reason: "CPU builds cannot require accelerator memory",
        });
    }
    if build
        .backends
        .iter()
        .any(|backend| *backend != LocalModelBackend::Cpu)
        && build.min_vram_gb.is_none()
    {
        return Err(LocalModelCatalogError::ImpossibleMemory {
            model_id: model_id.to_owned(),
            file: build.file.clone(),
            reason: "accelerated builds must declare min_vram_gb",
        });
    }
    Ok(())
}

fn invalid_model_field(
    model: &RawModel,
    field: &'static str,
    value: &str,
) -> LocalModelCatalogError {
    LocalModelCatalogError::InvalidModelField {
        model_id: model.id.clone(),
        field,
        value: value.to_owned(),
    }
}

fn invalid_build_field(
    model_id: &str,
    file: &str,
    field: &'static str,
    value: &str,
) -> LocalModelCatalogError {
    LocalModelCatalogError::InvalidBuildField {
        model_id: model_id.to_owned(),
        file: file.to_owned(),
        field,
        value: value.to_owned(),
    }
}

fn valid_repository(repo: &str) -> bool {
    let mut parts = repo.split('/');
    let Some(owner) = parts.next() else {
        return false;
    };
    let Some(name) = parts.next() else {
        return false;
    };
    parts.next().is_none() && valid_repository_part(owner) && valid_repository_part(name)
}

fn valid_repository_part(part: &str) -> bool {
    !part.is_empty()
        && part
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}
