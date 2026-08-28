use std::{env, ffi::OsString, path::Path, sync::Arc};

use noema_home::SystemErrorLogger;
use noema_providers::{
    DEFAULT_OPENROUTER_BASE_URL, OpenRouterProviderConfig, ProviderConfig, ProviderCredential,
    ProviderCredentialAccess, ProviderCredentialAccessHandle, ProviderCredentialFuture,
    ProviderError, ProviderHandle, ProviderKind, ReasoningEffort, hosted_provider_from_config,
    validate_openrouter_api_key,
};

const OPENROUTER_API_KEY_ENV: &str = "OPENROUTER_API_KEY";

pub(crate) struct OpenRouterProviderSpec {
    pub(crate) model: String,
    pub(crate) reasoning_effort: Option<ReasoningEffort>,
    pub(crate) timeout_seconds: u64,
    pub(crate) base_url: Option<String>,
}

impl OpenRouterProviderSpec {
    fn provider_config(&self) -> Result<ProviderConfig, String> {
        let model = self.model.trim();
        if model.is_empty() {
            return Err("OpenRouter model cannot be blank".to_string());
        }
        if self.timeout_seconds == 0 {
            return Err("OpenRouter timeout must be greater than zero seconds".to_string());
        }
        let base_url = self
            .base_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(DEFAULT_OPENROUTER_BASE_URL)
            .to_string();
        Ok(ProviderConfig::OpenRouter(OpenRouterProviderConfig {
            base_url,
            default_model: model.to_string(),
            tool_classification_model: None,
            reasoning_effort: self.reasoning_effort,
            timeout_seconds: self.timeout_seconds,
            system_errors: None,
        }))
    }
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
        spec: &OpenRouterProviderSpec,
    ) -> Result<ProviderHandle, String> {
        let config = spec.provider_config()?;
        let credentials: ProviderCredentialAccessHandle = self.credentials.clone();
        hosted_provider_from_config(config, credentials, None, None, self.system_errors.clone())
            .map(|(_, provider)| provider)
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
            && provider_account_id == ProviderKind::OpenRouter.default_account_id()
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

#[cfg(test)]
mod tests {
    use super::*;

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
            .api_key("openrouter", ProviderKind::OpenRouter.default_account_id())
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
