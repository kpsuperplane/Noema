use super::error::ConfigError;
use figment::{
    Figment,
    providers::{Format, Yaml},
};
use serde::Deserialize;
use std::path::Path;

#[derive(Default, Deserialize)]
#[serde(default)]
struct FileSecretPolicy {
    openai: FileOpenAiSecretPolicy,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct FileOpenAiSecretPolicy {
    api_key: Option<serde::de::IgnoredAny>,
}

pub(super) fn validate_file_config(path: &Path) -> Result<(), ConfigError> {
    let policy = Figment::from(Yaml::file(path))
        .extract::<FileSecretPolicy>()
        .map_err(|source| ConfigError::ParseConfig {
            path: path.to_path_buf(),
            source: Box::new(source),
        })?;
    if policy.openai.api_key.is_some() {
        return Err(ConfigError::InvalidConfig {
            message: "openai.api_key must be supplied through NOEMA_OPENAI__API_KEY".to_string(),
        });
    }
    Ok(())
}
