use std::{
    env,
    ffi::OsString,
    path::{Path, PathBuf},
    sync::Arc,
};

use noema_home::SystemErrorLogger;
use noema_providers::{
    CodexOAuthConfig, CodexProviderConfig, DEFAULT_CODEX_BASE_URL, DEFAULT_OPENAI_BASE_URL,
    DEFAULT_OPENROUTER_BASE_URL, FoundationLocalProviderConfig, OPENAI_API_KEY_ENV,
    OPENROUTER_PROVIDER_ACCOUNT_ID, OpenAiProviderConfig, OpenRouterProviderConfig, ProviderConfig,
    ProviderCredential, ProviderCredentialAccess, ProviderCredentialAccessHandle,
    ProviderCredentialFuture, ProviderError, ProviderHandle, ReasoningEffort,
    hosted_provider_from_config, validate_openrouter_api_key,
};
use serde::{Deserialize, Serialize};

const OPENROUTER_API_KEY_ENV: &str = "OPENROUTER_API_KEY";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum HostedProviderKind {
    Codex,
    #[serde(rename = "openai")]
    OpenAi,
    #[serde(rename = "openrouter")]
    OpenRouter,
    FoundationLocal,
}

impl HostedProviderKind {
    fn provider_config(
        self,
        spec: &HostedProviderSpec,
        openai_api_key: Option<String>,
    ) -> Result<ProviderConfig, String> {
        let model = required_model(&spec.model)?;
        let timeout_seconds = positive_timeout(spec.timeout_seconds)?;
        let base_url = |default: &str| {
            spec.base_url
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(default)
                .to_string()
        };

        Ok(match self {
            Self::Codex => ProviderConfig::Codex(CodexProviderConfig {
                base_url: base_url(DEFAULT_CODEX_BASE_URL),
                default_model: Some(model),
                tool_classification_model: None,
                reasoning_effort: spec.reasoning_effort,
                timeout_seconds,
                client_version: None,
                oauth: CodexOAuthConfig::default(),
                system_errors: None,
            }),
            Self::OpenAi => ProviderConfig::OpenAi(OpenAiProviderConfig {
                api_key: openai_api_key
                    .ok_or_else(|| format!("{OPENAI_API_KEY_ENV} is not set or is blank"))?,
                base_url: base_url(DEFAULT_OPENAI_BASE_URL),
                organization_id: None,
                project_id: None,
                default_model: model,
                tool_classification_model: None,
                reasoning_effort: spec.reasoning_effort,
                timeout_seconds,
                system_errors: None,
            }),
            Self::OpenRouter => ProviderConfig::OpenRouter(OpenRouterProviderConfig {
                base_url: base_url(DEFAULT_OPENROUTER_BASE_URL),
                default_model: model,
                tool_classification_model: None,
                reasoning_effort: spec.reasoning_effort,
                timeout_seconds,
                system_errors: None,
            }),
            Self::FoundationLocal => {
                ProviderConfig::FoundationLocal(FoundationLocalProviderConfig {
                    default_profile: model,
                    bridge_path: spec.bridge_path.clone(),
                    system_errors: None,
                })
            }
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HostedProviderSpec {
    pub(crate) kind: HostedProviderKind,
    pub(crate) model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) reasoning_effort: Option<ReasoningEffort>,
    pub(crate) timeout_seconds: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) bridge_path: Option<PathBuf>,
}

pub(crate) struct HostedProviderContext {
    credentials: Arc<OpenRouterEnvCredentials>,
    system_errors: SystemErrorLogger,
}

impl HostedProviderContext {
    pub(crate) fn from_process_env(run_root: &Path) -> Result<Self, String> {
        let api_key = openrouter_api_key(env::var_os(OPENROUTER_API_KEY_ENV))?;
        Ok(Self {
            credentials: Arc::new(OpenRouterEnvCredentials {
                api_key: api_key.into(),
            }),
            system_errors: SystemErrorLogger::new(run_root.join("provider-errors.log")),
        })
    }

    pub(crate) fn build_provider(
        &self,
        spec: &HostedProviderSpec,
    ) -> Result<(String, ProviderHandle), String> {
        let openai_api_key = (spec.kind == HostedProviderKind::OpenAi)
            .then(|| {
                env::var(OPENAI_API_KEY_ENV).ok().and_then(|value| {
                    let value = value.trim().to_string();
                    (!value.is_empty()).then_some(value)
                })
            })
            .flatten();
        let config = spec.kind.provider_config(spec, openai_api_key)?;
        let credentials: ProviderCredentialAccessHandle = self.credentials.clone();
        hosted_provider_from_config(config, credentials, None, None, self.system_errors.clone())
            .map_err(|error| format!("failed to construct hosted provider: {error}"))
    }

    pub(crate) async fn preflight_openrouter_models(
        &self,
        models: &[String],
    ) -> Result<(), String> {
        let profiles = validate_openrouter_api_key(self.credentials.api_key.expose_secret())
            .await
            .map_err(|error| format!("OpenRouter credential/catalog preflight failed: {error}"))?;
        let available = profiles
            .into_iter()
            .map(|profile| profile.id)
            .collect::<std::collections::HashSet<_>>();
        let missing = models
            .iter()
            .filter(|model| !available.contains(model.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        if missing.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "OpenRouter API key cannot access planned models: {}",
                missing.join(", ")
            ))
        }
    }
}

#[derive(Clone, Debug)]
struct OpenRouterEnvCredentials {
    api_key: ProviderCredential,
}

impl ProviderCredentialAccess for OpenRouterEnvCredentials {
    fn api_key<'a>(
        &'a self,
        provider_kind: &'a str,
        provider_account_id: &'a str,
    ) -> ProviderCredentialFuture<'a> {
        let result = if provider_kind == "openrouter"
            && provider_account_id == OPENROUTER_PROVIDER_ACCOUNT_ID
        {
            Ok(self.api_key.clone())
        } else {
            Err(ProviderError::MissingCredentials {
                provider: provider_kind.to_string(),
                credential: "API key".to_string(),
            })
        };
        Box::pin(async move { result })
    }

    fn codex_access_token<'a>(
        &'a self,
        _provider_account_id: &'a str,
    ) -> ProviderCredentialFuture<'a> {
        missing_codex_credential()
    }

    fn refresh_codex_access_token<'a>(
        &'a self,
        _provider_account_id: &'a str,
    ) -> ProviderCredentialFuture<'a> {
        missing_codex_credential()
    }
}

fn missing_codex_credential<'a>() -> ProviderCredentialFuture<'a> {
    Box::pin(async {
        Err(ProviderError::MissingCredentials {
            provider: "codex".to_string(),
            credential: "access token".to_string(),
        })
    })
}

fn openrouter_api_key(value: Option<OsString>) -> Result<String, String> {
    let value = value
        .ok_or_else(|| format!("{OPENROUTER_API_KEY_ENV} is not set or is blank"))?
        .into_string()
        .map_err(|_| format!("{OPENROUTER_API_KEY_ENV} is not valid Unicode"))?;
    let value = value.trim();
    if value.is_empty() {
        Err(format!("{OPENROUTER_API_KEY_ENV} is not set or is blank"))
    } else {
        Ok(value.to_string())
    }
}

fn required_model(model: &str) -> Result<String, String> {
    let model = model.trim();
    (!model.is_empty())
        .then(|| model.to_string())
        .ok_or_else(|| "hosted provider model cannot be blank".to_string())
}

fn positive_timeout(timeout_seconds: u64) -> Result<u64, String> {
    (timeout_seconds > 0)
        .then_some(timeout_seconds)
        .ok_or_else(|| "hosted provider timeout must be greater than zero seconds".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(kind: HostedProviderKind) -> HostedProviderSpec {
        HostedProviderSpec {
            kind,
            model: " model-x ".to_string(),
            reasoning_effort: Some(ReasoningEffort::High),
            timeout_seconds: 42,
            base_url: Some(" https://example.test/v1/ ".to_string()),
            bridge_path: Some(PathBuf::from("bridge")),
        }
    }

    #[test]
    fn hosted_specs_convert_without_network_or_credentials() {
        let config = HostedProviderKind::Codex
            .provider_config(&spec(HostedProviderKind::Codex), None)
            .expect("Codex config");
        let ProviderConfig::Codex(config) = config else {
            panic!("expected Codex config");
        };
        assert_eq!(config.default_model.as_deref(), Some("model-x"));
        assert_eq!(config.reasoning_effort, Some(ReasoningEffort::High));
        assert_eq!(config.timeout_seconds, 42);
        assert_eq!(config.base_url, "https://example.test/v1/");

        let config = HostedProviderKind::FoundationLocal
            .provider_config(&spec(HostedProviderKind::FoundationLocal), None)
            .expect("Foundation config");
        let ProviderConfig::FoundationLocal(config) = config else {
            panic!("expected Foundation config");
        };
        assert_eq!(config.default_profile, "model-x");
        assert_eq!(config.bridge_path, Some(PathBuf::from("bridge")));
    }

    #[test]
    fn openrouter_api_key_requires_a_non_blank_environment_value() {
        assert!(openrouter_api_key(None).is_err());
        assert!(openrouter_api_key(Some(OsString::from("  "))).is_err());
        assert_eq!(
            openrouter_api_key(Some(OsString::from("  secret  "))).expect("API key"),
            "secret"
        );
    }

    #[tokio::test]
    async fn environment_credentials_are_scoped_to_the_openrouter_default_account() {
        let credentials = OpenRouterEnvCredentials {
            api_key: "secret".to_string().into(),
        };
        let credential = credentials
            .api_key("openrouter", OPENROUTER_PROVIDER_ACCOUNT_ID)
            .await
            .expect("OpenRouter credential");
        assert_eq!(credential.expose_secret(), "secret");
        assert!(
            credentials
                .api_key("openrouter", "provider_account:openrouter:other")
                .await
                .is_err()
        );
    }
}
