//! Stable neutral capability metadata.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Stable capability identifiers exposed by provider accounts.
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
    /// Return the stable identifier.
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

/// Operational trust contract for a capability provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum ReliabilityContract {
    /// First-party Noema-owned behavior.
    FirstParty,
    /// Hosted model or tool behavior.
    HostedProvider,
    /// Best-effort public integration.
    BestEffortPublic,
}

impl ReliabilityContract {
    /// Return the stable storage label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FirstParty => "first_party",
            Self::HostedProvider => "hosted_provider",
            Self::BestEffortPublic => "best_effort_public",
        }
    }
}

/// Data handling class for an invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum DataFlowClass {
    /// Prompt data flows to a model provider.
    ModelProviderPrompt,
    /// Prompt data remains on-device.
    LocalInference,
    /// Query data flows to a search backend.
    TrustedExternalSearchQuery,
    /// A remote web page is fetched directly.
    ExternalWebFetch,
}

impl DataFlowClass {
    /// Return the stable storage label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ModelProviderPrompt => "model_provider_prompt",
            Self::LocalInference => "local_inference",
            Self::TrustedExternalSearchQuery => "trusted_external_search_query",
            Self::ExternalWebFetch => "external_web_fetch",
        }
    }
}

/// Persistence policy for capability results.
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
    /// Return the stable storage label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CompactMetadata => "compact_metadata",
            Self::CompactContent => "compact_content",
        }
    }
}

/// Neutral behavior metadata for a capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct CapabilityFeatures {
    /// Whether results include citations.
    pub citations: bool,
    /// Whether direct URL fetch is supported.
    pub direct_url_fetch: bool,
    /// Whether JavaScript rendering is supported.
    pub js_rendering: bool,
    /// Whether authenticated browsing context is supported.
    pub authenticated_context: bool,
    /// Result persistence behavior.
    pub result_persistence: ResultPersistencePolicy,
}
