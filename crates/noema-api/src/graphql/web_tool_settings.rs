use async_graphql::{InputObject, Result, SimpleObject};
use noema_providers::{
    ProviderAccountRecord, ProviderCapability, ProviderCapabilityAccountReference,
    ProviderCapabilityAssignmentKey, ProviderCapabilityAssignmentPersistence,
    ProviderCapabilityStatus, ReplaceProviderCapabilityRouteRequest,
    UpsertProviderCapabilityAssignmentRequest, default_web_provider_account_id,
};

use noema_capabilities::{CapabilityId, ToolName};
use noema_store::NoemaStore;

use super::{errors::graphql_error, schema::GraphqlState};

#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "WebToolSettings")]
pub struct GraphqlWebToolSettings {
    pub search: GraphqlWebToolBindingSettings,
    pub fetch: GraphqlWebToolBindingSettings,
    pub browse: GraphqlWebToolBindingSettings,
}

#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "WebToolBindingSettings")]
pub struct GraphqlWebToolBindingSettings {
    pub tool_name: String,
    pub capability_id: String,
    pub active_provider_account_id: String,
    pub provider_route_account_ids: Vec<String>,
    pub provider_options: Vec<GraphqlWebToolProviderOption>,
}

#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "WebToolProviderOption")]
pub struct GraphqlWebToolProviderOption {
    pub provider_account_id: String,
    pub provider_kind: String,
    pub account_key: String,
    pub display_name: String,
    pub capability_id: String,
    pub reliability_contract: String,
    pub data_flow_class: String,
    pub citations: bool,
    pub direct_url_fetch: bool,
    pub js_rendering: bool,
    pub authenticated_context: bool,
}

#[derive(Clone, Debug, InputObject)]
#[graphql(name = "SaveWebToolProviderBindingInput")]
pub struct GraphqlSaveWebToolProviderBindingInput {
    pub tool_name: String,
    pub capability_id: String,
    pub provider_account_id: String,
}

#[derive(Clone, Debug, InputObject)]
#[graphql(name = "SaveBrowserProviderRouteInput")]
pub struct GraphqlSaveBrowserProviderRouteInput {
    pub provider_account_ids: Vec<String>,
}

#[derive(Clone, Debug)]
struct SelectableProviderAccount {
    account: ProviderAccountRecord,
    reference: ProviderCapabilityAccountReference,
}

#[derive(Clone, Debug)]
struct NativeWebProvider {
    account: ProviderAccountRecord,
    display_name: String,
}

pub(super) async fn web_tool_settings(state: &GraphqlState) -> Result<GraphqlWebToolSettings> {
    let store = state.store()?;
    let accounts = selectable_accounts(state).await?;
    let native_provider = native_web_provider(state, &accounts).await?;
    let has_override = web_provider_override_exists(store).await?;

    Ok(GraphqlWebToolSettings {
        search: binding_settings(
            store,
            &accounts,
            native_provider.as_ref(),
            !has_override,
            CapabilityId::WebSearch,
        )
        .await?,
        fetch: binding_settings(
            store,
            &accounts,
            native_provider.as_ref(),
            !has_override,
            CapabilityId::WebFetch,
        )
        .await?,
        browse: binding_settings(store, &accounts, None, false, CapabilityId::WebBrowse).await?,
    })
}

async fn native_web_provider(
    state: &GraphqlState,
    accounts: &[SelectableProviderAccount],
) -> Result<Option<NativeWebProvider>> {
    let store = state.store()?;
    if store
        .get_agent_runtime_preference("agent:primary")
        .await
        .map_err(graphql_error)?
        .is_none()
    {
        return Ok(None);
    }
    let preference = store
        .effective_agent_provider_selection("agent:primary")
        .await
        .map_err(graphql_error)?;
    let provider_instance_key = preference
        .provider_instance_key
        .as_ref()
        .ok_or_else(|| async_graphql::Error::new("primary provider instance is unavailable"))?;
    let Ok(provider) = state.provider_registry()?.lease(provider_instance_key) else {
        return Ok(None);
    };
    let capabilities = provider
        .operations()
        .tool_capabilities(preference.model_profile.as_deref());
    let Some(display_name) = capabilities.hosted_web_provider_name else {
        return Ok(None);
    };
    let Some(account) = accounts
        .iter()
        .find(|selectable| selectable.account.provider_account_id == preference.provider_account_id)
        .map(|selectable| selectable.account.clone())
    else {
        return Ok(None);
    };

    Ok(Some(NativeWebProvider {
        account,
        display_name: display_name.to_string(),
    }))
}

pub(super) async fn save_web_tool_provider_binding(
    state: &GraphqlState,
    input: GraphqlSaveWebToolProviderBindingInput,
) -> Result<GraphqlWebToolBindingSettings> {
    let store = state.store()?;
    let capability_id = parse_web_capability(&input.capability_id)?;
    let expected_tool_name = capability_id.as_str();
    if input.tool_name != expected_tool_name {
        return Err(async_graphql::Error::new(
            "tool and capability do not match",
        ));
    }

    let accounts = selectable_accounts(state).await?;
    let native_provider = native_web_provider(state, &accounts).await?;
    if native_provider
        .as_ref()
        .is_some_and(|provider| provider.account.provider_account_id == input.provider_account_id)
    {
        let keys = web_assignment_keys()?;
        ProviderCapabilityAssignmentPersistence::clear_provider_capability_assignments(
            store, &keys,
        )
        .await
        .map_err(graphql_error)?;
        let settings = web_tool_settings(state).await?;
        return Ok(match capability_id {
            CapabilityId::WebSearch => settings.search,
            CapabilityId::WebFetch => settings.fetch,
            CapabilityId::WebBrowse => settings.browse,
            CapabilityId::ModelGenerate | CapabilityId::ModelClassify => unreachable!(),
        });
    }
    let account_reference =
        selectable_account_reference(&accounts, capability_id, &input.provider_account_id)
            .ok_or_else(|| {
                async_graphql::Error::new(
                    "provider account does not supply the requested capability",
                )
            })?;

    ProviderCapabilityAssignmentPersistence::upsert_provider_capability_assignment(
        store,
        UpsertProviderCapabilityAssignmentRequest::new(
            ToolName::new(input.tool_name).map_err(graphql_error)?,
            capability_id,
            account_reference,
        )
        .map_err(graphql_error)?,
    )
    .await
    .map_err(graphql_error)?;

    let settings = web_tool_settings(state).await?;
    Ok(match capability_id {
        CapabilityId::WebSearch => settings.search,
        CapabilityId::WebFetch => settings.fetch,
        CapabilityId::WebBrowse => settings.browse,
        CapabilityId::ModelGenerate | CapabilityId::ModelClassify => unreachable!(),
    })
}

pub(super) async fn save_browser_provider_route(
    state: &GraphqlState,
    input: GraphqlSaveBrowserProviderRouteInput,
) -> Result<GraphqlWebToolBindingSettings> {
    let store = state.store()?;
    let accounts = selectable_accounts(state).await?;
    let references = input
        .provider_account_ids
        .iter()
        .map(|provider_account_id| {
            selectable_account_reference(&accounts, CapabilityId::WebBrowse, provider_account_id)
                .ok_or_else(|| {
                    async_graphql::Error::new(
                        "provider account does not supply interactive browsing",
                    )
                })
        })
        .collect::<Result<Vec<_>>>()?;
    let request = ReplaceProviderCapabilityRouteRequest::new(
        ToolName::new(CapabilityId::WebBrowse.as_str()).map_err(graphql_error)?,
        CapabilityId::WebBrowse,
        references,
    )
    .map_err(graphql_error)?;
    ProviderCapabilityAssignmentPersistence::replace_provider_capability_route(store, request)
        .await
        .map_err(graphql_error)?;
    Ok(web_tool_settings(state).await?.browse)
}

async fn selectable_accounts(state: &GraphqlState) -> Result<Vec<SelectableProviderAccount>> {
    Ok(state
        .provider_account_operations()?
        .active_accounts()
        .await
        .map_err(graphql_error)?
        .into_iter()
        .map(|account| SelectableProviderAccount {
            reference: ProviderCapabilityAccountReference::persisted(
                account.provider_account_id.clone(),
            ),
            account,
        })
        .collect())
}

async fn binding_settings(
    store: &NoemaStore,
    accounts: &[SelectableProviderAccount],
    native_provider: Option<&NativeWebProvider>,
    use_native_default: bool,
    capability_id: CapabilityId,
) -> Result<GraphqlWebToolBindingSettings> {
    let tool_name = capability_id.as_str();
    let provider_options = provider_options(accounts, native_provider, capability_id);
    let default_provider_account_id = default_provider_account_id(capability_id).to_string();
    let tool_name = ToolName::new(tool_name).map_err(graphql_error)?;
    let key =
        ProviderCapabilityAssignmentKey::new(tool_name, capability_id).map_err(graphql_error)?;
    let configured_provider_account_ids =
        ProviderCapabilityAssignmentPersistence::provider_capability_route(store, &key)
            .await
            .map_err(graphql_error)?
            .into_iter()
            .map(|binding| binding.provider_account_id)
            .filter(|provider_account_id| {
                provider_options
                    .iter()
                    .any(|option| option.provider_account_id == *provider_account_id)
            })
            .collect::<Vec<_>>();
    let configured_provider_account_id = configured_provider_account_ids
        .first()
        .cloned()
        .unwrap_or_else(|| default_provider_account_id.clone());
    let active_provider_account_id = if use_native_default {
        native_provider
            .map(|provider| provider.account.provider_account_id.clone())
            .unwrap_or(configured_provider_account_id)
    } else {
        configured_provider_account_id
    };

    Ok(GraphqlWebToolBindingSettings {
        tool_name: key.tool_name_str().to_string(),
        capability_id: capability_id.as_str().to_string(),
        provider_route_account_ids: if configured_provider_account_ids.is_empty()
            || use_native_default
        {
            vec![active_provider_account_id.clone()]
        } else {
            configured_provider_account_ids
        },
        active_provider_account_id,
        provider_options,
    })
}

fn provider_options(
    accounts: &[SelectableProviderAccount],
    native_provider: Option<&NativeWebProvider>,
    capability_id: CapabilityId,
) -> Vec<GraphqlWebToolProviderOption> {
    let mut options = native_provider
        .map(|provider| native_provider_option(provider, capability_id))
        .into_iter()
        .collect::<Vec<_>>();
    options.extend(
        accounts
            .iter()
            .filter_map(|selectable| {
                selectable
                    .account
                    .capabilities
                    .iter()
                    .find(|capability| {
                        capability.capability_id == capability_id
                            && capability.status == ProviderCapabilityStatus::Available
                    })
                    .map(|capability| option_from_account(&selectable.account, capability))
            })
            .collect::<Vec<_>>(),
    );
    options
}

fn native_provider_option(
    provider: &NativeWebProvider,
    capability_id: CapabilityId,
) -> GraphqlWebToolProviderOption {
    GraphqlWebToolProviderOption {
        provider_account_id: provider.account.provider_account_id.clone(),
        provider_kind: provider.account.provider_kind.clone(),
        account_key: provider.account.account_key.clone(),
        display_name: provider.display_name.clone(),
        capability_id: capability_id.as_str().to_string(),
        reliability_contract: "hosted_provider".to_string(),
        data_flow_class: match capability_id {
            CapabilityId::WebSearch => "trusted_external_search_query",
            CapabilityId::WebFetch => "external_web_fetch",
            CapabilityId::WebBrowse => "external_web_browse",
            CapabilityId::ModelGenerate | CapabilityId::ModelClassify => unreachable!(),
        }
        .to_string(),
        citations: capability_id == CapabilityId::WebSearch,
        direct_url_fetch: capability_id == CapabilityId::WebFetch,
        js_rendering: capability_id == CapabilityId::WebBrowse,
        authenticated_context: capability_id == CapabilityId::WebBrowse,
    }
}

async fn web_provider_override_exists(store: &NoemaStore) -> Result<bool> {
    for key in web_assignment_keys()? {
        if ProviderCapabilityAssignmentPersistence::provider_capability_assignment(store, &key)
            .await
            .map_err(graphql_error)?
            .is_some()
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn web_assignment_keys() -> Result<[ProviderCapabilityAssignmentKey; 2]> {
    Ok([
        web_assignment_key(CapabilityId::WebSearch)?,
        web_assignment_key(CapabilityId::WebFetch)?,
    ])
}

fn web_assignment_key(capability_id: CapabilityId) -> Result<ProviderCapabilityAssignmentKey> {
    ProviderCapabilityAssignmentKey::new(
        ToolName::new(capability_id.as_str()).map_err(graphql_error)?,
        capability_id,
    )
    .map_err(graphql_error)
}

fn selectable_account_reference(
    accounts: &[SelectableProviderAccount],
    capability_id: CapabilityId,
    provider_account_id: &str,
) -> Option<ProviderCapabilityAccountReference> {
    accounts
        .iter()
        .find(|selectable| {
            selectable.account.provider_account_id == provider_account_id
                && selectable.account.capabilities.iter().any(|capability| {
                    capability.capability_id == capability_id
                        && capability.status == ProviderCapabilityStatus::Available
                })
        })
        .map(|selectable| selectable.reference.clone())
}

fn option_from_account(
    account: &ProviderAccountRecord,
    capability: &ProviderCapability,
) -> GraphqlWebToolProviderOption {
    GraphqlWebToolProviderOption {
        provider_account_id: account.provider_account_id.clone(),
        provider_kind: account.provider_kind.clone(),
        account_key: account.account_key.clone(),
        display_name: account.display_name.clone(),
        capability_id: capability.capability_id.as_str().to_string(),
        reliability_contract: capability.reliability_contract.as_str().to_string(),
        data_flow_class: capability.data_flow_class.as_str().to_string(),
        citations: capability.features.citations,
        direct_url_fetch: capability.features.direct_url_fetch,
        js_rendering: capability.features.js_rendering,
        authenticated_context: capability.features.authenticated_context,
    }
}

fn parse_web_capability(capability_id: &str) -> Result<CapabilityId> {
    for candidate in [
        CapabilityId::WebSearch,
        CapabilityId::WebFetch,
        CapabilityId::WebBrowse,
    ] {
        if capability_id == candidate.as_str() {
            return Ok(candidate);
        }
    }
    Err(async_graphql::Error::new(
        "capability is not a supported web tool capability",
    ))
}

const fn default_provider_account_id(capability_id: CapabilityId) -> &'static str {
    match default_web_provider_account_id(capability_id) {
        Some(account_id) => account_id,
        None => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{graphql::schema::GraphqlState, test_support::test_store};
    use noema_providers::{ProviderAccountStatus, ProviderToolCapabilities};
    use std::sync::Arc;

    #[derive(Debug)]
    struct HostedWebTestProvider;

    impl noema_providers::ProviderOperations for HostedWebTestProvider {
        fn tool_capabilities(&self, _model: Option<&str>) -> ProviderToolCapabilities {
            ProviderToolCapabilities {
                hosted_web_provider_name: Some("OpenAI"),
                ..ProviderToolCapabilities::default()
            }
        }

        fn generate_streaming<'a>(
            &'a self,
            _request: noema_providers::GenerateRequest,
            _on_event: &'a mut (dyn FnMut(noema_providers::GenerateStreamEvent) + Send),
        ) -> noema_providers::ProviderOperationFuture<'a, noema_providers::GenerateResponse>
        {
            Box::pin(async { unreachable!("settings test does not generate") })
        }
    }

    #[tokio::test]
    async fn web_tool_settings_treats_openai_as_the_default_selectable_provider() {
        let store = test_store().await;
        let exa_account_id =
            create_exa_provider_account(&store, ProviderAccountStatus::Authenticated).await;
        noema_store::test_support::initialize_codex_provider_selections(&store)
            .await
            .expect("initialize Codex selection");
        let preference = store
            .get_agent_runtime_preference("agent:primary")
            .await
            .expect("primary preference")
            .expect("initialized primary preference");
        let registry = Arc::new(noema_providers::ProviderRegistry::new());
        registry
            .register(
                preference.provider_instance_key,
                Arc::new(HostedWebTestProvider),
            )
            .expect("register hosted provider");
        let state = GraphqlState::for_tests_with_store(store).with_provider_registry(registry);

        let settings = web_tool_settings(&state).await.expect("settings");
        assert_eq!(
            settings.search.active_provider_account_id,
            "provider_account:codex:default"
        );
        assert_eq!(
            settings.fetch.active_provider_account_id,
            "provider_account:codex:default"
        );
        assert_eq!(
            settings.browse.active_provider_account_id,
            OBSCURA_SYSTEM_ACCOUNT_ID
        );
        assert!(settings.browse.provider_options.iter().any(|option| {
            option.provider_kind == "obscura" && option.js_rendering && option.authenticated_context
        }));
        assert!([&settings.search, &settings.fetch].into_iter().all(|tool| {
            tool.provider_options.iter().any(|option| {
                option.provider_account_id == "provider_account:codex:default"
                    && option.display_name == "OpenAI"
            })
        }));

        save_web_tool_provider_binding(
            &state,
            GraphqlSaveWebToolProviderBindingInput {
                tool_name: CapabilityId::WebSearch.as_str().to_string(),
                capability_id: CapabilityId::WebSearch.as_str().to_string(),
                provider_account_id: exa_account_id.clone(),
            },
        )
        .await
        .expect("select Exa search");
        let settings = web_tool_settings(&state)
            .await
            .expect("configured settings");

        assert_eq!(settings.search.active_provider_account_id, exa_account_id);
        assert_eq!(
            settings.fetch.active_provider_account_id,
            DIRECT_HTTP_SYSTEM_ACCOUNT_ID
        );

        save_web_tool_provider_binding(
            &state,
            GraphqlSaveWebToolProviderBindingInput {
                tool_name: CapabilityId::WebFetch.as_str().to_string(),
                capability_id: CapabilityId::WebFetch.as_str().to_string(),
                provider_account_id: "provider_account:codex:default".to_string(),
            },
        )
        .await
        .expect("restore OpenAI web tools");
        let settings = web_tool_settings(&state).await.expect("native settings");

        assert_eq!(
            settings.search.active_provider_account_id,
            "provider_account:codex:default"
        );
        assert_eq!(
            settings.fetch.active_provider_account_id,
            "provider_account:codex:default"
        );
    }

    #[tokio::test]
    async fn web_tool_settings_filters_capabilities_and_ignores_stale_bindings() {
        let store = test_store().await;
        store
            .ensure_default_provider_account()
            .await
            .expect("codex account");
        let exa_account_id =
            create_exa_provider_account(&store, ProviderAccountStatus::Authenticated).await;
        let state = GraphqlState::for_tests_with_store(store.clone());

        let settings = web_tool_settings(&state).await.expect("settings");

        assert_eq!(
            settings.search.active_provider_account_id,
            "provider_account:duckduckgo_public:system"
        );
        assert_eq!(
            settings.fetch.active_provider_account_id,
            "provider_account:direct_http:system"
        );
        assert!(settings.search.provider_options.iter().any(|option| {
            option.provider_account_id == "provider_account:duckduckgo_public:system"
        }));
        assert!(
            settings
                .search
                .provider_options
                .iter()
                .any(|option| option.provider_account_id == exa_account_id)
        );
        assert!(
            settings.fetch.provider_options.iter().any(|option| {
                option.provider_account_id == "provider_account:direct_http:system"
            })
        );
        assert!(
            !settings
                .search
                .provider_options
                .iter()
                .any(|option| { option.provider_kind == "codex" })
        );
        assert!(
            !settings
                .fetch
                .provider_options
                .iter()
                .any(|option| { option.provider_kind == "openai" })
        );
        crate::test_support::save_provider_capability_assignment_for_tests(
            &store,
            "web.search",
            "web.search",
            ProviderCapabilityAccountReference::persisted(exa_account_id.clone()),
        )
        .await;
        store
            .update_provider_account_status(
                &exa_account_id,
                ProviderAccountStatus::Unknown,
                None,
                None,
            )
            .await
            .expect("make account non-selectable");
        let state = GraphqlState::for_tests_with_store(store);

        let settings = web_tool_settings(&state).await.expect("settings");

        assert_eq!(
            settings.search.active_provider_account_id,
            "provider_account:duckduckgo_public:system"
        );
        assert!(
            !settings
                .search
                .provider_options
                .iter()
                .any(|option| option.provider_account_id == exa_account_id)
        );
    }

    async fn create_exa_provider_account(
        store: &noema_store::NoemaStore,
        status: ProviderAccountStatus,
    ) -> String {
        crate::test_support::create_exa_provider_account_for_tests(
            store,
            "Exa research",
            status,
            serde_json::json!({}),
        )
        .await
        .provider_account_id
    }
}
