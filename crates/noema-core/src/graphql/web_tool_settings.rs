use async_graphql::{InputObject, Result, SimpleObject};
use noema_providers::{
    ProviderAccountRecord, ProviderCapability, ProviderCapabilityAccountReference,
    ProviderCapabilityAssignmentKey, ProviderCapabilityAssignmentPersistence,
    ProviderCapabilityStatus, UpsertProviderCapabilityAssignmentRequest, system_provider_accounts,
};

use crate::NoemaStore;
use noema_capabilities::{CapabilityId, ToolName};

use super::{errors::graphql_error, schema::GraphqlState};

const WEB_SEARCH_TOOL: &str = "web.search";
const WEB_FETCH_TOOL: &str = "web.fetch";
const DUCKDUCKGO_SYSTEM_ACCOUNT_ID: &str = "provider_account:duckduckgo_public:system";
const DIRECT_HTTP_SYSTEM_ACCOUNT_ID: &str = "provider_account:direct_http:system";

#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "WebToolSettings")]
pub struct GraphqlWebToolSettings {
    pub search: GraphqlWebToolBindingSettings,
    pub fetch: GraphqlWebToolBindingSettings,
}

#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "WebToolBindingSettings")]
pub struct GraphqlWebToolBindingSettings {
    pub tool_name: String,
    pub capability_id: String,
    pub active_provider_account_id: String,
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
}

#[derive(Clone, Debug, InputObject)]
#[graphql(name = "SaveWebToolProviderBindingInput")]
pub struct GraphqlSaveWebToolProviderBindingInput {
    pub tool_name: String,
    pub capability_id: String,
    pub provider_account_id: String,
}

#[derive(Clone, Debug)]
struct SelectableProviderAccount {
    account: ProviderAccountRecord,
    reference: ProviderCapabilityAccountReference,
}

pub(super) async fn web_tool_settings(state: &GraphqlState) -> Result<GraphqlWebToolSettings> {
    let store = state.store()?;
    let accounts = selectable_accounts(state).await?;

    Ok(GraphqlWebToolSettings {
        search: binding_settings(store, &accounts, CapabilityId::WebSearch).await?,
        fetch: binding_settings(store, &accounts, CapabilityId::WebFetch).await?,
    })
}

pub(super) async fn save_web_tool_provider_binding(
    state: &GraphqlState,
    input: GraphqlSaveWebToolProviderBindingInput,
) -> Result<GraphqlWebToolBindingSettings> {
    let store = state.store()?;
    let capability_id = parse_web_capability(&input.capability_id)?;
    let expected_tool_name = tool_name_for_capability(capability_id);
    if input.tool_name != expected_tool_name {
        return Err(async_graphql::Error::new(
            "tool and capability do not match",
        ));
    }

    let accounts = selectable_accounts(state).await?;
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

    binding_settings(store, &accounts, capability_id).await
}

async fn selectable_accounts(state: &GraphqlState) -> Result<Vec<SelectableProviderAccount>> {
    let mut accounts = Vec::new();
    for account in system_provider_accounts() {
        accounts.push(SelectableProviderAccount {
            reference: ProviderCapabilityAccountReference::validated_system(
                account.provider_account_id.clone(),
            )
            .map_err(graphql_error)?,
            account,
        });
    }
    accounts.extend(
        state
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
            }),
    );
    Ok(accounts)
}

async fn binding_settings(
    store: &NoemaStore,
    accounts: &[SelectableProviderAccount],
    capability_id: CapabilityId,
) -> Result<GraphqlWebToolBindingSettings> {
    let tool_name = tool_name_for_capability(capability_id);
    let provider_options = provider_options(accounts, capability_id);
    let default_provider_account_id = default_provider_account_id(capability_id).to_string();
    let tool_name = ToolName::new(tool_name).map_err(graphql_error)?;
    let key =
        ProviderCapabilityAssignmentKey::new(tool_name, capability_id).map_err(graphql_error)?;
    let active_provider_account_id =
        ProviderCapabilityAssignmentPersistence::provider_capability_assignment(store, &key)
            .await
            .map_err(graphql_error)?
            .map(|binding| binding.provider_account_id)
            .filter(|provider_account_id| {
                provider_options
                    .iter()
                    .any(|option| option.provider_account_id == *provider_account_id)
            })
            .unwrap_or(default_provider_account_id);

    Ok(GraphqlWebToolBindingSettings {
        tool_name: key.tool_name_str().to_string(),
        capability_id: capability_id.as_str().to_string(),
        active_provider_account_id,
        provider_options,
    })
}

fn provider_options(
    accounts: &[SelectableProviderAccount],
    capability_id: CapabilityId,
) -> Vec<GraphqlWebToolProviderOption> {
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
        .collect()
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
    }
}

fn parse_web_capability(capability_id: &str) -> Result<CapabilityId> {
    match capability_id {
        WEB_SEARCH_TOOL => Ok(CapabilityId::WebSearch),
        WEB_FETCH_TOOL => Ok(CapabilityId::WebFetch),
        _ => Err(async_graphql::Error::new(
            "capability is not a supported web tool capability",
        )),
    }
}

const fn tool_name_for_capability(capability_id: CapabilityId) -> &'static str {
    match capability_id {
        CapabilityId::WebSearch => WEB_SEARCH_TOOL,
        CapabilityId::WebFetch => WEB_FETCH_TOOL,
        CapabilityId::ModelGenerate | CapabilityId::ModelClassify => unreachable!(),
    }
}

const fn default_provider_account_id(capability_id: CapabilityId) -> &'static str {
    match capability_id {
        CapabilityId::WebSearch => DUCKDUCKGO_SYSTEM_ACCOUNT_ID,
        CapabilityId::WebFetch => DIRECT_HTTP_SYSTEM_ACCOUNT_ID,
        CapabilityId::ModelGenerate | CapabilityId::ModelClassify => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{graphql::schema::GraphqlState, test_support::test_store};
    use noema_providers::ProviderAccountStatus;

    #[tokio::test]
    async fn web_tool_settings_lists_only_matching_available_capabilities() {
        let store = test_store().await;
        store
            .ensure_default_provider_account()
            .await
            .expect("codex account");
        let exa_account_id =
            create_exa_provider_account(&store, ProviderAccountStatus::Authenticated).await;
        let state = GraphqlState::for_tests_with_store(store);

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
    }

    #[tokio::test]
    async fn web_tool_settings_falls_back_when_saved_binding_is_not_selectable() {
        let store = test_store().await;
        let exa_account_id =
            create_exa_provider_account(&store, ProviderAccountStatus::Authenticated).await;
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

    #[tokio::test]
    async fn web_tool_settings_include_active_non_default_exa_accounts() {
        let store = test_store().await;
        let exa_account_id =
            create_exa_provider_account(&store, ProviderAccountStatus::Authenticated).await;
        let state = GraphqlState::for_tests_with_store(store);

        let settings = web_tool_settings(&state).await.expect("settings");

        assert!(
            settings
                .search
                .provider_options
                .iter()
                .any(|option| { option.provider_account_id == exa_account_id })
        );
        assert!(
            settings
                .fetch
                .provider_options
                .iter()
                .any(|option| { option.provider_account_id == exa_account_id })
        );
    }

    #[tokio::test]
    async fn save_web_tool_provider_binding_rejects_mismatched_tool_and_capability() {
        let store = test_store().await;
        let state = GraphqlState::for_tests_with_store(store);

        let error = save_web_tool_provider_binding(
            &state,
            GraphqlSaveWebToolProviderBindingInput {
                tool_name: "web.fetch".to_string(),
                capability_id: "web.search".to_string(),
                provider_account_id: "provider_account:duckduckgo_public:system".to_string(),
            },
        )
        .await
        .expect_err("mismatched pair should fail");

        assert!(
            error.message.contains("tool and capability do not match"),
            "{error:?}"
        );
    }

    #[tokio::test]
    async fn save_web_tool_provider_binding_rejects_unavailable_provider_capability() {
        let store = test_store().await;
        let exa_account_id =
            create_exa_provider_account(&store, ProviderAccountStatus::Unknown).await;
        let state = GraphqlState::for_tests_with_store(store);

        let error = save_web_tool_provider_binding(
            &state,
            GraphqlSaveWebToolProviderBindingInput {
                tool_name: "web.search".to_string(),
                capability_id: "web.search".to_string(),
                provider_account_id: exa_account_id,
            },
        )
        .await
        .expect_err("account-dependent capability should fail");

        assert!(
            error
                .message
                .contains("provider account does not supply the requested capability"),
            "{error:?}"
        );
    }

    #[tokio::test]
    async fn save_web_tool_provider_binding_persists_selectable_provider() {
        let store = test_store().await;
        let exa_account_id =
            create_exa_provider_account(&store, ProviderAccountStatus::Authenticated).await;
        let state = GraphqlState::for_tests_with_store(store.clone());

        let saved = save_web_tool_provider_binding(
            &state,
            GraphqlSaveWebToolProviderBindingInput {
                tool_name: "web.search".to_string(),
                capability_id: "web.search".to_string(),
                provider_account_id: exa_account_id.clone(),
            },
        )
        .await
        .expect("save binding");

        assert_eq!(saved.active_provider_account_id, exa_account_id);
        let binding = store
            .provider_capability_binding("web.search", "web.search")
            .await
            .expect("binding lookup")
            .expect("binding row");
        assert_eq!(
            binding.provider_account_id,
            saved.active_provider_account_id
        );
    }

    #[tokio::test]
    async fn save_web_tool_provider_binding_persists_system_provider_without_durable_row() {
        let store = test_store().await;
        let state = GraphqlState::for_tests_with_store(store.clone());

        let saved = save_web_tool_provider_binding(
            &state,
            GraphqlSaveWebToolProviderBindingInput {
                tool_name: "web.search".to_string(),
                capability_id: "web.search".to_string(),
                provider_account_id: "provider_account:duckduckgo_public:system".to_string(),
            },
        )
        .await
        .expect("save system binding");

        assert_eq!(
            saved.active_provider_account_id,
            "provider_account:duckduckgo_public:system"
        );
        let binding = store
            .provider_capability_binding("web.search", "web.search")
            .await
            .expect("binding lookup")
            .expect("binding row");
        assert_eq!(
            binding.provider_account_id,
            "provider_account:duckduckgo_public:system"
        );
    }

    async fn create_exa_provider_account(
        store: &crate::NoemaStore,
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
