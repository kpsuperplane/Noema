#![cfg_attr(not(test), allow(dead_code))]

use crate::NoemaStore;
use noema_capabilities::{CapabilityId, ToolName};
use noema_providers::{
    ProviderAccountPersistence, ProviderCapabilityAssignmentPersistence, ProviderCapabilityStatus,
    ProviderPersistenceError, system_provider_accounts,
};

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
    let Some(binding) = ProviderCapabilityAssignmentPersistence::provider_capability_assignment(
        store,
        &tool_name,
        expected_capability,
    )
    .await?
    else {
        return Ok(default_provider(tool_name.as_str()));
    };

    match load_provider_account(store, &binding.provider_account_id).await? {
        Some(account) => {
            let Some(capability) = account
                .capabilities
                .iter()
                .find(|capability| capability.capability_id == expected_capability)
            else {
                return Ok(fallback_provider(
                    tool_name.as_str(),
                    binding.provider_account_id,
                    format!(
                        "bound provider account does not declare {}",
                        expected_capability.as_str()
                    ),
                ));
            };

            if capability.status != ProviderCapabilityStatus::Available {
                return Ok(fallback_provider(
                    tool_name.as_str(),
                    binding.provider_account_id,
                    format!(
                        "bound provider capability {} is {}",
                        expected_capability.as_str(),
                        capability.status.as_str()
                    ),
                ));
            }

            Ok(ResolvedWebProvider {
                credential_revision: credential_revision(&account),
                provider_account_id: account.provider_account_id,
                provider_kind: account.provider_kind,
                account_key: account.account_key,
                fallback_from: None,
                fallback_reason: None,
            })
        }
        None => Ok(fallback_provider(
            tool_name.as_str(),
            binding.provider_account_id,
            "bound provider account is no longer available".to_string(),
        )),
    }
}

async fn load_provider_account(
    store: &NoemaStore,
    provider_account_id: &str,
) -> Result<Option<noema_providers::ProviderAccountRecord>, ProviderPersistenceError> {
    if let Some(account) =
        ProviderAccountPersistence::provider_account(store, provider_account_id).await?
    {
        return Ok(Some(account));
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
    use crate::store::tests::test_store;
    use noema_capabilities::CapabilityId;
    use noema_providers::{ProviderAccountStatus, ProviderAuthMethod, ProviderCapabilityStatus};
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

    #[tokio::test]
    async fn falls_back_when_bound_provider_account_is_missing() {
        let store = test_store().await;
        insert_binding_row(
            &store,
            WEB_SEARCH_TOOL,
            WEB_SEARCH_TOOL,
            "provider_account:openai:missing",
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
    async fn falls_back_when_bound_account_lacks_requested_capability() {
        let store = test_store().await;
        let account = store
            .ensure_default_provider_account()
            .await
            .expect("codex account");
        insert_binding_row(
            &store,
            WEB_SEARCH_TOOL,
            WEB_SEARCH_TOOL,
            &account.provider_account_id,
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
        insert_provider_account(
            &store,
            "provider_account:exa:test",
            "exa",
            "test",
            ProviderAccountStatus::Unknown,
        )
        .await;
        store
            .upsert_provider_capability_binding(
                WEB_SEARCH_TOOL,
                WEB_SEARCH_TOOL,
                "provider_account:exa:test",
            )
            .await
            .expect("save binding");

        let resolved = resolve_web_search_provider(&store).await.expect("resolve");

        assert_eq!(
            resolved.provider_account_id,
            "provider_account:duckduckgo_public:system"
        );
        assert_eq!(
            resolved.fallback_from.as_deref(),
            Some("provider_account:exa:test")
        );
        assert_eq!(
            resolved.fallback_reason.as_deref(),
            Some("bound provider capability web.search is account_dependent")
        );
    }

    #[tokio::test]
    async fn fetch_falls_back_when_stale_binding_points_to_account_without_fetch_capability() {
        let store = test_store().await;
        insert_provider_account(
            &store,
            "provider_account:openai:test",
            "openai",
            "test",
            ProviderAccountStatus::Authenticated,
        )
        .await;
        insert_binding_row(
            &store,
            WEB_FETCH_TOOL,
            WEB_FETCH_TOOL,
            "provider_account:openai:test",
        )
        .await;

        let resolved = resolve_web_fetch_provider(&store).await.expect("resolve");

        assert_eq!(
            resolved.provider_account_id,
            "provider_account:direct_http:system"
        );
        assert_eq!(
            resolved.fallback_from.as_deref(),
            Some("provider_account:openai:test")
        );
        assert_eq!(
            resolved.fallback_reason.as_deref(),
            Some("bound provider account does not declare web.fetch")
        );
    }

    #[tokio::test]
    async fn actor_accessor_uses_same_resolution_logic() {
        let store = test_store().await;
        insert_provider_account(
            &store,
            "provider_account:exa:test",
            "exa",
            "test",
            ProviderAccountStatus::Authenticated,
        )
        .await;
        store
            .upsert_provider_capability_binding(
                WEB_SEARCH_TOOL,
                WEB_SEARCH_TOOL,
                "provider_account:exa:test",
            )
            .await
            .expect("save binding");
        let actor = CodexRuntimeActor::new(
            "codex".to_string(),
            HashMap::new(),
            store.clone(),
            store.system_error_logger(),
        )
        .await
        .expect("actor");

        let resolved = actor.resolved_web_search_provider().await.expect("resolve");

        assert_eq!(resolved.provider_account_id, "provider_account:exa:test");
        assert_eq!(resolved.provider_kind, "exa");
    }

    #[tokio::test]
    async fn fetch_actor_accessor_uses_same_resolution_logic() {
        let store = test_store().await;
        let actor = CodexRuntimeActor::new(
            "codex".to_string(),
            HashMap::new(),
            store.clone(),
            store.system_error_logger(),
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

    async fn insert_provider_account(
        store: &NoemaStore,
        provider_account_id: &str,
        provider_kind: &str,
        account_key: &str,
        status: ProviderAccountStatus,
    ) {
        crate::store::tests::insert_provider_account_for_tests(
            store,
            provider_account_id,
            provider_kind,
            account_key,
            &format!("{provider_kind} {account_key}"),
            ProviderAuthMethod::SecretInput,
            false,
            status,
            serde_json::json!({}),
        )
        .await;
    }

    async fn insert_binding_row(
        store: &NoemaStore,
        tool_name: &str,
        capability_id: &str,
        provider_account_id: &str,
    ) {
        crate::store::tests::insert_provider_capability_binding_for_tests(
            store,
            tool_name,
            capability_id,
            provider_account_id,
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
