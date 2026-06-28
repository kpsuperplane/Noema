//! Configuration loading and provider selection.

use crate::providers::{
    codex_oauth::DEFAULT_CODEX_BASE_URL,
    codex_responses::{CodexProviderConfig, DEFAULT_CODEX_MODEL, DEFAULT_CODEX_TIMEOUT_SECONDS},
    openai::{DEFAULT_OPENAI_TIMEOUT_SECONDS, OpenAiProviderConfig},
};
use crate::{NOEMA_HOME_ENV, NoemaPathError, NoemaPaths};
use figment::{
    Figment,
    providers::{Env, Format, Serialized, Yaml},
};
use serde::{Deserialize, Serialize};
use std::{
    env,
    path::{Path, PathBuf},
    str::FromStr,
};
use thiserror::Error;

/// Default provider used when config does not specify one.
pub const DEFAULT_PROVIDER: &str = "openai";
/// Default `OpenAI` model used when no model override is supplied.
pub const DEFAULT_OPENAI_MODEL: &str = "gpt-5.5";
/// Default `OpenAI` API base URL.
pub const DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
/// Default host for the local web UI.
pub const DEFAULT_WEB_HOST: &str = "127.0.0.1";
/// Default port for the local web UI.
pub const DEFAULT_WEB_PORT: u16 = 3737;
/// Environment variable used for `OpenAI` API credentials.
pub const OPENAI_API_KEY_ENV: &str = "NOEMA_OPENAI__API_KEY";

const CONFIG_ENV_KEYS: &[&str] = &[
    "provider",
    "model",
    "openai.api_key",
    "openai.base_url",
    "openai.timeout_seconds",
    "openai.organization_id",
    "openai.project_id",
    "codex.model",
    "codex.base_url",
    "codex.timeout_seconds",
    "web.host",
    "web.port",
];

/// Configuration values supplied directly by the CLI.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CliOverrides {
    /// Provider override, such as `openai` or `codex`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// Model override for the selected provider.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// OpenAI-specific overrides.
    #[serde(skip_serializing_if = "Option::is_none", rename = "openai")]
    pub openai_overrides: Option<CliOpenAiOverrides>,
}

impl CliOverrides {
    /// Build CLI overrides from parsed command-line options.
    #[must_use]
    pub fn new(provider: Option<String>, model: Option<String>, base_url: Option<String>) -> Self {
        Self {
            provider,
            model,
            openai_overrides: base_url.map(|base_url| CliOpenAiOverrides {
                base_url: Some(base_url),
            }),
        }
    }

    /// Return the `OpenAI` base URL override, if present.
    #[must_use]
    pub fn base_url(&self) -> Option<&str> {
        self.openai_overrides
            .as_ref()
            .and_then(|openai| openai.base_url.as_deref())
    }
}

/// OpenAI-specific CLI overrides.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CliOpenAiOverrides {
    /// OpenAI-compatible API base URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

/// Fully resolved configuration used by the runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedConfig {
    /// Selected provider configuration.
    pub provider: ProviderConfig,
    /// Local web UI configuration.
    pub web: WebConfig,
}

/// Configuration for the local web UI served by `noema start`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebConfig {
    /// Host/interface to bind.
    pub host: String,
    /// TCP port to bind.
    pub port: u16,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            host: DEFAULT_WEB_HOST.to_string(),
            port: DEFAULT_WEB_PORT,
        }
    }
}

impl WebConfig {
    /// Return the user-facing URL for this web UI configuration.
    #[must_use]
    pub fn url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }
}

/// Supported provider identifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderKind {
    /// Codex CLI provider.
    Codex,
    /// `OpenAI` Responses API provider.
    OpenAi,
}

impl ProviderKind {
    /// Return the stable config string for this provider.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::OpenAi => "openai",
        }
    }
}

impl FromStr for ProviderKind {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "codex" => Ok(Self::Codex),
            "openai" => Ok(Self::OpenAi),
            other => Err(other.to_string()),
        }
    }
}

/// Concrete configuration for the selected provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderConfig {
    /// Codex provider configuration.
    Codex(CodexProviderConfig),
    /// `OpenAI` provider configuration.
    OpenAi(OpenAiProviderConfig),
}

impl ProviderConfig {
    /// Return the provider kind for this configuration.
    #[must_use]
    pub fn kind(&self) -> ProviderKind {
        match self {
            Self::Codex(_) => ProviderKind::Codex,
            Self::OpenAi(_) => ProviderKind::OpenAi,
        }
    }

    /// Return the default model configured for this provider.
    #[must_use]
    pub fn model(&self) -> Option<&str> {
        match self {
            Self::Codex(config) => config.default_model.as_deref(),
            Self::OpenAi(config) => Some(config.default_model.as_str()),
        }
    }
}

/// Configuration loader.
#[derive(Debug, Clone, Default)]
pub struct Config;

impl Config {
    /// Load the configured provider from defaults, config file, environment, and CLI overrides.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when path resolution fails, the config file is
    /// missing or invalid, environment values are invalid, credentials are
    /// missing for the selected provider, or the provider is unsupported.
    pub fn load(
        path_override: Option<PathBuf>,
        cli: CliOverrides,
    ) -> Result<ResolvedConfig, ConfigError> {
        let raw = load_raw_config(path_override, cli)?;
        raw.resolve()
    }

    /// Load the Codex provider configuration without requiring `OpenAI` credentials.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when path resolution fails, the config file is
    /// missing or invalid, environment values are invalid, or Codex-specific
    /// values fail validation.
    pub fn load_codex(
        path_override: Option<PathBuf>,
        cli: CliOverrides,
    ) -> Result<CodexProviderConfig, ConfigError> {
        let raw = load_raw_config(path_override, cli)?;
        raw.resolve_codex_config()
    }

    /// Load daemon configuration without requiring non-daemon provider credentials.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when path resolution fails, the config file is
    /// missing or invalid, or Codex/web settings fail validation.
    pub fn load_daemon(
        path_override: Option<PathBuf>,
        cli: CliOverrides,
    ) -> Result<DaemonResolvedConfig, ConfigError> {
        let raw = load_raw_config(path_override, cli)?;
        raw.resolve_daemon_config()
    }
}

/// Fully resolved configuration needed by the local daemon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonResolvedConfig {
    /// Codex provider configuration used for daemon conversations.
    pub codex: CodexProviderConfig,
    /// Local web UI configuration.
    pub web: WebConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct RawConfig {
    provider: String,
    model: Option<String>,
    openai: RawOpenAiConfig,
    codex: RawCodexConfig,
    web: WebConfig,
}

impl Default for RawConfig {
    fn default() -> Self {
        Self {
            provider: DEFAULT_PROVIDER.to_string(),
            model: None,
            openai: RawOpenAiConfig::default(),
            codex: RawCodexConfig::default(),
            web: WebConfig::default(),
        }
    }
}

impl RawConfig {
    fn resolve(self) -> Result<ResolvedConfig, ConfigError> {
        let provider = ProviderKind::from_str(self.provider.trim()).map_err(|provider| {
            ConfigError::UnsupportedProvider {
                provider: provider.clone(),
            }
        })?;

        let provider = match provider {
            ProviderKind::OpenAi => ProviderConfig::OpenAi(self.resolve_openai_config()?),
            ProviderKind::Codex => ProviderConfig::Codex(self.resolve_codex_config()?),
        };

        Ok(ResolvedConfig {
            provider,
            web: self.web,
        })
    }

    fn resolve_daemon_config(self) -> Result<DaemonResolvedConfig, ConfigError> {
        let codex = self.resolve_codex_config()?;

        Ok(DaemonResolvedConfig {
            codex,
            web: self.web,
        })
    }

    fn resolve_openai_config(&self) -> Result<OpenAiProviderConfig, ConfigError> {
        let model = non_empty_option(self.model.as_deref())
            .unwrap_or(DEFAULT_OPENAI_MODEL)
            .to_string();
        let base_url = non_empty_option(Some(self.openai.base_url.as_str()))
            .unwrap_or(DEFAULT_OPENAI_BASE_URL)
            .trim_end_matches('/')
            .to_string();
        let api_key = non_empty_option(self.openai.api_key.as_deref()).ok_or_else(|| {
            ConfigError::MissingCredential {
                provider: "openai".to_string(),
                credential: OPENAI_API_KEY_ENV.to_string(),
            }
        })?;
        let timeout_seconds =
            require_positive(self.openai.timeout_seconds, "NOEMA_OPENAI__TIMEOUT_SECONDS")?;

        Ok(OpenAiProviderConfig {
            api_key: api_key.to_string(),
            base_url,
            organization_id: non_empty_option(self.openai.organization_id.as_deref())
                .map(ToString::to_string),
            project_id: non_empty_option(self.openai.project_id.as_deref())
                .map(ToString::to_string),
            default_model: model,
            timeout_seconds,
        })
    }

    fn resolve_codex_config(&self) -> Result<CodexProviderConfig, ConfigError> {
        let timeout_seconds =
            require_positive(self.codex.timeout_seconds, "NOEMA_CODEX__TIMEOUT_SECONDS")?;
        let base_url = non_empty_option(Some(self.codex.base_url.as_str()))
            .unwrap_or(DEFAULT_CODEX_BASE_URL)
            .trim_end_matches('/')
            .to_string();

        Ok(CodexProviderConfig {
            base_url,
            default_model: non_empty_option(self.model.as_deref())
                .or_else(|| non_empty_option(self.codex.model.as_deref()))
                .or(Some(DEFAULT_CODEX_MODEL))
                .map(ToString::to_string),
            timeout_seconds,
            account_home: None,
            oauth: Default::default(),
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct RawOpenAiConfig {
    api_key: Option<String>,
    base_url: String,
    organization_id: Option<String>,
    project_id: Option<String>,
    timeout_seconds: u64,
}

impl Default for RawOpenAiConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            base_url: DEFAULT_OPENAI_BASE_URL.to_string(),
            organization_id: None,
            project_id: None,
            timeout_seconds: DEFAULT_OPENAI_TIMEOUT_SECONDS,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct RawCodexConfig {
    base_url: String,
    model: Option<String>,
    timeout_seconds: u64,
}

impl Default for RawCodexConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_CODEX_BASE_URL.to_string(),
            model: Some(DEFAULT_CODEX_MODEL.to_string()),
            timeout_seconds: DEFAULT_CODEX_TIMEOUT_SECONDS,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FileConfig {
    provider: Option<String>,
    model: Option<String>,
    openai: FileOpenAiConfig,
    codex: FileCodexConfig,
    web: FileWebConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FileOpenAiConfig {
    base_url: Option<String>,
    organization_id: Option<String>,
    project_id: Option<String>,
    timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FileCodexConfig {
    base_url: Option<String>,
    model: Option<String>,
    timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FileWebConfig {
    host: Option<String>,
    port: Option<u16>,
}

/// Errors produced while resolving configuration.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// An explicitly requested config file does not exist.
    #[error("config file not found: {}", path.display())]
    ConfigFileNotFound {
        /// Requested config path.
        path: PathBuf,
    },

    /// A config file exists but does not match the supported schema.
    #[error("failed to parse config file {}: {source}", path.display())]
    ParseConfig {
        /// Config path that failed to parse.
        path: PathBuf,
        /// Parser error.
        source: Box<figment::Error>,
    },

    /// Layered configuration could not be extracted.
    #[error("failed to load configuration: {0}")]
    Load(Box<figment::Error>),

    /// The configured provider is not supported.
    #[error("unsupported provider: {provider}")]
    UnsupportedProvider {
        /// Unsupported provider value.
        provider: String,
    },

    /// Required credentials were missing.
    #[error("missing credentials for {provider}: {credential}")]
    MissingCredential {
        /// Provider that needs the credential.
        provider: String,
        /// Missing credential name.
        credential: String,
    },

    /// An integer setting failed validation.
    #[error("invalid integer value for {name}: {value}")]
    InvalidInteger {
        /// Setting name.
        name: String,
        /// Invalid value.
        value: String,
    },

    /// Path resolution failed.
    #[error(transparent)]
    Path(#[from] NoemaPathError),
}

impl From<figment::Error> for ConfigError {
    fn from(source: figment::Error) -> Self {
        Self::Load(Box::new(source))
    }
}

fn load_raw_config(
    path_override: Option<PathBuf>,
    cli: CliOverrides,
) -> Result<RawConfig, ConfigError> {
    load_raw_config_from_sources(
        path_override,
        cli,
        default_config_path()?,
        Figment::from(config_env_provider()),
    )
}

fn load_raw_config_from_sources(
    path_override: Option<PathBuf>,
    cli: CliOverrides,
    default_config_path: Option<PathBuf>,
    env: Figment,
) -> Result<RawConfig, ConfigError> {
    let mut figment = Figment::from(Serialized::defaults(RawConfig::default()));

    if let Some(config_path) = resolved_config_path(path_override, default_config_path)? {
        validate_file_config(&config_path)?;
        figment = figment.merge(Yaml::file(config_path));
    }

    figment = figment.merge(env);
    figment = figment.merge(Serialized::defaults(cli));

    Ok(figment.extract()?)
}

fn resolved_config_path(
    path_override: Option<PathBuf>,
    default_config_path: Option<PathBuf>,
) -> Result<Option<PathBuf>, ConfigError> {
    match path_override {
        Some(path) => {
            if path.exists() {
                Ok(Some(path))
            } else {
                Err(ConfigError::ConfigFileNotFound { path })
            }
        }
        None => Ok(default_config_path.filter(|path| path.exists())),
    }
}

fn default_config_path() -> Result<Option<PathBuf>, ConfigError> {
    let noema_home = env::var_os(NOEMA_HOME_ENV);
    let home = env::var_os("HOME");

    if noema_home.is_none() && home.is_none() {
        return Ok(None);
    }

    Ok(Some(
        NoemaPaths::from_env_values(noema_home, home)?.config_path(),
    ))
}

fn validate_file_config(path: &Path) -> Result<(), ConfigError> {
    Figment::from(Yaml::file(path))
        .extract::<FileConfig>()
        .map(|_| ())
        .map_err(|source| ConfigError::ParseConfig {
            path: path.to_path_buf(),
            source: Box::new(source),
        })
}

fn config_env_provider() -> Env {
    Env::prefixed("NOEMA_")
        .split("__")
        .ignore(&["home"])
        .filter_map(|key| normalize_config_env_key(key.as_str()).map(Into::into))
}

fn normalize_config_env_key(key: &str) -> Option<String> {
    let key = key.to_ascii_lowercase();
    let normalized = key.as_str();

    CONFIG_ENV_KEYS
        .contains(&normalized)
        .then(|| normalized.to_string())
}

fn non_empty_option(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn require_positive(value: u64, name: &str) -> Result<u64, ConfigError> {
    if value == 0 {
        Err(ConfigError::InvalidInteger {
            name: name.to_string(),
            value: value.to_string(),
        })
    } else {
        Ok(value)
    }
}

#[cfg(test)]
mod tests;
