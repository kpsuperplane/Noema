//! Configuration loading and provider selection.

use crate::providers::{
    codex::{
        CodexProviderConfig, DEFAULT_CODEX_STARTUP_TIMEOUT_SECONDS,
        DEFAULT_CODEX_TURN_TIMEOUT_SECONDS,
    },
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
    "codex.command",
    "codex.model",
    "codex.sandbox",
    "codex.ephemeral",
    "codex.ignore_rules",
    "codex.ignore_user_config",
    "codex.startup_timeout_seconds",
    "codex.turn_timeout_seconds",
    "codex.home",
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
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct RawConfig {
    provider: String,
    model: Option<String>,
    openai: RawOpenAiConfig,
    codex: RawCodexConfig,
}

impl Default for RawConfig {
    fn default() -> Self {
        Self {
            provider: DEFAULT_PROVIDER.to_string(),
            model: None,
            openai: RawOpenAiConfig::default(),
            codex: RawCodexConfig::default(),
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

        Ok(ResolvedConfig { provider })
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
        let startup_timeout_seconds = require_positive(
            self.codex.startup_timeout_seconds,
            "NOEMA_CODEX__STARTUP_TIMEOUT_SECONDS",
        )?;
        let turn_timeout_seconds = require_positive(
            self.codex.turn_timeout_seconds,
            "NOEMA_CODEX__TURN_TIMEOUT_SECONDS",
        )?;

        Ok(CodexProviderConfig {
            command: non_empty_option(Some(self.codex.command.as_str()))
                .unwrap_or("codex")
                .to_string(),
            default_model: non_empty_option(self.model.as_deref())
                .or_else(|| non_empty_option(self.codex.model.as_deref()))
                .map(ToString::to_string),
            sandbox: non_empty_option(Some(self.codex.sandbox.as_str()))
                .unwrap_or("read-only")
                .to_string(),
            ephemeral: self.codex.ephemeral,
            ignore_rules: self.codex.ignore_rules,
            ignore_user_config: self.codex.ignore_user_config,
            startup_timeout_seconds,
            turn_timeout_seconds,
            codex_home: non_empty_option(self.codex.home.as_deref()).map(ToString::to_string),
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
    command: String,
    model: Option<String>,
    sandbox: String,
    ephemeral: bool,
    ignore_rules: bool,
    ignore_user_config: bool,
    startup_timeout_seconds: u64,
    turn_timeout_seconds: u64,
    home: Option<String>,
}

impl Default for RawCodexConfig {
    fn default() -> Self {
        let default = CodexProviderConfig::default();
        Self {
            command: default.command,
            model: default.default_model,
            sandbox: default.sandbox,
            ephemeral: default.ephemeral,
            ignore_rules: default.ignore_rules,
            ignore_user_config: default.ignore_user_config,
            startup_timeout_seconds: DEFAULT_CODEX_STARTUP_TIMEOUT_SECONDS,
            turn_timeout_seconds: DEFAULT_CODEX_TURN_TIMEOUT_SECONDS,
            home: default.codex_home,
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
    command: Option<String>,
    model: Option<String>,
    sandbox: Option<String>,
    ephemeral: Option<bool>,
    ignore_rules: Option<bool>,
    ignore_user_config: Option<bool>,
    startup_timeout_seconds: Option<u64>,
    turn_timeout_seconds: Option<u64>,
    home: Option<String>,
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
        .only(CONFIG_ENV_KEYS)
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
mod tests {
    use super::*;
    use serde_json::{Number, Value};
    use tempfile::NamedTempFile;

    fn write_config(contents: &str) -> NamedTempFile {
        let file = NamedTempFile::new().expect("temp config");
        std::fs::write(file.path(), contents).expect("write config");
        file
    }

    fn load_resolved(
        path_override: Option<PathBuf>,
        cli: CliOverrides,
        default_config_path: Option<PathBuf>,
        env: &[(&str, &str)],
    ) -> Result<ResolvedConfig, ConfigError> {
        load_raw_config_from_sources(path_override, cli, default_config_path, test_env(env))?
            .resolve()
    }

    fn load_codex_config(
        path_override: Option<PathBuf>,
        cli: CliOverrides,
        default_config_path: Option<PathBuf>,
        env: &[(&str, &str)],
    ) -> Result<CodexProviderConfig, ConfigError> {
        load_raw_config_from_sources(path_override, cli, default_config_path, test_env(env))?
            .resolve_codex_config()
    }

    fn test_env(env: &[(&str, &str)]) -> Figment {
        env.iter().fold(Figment::new(), |figment, (key, value)| {
            let Some(path) = normalize_env_key(key) else {
                return figment;
            };

            if CONFIG_ENV_KEYS.contains(&path.as_str()) {
                figment.merge(Serialized::default(&path, parse_env_value(value)))
            } else {
                figment
            }
        })
    }

    fn normalize_env_key(key: &str) -> Option<String> {
        key.strip_prefix("NOEMA_")
            .map(|key| key.to_ascii_lowercase().replace("__", "."))
    }

    fn parse_env_value(value: &str) -> Value {
        match value {
            "true" => Value::Bool(true),
            "false" => Value::Bool(false),
            _ => value
                .parse::<u64>()
                .ok()
                .map(Number::from)
                .map_or_else(|| Value::String(value.to_string()), Value::Number),
        }
    }

    #[test]
    fn default_openai_config_requires_normalized_api_key() {
        let error = load_resolved(None, CliOverrides::default(), None, &[]).unwrap_err();

        assert!(matches!(
            error,
            ConfigError::MissingCredential { provider, credential }
                if provider == "openai" && credential == OPENAI_API_KEY_ENV
        ));
    }

    #[test]
    fn resolves_openai_from_figment_layers_in_precedence_order() {
        let file = write_config(
            r"
provider: openai
model: yaml-model
openai:
  base_url: https://yaml.example/v1
  organization_id: yaml-org
  project_id: yaml-project
  timeout_seconds: 22
",
        );

        let resolved = load_resolved(
            Some(file.path().to_path_buf()),
            CliOverrides::new(
                None,
                Some("cli-model".to_string()),
                Some("https://cli.example/v1".to_string()),
            ),
            None,
            &[
                (OPENAI_API_KEY_ENV, "env-key"),
                ("NOEMA_OPENAI__BASE_URL", "https://env.example/v1"),
                ("NOEMA_OPENAI__ORGANIZATION_ID", "env-org"),
                ("NOEMA_OPENAI__PROJECT_ID", "env-project"),
                ("NOEMA_OPENAI__TIMEOUT_SECONDS", "33"),
            ],
        )
        .expect("config should resolve");

        assert_eq!(resolved.provider.kind(), ProviderKind::OpenAi);

        let ProviderConfig::OpenAi(openai) = resolved.provider else {
            panic!("expected openai config");
        };

        assert_eq!(openai.api_key, "env-key");
        assert_eq!(openai.default_model, "cli-model");
        assert_eq!(openai.base_url, "https://cli.example/v1");
        assert_eq!(openai.organization_id.as_deref(), Some("env-org"));
        assert_eq!(openai.project_id.as_deref(), Some("env-project"));
        assert_eq!(openai.timeout_seconds, 33);
    }

    #[test]
    fn reads_yaml_config() {
        let file = write_config(
            r"
provider: openai
model: yaml-model
openai:
  base_url: https://yaml.example/v1
  organization_id: yaml-org
  project_id: yaml-project
  timeout_seconds: 44
",
        );

        let resolved = load_resolved(
            Some(file.path().to_path_buf()),
            CliOverrides::default(),
            None,
            &[(OPENAI_API_KEY_ENV, "env-key")],
        )
        .expect("config should load");

        let ProviderConfig::OpenAi(openai) = resolved.provider else {
            panic!("expected openai config");
        };

        assert_eq!(openai.default_model, "yaml-model");
        assert_eq!(openai.base_url, "https://yaml.example/v1");
        assert_eq!(openai.organization_id.as_deref(), Some("yaml-org"));
        assert_eq!(openai.project_id.as_deref(), Some("yaml-project"));
        assert_eq!(openai.timeout_seconds, 44);
    }

    #[test]
    fn reads_default_config_from_noema_home_env() {
        let dir = tempfile::tempdir().expect("temp dir");
        let noema_home = dir.path().join("custom-noema");
        std::fs::create_dir_all(&noema_home).expect("create noema home");
        std::fs::write(
            noema_home.join("config.yaml"),
            r"
provider: openai
model: noema-home-model
",
        )
        .expect("write config");

        let resolved = load_resolved(
            None,
            CliOverrides::default(),
            Some(noema_home.join("config.yaml")),
            &[(OPENAI_API_KEY_ENV, "env-key")],
        )
        .expect("config should load");

        let ProviderConfig::OpenAi(openai) = resolved.provider else {
            panic!("expected openai config");
        };

        assert_eq!(openai.default_model, "noema-home-model");
    }

    #[test]
    fn reads_default_config_from_home_dot_noema_without_noema_home() {
        let dir = tempfile::tempdir().expect("temp dir");
        let home = dir.path().join("home");
        let noema_home = home.join(".noema");
        std::fs::create_dir_all(&noema_home).expect("create noema home");
        std::fs::write(
            noema_home.join("config.yaml"),
            r"
provider: openai
model: home-model
",
        )
        .expect("write config");

        let resolved = load_resolved(
            None,
            CliOverrides::default(),
            Some(home.join(".noema/config.yaml")),
            &[(OPENAI_API_KEY_ENV, "env-key")],
        )
        .expect("config should load");

        let ProviderConfig::OpenAi(openai) = resolved.provider else {
            panic!("expected openai config");
        };

        assert_eq!(openai.default_model, "home-model");
    }

    #[test]
    fn noema_home_env_controls_path_but_is_not_config() {
        let dir = tempfile::tempdir().expect("temp dir");
        let noema_home = dir.path().join("noema");
        std::fs::create_dir_all(&noema_home).expect("create noema home");
        std::fs::write(
            noema_home.join("config.yaml"),
            r"
provider: codex
",
        )
        .expect("write config");

        let resolved = load_resolved(
            None,
            CliOverrides::default(),
            Some(noema_home.join("config.yaml")),
            &[(NOEMA_HOME_ENV, "/ignored/as/config")],
        )
        .expect("config should load");

        assert_eq!(resolved.provider.kind(), ProviderKind::Codex);
    }

    #[test]
    fn codex_provider_does_not_require_openai_api_key() {
        let resolved = load_resolved(
            None,
            CliOverrides::new(Some("codex".to_string()), None, None),
            None,
            &[],
        )
        .expect("codex config should resolve without OpenAI API key");

        assert_eq!(resolved.provider.kind(), ProviderKind::Codex);

        let ProviderConfig::Codex(codex) = resolved.provider else {
            panic!("expected codex config");
        };

        assert_eq!(codex.command, "codex");
        assert_eq!(codex.sandbox, "read-only");
        assert!(codex.ephemeral);
        assert!(codex.ignore_rules);
        assert!(!codex.ignore_user_config);
        assert_eq!(
            codex.startup_timeout_seconds,
            DEFAULT_CODEX_STARTUP_TIMEOUT_SECONDS
        );
        assert_eq!(
            codex.turn_timeout_seconds,
            DEFAULT_CODEX_TURN_TIMEOUT_SECONDS
        );
        assert_eq!(codex.codex_home, None);
    }

    #[test]
    fn codex_config_reads_yaml_and_normalized_env_overrides() {
        let file = write_config(
            r"
provider: codex
model: yaml-model
codex:
  command: yaml-codex
  model: yaml-codex-model
  sandbox: workspace-write
  ephemeral: false
  ignore_rules: false
  ignore_user_config: true
  startup_timeout_seconds: 45
  turn_timeout_seconds: 120
  home: /tmp/yaml-codex-home
",
        );

        let resolved = load_resolved(
            Some(file.path().to_path_buf()),
            CliOverrides::default(),
            None,
            &[
                ("NOEMA_MODEL", "env-model"),
                ("NOEMA_CODEX__COMMAND", "env-codex"),
                ("NOEMA_CODEX__EPHEMERAL", "true"),
                ("NOEMA_CODEX__STARTUP_TIMEOUT_SECONDS", "67"),
                ("NOEMA_CODEX__TURN_TIMEOUT_SECONDS", "123"),
                ("NOEMA_CODEX__HOME", "/tmp/env-codex-home"),
            ],
        )
        .expect("codex config should resolve");

        assert_eq!(resolved.provider.kind(), ProviderKind::Codex);

        let ProviderConfig::Codex(codex) = resolved.provider else {
            panic!("expected codex config");
        };

        assert_eq!(codex.default_model.as_deref(), Some("env-model"));
        assert_eq!(codex.command, "env-codex");
        assert_eq!(codex.sandbox, "workspace-write");
        assert!(codex.ephemeral);
        assert!(!codex.ignore_rules);
        assert!(codex.ignore_user_config);
        assert_eq!(codex.startup_timeout_seconds, 67);
        assert_eq!(codex.turn_timeout_seconds, 123);
        assert_eq!(codex.codex_home.as_deref(), Some("/tmp/env-codex-home"));
    }

    #[test]
    fn load_codex_ignores_default_openai_provider_credentials() {
        let codex = load_codex_config(None, CliOverrides::default(), None, &[])
            .expect("codex config should resolve");

        assert_eq!(codex.command, "codex");
    }

    #[test]
    fn old_openai_api_key_env_is_ignored() {
        let error = load_resolved(
            None,
            CliOverrides::default(),
            None,
            &[("OPENAI_API_KEY", "old-key")],
        )
        .unwrap_err();

        assert!(matches!(
            error,
            ConfigError::MissingCredential { credential, .. } if credential == OPENAI_API_KEY_ENV
        ));
    }

    #[test]
    fn yaml_openai_api_key_is_rejected() {
        let file = write_config(
            r"
provider: openai
openai:
  api_key: not-allowed
",
        );

        let error = load_resolved(
            Some(file.path().to_path_buf()),
            CliOverrides::default(),
            None,
            &[],
        )
        .unwrap_err();

        assert!(matches!(error, ConfigError::ParseConfig { .. }));
    }

    #[test]
    fn missing_explicit_config_file_is_an_error() {
        let missing = PathBuf::from("/tmp/noema-missing-config.yaml");
        let error = Config::load(Some(missing.clone()), CliOverrides::default()).unwrap_err();

        assert!(matches!(
            error,
            ConfigError::ConfigFileNotFound { path } if path == missing
        ));
    }

    #[test]
    fn invalid_codex_bool_env_is_an_error() {
        let error = load_resolved(
            None,
            CliOverrides::default(),
            None,
            &[
                ("NOEMA_PROVIDER", "codex"),
                ("NOEMA_CODEX__EPHEMERAL", "sometimes"),
            ],
        )
        .unwrap_err();

        assert!(matches!(error, ConfigError::Load(_)));
    }

    #[test]
    fn invalid_codex_timeout_env_is_an_error() {
        let error = load_resolved(
            None,
            CliOverrides::default(),
            None,
            &[
                ("NOEMA_PROVIDER", "codex"),
                ("NOEMA_CODEX__TURN_TIMEOUT_SECONDS", "abc"),
            ],
        )
        .unwrap_err();

        assert!(matches!(error, ConfigError::Load(_)));
    }

    #[test]
    fn zero_codex_timeout_is_an_error() {
        let error = load_resolved(
            None,
            CliOverrides::default(),
            None,
            &[
                ("NOEMA_PROVIDER", "codex"),
                ("NOEMA_CODEX__TURN_TIMEOUT_SECONDS", "0"),
            ],
        )
        .unwrap_err();

        assert!(matches!(error, ConfigError::InvalidInteger { .. }));
    }

    #[test]
    fn unsupported_provider_is_an_error() {
        let error = load_resolved(
            None,
            CliOverrides::default(),
            None,
            &[("NOEMA_PROVIDER", "unknown")],
        )
        .unwrap_err();

        assert!(matches!(error, ConfigError::UnsupportedProvider { .. }));
    }

    #[test]
    fn generated_config_template_parses() {
        let file = write_config(crate::DEFAULT_NOEMA_CONFIG_YAML);

        let codex = load_codex_config(
            Some(file.path().to_path_buf()),
            CliOverrides::default(),
            None,
            &[],
        )
        .expect("generated config should parse");

        assert_eq!(codex.command, "codex");
        assert_eq!(codex.sandbox, "read-only");
    }
}
