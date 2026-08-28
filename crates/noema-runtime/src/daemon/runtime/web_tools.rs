#![cfg_attr(not(test), allow(dead_code))]

use noema_capabilities::{
    CapabilityDestination, CapabilityDestinationError, CapabilityId, ToolName,
};
use noema_providers::{
    ProviderAccountPersistence, ProviderAuthMethod, ProviderCapabilityAssignmentKey,
    ProviderCapabilityAssignmentPersistence, ProviderCapabilityStatus, ProviderPersistenceError,
    default_web_provider_account_id,
};
use noema_store::NoemaStore;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::daemon) struct ResolvedWebProvider {
    pub provider_account_id: String,
    pub provider_kind: String,
    pub account_key: String,
    pub auth_method: ProviderAuthMethod,
    pub credential_revision: u64,
    pub capability_status: ProviderCapabilityStatus,
    pub fallback_from: Option<String>,
    pub fallback_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::daemon) struct ResolvedBrowserProviderRoute {
    pub providers: Vec<ResolvedWebProvider>,
    pub digest: String,
}

pub(in crate::daemon) async fn resolve_web_search_provider(
    store: &NoemaStore,
) -> Result<ResolvedWebProvider, ProviderPersistenceError> {
    resolve_bound_provider(store, CapabilityId::WebSearch).await
}

pub(in crate::daemon) async fn resolve_web_fetch_provider(
    store: &NoemaStore,
) -> Result<ResolvedWebProvider, ProviderPersistenceError> {
    resolve_bound_provider(store, CapabilityId::WebFetch).await
}

pub(in crate::daemon) async fn resolve_web_browse_route(
    store: &NoemaStore,
) -> Result<ResolvedBrowserProviderRoute, ProviderPersistenceError> {
    let key = ProviderCapabilityAssignmentKey::new(
        ToolName::new(CapabilityId::WebBrowse.as_str()).map_err(|_| {
            ProviderPersistenceError::InvalidRequest {
                kind: "web_tool_name",
            }
        })?,
        CapabilityId::WebBrowse,
    )?;
    let assignments =
        ProviderCapabilityAssignmentPersistence::provider_capability_route(store, &key).await?;
    let providers = if assignments.is_empty() {
        vec![load_default_provider(store, CapabilityId::WebBrowse).await?]
    } else {
        let mut providers = Vec::with_capacity(assignments.len());
        for assignment in assignments {
            let provider_account_id = assignment.provider_account_id;
            let account = load_provider_account(store, &provider_account_id)
                .await?
                .ok_or_else(|| ProviderPersistenceError::AccountNotFound {
                    provider_account_id: provider_account_id.clone(),
                })?;
            let Some(capability) = account
                .capabilities
                .iter()
                .find(|capability| capability.capability_id == CapabilityId::WebBrowse)
            else {
                return Err(ProviderPersistenceError::InvalidRequest {
                    kind: "browser_provider_route_capability",
                });
            };
            providers.push(ResolvedWebProvider {
                credential_revision: credential_revision(&account),
                capability_status: capability.status,
                auth_method: account.auth_method,
                provider_account_id: account.provider_account_id,
                provider_kind: account.provider_kind,
                account_key: account.account_key,
                fallback_from: None,
                fallback_reason: None,
            });
        }
        providers
    };
    let bytes = serde_json::to_vec(
        &providers
            .iter()
            .map(|provider| {
                (
                    provider.provider_account_id.as_str(),
                    provider.provider_kind.as_str(),
                    provider.account_key.as_str(),
                    provider.auth_method.as_str(),
                    provider.credential_revision,
                    provider.capability_status.as_str(),
                )
            })
            .collect::<Vec<_>>(),
    )
    .map_err(|_| ProviderPersistenceError::Invariant {
        operation: "browser_provider_route_digest",
    })?;
    let digest = ring::digest::digest(&ring::digest::SHA256, &bytes)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Ok(ResolvedBrowserProviderRoute { providers, digest })
}

pub(in crate::daemon) async fn web_provider_override_exists(
    store: &NoemaStore,
) -> Result<bool, ProviderPersistenceError> {
    for (tool_name, capability_id) in [
        (CapabilityId::WebSearch.as_str(), CapabilityId::WebSearch),
        (CapabilityId::WebFetch.as_str(), CapabilityId::WebFetch),
    ] {
        let key = ProviderCapabilityAssignmentKey::new(
            ToolName::new(tool_name).map_err(|_| ProviderPersistenceError::InvalidRequest {
                kind: "web_tool_name",
            })?,
            capability_id,
        )?;
        if ProviderCapabilityAssignmentPersistence::provider_capability_assignment(store, &key)
            .await?
            .is_some()
        {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(in crate::daemon) async fn resolve_web_destination(
    store: &NoemaStore,
    tool_name: &str,
) -> Result<CapabilityDestination, ProviderPersistenceError> {
    if tool_name.starts_with("web.browse.") {
        let route = resolve_web_browse_route(store).await?;
        return CapabilityDestination::new(
            CapabilityId::WebBrowse.as_str(),
            "browser_provider_route",
            None::<String>,
            format!("route:{}", route.digest),
        )
        .map_err(destination_error);
    }
    let resolved = match tool_name {
        name if name == CapabilityId::WebSearch.as_str() => {
            resolve_web_search_provider(store).await?
        }
        name if name == CapabilityId::WebFetch.as_str() => {
            resolve_web_fetch_provider(store).await?
        }
        noema_capabilities::file::FILE_DOWNLOAD_TOOL => {
            load_default_provider(store, CapabilityId::WebFetch).await?
        }
        _ => {
            return Err(ProviderPersistenceError::InvalidRequest {
                kind: "web_tool_name",
            });
        }
    };
    CapabilityDestination::new(
        resolved.provider_kind,
        resolved.provider_account_id,
        Some(resolved.account_key),
        format!("credential:{}", resolved.credential_revision),
    )
    .map_err(destination_error)
}

fn destination_error(_error: CapabilityDestinationError) -> ProviderPersistenceError {
    ProviderPersistenceError::InvalidRequest {
        kind: "web_provider_destination",
    }
}

async fn resolve_bound_provider(
    store: &NoemaStore,
    expected_capability: CapabilityId,
) -> Result<ResolvedWebProvider, ProviderPersistenceError> {
    let tool_name = ToolName::new(expected_capability.as_str()).map_err(|_| {
        ProviderPersistenceError::InvalidRequest {
            kind: "web_tool_name",
        }
    })?;
    let key = ProviderCapabilityAssignmentKey::new(tool_name.clone(), expected_capability)?;
    let Some(binding) =
        ProviderCapabilityAssignmentPersistence::provider_capability_assignment(store, &key)
            .await?
    else {
        return load_default_provider(store, expected_capability).await;
    };

    let provider_account_id = binding.provider_account_id;
    let account = load_provider_account(store, &provider_account_id).await?;
    let selected = match account {
        Some(account) => resolved_account(account, expected_capability),
        None => Err("bound provider account is no longer available".to_string()),
    };
    match selected {
        Ok(provider) if provider.capability_status == ProviderCapabilityStatus::Available => {
            Ok(provider)
        }
        Ok(provider) => {
            let reason = format!(
                "bound provider capability {} is {}",
                expected_capability.as_str(),
                provider.capability_status.as_str()
            );
            fallback_provider(store, expected_capability, provider_account_id, reason).await
        }
        Err(reason) => {
            fallback_provider(store, expected_capability, provider_account_id, reason).await
        }
    }
}

async fn load_provider_account(
    store: &NoemaStore,
    provider_account_id: &str,
) -> Result<Option<noema_providers::ProviderAccountRecord>, ProviderPersistenceError> {
    ProviderAccountPersistence::provider_account(store, provider_account_id).await
}

fn resolved_account(
    account: noema_providers::ProviderAccountRecord,
    capability_id: CapabilityId,
) -> Result<ResolvedWebProvider, String> {
    let capability = account
        .capabilities
        .iter()
        .find(|capability| capability.capability_id == capability_id)
        .ok_or_else(|| {
            format!(
                "bound provider account does not declare {}",
                capability_id.as_str()
            )
        })?;
    Ok(ResolvedWebProvider {
        credential_revision: credential_revision(&account),
        capability_status: capability.status,
        auth_method: account.auth_method,
        provider_account_id: account.provider_account_id,
        provider_kind: account.provider_kind,
        account_key: account.account_key,
        fallback_from: None,
        fallback_reason: None,
    })
}

pub(in crate::daemon) async fn load_default_provider(
    store: &NoemaStore,
    capability_id: CapabilityId,
) -> Result<ResolvedWebProvider, ProviderPersistenceError> {
    let account_id = default_web_provider_account_id(capability_id).ok_or(
        ProviderPersistenceError::InvalidRequest {
            kind: "web_capability_default",
        },
    )?;
    let account = load_provider_account(store, account_id).await?.ok_or(
        ProviderPersistenceError::Invariant {
            operation: "load_default_web_provider_account",
        },
    )?;
    let resolved = resolved_account(account, capability_id).map_err(|_| {
        ProviderPersistenceError::Invariant {
            operation: "resolve_default_web_provider_account",
        }
    })?;
    if resolved.capability_status != ProviderCapabilityStatus::Available {
        return Err(ProviderPersistenceError::Invariant {
            operation: "default_web_provider_account_unavailable",
        });
    }
    Ok(resolved)
}

async fn fallback_provider(
    store: &NoemaStore,
    capability_id: CapabilityId,
    provider_account_id: String,
    fallback_reason: String,
) -> Result<ResolvedWebProvider, ProviderPersistenceError> {
    let mut fallback = load_default_provider(store, capability_id).await?;
    fallback.fallback_from = Some(provider_account_id);
    fallback.fallback_reason = Some(fallback_reason);
    Ok(fallback)
}

fn credential_revision(account: &noema_providers::ProviderAccountRecord) -> u64 {
    account
        .metadata
        .get("credentialRevision")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::test_store;
    use noema_capabilities::CapabilityId;
    use noema_providers::{
        NewProviderAccount, ProviderAccountPersistence, ProviderAccountStatus, ProviderAuthMethod,
        ProviderCapabilityAccountReference, ReplaceProviderCapabilityRouteRequest,
    };

    #[tokio::test]
    async fn resolves_system_defaults_without_bindings() {
        let store = test_store().await;
        let search = resolve_web_search_provider(&store).await.expect("search");
        let fetch = resolve_web_fetch_provider(&store).await.expect("fetch");

        assert_eq!(
            search.provider_account_id,
            "provider_account:duckduckgo_public:system"
        );
        assert_eq!(search.provider_kind, "duckduckgo_public");
        assert_eq!(
            fetch.provider_account_id,
            "provider_account:direct_http:system"
        );
        assert_eq!(fetch.provider_kind, "direct_http");
        assert!([search, fetch].iter().all(|resolved| {
            resolved.account_key == "system"
                && resolved.auth_method == ProviderAuthMethod::None
                && resolved.fallback_from.is_none()
                && resolved.fallback_reason.is_none()
        }));
    }

    #[tokio::test]
    async fn falls_back_when_bound_provider_account_is_missing() {
        let store = test_store().await;
        save_binding(
            &store,
            CapabilityId::WebSearch.as_str(),
            CapabilityId::WebSearch.as_str(),
            ProviderCapabilityAccountReference::persisted("provider_account:openai:missing"),
        )
        .await;

        let resolved = resolve_web_search_provider(&store).await.expect("resolve");

        assert_eq!(
            resolved.provider_account_id,
            "provider_account:duckduckgo_public:system"
        );
        assert_eq!(
            resolved.fallback_from.as_deref(),
            Some("provider_account:openai:missing")
        );
        assert_eq!(
            resolved.fallback_reason.as_deref(),
            Some("bound provider account is no longer available")
        );
    }

    #[tokio::test]
    async fn falls_back_when_bound_capability_is_not_available() {
        let store = test_store().await;
        let provider_account_id =
            create_exa_provider_account(&store, ProviderAccountStatus::Authenticated).await;
        save_binding(
            &store,
            CapabilityId::WebSearch.as_str(),
            CapabilityId::WebSearch.as_str(),
            ProviderCapabilityAccountReference::persisted(provider_account_id.clone()),
        )
        .await;
        store
            .update_provider_account_status(
                &provider_account_id,
                ProviderAccountStatus::Unknown,
                None,
                None,
            )
            .await
            .expect("make Exa unavailable");

        let resolved = resolve_web_search_provider(&store).await.expect("resolve");

        assert_eq!(
            resolved.provider_account_id,
            "provider_account:duckduckgo_public:system"
        );
        assert_eq!(
            resolved.fallback_from.as_deref(),
            Some(provider_account_id.as_str())
        );
        assert_eq!(
            resolved.fallback_reason.as_deref(),
            Some("bound provider capability web.search is account_dependent")
        );
    }

    #[tokio::test]
    async fn browser_route_digest_fences_capability_state_changes() {
        let store = test_store().await;
        let kernel = ProviderAccountPersistence::create_provider_account(
            &store,
            NewProviderAccount {
                provider_kind: "kernel".to_string(),
                display_name: None,
                auth_method: ProviderAuthMethod::SecretInput,
                status: ProviderAccountStatus::Authenticated,
                metadata: serde_json::json!({"credentialRevision": 2}),
            },
        )
        .await
        .expect("Kernel account");
        ProviderCapabilityAssignmentPersistence::replace_provider_capability_route(
            &store,
            ReplaceProviderCapabilityRouteRequest::new(
                ToolName::new(CapabilityId::WebBrowse.as_str()).expect("browse tool"),
                CapabilityId::WebBrowse,
                vec![
                    ProviderCapabilityAccountReference::persisted(
                        "provider_account:obscura:system",
                    ),
                    ProviderCapabilityAccountReference::persisted(
                        kernel.provider_account_id.clone(),
                    ),
                ],
            )
            .expect("browser route"),
        )
        .await
        .expect("save browser route");
        let available = resolve_web_browse_route(&store)
            .await
            .expect("available route");

        store
            .update_provider_account_status(
                &kernel.provider_account_id,
                ProviderAccountStatus::Unauthenticated,
                Some("auth_failed"),
                None,
            )
            .await
            .expect("change capability state");
        let unavailable = resolve_web_browse_route(&store)
            .await
            .expect("unavailable route");

        assert_ne!(available.digest, unavailable.digest);
        assert_ne!(
            available.providers[1].capability_status,
            unavailable.providers[1].capability_status
        );
    }

    async fn create_exa_provider_account(
        store: &NoemaStore,
        status: ProviderAccountStatus,
    ) -> String {
        crate::test_support::create_exa_provider_account_for_tests(
            store,
            "Exa test",
            status,
            serde_json::json!({}),
        )
        .await
        .provider_account_id
    }

    async fn save_binding(
        store: &NoemaStore,
        tool_name: &str,
        capability_id: &str,
        account_reference: ProviderCapabilityAccountReference,
    ) {
        crate::test_support::save_provider_capability_assignment_for_tests(
            store,
            tool_name,
            capability_id,
            account_reference,
        )
        .await;
    }
}
