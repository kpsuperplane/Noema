use std::{env, path::PathBuf, sync::Arc};

use noema_home::{NoemaPaths, SystemErrorLogger};
use noema_providers::{
    CodexOAuthConfig, CodexProviderConfig, DEFAULT_CODEX_BASE_URL, DEFAULT_OPENAI_BASE_URL,
    DEFAULT_OPENROUTER_BASE_URL, FoundationLocalProviderConfig, OPENAI_API_KEY_ENV,
    OpenAiProviderConfig, OpenRouterProviderConfig, ProviderAccountPersistenceHandle,
    ProviderAccountService, ProviderConfig, ProviderHandle, ReasoningEffort,
    hosted_provider_from_config,
};
use noema_store::{NoemaStore, StoreConfig};
use serde::{Deserialize, Serialize};

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
    account_service: ProviderAccountService,
    account_persistence: ProviderAccountPersistenceHandle,
    system_errors: SystemErrorLogger,
}

impl HostedProviderContext {
    pub(crate) async fn from_process_env() -> Result<Self, String> {
        let paths = NoemaPaths::from_process_env()
            .map_err(|error| format!("failed to resolve Noema paths: {error}"))?;
        let system_errors = SystemErrorLogger::from_paths(&paths);
        let store = NoemaStore::open(&StoreConfig::new(paths.sqlite_db_path()))
            .await
            .map_err(|error| format!("failed to open Noema provider store: {error}"))?;
        let account_persistence: ProviderAccountPersistenceHandle = Arc::new(store.clone());
        let account_service = ProviderAccountService::new_with_codex_oauth(
            paths,
            Arc::new(store.clone()),
            Arc::new(store),
            system_errors.clone(),
            CodexOAuthConfig::default(),
        )
        .map_err(|error| format!("failed to open Noema provider credentials: {error}"))?;
        Ok(Self {
            account_service,
            account_persistence,
            system_errors,
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
        hosted_provider_from_config(
            config,
            self.account_service.credentials(),
            Some(self.account_persistence.clone()),
            self.system_errors.clone(),
        )
        .map_err(|error| format!("failed to construct hosted provider: {error}"))
    }

    pub(crate) async fn shutdown(self) {
        self.account_service.shutdown().await;
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
}
