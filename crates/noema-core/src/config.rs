use crate::providers::{
    codex::{
        CodexProviderConfig, DEFAULT_CODEX_STARTUP_TIMEOUT_SECONDS,
        DEFAULT_CODEX_TURN_TIMEOUT_SECONDS,
    },
    openai::{DEFAULT_OPENAI_TIMEOUT_SECONDS, OpenAiProviderConfig},
};
use serde::Deserialize;
use std::{env, fs, path::PathBuf, str::FromStr};
use thiserror::Error;

pub const DEFAULT_PROVIDER: &str = "openai";
pub const DEFAULT_OPENAI_MODEL: &str = "gpt-5.5";
pub const DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com/v1";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CliOverrides {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedConfig {
    pub provider: ProviderConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderKind {
    Codex,
    OpenAi,
}

impl ProviderKind {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderConfig {
    Codex(CodexProviderConfig),
    OpenAi(OpenAiProviderConfig),
}

impl ProviderConfig {
    pub fn kind(&self) -> ProviderKind {
        match self {
            Self::Codex(_) => ProviderKind::Codex,
            Self::OpenAi(_) => ProviderKind::OpenAi,
        }
    }

    pub fn model(&self) -> Option<&str> {
        match self {
            Self::Codex(config) => config.default_model.as_deref(),
            Self::OpenAi(config) => Some(config.default_model.as_str()),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub codex: CodexFileConfig,
    pub openai: OpenAiFileConfig,
}

impl Config {
    pub fn load(
        path_override: Option<PathBuf>,
        cli: CliOverrides,
    ) -> Result<ResolvedConfig, ConfigError> {
        Self::load_with_env(path_override, cli, EnvVars::from_process())
    }

    pub fn load_with_env(
        path_override: Option<PathBuf>,
        cli: CliOverrides,
        env: EnvVars,
    ) -> Result<ResolvedConfig, ConfigError> {
        let file_config = Self::read_config_file(path_override)?;
        file_config.resolve(cli, env)
    }

    pub fn load_codex(
        path_override: Option<PathBuf>,
        cli: CliOverrides,
    ) -> Result<CodexProviderConfig, ConfigError> {
        Self::load_codex_with_env(path_override, cli, EnvVars::from_process())
    }

    pub fn load_codex_with_env(
        path_override: Option<PathBuf>,
        cli: CliOverrides,
        env: EnvVars,
    ) -> Result<CodexProviderConfig, ConfigError> {
        let file_config = Self::read_config_file(path_override)?;
        file_config.resolve_codex_config(&cli, &env)
    }

    fn read_config_file(path_override: Option<PathBuf>) -> Result<Self, ConfigError> {
        let Some(config_path) = path_override.or_else(default_config_path) else {
            return Ok(Self::default());
        };

        if !config_path.exists() {
            return if config_path == default_config_path().unwrap_or_default() {
                Ok(Self::default())
            } else {
                Err(ConfigError::ConfigFileNotFound { path: config_path })
            };
        }

        let contents =
            fs::read_to_string(&config_path).map_err(|source| ConfigError::ReadFile {
                path: config_path.clone(),
                source,
            })?;

        serde_yaml::from_str(&contents).map_err(|source| ConfigError::ParseYaml {
            path: config_path,
            source,
        })
    }

    fn resolve(self, cli: CliOverrides, env: EnvVars) -> Result<ResolvedConfig, ConfigError> {
        let provider = first_non_empty([
            cli.provider.as_deref(),
            env.noema_provider.as_deref(),
            self.provider.as_deref(),
            Some(DEFAULT_PROVIDER),
        ])
        .unwrap_or(DEFAULT_PROVIDER);

        let provider = ProviderKind::from_str(provider)
            .map_err(|provider| ConfigError::UnsupportedProvider { provider })?;

        let provider = match provider {
            ProviderKind::OpenAi => ProviderConfig::OpenAi(self.resolve_openai_config(&cli, &env)?),
            ProviderKind::Codex => ProviderConfig::Codex(self.resolve_codex_config(&cli, &env)?),
        };

        Ok(ResolvedConfig { provider })
    }

    fn resolve_openai_config(
        &self,
        cli: &CliOverrides,
        env: &EnvVars,
    ) -> Result<OpenAiProviderConfig, ConfigError> {
        let model = first_non_empty([
            cli.model.as_deref(),
            env.noema_model.as_deref(),
            self.model.as_deref(),
            Some(DEFAULT_OPENAI_MODEL),
        ])
        .unwrap_or(DEFAULT_OPENAI_MODEL)
        .to_string();

        let base_url = first_non_empty([
            cli.base_url.as_deref(),
            env.openai_base_url.as_deref(),
            self.openai.base_url.as_deref(),
            Some(DEFAULT_OPENAI_BASE_URL),
        ])
        .unwrap_or(DEFAULT_OPENAI_BASE_URL)
        .trim_end_matches('/')
        .to_string();

        let api_key = first_non_empty([env.openai_api_key.as_deref()]).ok_or_else(|| {
            ConfigError::MissingCredential {
                provider: "openai".to_string(),
                credential: "OPENAI_API_KEY".to_string(),
            }
        })?;

        let organization_id = first_non_empty([
            env.openai_org_id.as_deref(),
            self.openai.organization_id.as_deref(),
        ])
        .map(ToString::to_string);

        let project_id = first_non_empty([
            env.openai_project_id.as_deref(),
            self.openai.project_id.as_deref(),
        ])
        .map(ToString::to_string);

        let timeout_seconds = first_u64(
            env.openai_timeout_seconds.as_deref(),
            self.openai.timeout_seconds,
            DEFAULT_OPENAI_TIMEOUT_SECONDS,
            "OPENAI_TIMEOUT_SECONDS",
        )?;

        Ok(OpenAiProviderConfig {
            api_key: api_key.to_string(),
            base_url,
            organization_id,
            project_id,
            default_model: model,
            timeout_seconds,
        })
    }

    fn resolve_codex_config(
        &self,
        cli: &CliOverrides,
        env: &EnvVars,
    ) -> Result<CodexProviderConfig, ConfigError> {
        let default_codex = CodexProviderConfig::default();
        let model = first_non_empty([
            cli.model.as_deref(),
            env.noema_model.as_deref(),
            self.codex.model.as_deref(),
            self.model.as_deref(),
        ])
        .map(ToString::to_string);

        let command = first_non_empty([
            env.noema_codex_command.as_deref(),
            self.codex.command.as_deref(),
            Some(default_codex.command.as_str()),
        ])
        .unwrap_or(default_codex.command.as_str())
        .to_string();

        let sandbox = first_non_empty([
            env.noema_codex_sandbox.as_deref(),
            self.codex.sandbox.as_deref(),
            Some(default_codex.sandbox.as_str()),
        ])
        .unwrap_or(default_codex.sandbox.as_str())
        .to_string();

        let ephemeral = first_bool(
            env.noema_codex_ephemeral.as_deref(),
            self.codex.ephemeral,
            default_codex.ephemeral,
            "NOEMA_CODEX_EPHEMERAL",
        )?;

        let ignore_rules = first_bool(
            env.noema_codex_ignore_rules.as_deref(),
            self.codex.ignore_rules,
            default_codex.ignore_rules,
            "NOEMA_CODEX_IGNORE_RULES",
        )?;

        let ignore_user_config = first_bool(
            env.noema_codex_ignore_user_config.as_deref(),
            self.codex.ignore_user_config,
            default_codex.ignore_user_config,
            "NOEMA_CODEX_IGNORE_USER_CONFIG",
        )?;

        let startup_timeout_seconds = first_u64(
            env.noema_codex_startup_timeout_seconds.as_deref(),
            self.codex.startup_timeout_seconds,
            DEFAULT_CODEX_STARTUP_TIMEOUT_SECONDS,
            "NOEMA_CODEX_STARTUP_TIMEOUT_SECONDS",
        )?;

        let turn_timeout_seconds = first_u64(
            first_non_empty([
                env.noema_codex_turn_timeout_seconds.as_deref(),
                env.noema_codex_timeout_seconds.as_deref(),
            ]),
            self.codex
                .turn_timeout_seconds
                .or(self.codex.timeout_seconds),
            DEFAULT_CODEX_TURN_TIMEOUT_SECONDS,
            "NOEMA_CODEX_TURN_TIMEOUT_SECONDS",
        )?;

        let codex_home = first_non_empty([
            env.noema_codex_home.as_deref(),
            self.codex.codex_home.as_deref(),
        ])
        .map(ToString::to_string);

        Ok(CodexProviderConfig {
            command,
            default_model: model,
            sandbox,
            ephemeral,
            ignore_rules,
            ignore_user_config,
            startup_timeout_seconds,
            turn_timeout_seconds,
            codex_home,
        })
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OpenAiFileConfig {
    pub base_url: Option<String>,
    pub organization_id: Option<String>,
    pub project_id: Option<String>,
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CodexFileConfig {
    pub command: Option<String>,
    pub model: Option<String>,
    pub sandbox: Option<String>,
    pub ephemeral: Option<bool>,
    pub ignore_rules: Option<bool>,
    pub ignore_user_config: Option<bool>,
    pub startup_timeout_seconds: Option<u64>,
    pub turn_timeout_seconds: Option<u64>,
    pub timeout_seconds: Option<u64>,
    pub codex_home: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnvVars {
    pub noema_provider: Option<String>,
    pub noema_model: Option<String>,
    pub noema_codex_command: Option<String>,
    pub noema_codex_sandbox: Option<String>,
    pub noema_codex_ephemeral: Option<String>,
    pub noema_codex_ignore_rules: Option<String>,
    pub noema_codex_ignore_user_config: Option<String>,
    pub noema_codex_startup_timeout_seconds: Option<String>,
    pub noema_codex_turn_timeout_seconds: Option<String>,
    pub noema_codex_timeout_seconds: Option<String>,
    pub noema_codex_home: Option<String>,
    pub openai_api_key: Option<String>,
    pub openai_base_url: Option<String>,
    pub openai_timeout_seconds: Option<String>,
    pub openai_org_id: Option<String>,
    pub openai_project_id: Option<String>,
}

impl EnvVars {
    pub fn from_process() -> Self {
        Self {
            noema_provider: env::var("NOEMA_PROVIDER").ok(),
            noema_model: env::var("NOEMA_MODEL").ok(),
            noema_codex_command: env::var("NOEMA_CODEX_COMMAND").ok(),
            noema_codex_sandbox: env::var("NOEMA_CODEX_SANDBOX").ok(),
            noema_codex_ephemeral: env::var("NOEMA_CODEX_EPHEMERAL").ok(),
            noema_codex_ignore_rules: env::var("NOEMA_CODEX_IGNORE_RULES").ok(),
            noema_codex_ignore_user_config: env::var("NOEMA_CODEX_IGNORE_USER_CONFIG").ok(),
            noema_codex_startup_timeout_seconds: env::var("NOEMA_CODEX_STARTUP_TIMEOUT_SECONDS")
                .ok(),
            noema_codex_turn_timeout_seconds: env::var("NOEMA_CODEX_TURN_TIMEOUT_SECONDS").ok(),
            noema_codex_timeout_seconds: env::var("NOEMA_CODEX_TIMEOUT_SECONDS").ok(),
            noema_codex_home: env::var("NOEMA_CODEX_HOME").ok(),
            openai_api_key: env::var("OPENAI_API_KEY").ok(),
            openai_base_url: env::var("OPENAI_BASE_URL").ok(),
            openai_timeout_seconds: env::var("OPENAI_TIMEOUT_SECONDS").ok(),
            openai_org_id: env::var("OPENAI_ORG_ID").ok(),
            openai_project_id: env::var("OPENAI_PROJECT_ID").ok(),
        }
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("config file not found: {}", path.display())]
    ConfigFileNotFound { path: PathBuf },

    #[error("failed to read config file {}: {source}", path.display())]
    ReadFile {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse config file {}: {source}", path.display())]
    ParseYaml {
        path: PathBuf,
        source: serde_yaml::Error,
    },

    #[error("unsupported provider: {provider}")]
    UnsupportedProvider { provider: String },

    #[error("missing credentials for {provider}: {credential}")]
    MissingCredential {
        provider: String,
        credential: String,
    },

    #[error("invalid boolean value for {name}: {value}")]
    InvalidBool { name: String, value: String },

    #[error("invalid integer value for {name}: {value}")]
    InvalidInteger { name: String, value: String },
}

fn first_non_empty<'a>(values: impl IntoIterator<Item = Option<&'a str>>) -> Option<&'a str> {
    values
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|value| !value.is_empty())
}

fn default_config_path() -> Option<PathBuf> {
    env::var_os("HOME").map(|home| PathBuf::from(home).join(".noema/config.yaml"))
}

fn first_bool(
    env_value: Option<&str>,
    file_value: Option<bool>,
    default_value: bool,
    name: &str,
) -> Result<bool, ConfigError> {
    match env_value {
        Some(value) => parse_bool(value).ok_or_else(|| ConfigError::InvalidBool {
            name: name.to_string(),
            value: value.to_string(),
        }),
        None => Ok(file_value.unwrap_or(default_value)),
    }
}

fn first_u64(
    env_value: Option<&str>,
    file_value: Option<u64>,
    default_value: u64,
    name: &str,
) -> Result<u64, ConfigError> {
    match env_value {
        Some(value) => value
            .parse::<u64>()
            .ok()
            .filter(|value| *value > 0)
            .ok_or_else(|| ConfigError::InvalidInteger {
                name: name.to_string(),
                value: value.to_string(),
            }),
        None => Ok(file_value.unwrap_or(default_value)),
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn write_config(contents: &str) -> NamedTempFile {
        let file = NamedTempFile::new().expect("temp config");
        std::fs::write(file.path(), contents).expect("write config");
        file
    }

    fn empty_env() -> EnvVars {
        EnvVars::default()
    }

    #[test]
    fn default_openai_config_requires_api_key() {
        let error = Config::default()
            .resolve(CliOverrides::default(), empty_env())
            .unwrap_err();

        assert!(matches!(
            error,
            ConfigError::MissingCredential { provider, credential }
                if provider == "openai" && credential == "OPENAI_API_KEY"
        ));
    }

    #[test]
    fn resolves_openai_from_env_and_cli_precedence() {
        let config = Config {
            provider: Some("openai".to_string()),
            model: Some("yaml-model".to_string()),
            openai: OpenAiFileConfig {
                base_url: Some("https://yaml.example/v1".to_string()),
                organization_id: Some("yaml-org".to_string()),
                project_id: Some("yaml-project".to_string()),
                timeout_seconds: Some(22),
            },
            ..Config::default()
        };

        let resolved = config
            .resolve(
                CliOverrides {
                    provider: None,
                    model: Some("cli-model".to_string()),
                    base_url: Some("https://cli.example/v1".to_string()),
                },
                EnvVars {
                    openai_api_key: Some("env-key".to_string()),
                    openai_base_url: Some("https://env.example/v1".to_string()),
                    openai_org_id: Some("env-org".to_string()),
                    openai_project_id: Some("env-project".to_string()),
                    openai_timeout_seconds: Some("33".to_string()),
                    ..empty_env()
                },
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
            r#"
provider: openai
model: yaml-model
openai:
  base_url: https://yaml.example/v1
  organization_id: yaml-org
  project_id: yaml-project
  timeout_seconds: 44
"#,
        );

        let resolved = Config::load_with_env(
            Some(file.path().to_path_buf()),
            CliOverrides::default(),
            EnvVars {
                openai_api_key: Some("env-key".to_string()),
                ..empty_env()
            },
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
    fn codex_provider_does_not_require_openai_api_key() {
        let resolved = Config::default()
            .resolve(
                CliOverrides {
                    provider: Some("codex".to_string()),
                    model: None,
                    base_url: None,
                },
                empty_env(),
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
    fn codex_config_reads_yaml_and_env_overrides() {
        let file = write_config(
            r#"
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
  codex_home: /tmp/yaml-codex-home
"#,
        );

        let resolved = Config::load_with_env(
            Some(file.path().to_path_buf()),
            CliOverrides::default(),
            EnvVars {
                noema_model: Some("env-model".to_string()),
                noema_codex_command: Some("env-codex".to_string()),
                noema_codex_ephemeral: Some("true".to_string()),
                noema_codex_startup_timeout_seconds: Some("67".to_string()),
                noema_codex_turn_timeout_seconds: Some("123".to_string()),
                noema_codex_home: Some("/tmp/env-codex-home".to_string()),
                ..empty_env()
            },
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
        let codex = Config::load_codex_with_env(None, CliOverrides::default(), empty_env())
            .expect("codex config should resolve");

        assert_eq!(codex.command, "codex");
    }

    #[test]
    fn invalid_codex_bool_env_is_an_error() {
        let error = Config::default()
            .resolve(
                CliOverrides {
                    provider: Some("codex".to_string()),
                    model: None,
                    base_url: None,
                },
                EnvVars {
                    noema_codex_ephemeral: Some("sometimes".to_string()),
                    ..empty_env()
                },
            )
            .unwrap_err();

        assert!(matches!(error, ConfigError::InvalidBool { .. }));
    }

    #[test]
    fn invalid_codex_timeout_env_is_an_error() {
        let error = Config::default()
            .resolve(
                CliOverrides {
                    provider: Some("codex".to_string()),
                    model: None,
                    base_url: None,
                },
                EnvVars {
                    noema_codex_turn_timeout_seconds: Some("0".to_string()),
                    ..empty_env()
                },
            )
            .unwrap_err();

        assert!(matches!(error, ConfigError::InvalidInteger { .. }));
    }

    #[test]
    fn unsupported_provider_is_an_error() {
        let error = Config::default()
            .resolve(
                CliOverrides {
                    provider: Some("unknown".to_string()),
                    model: None,
                    base_url: None,
                },
                empty_env(),
            )
            .unwrap_err();

        assert!(matches!(error, ConfigError::UnsupportedProvider { .. }));
    }
}
