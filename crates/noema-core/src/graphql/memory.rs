use async_graphql::{Enum, InputObject, Result, SimpleObject};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{
    MemoryServiceMode, MemoryServiceSettingsRecord, MemoryServiceStatus, MemoryServiceStatusRecord,
    SaveMemoryServiceSettings,
};

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

impl From<MemoryServiceStatus> for GraphqlMemoryServiceStatusKind {
    fn from(value: MemoryServiceStatus) -> Self {
        match value {
            MemoryServiceStatus::NotConfigured => Self::NotConfigured,
            MemoryServiceStatus::Starting => Self::Starting,
            MemoryServiceStatus::Ready => Self::Ready,
            MemoryServiceStatus::Unavailable => Self::Unavailable,
            MemoryServiceStatus::AuthError => Self::AuthError,
        }
    }
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

impl From<MemoryServiceStatusRecord> for GraphqlMemoryServiceStatus {
    fn from(record: MemoryServiceStatusRecord) -> Self {
        Self {
            status: record.status.into(),
            checked_at: record.checked_at,
            last_error_code: record.last_error_code,
            last_error_message: record.last_error_message,
        }
    }
}

/// Memory service settings exposed to Settings.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "MemorySettings")]
pub struct GraphqlMemorySettings {
    /// Supermemory service mode.
    pub mode: GraphqlMemoryServiceMode,
    /// Base URL for the Supermemory service.
    pub base_url: String,
    /// Managed service port, when configured.
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
    /// Base URL for the Supermemory service.
    pub base_url: String,
    /// Managed service port, when configured.
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
    let base_url = input.base_url.trim();
    if base_url.is_empty() {
        return Err(async_graphql::Error::new(
            "memory service base URL is required",
        ));
    }
    let port = input.port.map(parse_memory_service_port).transpose()?;
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
            (None, None) => (None, None, None, None),
            (Some(_), None) => {
                return Err(async_graphql::Error::new("model profile is required"));
            }
            (None, Some(_)) => {
                return Err(async_graphql::Error::new("provider account is required"));
            }
        };

    store
        .save_memory_service_settings(SaveMemoryServiceSettings {
            mode: input.mode.into(),
            base_url: base_url.to_string(),
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
    let checked_at = Some(now_rfc3339()?);
    let status = match reqwest::Client::new().get(&settings.base_url).send().await {
        Ok(response) if response.status().is_success() => MemoryServiceStatusRecord {
            status_id: "default".to_string(),
            status: MemoryServiceStatus::Ready,
            checked_at,
            last_error_code: None,
            last_error_message: None,
        },
        Ok(response) if response.status().as_u16() == 401 || response.status().as_u16() == 403 => {
            MemoryServiceStatusRecord {
                status_id: "default".to_string(),
                status: MemoryServiceStatus::AuthError,
                checked_at,
                last_error_code: Some("auth_error".to_string()),
                last_error_message: Some("memory service rejected authentication".to_string()),
            }
        }
        Ok(response) => MemoryServiceStatusRecord {
            status_id: "default".to_string(),
            status: MemoryServiceStatus::Unavailable,
            checked_at,
            last_error_code: Some(format!("http_{}", response.status().as_u16())),
            last_error_message: Some("memory service returned an unsuccessful status".to_string()),
        },
        Err(error) => MemoryServiceStatusRecord {
            status_id: "default".to_string(),
            status: MemoryServiceStatus::Unavailable,
            checked_at,
            last_error_code: Some("request_failed".to_string()),
            last_error_message: Some(sanitize_error_message(&error.to_string())),
        },
    };

    let saved = store
        .save_memory_service_status(status)
        .await
        .map_err(graphql_error)?;
    Ok(saved.into())
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
    let status = store.memory_service_status().await.map_err(graphql_error)?;
    Ok(memory_settings_from_parts(settings, status, &accounts))
}

fn memory_settings_from_parts(
    settings: MemoryServiceSettingsRecord,
    status: MemoryServiceStatusRecord,
    accounts: &[crate::ProviderAccountRecord],
) -> GraphqlMemorySettings {
    GraphqlMemorySettings {
        mode: settings.mode.into(),
        base_url: settings.base_url,
        port: settings.port.map(i32::from),
        status: status.into(),
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
