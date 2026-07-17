#![cfg_attr(not(test), allow(dead_code))]

use noema_capabilities::{CapabilityId, ToolName};
use noema_providers::{
    ProviderAccountPersistence, ProviderCapabilityAssignmentKey,
    ProviderCapabilityAssignmentPersistence, ProviderCapabilityStatus, ProviderPersistenceError,
    system_provider_accounts,
};
use noema_store::NoemaStore;

const WEB_SEARCH_TOOL: &str = "web.search";
const WEB_FETCH_TOOL: &str = "web.fetch";
const SYSTEM_ACCOUNT_KEY: &str = "system";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::daemon) struct ResolvedWebProvider {
    pub provider_account_id: String,
    pub provider_kind: String,
    pub account_key: String,
    pub credential_revision: u64,
    pub fallback_from: Option<String>,
    pub fallback_reason: Option<String>,
}

pub(in crate::daemon) async fn resolve_web_search_provider(
    store: &NoemaStore,
) -> Result<ResolvedWebProvider, ProviderPersistenceError> {
    resolve_bound_provider(store, WEB_SEARCH_TOOL, CapabilityId::WebSearch).await
}

pub(in crate::daemon) async fn resolve_web_fetch_provider(
    store: &NoemaStore,
) -> Result<ResolvedWebProvider, ProviderPersistenceError> {
    resolve_bound_provider(store, WEB_FETCH_TOOL, CapabilityId::WebFetch).await
}

async fn resolve_bound_provider(
    store: &NoemaStore,
    tool_name: &str,
    expected_capability: CapabilityId,
) -> Result<ResolvedWebProvider, ProviderPersistenceError> {
    let tool_name =
        ToolName::new(tool_name).map_err(|_| ProviderPersistenceError::InvalidRequest {
            kind: "web_tool_name",
        })?;
    let key = ProviderCapabilityAssignmentKey::new(tool_name.clone(), expected_capability)?;
    let Some(binding) =
        ProviderCapabilityAssignmentPersistence::provider_capability_assignment(store, &key)
            .await?
    else {
        return Ok(default_provider(tool_name.as_str()));
    };

    let provider_account_id = binding.provider_account_id;
    let account = load_provider_account(store, &provider_account_id).await?;
    Ok(resolve_bound_account(
        tool_name.as_str(),
        expected_capability,
        provider_account_id,
        account,
    ))
}

fn resolve_bound_account(
    tool_name: &str,
    expected_capability: CapabilityId,
    provider_account_id: String,
    account: Option<noema_providers::ProviderAccountRecord>,
) -> ResolvedWebProvider {
    match account {
        Some(account) => {
            let Some(capability) = account
                .capabilities
                .iter()
                .find(|capability| capability.capability_id == expected_capability)
            else {
                return fallback_provider(
                    tool_name,
                    provider_account_id,
                    format!(
                        "bound provider account does not declare {}",
                        expected_capability.as_str()
                    ),
                );
            };

            if capability.status != ProviderCapabilityStatus::Available {
                return fallback_provider(
                    tool_name,
                    provider_account_id,
                    format!(
                        "bound provider capability {} is {}",
                        expected_capability.as_str(),
                        capability.status.as_str()
                    ),
                );
            }

            ResolvedWebProvider {
                credential_revision: credential_revision(&account),
                provider_account_id: account.provider_account_id,
                provider_kind: account.provider_kind,
                account_key: account.account_key,
                fallback_from: None,
                fallback_reason: None,
            }
        }
        None => fallback_provider(
            tool_name,
            provider_account_id,
            "bound provider account is no longer available".to_string(),
        ),
    }
}

async fn load_provider_account(
    store: &NoemaStore,
    provider_account_id: &str,
) -> Result<Option<noema_providers::ProviderAccountRecord>, ProviderPersistenceError> {
    if let Some(account) =
        ProviderAccountPersistence::provider_account(store, provider_account_id).await?
    {
        return Ok(Some(noema_providers::provider_account_from_persisted(
            account,
        )));
    }

    Ok(system_provider_accounts()
        .into_iter()
        .find(|account| account.provider_account_id == provider_account_id))
}

fn default_provider(tool_name: &str) -> ResolvedWebProvider {
    match tool_name {
        WEB_SEARCH_TOOL => ResolvedWebProvider {
            provider_account_id: format!(
                "provider_account:{}:{SYSTEM_ACCOUNT_KEY}",
                crate::search::types::DUCKDUCKGO_PUBLIC_PROVIDER_ID
            ),
            provider_kind: crate::search::types::DUCKDUCKGO_PUBLIC_PROVIDER_ID.to_string(),
            account_key: SYSTEM_ACCOUNT_KEY.to_string(),
            credential_revision: 0,
            fallback_from: None,
            fallback_reason: None,
        },
        WEB_FETCH_TOOL => ResolvedWebProvider {
            provider_account_id: format!(
                "provider_account:{}:{SYSTEM_ACCOUNT_KEY}",
                crate::web_fetch::types::DIRECT_HTTP_PROVIDER_ID
            ),
            provider_kind: crate::web_fetch::types::DIRECT_HTTP_PROVIDER_ID.to_string(),
            account_key: SYSTEM_ACCOUNT_KEY.to_string(),
            credential_revision: 0,
            fallback_from: None,
            fallback_reason: None,
        },
        _ => unreachable!("unsupported web tool provider resolver"),
    }
}

fn fallback_provider(
    tool_name: &str,
    provider_account_id: String,
    fallback_reason: String,
) -> ResolvedWebProvider {
    let mut fallback = default_provider(tool_name);
    fallback.fallback_from = Some(provider_account_id);
    fallback.fallback_reason = Some(fallback_reason);
    fallback
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
    use crate::daemon::runtime::actor::CodexRuntimeActor;
    use crate::test_support::test_store;
    use noema_capabilities::CapabilityId;
    use noema_providers::{
        ProviderAccountStatus, ProviderCapabilityAccountReference, ProviderCapabilityStatus,
    };
    use std::collections::HashMap;

    #[tokio::test]
    async fn resolves_duckduckgo_default_without_binding() {
        let store = test_store().await;
        let resolved = resolve_web_search_provider(&store).await.expect("resolve");

        assert_eq!(
            resolved.provider_account_id,
            "provider_account:duckduckgo_public:system"
        );
        assert_eq!(resolved.provider_kind, "duckduckgo_public");
        assert_eq!(resolved.account_key, "system");
        assert!(resolved.fallback_from.is_none());
        assert!(resolved.fallback_reason.is_none());
    }

    #[tokio::test]
    async fn resolves_direct_http_default_without_binding() {
        let store = test_store().await;
        let resolved = resolve_web_fetch_provider(&store).await.expect("resolve");

        assert_eq!(
            resolved.provider_account_id,
            "provider_account:direct_http:system"
        );
        assert_eq!(resolved.provider_kind, "direct_http");
        assert_eq!(resolved.account_key, "system");
        assert!(resolved.fallback_from.is_none());
        assert!(resolved.fallback_reason.is_none());
    }

    #[test]
    fn falls_back_when_bound_provider_account_is_missing() {
        let resolved = resolve_bound_account(
            WEB_SEARCH_TOOL,
            CapabilityId::WebSearch,
            "provider_account:openai:missing".to_string(),
            None,
        );

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
    async fn falls_back_when_bound_account_lacks_requested_capability() {
        let store = test_store().await;
        let account = store
            .ensure_default_provider_account()
            .await
            .expect("codex account");
        save_binding(
            &store,
            WEB_SEARCH_TOOL,
            WEB_SEARCH_TOOL,
            ProviderCapabilityAccountReference::persisted(account.provider_account_id.clone()),
        )
        .await;

        let resolved = resolve_web_search_provider(&store).await.expect("resolve");

        assert_eq!(
            resolved.provider_account_id,
            "provider_account:duckduckgo_public:system"
        );
        assert_eq!(
            resolved.fallback_from.as_deref(),
            Some(account.provider_account_id.as_str())
        );
        assert_eq!(
            resolved.fallback_reason.as_deref(),
            Some("bound provider account does not declare web.search")
        );
    }

    #[tokio::test]
    async fn falls_back_when_bound_capability_is_not_available() {
        let store = test_store().await;
        let provider_account_id =
            create_exa_provider_account(&store, ProviderAccountStatus::Unknown).await;
        save_binding(
            &store,
            WEB_SEARCH_TOOL,
            WEB_SEARCH_TOOL,
            ProviderCapabilityAccountReference::persisted(provider_account_id.clone()),
        )
        .await;

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
    async fn fetch_falls_back_when_stale_binding_points_to_account_without_fetch_capability() {
        let store = test_store().await;
        let account = store
            .ensure_default_provider_account()
            .await
            .expect("codex account");
        save_binding(
            &store,
            WEB_FETCH_TOOL,
            WEB_FETCH_TOOL,
            ProviderCapabilityAccountReference::persisted(account.provider_account_id.clone()),
        )
        .await;

        let resolved = resolve_web_fetch_provider(&store).await.expect("resolve");

        assert_eq!(
            resolved.provider_account_id,
            "provider_account:direct_http:system"
        );
        assert_eq!(
            resolved.fallback_from.as_deref(),
            Some(account.provider_account_id.as_str())
        );
        assert_eq!(
            resolved.fallback_reason.as_deref(),
            Some("bound provider account does not declare web.fetch")
        );
    }

    #[tokio::test]
    async fn actor_accessor_uses_same_resolution_logic() {
        let store = test_store().await;
        let provider_account_id =
            create_exa_provider_account(&store, ProviderAccountStatus::Authenticated).await;
        save_binding(
            &store,
            WEB_SEARCH_TOOL,
            WEB_SEARCH_TOOL,
            ProviderCapabilityAccountReference::persisted(provider_account_id.clone()),
        )
        .await;
        let actor = CodexRuntimeActor::new(
            "codex".to_string(),
            HashMap::from([(
                "codex".to_string(),
                crate::test_support::ready_test_provider(),
            )]),
            store.clone(),
            crate::test_support::system_error_logger(),
        )
        .await
        .expect("actor");

        let resolved = actor.resolved_web_search_provider().await.expect("resolve");

        assert_eq!(resolved.provider_account_id, provider_account_id);
        assert_eq!(resolved.provider_kind, "exa");
    }

    #[tokio::test]
    async fn fetch_actor_accessor_uses_same_resolution_logic() {
        let store = test_store().await;
        let actor = CodexRuntimeActor::new(
            "codex".to_string(),
            HashMap::from([(
                "codex".to_string(),
                crate::test_support::ready_test_provider(),
            )]),
            store.clone(),
            crate::test_support::system_error_logger(),
        )
        .await
        .expect("actor");

        let resolved = actor.resolved_web_fetch_provider().await.expect("resolve");

        assert_eq!(
            resolved.provider_account_id,
            "provider_account:direct_http:system"
        );
        assert_eq!(resolved.provider_kind, "direct_http");
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

    #[test]
    fn provider_capability_status_strings_distinguish_available_and_account_dependent() {
        assert_ne!(
            ProviderCapabilityStatus::Available.as_str(),
            ProviderCapabilityStatus::AccountDependent.as_str()
        );
        assert_eq!(CapabilityId::WebSearch.as_str(), WEB_SEARCH_TOOL);
        assert_eq!(CapabilityId::WebFetch.as_str(), WEB_FETCH_TOOL);
    }
}
