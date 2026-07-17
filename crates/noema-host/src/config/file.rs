use super::error::ConfigError;
use figment::{
    Figment,
    providers::{Format, Yaml},
};
use noema_providers::ReasoningEffort;
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FileConfig {
    provider: Option<String>,
    model: Option<String>,
    reasoning_effort: Option<ReasoningEffort>,
    tool_classification_model: Option<String>,
    openai: FileOpenAiConfig,
    codex: FileCodexConfig,
    foundation_local: FileFoundationLocalConfig,
    local_models: FileLocalModelsConfig,
    web: FileWebConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FileOpenAiConfig {
    base_url: Option<String>,
    organization_id: Option<String>,
    project_id: Option<String>,
    tool_classification_model: Option<String>,
    timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FileCodexConfig {
    base_url: Option<String>,
    model: Option<String>,
    reasoning_effort: Option<ReasoningEffort>,
    tool_classification_model: Option<String>,
    timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FileFoundationLocalConfig {
    default_profile: Option<String>,
    bridge_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FileLocalModelsConfig {
    default_model: Option<String>,
    preferred_backend: Option<String>,
    context_window_tokens: Option<u32>,
    timeout_seconds: Option<u64>,
    startup_timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FileWebConfig {
    host: Option<String>,
    port: Option<u16>,
}

pub(super) fn validate_file_config(path: &Path) -> Result<(), ConfigError> {
    Figment::from(Yaml::file(path))
        .extract::<FileConfig>()
        .map(|_| ())
        .map_err(|source| ConfigError::ParseConfig {
            path: path.to_path_buf(),
            source: Box::new(source),
        })
}
