use super::*;

pub(in crate::graphql) async fn check_memory_service(
    state: &GraphqlState,
) -> Result<GraphqlMemoryServiceStatus> {
    let memory_service = resolve_memory_service(state).await?;
    memory_service_status(state, &memory_service).await
}

pub(super) async fn memory_service_status(
    state: &GraphqlState,
    memory_service: &MemoryServiceSnapshot,
) -> Result<GraphqlMemoryServiceStatus> {
    let settings = memory_service.settings();
    if settings.mode == MemoryServiceMode::External && settings.base_url.is_none() {
        return Err(async_graphql::Error::new(
            "memory service base URL is required",
        ));
    }
    let Some(memory_operations) = memory_service.operations() else {
        return missing_memory_operations_status(state, settings).await;
    };
    let checked_at = Some(now_rfc3339()?);
    let status = match memory_operations.check_readiness().await {
        Ok(readiness) if readiness.ready => GraphqlMemoryServiceStatus {
            status: GraphqlMemoryServiceStatusKind::Ready,
            checked_at,
            last_error_code: None,
            last_error_message: None,
        },
        Ok(_) => GraphqlMemoryServiceStatus {
            status: GraphqlMemoryServiceStatusKind::Unavailable,
            checked_at,
            last_error_code: Some("not_ready".to_string()),
            last_error_message: Some("memory service is not ready".to_string()),
        },
        Err(MemoryOperationError::ServiceUnavailable)
            if settings.mode == MemoryServiceMode::Managed =>
        {
            return managed_memory_unavailable_status(state).await;
        }
        Err(error) => GraphqlMemoryServiceStatus {
            status: if error == MemoryOperationError::AuthenticationRejected {
                GraphqlMemoryServiceStatusKind::AuthError
            } else {
                GraphqlMemoryServiceStatusKind::Unavailable
            },
            checked_at,
            last_error_code: Some(match error {
                MemoryOperationError::TimedOut => "request_failed".to_string(),
                MemoryOperationError::UnsuccessfulStatus(status) => {
                    format!("http_{status}")
                }
                _ => error.code().to_string(),
            }),
            last_error_message: Some(error.to_string()),
        },
    };

    Ok(status)
}

pub(super) async fn managed_memory_unavailable_status(
    state: &GraphqlState,
) -> Result<GraphqlMemoryServiceStatus> {
    let last_error_message = state.memory_startup_error().map(sanitize_error_message);
    Ok(GraphqlMemoryServiceStatus {
        status: GraphqlMemoryServiceStatusKind::Unavailable,
        checked_at: Some(now_rfc3339()?),
        last_error_code: Some("mnemosyne_unavailable".to_string()),
        last_error_message: Some(
            last_error_message.unwrap_or_else(|| "Managed Mnemosyne is not running".to_string()),
        ),
    })
}

pub(super) async fn missing_memory_operations_status(
    state: &GraphqlState,
    settings: &MemoryServiceSettingsRecord,
) -> Result<GraphqlMemoryServiceStatus> {
    match settings.mode {
        MemoryServiceMode::Managed => managed_memory_unavailable_status(state).await,
        MemoryServiceMode::External if settings.base_url.is_none() => {
            Ok(GraphqlMemoryServiceStatus {
                status: GraphqlMemoryServiceStatusKind::NotConfigured,
                checked_at: Some(now_rfc3339()?),
                last_error_code: Some("mnemosyne_not_configured".to_string()),
                last_error_message: Some("memory service base URL is required".to_string()),
            })
        }
        MemoryServiceMode::External => Ok(GraphqlMemoryServiceStatus {
            status: GraphqlMemoryServiceStatusKind::Unavailable,
            checked_at: Some(now_rfc3339()?),
            last_error_code: Some(MemoryOperationError::ServiceUnavailable.code().to_string()),
            last_error_message: Some(MemoryOperationError::ServiceUnavailable.to_string()),
        }),
    }
}

pub(super) async fn memory_settings_from_store(
    state: &GraphqlState,
) -> Result<GraphqlMemorySettings> {
    let store = state.store()?;
    let accounts = active_default_model_accounts(state).await?;
    let memory_service = resolve_memory_service(state).await?;
    let settings = memory_service.settings().clone();
    let status = memory_service_status(state, &memory_service).await?;
    let model_options = model_options_from_accounts(store, &accounts).await?;
    Ok(memory_settings_from_parts(settings, status, model_options))
}

pub(super) fn memory_settings_from_parts(
    settings: MemoryServiceSettingsRecord,
    status: GraphqlMemoryServiceStatus,
    model_options: Vec<GraphqlAgentModelProviderOption>,
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
        model_options,
    }
}

pub(super) fn parse_memory_service_port(port: i32) -> Result<u16> {
    u16::try_from(port)
        .ok()
        .filter(|port| *port > 0)
        .ok_or_else(|| async_graphql::Error::new("memory service port must be between 1 and 65535"))
}

pub(super) fn now_rfc3339() -> Result<String> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| async_graphql::Error::new(error.to_string()))
}

pub(super) fn sanitize_error_message(message: &str) -> String {
    let message = message.trim();
    if message.is_empty() {
        return "memory service request failed".to_string();
    }
    message.chars().take(240).collect()
}
