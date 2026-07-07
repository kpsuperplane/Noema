//! Static provider capability vocabulary and declarations.

use crate::ProviderAccountStatus;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Stable capability identifiers exposed by Noema provider accounts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum CapabilityId {
    /// Text or multimodal generation.
    ModelGenerate,
    /// Classification or lightweight analysis.
    ModelClassify,
    /// Hosted or public web search.
    WebSearch,
    /// Direct web content fetch.
    WebFetch,
}

impl CapabilityId {
    /// Return the stable identifier string for this capability.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ModelGenerate => "model.generate",
            Self::ModelClassify => "model.classify",
            Self::WebSearch => "web.search",
            Self::WebFetch => "web.fetch",
        }
    }
}

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

/// Operational trust contract for a capability provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum ReliabilityContract {
    /// First-party Noema-owned behavior.
    FirstParty,
    /// Hosted model or tool provider behavior.
    HostedProvider,
    /// Best-effort public integration without strict guarantees.
    BestEffortPublic,
}

impl ReliabilityContract {
    /// Return the stable storage string for this contract.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FirstParty => "first_party",
            Self::HostedProvider => "hosted_provider",
            Self::BestEffortPublic => "best_effort_public",
        }
    }
}

/// Data handling class for a capability invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum DataFlowClass {
    /// Prompt data flows to a model provider.
    ModelProviderPrompt,
    /// Query data flows to a trusted search backend.
    TrustedExternalSearchQuery,
    /// A remote web page is fetched directly.
    ExternalWebFetch,
}

impl DataFlowClass {
    /// Return the stable storage string for this data-flow class.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ModelProviderPrompt => "model_provider_prompt",
            Self::TrustedExternalSearchQuery => "trusted_external_search_query",
            Self::ExternalWebFetch => "external_web_fetch",
        }
    }
}

/// Persistence policy for results returned by a capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum ResultPersistencePolicy {
    /// Persist compact metadata only.
    CompactMetadata,
    /// Persist compact content payloads.
    CompactContent,
}

impl ResultPersistencePolicy {
    /// Return the stable storage string for this persistence policy.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CompactMetadata => "compact_metadata",
            Self::CompactContent => "compact_content",
        }
    }
}

/// Feature flags and behavior metadata for a capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct CapabilityFeatures {
    /// Whether results include source citations.
    pub citations: bool,
    /// Whether direct URL fetch is supported.
    pub direct_url_fetch: bool,
    /// Whether JavaScript rendering is supported.
    pub js_rendering: bool,
    /// Whether authenticated browsing context is supported.
    pub authenticated_context: bool,
    /// How result payloads should persist.
    pub result_persistence: ResultPersistencePolicy,
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
            model_capability(provider_kind, account_key, CapabilityId::ModelGenerate, status),
            model_capability(provider_kind, account_key, CapabilityId::ModelClassify, status),
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
        ],
        "codex" | "foundation_local" => vec![
            model_capability(provider_kind, account_key, CapabilityId::ModelGenerate, status),
            model_capability(provider_kind, account_key, CapabilityId::ModelClassify, status),
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
    fn openai_account_declares_models_and_hosted_search() {
        let capabilities =
            capabilities_for_provider_account("openai", "default", ProviderAccountStatus::Authenticated);
        let ids = capabilities
            .iter()
            .map(|capability| capability.capability_id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(ids, vec!["model.generate", "model.classify", "web.search"]);
        assert!(capabilities.iter().any(|capability| {
            capability.provider_kind == "openai"
                && capability.account_key == "default"
                && capability.capability_id == CapabilityId::WebSearch
                && capability.features.citations
                && !capability.features.direct_url_fetch
        }));
    }

    #[test]
    fn codex_account_does_not_declare_native_web_search() {
        let capabilities =
            capabilities_for_provider_account("codex", "default", ProviderAccountStatus::Authenticated);

        assert!(capabilities
            .iter()
            .any(|capability| capability.capability_id == CapabilityId::ModelGenerate));
        assert!(!capabilities
            .iter()
            .any(|capability| capability.capability_id == CapabilityId::WebSearch));
    }

    #[test]
    fn system_web_providers_have_no_secret_requirements() {
        let search =
            capabilities_for_provider_account("duckduckgo_public", "system", ProviderAccountStatus::Authenticated);
        let fetch =
            capabilities_for_provider_account("direct_http", "system", ProviderAccountStatus::Authenticated);

        assert_eq!(search[0].capability_id, CapabilityId::WebSearch);
        assert_eq!(search[0].reliability_contract, ReliabilityContract::BestEffortPublic);
        assert_eq!(fetch[0].capability_id, CapabilityId::WebFetch);
        assert!(fetch[0].features.direct_url_fetch);
    }

    #[test]
    fn firecrawl_is_not_declared() {
        let capabilities =
            capabilities_for_provider_account("firecrawl", "default", ProviderAccountStatus::Authenticated);

        assert!(capabilities.is_empty());
    }
}
