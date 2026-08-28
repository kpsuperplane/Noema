//! Static provider capability vocabulary and declarations.

use noema_capabilities::{
    CapabilityFeatures, CapabilityId, DataFlowClass, ReliabilityContract, ResultPersistencePolicy,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    DIRECT_HTTP_PROVIDER_ACCOUNT_ID, DUCKDUCKGO_PUBLIC_PROVIDER_ACCOUNT_ID,
    OBSCURA_BROWSER_PROVIDER_ACCOUNT_ID, ProviderAccountStatus,
};

/// Availability state for a capability on a concrete provider account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum ProviderCapabilityStatus {
    /// Capability is usable now.
    Available,
    /// Capability depends on account readiness checks.
    AccountDependent,
    /// Capability cannot be used.
    Unavailable,
}

impl ProviderCapabilityStatus {
    /// Return the stable storage string for this status.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::AccountDependent => "account_dependent",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Declared capability for a specific provider account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct ProviderCapability {
    /// Stable provider family identifier.
    pub provider_kind: String,
    /// Provider-local account key.
    pub account_key: String,
    /// Stable capability id.
    pub capability_id: CapabilityId,
    /// Current capability availability.
    pub status: ProviderCapabilityStatus,
    /// Operational trust contract.
    pub reliability_contract: ReliabilityContract,
    /// Data handling class.
    pub data_flow_class: DataFlowClass,
    /// Capability behavior metadata.
    pub features: CapabilityFeatures,
}

/// Persisted assignment from a model-visible tool to a provider capability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderCapabilityAssignment {
    /// Stable assignment id derived from tool and capability.
    pub assignment_id: String,
    /// Model-visible tool name.
    pub tool_name: String,
    /// Bound provider capability id.
    pub capability_id: String,
    /// Provider account selected for this assignment.
    pub provider_account_id: String,
}

/// Return whether a tool/capability pair is supported for durable assignment.
#[must_use]
pub fn provider_capability_assignment_pair_is_supported(
    tool_name: &str,
    capability_id: &str,
) -> bool {
    matches!(
        (tool_name, capability_id),
        ("web.search", "web.search") | ("web.fetch", "web.fetch") | ("web.browse", "web.browse")
    )
}
/// Return the permanent default account id for a web capability.
#[must_use]
pub const fn default_web_provider_account_id(capability_id: CapabilityId) -> Option<&'static str> {
    match capability_id {
        CapabilityId::WebSearch => Some(DUCKDUCKGO_PUBLIC_PROVIDER_ACCOUNT_ID),
        CapabilityId::WebFetch => Some(DIRECT_HTTP_PROVIDER_ACCOUNT_ID),
        CapabilityId::WebBrowse => Some(OBSCURA_BROWSER_PROVIDER_ACCOUNT_ID),
        CapabilityId::ModelGenerate | CapabilityId::ModelClassify => None,
    }
}
/// Return the static capabilities declared for a provider account.
#[must_use]
pub fn capabilities_for_provider_account(
    provider_kind: &str,
    account_key: &str,
    account_status: ProviderAccountStatus,
) -> Vec<ProviderCapability> {
    let status = capability_status_for_account(account_status);
    match provider_kind {
        "openai" | "codex" | "openrouter" | "foundation_local" => model_capabilities(
            provider_kind,
            account_key,
            status,
            ReliabilityContract::HostedProvider,
            DataFlowClass::ModelProviderPrompt,
        ),
        "local_models" => model_capabilities(
            provider_kind,
            account_key,
            status,
            ReliabilityContract::FirstParty,
            DataFlowClass::LocalInference,
        ),
        "duckduckgo_public" => vec![web_capability(
            provider_kind,
            account_key,
            CapabilityId::WebSearch,
            ProviderCapabilityStatus::Available,
            ReliabilityContract::BestEffortPublic,
            false,
            false,
        )],
        "direct_http" => vec![web_capability(
            provider_kind,
            account_key,
            CapabilityId::WebFetch,
            ProviderCapabilityStatus::Available,
            ReliabilityContract::FirstParty,
            false,
            false,
        )],
        "obscura" => vec![web_capability(
            provider_kind,
            account_key,
            CapabilityId::WebBrowse,
            ProviderCapabilityStatus::Available,
            ReliabilityContract::FirstParty,
            false,
            true,
        )],
        "kernel" => vec![web_capability(
            provider_kind,
            account_key,
            CapabilityId::WebBrowse,
            status,
            ReliabilityContract::HostedProvider,
            false,
            true,
        )],
        "exa" => hosted_web_capabilities(provider_kind, account_key, status, false),
        "tinyfish" | "firecrawl" => {
            hosted_web_capabilities(provider_kind, account_key, status, true)
        }
        _ => Vec::new(),
    }
}
fn hosted_web_capabilities(
    provider_kind: &str,
    account_key: &str,
    status: ProviderCapabilityStatus,
    js_fetch: bool,
) -> Vec<ProviderCapability> {
    [CapabilityId::WebSearch, CapabilityId::WebFetch]
        .into_iter()
        .map(|capability_id| {
            web_capability(
                provider_kind,
                account_key,
                capability_id,
                status,
                ReliabilityContract::HostedProvider,
                true,
                js_fetch && capability_id == CapabilityId::WebFetch,
            )
        })
        .collect()
}
fn model_capabilities(
    provider_kind: &str,
    account_key: &str,
    status: ProviderCapabilityStatus,
    reliability_contract: ReliabilityContract,
    data_flow_class: DataFlowClass,
) -> Vec<ProviderCapability> {
    [CapabilityId::ModelGenerate, CapabilityId::ModelClassify]
        .into_iter()
        .map(|capability_id| ProviderCapability {
            provider_kind: provider_kind.to_string(),
            account_key: account_key.to_string(),
            capability_id,
            status,
            reliability_contract,
            data_flow_class,
            features: CapabilityFeatures {
                citations: false,
                direct_url_fetch: false,
                js_rendering: false,
                authenticated_context: false,
                result_persistence: ResultPersistencePolicy::CompactMetadata,
            },
        })
        .collect()
}

fn web_capability(
    provider_kind: &str,
    account_key: &str,
    capability_id: CapabilityId,
    status: ProviderCapabilityStatus,
    reliability_contract: ReliabilityContract,
    citations: bool,
    js_rendering: bool,
) -> ProviderCapability {
    let (data_flow_class, direct_url_fetch, result_persistence) = match capability_id {
        CapabilityId::WebSearch => (
            DataFlowClass::TrustedExternalSearchQuery,
            false,
            ResultPersistencePolicy::CompactMetadata,
        ),
        CapabilityId::WebFetch => (
            DataFlowClass::ExternalWebFetch,
            true,
            ResultPersistencePolicy::CompactContent,
        ),
        CapabilityId::WebBrowse => (
            DataFlowClass::ExternalWebBrowse,
            true,
            ResultPersistencePolicy::CompactMetadata,
        ),
        _ => unreachable!("web capability helper only accepts web capability ids"),
    };
    ProviderCapability {
        provider_kind: provider_kind.to_string(),
        account_key: account_key.to_string(),
        capability_id,
        status,
        reliability_contract,
        data_flow_class,
        features: CapabilityFeatures {
            citations,
            direct_url_fetch,
            js_rendering,
            authenticated_context: capability_id == CapabilityId::WebBrowse,
            result_persistence,
        },
    }
}

const fn capability_status_for_account(
    account_status: ProviderAccountStatus,
) -> ProviderCapabilityStatus {
    match account_status {
        ProviderAccountStatus::Authenticated => ProviderCapabilityStatus::Available,
        ProviderAccountStatus::Unknown | ProviderAccountStatus::Checking => {
            ProviderCapabilityStatus::AccountDependent
        }
        ProviderAccountStatus::Unauthenticated | ProviderAccountStatus::Unavailable => {
            ProviderCapabilityStatus::Unavailable
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_model_capability_policy() {
        let assert_status = |account_status, expected| {
            assert!(
                capabilities_for_provider_account("codex", "default", account_status)
                    .iter()
                    .all(|capability| capability.status == expected)
            );
        };
        assert_status(
            ProviderAccountStatus::Authenticated,
            ProviderCapabilityStatus::Available,
        );
        for account_status in [
            ProviderAccountStatus::Unknown,
            ProviderAccountStatus::Checking,
        ] {
            assert_status(account_status, ProviderCapabilityStatus::AccountDependent);
        }
        for account_status in [
            ProviderAccountStatus::Unauthenticated,
            ProviderAccountStatus::Unavailable,
        ] {
            assert_status(account_status, ProviderCapabilityStatus::Unavailable);
        }
        let assert_contract = |provider_kind, reliability, data_flow| {
            let capabilities = capabilities_for_provider_account(
                provider_kind,
                "default",
                ProviderAccountStatus::Authenticated,
            );
            assert!(capabilities.iter().all(|capability| {
                capability.reliability_contract == reliability
                    && capability.data_flow_class == data_flow
            }));
        };
        assert_contract(
            "codex",
            ReliabilityContract::HostedProvider,
            DataFlowClass::ModelProviderPrompt,
        );
        assert_contract(
            "local_models",
            ReliabilityContract::FirstParty,
            DataFlowClass::LocalInference,
        );
    }

    #[test]
    fn account_readiness_controls_model_capability_status() {
        assert_model_capability_policy();
    }

    #[test]
    fn hosted_and_local_accounts_expose_distinct_reliability_contracts() {
        assert_model_capability_policy();
    }

    #[test]
    fn kernel_browser_capability_tracks_account_readiness_as_hosted() {
        let available = capabilities_for_provider_account(
            "kernel",
            "account",
            ProviderAccountStatus::Authenticated,
        );
        assert_eq!(available.len(), 1);
        assert_eq!(available[0].capability_id, CapabilityId::WebBrowse);
        assert_eq!(
            available[0].reliability_contract,
            ReliabilityContract::HostedProvider
        );
        assert_eq!(
            available[0].data_flow_class,
            DataFlowClass::ExternalWebBrowse
        );
        assert_eq!(available[0].status, ProviderCapabilityStatus::Available);

        let unavailable = capabilities_for_provider_account(
            "kernel",
            "account",
            ProviderAccountStatus::Unauthenticated,
        );
        assert_eq!(unavailable[0].status, ProviderCapabilityStatus::Unavailable);

        for (provider_kind, js_fetch) in [("exa", false), ("tinyfish", true), ("firecrawl", true)] {
            let capabilities = capabilities_for_provider_account(
                provider_kind,
                "account",
                ProviderAccountStatus::Authenticated,
            );
            assert_eq!(capabilities.len(), 2);
            assert!(capabilities.iter().all(|capability| {
                capability.reliability_contract == ReliabilityContract::HostedProvider
                    && capability.features.citations
                    && !capability.features.authenticated_context
            }));
            let fetch = capabilities
                .iter()
                .find(|capability| capability.capability_id == CapabilityId::WebFetch)
                .expect("fetch capability");
            assert!(fetch.features.direct_url_fetch);
            assert_eq!(fetch.features.js_rendering, js_fetch);
        }
    }
}
