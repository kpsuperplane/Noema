use async_graphql::{Enum, InputObject, Result, SimpleObject};
use std::time::Duration;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{MemoryServiceMode, MemoryServiceSettingsRecord, SaveMemoryServiceSettings};

const MEMORY_SERVICE_READINESS_TIMEOUT: Duration = Duration::from_secs(2);

use super::{
    agents::{
        GraphqlAgentModelPreference, GraphqlAgentModelProviderOption, GraphqlReasoningEffort,
        option_from_account, profiles_from_account, provider_disabled_reason,
        refresh_missing_model_profiles, validate_reasoning_effort_for_profile,
    },
    errors::graphql_error,
    schema::GraphqlState,
};

/// Memory service operating mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "MemoryServiceMode")]
pub enum GraphqlMemoryServiceMode {
    /// Noema manages a local Supermemory service.
    Managed,
    /// Noema connects to an externally managed Supermemory service.
    External,
}

impl From<MemoryServiceMode> for GraphqlMemoryServiceMode {
    fn from(value: MemoryServiceMode) -> Self {
        match value {
            MemoryServiceMode::Managed => Self::Managed,
            MemoryServiceMode::External => Self::External,
        }
    }
}

impl From<GraphqlMemoryServiceMode> for MemoryServiceMode {
    fn from(value: GraphqlMemoryServiceMode) -> Self {
        match value {
            GraphqlMemoryServiceMode::Managed => Self::Managed,
            GraphqlMemoryServiceMode::External => Self::External,
        }
    }
}

/// Memory service readiness status kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "MemoryServiceStatusKind")]
pub enum GraphqlMemoryServiceStatusKind {
    /// Service configuration has not been checked.
    NotConfigured,
    /// Managed service startup is in progress.
    Starting,
    /// Service is reachable and ready.
    Ready,
    /// Service is not reachable.
    Unavailable,
    /// Service authentication failed.
    AuthError,
}

/// Memory service readiness exposed to Settings.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "MemoryServiceStatus")]
pub struct GraphqlMemoryServiceStatus {
    /// Current readiness status.
    pub status: GraphqlMemoryServiceStatusKind,
    /// Last readiness check timestamp.
    pub checked_at: Option<String>,
    /// Sanitized last error code.
    pub last_error_code: Option<String>,
    /// Sanitized last error message.
    pub last_error_message: Option<String>,
}

/// Memory service settings exposed to Settings.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "MemorySettings")]
pub struct GraphqlMemorySettings {
    /// Supermemory service mode.
    pub mode: GraphqlMemoryServiceMode,
    /// External Supermemory service base URL.
    pub base_url: Option<String>,
    /// External service port, when configured.
    pub port: Option<i32>,
    /// Current readiness status.
    pub status: GraphqlMemoryServiceStatus,
    /// Current persisted extraction model preference, when configured.
    pub model_preference: Option<GraphqlAgentModelPreference>,
    /// Provider/profile options available for memory extraction.
    pub model_options: Vec<GraphqlAgentModelProviderOption>,
}

/// Input for saving memory service settings.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "SaveMemoryServiceSettingsInput")]
pub struct GraphqlSaveMemoryServiceSettingsInput {
    /// Supermemory service mode.
    pub mode: GraphqlMemoryServiceMode,
    /// External Supermemory service base URL.
    pub base_url: Option<String>,
    /// External service port, when configured.
    pub port: Option<i32>,
    /// Provider account id to use for memory extraction.
    pub provider_account_id: Option<String>,
    /// Provider-specific model id or profile id.
    pub model_profile: Option<String>,
    /// Optional explicit reasoning effort for reasoning-capable model profiles.
    pub reasoning_effort: Option<GraphqlReasoningEffort>,
}

pub(super) async fn memory_settings(state: &GraphqlState) -> Result<GraphqlMemorySettings> {
    memory_settings_from_store(state).await
}

pub(super) async fn save_memory_service_settings(
    state: &GraphqlState,
    input: GraphqlSaveMemoryServiceSettingsInput,
) -> Result<GraphqlMemorySettings> {
    let store = state.store()?;
    let mode: MemoryServiceMode = input.mode.into();
    let (base_url, port) = match mode {
        MemoryServiceMode::External => {
            let base_url = input.base_url.as_deref().unwrap_or("").trim();
            if base_url.is_empty() {
                return Err(async_graphql::Error::new(
                    "memory service base URL is required",
                ));
            }
            (
                Some(base_url.to_string()),
                input.port.map(parse_memory_service_port).transpose()?,
            )
        }
        MemoryServiceMode::Managed => (None, None),
    };
    let (provider_account_id, provider_kind, model_profile, reasoning_effort) =
        match (input.provider_account_id, input.model_profile) {
            (Some(provider_account_id), Some(model_profile)) => {
                let model_profile = model_profile.trim().to_string();
                if model_profile.is_empty() {
                    return Err(async_graphql::Error::new("model profile is required"));
                }
                let account = store
                    .get_provider_account(&provider_account_id)
                    .await
                    .map_err(graphql_error)?
                    .ok_or_else(|| async_graphql::Error::new("provider account not found"))?;
                if !account.is_active || !account.is_default {
                    return Err(async_graphql::Error::new(
                        "provider account is not selectable",
                    ));
                }
                if let Some(reason) = provider_disabled_reason(&account) {
                    return Err(async_graphql::Error::new(reason));
                }
                let profiles = profiles_from_account(&account, None);
                let Some(profile) = profiles.iter().find(|profile| profile.id == model_profile)
                else {
                    return Err(async_graphql::Error::new(
                        "model profile is not available for provider",
                    ));
                };
                let reasoning_effort =
                    validate_reasoning_effort_for_profile(profile, input.reasoning_effort)?;
                (
                    Some(account.provider_account_id),
                    Some(account.provider_kind),
                    Some(model_profile),
                    reasoning_effort,
                )
            }
            (None, None) => {
                if input.reasoning_effort.is_some() {
                    return Err(async_graphql::Error::new(
                        "reasoning effort requires a provider account and model profile",
                    ));
                }
                (None, None, None, None)
            }
            (Some(_), None) => {
                return Err(async_graphql::Error::new("model profile is required"));
            }
            (None, Some(_)) => {
                return Err(async_graphql::Error::new("provider account is required"));
            }
        };

    store
        .save_memory_service_settings(SaveMemoryServiceSettings {
            mode,
            base_url,
            port,
            provider_account_id,
            provider_kind,
            model_profile,
            reasoning_effort,
        })
        .await
        .map_err(graphql_error)?;

    memory_settings_from_store(state).await
}

pub(super) async fn check_memory_service(
    state: &GraphqlState,
) -> Result<GraphqlMemoryServiceStatus> {
    let store = state.store()?;
    let settings = store
        .memory_service_settings()
        .await
        .map_err(graphql_error)?;
    memory_service_status(state, &settings).await
}

async fn memory_service_status(
    state: &GraphqlState,
    settings: &MemoryServiceSettingsRecord,
) -> Result<GraphqlMemoryServiceStatus> {
    let base_url = match settings.mode {
        MemoryServiceMode::Managed => {
            let Some(connection) = state.supermemory_connection() else {
                return managed_supermemory_unavailable_status(state).await;
            };
            connection.base_url.as_str()
        }
        MemoryServiceMode::External => settings
            .base_url
            .as_deref()
            .ok_or_else(|| async_graphql::Error::new("memory service base URL is required"))?,
    };
    let checked_at = Some(now_rfc3339()?);
    let status = match memory_service_readiness_request(base_url)?.send().await {
        Ok(response) if response.status().is_success() => GraphqlMemoryServiceStatus {
            status: GraphqlMemoryServiceStatusKind::Ready,
            checked_at,
            last_error_code: None,
            last_error_message: None,
        },
        Ok(response) if response.status().as_u16() == 401 || response.status().as_u16() == 403 => {
            GraphqlMemoryServiceStatus {
                status: GraphqlMemoryServiceStatusKind::AuthError,
                checked_at,
                last_error_code: Some("auth_error".to_string()),
                last_error_message: Some("memory service rejected authentication".to_string()),
            }
        }
        Ok(response) => GraphqlMemoryServiceStatus {
            status: GraphqlMemoryServiceStatusKind::Unavailable,
            checked_at,
            last_error_code: Some(format!("http_{}", response.status().as_u16())),
            last_error_message: Some("memory service returned an unsuccessful status".to_string()),
        },
        Err(error) => GraphqlMemoryServiceStatus {
            status: GraphqlMemoryServiceStatusKind::Unavailable,
            checked_at,
            last_error_code: Some("request_failed".to_string()),
            last_error_message: Some(sanitize_error_message(&error.to_string())),
        },
    };

    Ok(status)
}

async fn managed_supermemory_unavailable_status(
    state: &GraphqlState,
) -> Result<GraphqlMemoryServiceStatus> {
    let last_error_message = match state.paths() {
        Ok(paths) => recent_supermemory_error_message(paths).await,
        Err(_) => None,
    };
    let last_error_message = last_error_message.or_else(|| {
        state
            .supermemory_startup_error()
            .map(sanitize_error_message)
    });
    Ok(GraphqlMemoryServiceStatus {
        status: GraphqlMemoryServiceStatusKind::Unavailable,
        checked_at: Some(now_rfc3339()?),
        last_error_code: Some("supermemory_unavailable".to_string()),
        last_error_message: Some(
            last_error_message.unwrap_or_else(|| "Managed Supermemory is not running".to_string()),
        ),
    })
}

fn memory_service_readiness_request(base_url: &str) -> Result<reqwest::RequestBuilder> {
    let client = reqwest::Client::builder()
        .timeout(MEMORY_SERVICE_READINESS_TIMEOUT)
        .build()
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    Ok(client.get(base_url))
}

async fn memory_settings_from_store(state: &GraphqlState) -> Result<GraphqlMemorySettings> {
    let store = state.store()?;
    super::provider_accounts::refresh_foundation_local_availability(state).await;
    let mut accounts = store
        .active_default_provider_accounts()
        .await
        .map_err(graphql_error)?;
    refresh_missing_model_profiles(state, store, &accounts).await;
    accounts = store
        .active_default_provider_accounts()
        .await
        .map_err(graphql_error)?;
    let settings = store
        .memory_service_settings()
        .await
        .map_err(graphql_error)?;
    let status = memory_service_status(state, &settings).await?;
    Ok(memory_settings_from_parts(settings, status, &accounts))
}

fn memory_settings_from_parts(
    settings: MemoryServiceSettingsRecord,
    status: GraphqlMemoryServiceStatus,
    accounts: &[crate::ProviderAccountRecord],
) -> GraphqlMemorySettings {
    GraphqlMemorySettings {
        mode: settings.mode.into(),
        base_url: settings.base_url,
        port: settings.port.map(i32::from),
        status,
        model_preference: match (
            settings.provider_kind,
            settings.provider_account_id,
            settings.model_profile,
        ) {
            (Some(provider_kind), Some(provider_account_id), Some(model_profile)) => {
                Some(GraphqlAgentModelPreference {
                    provider_kind,
                    provider_account_id,
                    model_profile,
                    reasoning_effort: settings.reasoning_effort.map(GraphqlReasoningEffort::from),
                })
            }
            _ => None,
        },
        model_options: accounts.iter().map(option_from_account).collect(),
    }
}

async fn recent_supermemory_error_message(paths: &crate::NoemaPaths) -> Option<String> {
    let text = tokio::fs::read_to_string(paths.supermemory_data_dir().join("error.log"))
        .await
        .ok()?;
    text.lines()
        .rev()
        .find_map(sanitize_supermemory_error_log_line)
}

fn sanitize_supermemory_error_log_line(line: &str) -> Option<String> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let line = line
        .split_once("] ")
        .map_or(line, |(_prefix, message)| message)
        .trim();
    let line = line
        .strip_prefix("fatal during startup:")
        .unwrap_or(line)
        .trim();
    let message = line
        .split_once(". ")
        .map_or(line, |(first_sentence, _rest)| first_sentence)
        .trim();
    if message.is_empty() {
        None
    } else if message.ends_with('.') {
        Some(message.to_string())
    } else {
        Some(format!("{message}."))
    }
}

fn parse_memory_service_port(port: i32) -> Result<u16> {
    u16::try_from(port)
        .ok()
        .filter(|port| *port > 0)
        .ok_or_else(|| async_graphql::Error::new("memory service port must be between 1 and 65535"))
}

fn now_rfc3339() -> Result<String> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| async_graphql::Error::new(error.to_string()))
}

fn sanitize_error_message(message: &str) -> String {
    let message = message.trim();
    if message.is_empty() {
        return "memory service request failed".to_string();
    }
    message.chars().take(240).collect()
}
