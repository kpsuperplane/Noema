//! Static provider capability vocabulary and declarations.

use crate::ProviderAccountStatus;
use noema_capabilities::{
    CapabilityFeatures, CapabilityId, DataFlowClass, ReliabilityContract, ResultPersistencePolicy,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

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

/// Return the static capabilities declared for a provider account.
#[must_use]
pub fn capabilities_for_provider_account(
    provider_kind: &str,
    account_key: &str,
    account_status: ProviderAccountStatus,
) -> Vec<ProviderCapability> {
    let status = capability_status_for_account(account_status);
    match provider_kind {
        "openai" => vec![
            model_capability(
                provider_kind,
                account_key,
                CapabilityId::ModelGenerate,
                status,
            ),
            model_capability(
                provider_kind,
                account_key,
                CapabilityId::ModelClassify,
                status,
            ),
        ],
        "codex" | "foundation_local" => vec![
            model_capability(
                provider_kind,
                account_key,
                CapabilityId::ModelGenerate,
                status,
            ),
            model_capability(
                provider_kind,
                account_key,
                CapabilityId::ModelClassify,
                status,
            ),
        ],
        "local_models" => vec![
            local_model_capability(
                provider_kind,
                account_key,
                CapabilityId::ModelGenerate,
                status,
            ),
            local_model_capability(
                provider_kind,
                account_key,
                CapabilityId::ModelClassify,
                status,
            ),
        ],
        "duckduckgo_public" => vec![ProviderCapability {
            provider_kind: provider_kind.to_string(),
            account_key: account_key.to_string(),
            capability_id: CapabilityId::WebSearch,
            status: ProviderCapabilityStatus::Available,
            reliability_contract: ReliabilityContract::BestEffortPublic,
            data_flow_class: DataFlowClass::TrustedExternalSearchQuery,
            features: CapabilityFeatures {
                citations: false,
                direct_url_fetch: false,
                js_rendering: false,
                authenticated_context: false,
                result_persistence: ResultPersistencePolicy::CompactMetadata,
            },
        }],
        "direct_http" => vec![ProviderCapability {
            provider_kind: provider_kind.to_string(),
            account_key: account_key.to_string(),
            capability_id: CapabilityId::WebFetch,
            status: ProviderCapabilityStatus::Available,
            reliability_contract: ReliabilityContract::FirstParty,
            data_flow_class: DataFlowClass::ExternalWebFetch,
            features: CapabilityFeatures {
                citations: false,
                direct_url_fetch: true,
                js_rendering: false,
                authenticated_context: false,
                result_persistence: ResultPersistencePolicy::CompactContent,
            },
        }],
        "exa" => vec![
            ProviderCapability {
                provider_kind: provider_kind.to_string(),
                account_key: account_key.to_string(),
                capability_id: CapabilityId::WebSearch,
                status,
                reliability_contract: ReliabilityContract::HostedProvider,
                data_flow_class: DataFlowClass::TrustedExternalSearchQuery,
                features: CapabilityFeatures {
                    citations: true,
                    direct_url_fetch: false,
                    js_rendering: false,
                    authenticated_context: false,
                    result_persistence: ResultPersistencePolicy::CompactMetadata,
                },
            },
            ProviderCapability {
                provider_kind: provider_kind.to_string(),
                account_key: account_key.to_string(),
                capability_id: CapabilityId::WebFetch,
                status,
                reliability_contract: ReliabilityContract::HostedProvider,
                data_flow_class: DataFlowClass::ExternalWebFetch,
                features: CapabilityFeatures {
                    citations: true,
                    direct_url_fetch: true,
                    js_rendering: false,
                    authenticated_context: false,
                    result_persistence: ResultPersistencePolicy::CompactContent,
                },
            },
        ],
        _ => Vec::new(),
    }
}

fn model_capability(
    provider_kind: &str,
    account_key: &str,
    capability_id: CapabilityId,
    status: ProviderCapabilityStatus,
) -> ProviderCapability {
    ProviderCapability {
        provider_kind: provider_kind.to_string(),
        account_key: account_key.to_string(),
        capability_id,
        status,
        reliability_contract: ReliabilityContract::HostedProvider,
        data_flow_class: DataFlowClass::ModelProviderPrompt,
        features: CapabilityFeatures {
            citations: false,
            direct_url_fetch: false,
            js_rendering: false,
            authenticated_context: false,
            result_persistence: ResultPersistencePolicy::CompactMetadata,
        },
    }
}

fn local_model_capability(
    provider_kind: &str,
    account_key: &str,
    capability_id: CapabilityId,
    status: ProviderCapabilityStatus,
) -> ProviderCapability {
    ProviderCapability {
        provider_kind: provider_kind.to_string(),
        account_key: account_key.to_string(),
        capability_id,
        status,
        reliability_contract: ReliabilityContract::FirstParty,
        data_flow_class: DataFlowClass::LocalInference,
        features: CapabilityFeatures {
            citations: false,
            direct_url_fetch: false,
            js_rendering: false,
            authenticated_context: false,
            result_persistence: ResultPersistencePolicy::CompactMetadata,
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
    use crate::ProviderAccountStatus;

    #[test]
    fn openai_account_declares_model_capabilities_only() {
        let capabilities = capabilities_for_provider_account(
            "openai",
            "default",
            ProviderAccountStatus::Authenticated,
        );
        let ids = capabilities
            .iter()
            .map(|capability| capability.capability_id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(ids, vec!["model.generate", "model.classify"]);
    }

    #[test]
    fn codex_account_does_not_declare_native_web_search() {
        let capabilities = capabilities_for_provider_account(
            "codex",
            "default",
            ProviderAccountStatus::Authenticated,
        );

        assert!(
            capabilities
                .iter()
                .any(|capability| capability.capability_id == CapabilityId::ModelGenerate)
        );
        assert!(
            !capabilities
                .iter()
                .any(|capability| capability.capability_id == CapabilityId::WebSearch)
        );
    }

    #[test]
    fn local_models_account_declares_first_party_local_inference() {
        let capabilities = capabilities_for_provider_account(
            "local_models",
            "default",
            ProviderAccountStatus::Authenticated,
        );

        assert_eq!(capabilities.len(), 2);
        assert!(capabilities.iter().all(|capability| {
            capability.reliability_contract == ReliabilityContract::FirstParty
                && capability.data_flow_class == DataFlowClass::LocalInference
        }));
    }

    #[test]
    fn system_web_providers_have_no_secret_requirements() {
        let search = capabilities_for_provider_account(
            "duckduckgo_public",
            "system",
            ProviderAccountStatus::Authenticated,
        );
        let fetch = capabilities_for_provider_account(
            "direct_http",
            "system",
            ProviderAccountStatus::Authenticated,
        );

        assert_eq!(search[0].capability_id, CapabilityId::WebSearch);
        assert_eq!(
            search[0].reliability_contract,
            ReliabilityContract::BestEffortPublic
        );
        assert_eq!(fetch[0].capability_id, CapabilityId::WebFetch);
        assert!(fetch[0].features.direct_url_fetch);
    }

    #[test]
    fn exa_account_declares_search_and_fetch_only() {
        let capabilities = capabilities_for_provider_account(
            "exa",
            "research",
            ProviderAccountStatus::Authenticated,
        );
        let ids = capabilities
            .iter()
            .map(|capability| capability.capability_id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(ids, vec!["web.search", "web.fetch"]);
        assert!(capabilities.iter().all(|capability| {
            capability.provider_kind == "exa"
                && capability.account_key == "research"
                && capability.reliability_contract == ReliabilityContract::HostedProvider
        }));
        assert!(capabilities.iter().any(|capability| {
            capability.capability_id == CapabilityId::WebSearch
                && capability.data_flow_class == DataFlowClass::TrustedExternalSearchQuery
                && capability.features.citations
                && !capability.features.direct_url_fetch
        }));
        assert!(capabilities.iter().any(|capability| {
            capability.capability_id == CapabilityId::WebFetch
                && capability.data_flow_class == DataFlowClass::ExternalWebFetch
                && capability.features.direct_url_fetch
        }));
    }

    #[test]
    fn unknown_and_checking_accounts_are_account_dependent() {
        for account_status in [
            ProviderAccountStatus::Unknown,
            ProviderAccountStatus::Checking,
        ] {
            let capabilities =
                capabilities_for_provider_account("openai", "default", account_status);

            assert!(!capabilities.is_empty());
            assert!(
                capabilities
                    .iter()
                    .all(|capability| capability.status
                        == ProviderCapabilityStatus::AccountDependent)
            );
        }
    }

    #[test]
    fn unauthenticated_and_unavailable_accounts_are_unavailable() {
        for account_status in [
            ProviderAccountStatus::Unauthenticated,
            ProviderAccountStatus::Unavailable,
        ] {
            let capabilities =
                capabilities_for_provider_account("openai", "default", account_status);

            assert!(!capabilities.is_empty());
            assert!(
                capabilities
                    .iter()
                    .all(|capability| capability.status == ProviderCapabilityStatus::Unavailable)
            );
        }
    }
}
