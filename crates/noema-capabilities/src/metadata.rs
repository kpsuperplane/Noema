//! Stable neutral capability metadata.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

macro_rules! metadata_vocabulary {
    (
        $(#[$enum_attr:meta])*
        pub enum $name:ident {
            $($(#[$variant_attr:meta])* $variant:ident = $label:literal),+ $(,)?
        }
    ) => {
        $(#[$enum_attr])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
        #[serde(rename_all = "snake_case")]
        #[ts(rename_all = "snake_case")]
        pub enum $name {
            $($(#[$variant_attr])* $variant),+
        }

        impl $name {
            /// Return this value's stable identifier.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $label),+ }
            }
        }
    };
}

metadata_vocabulary! {
    /// Stable capability identifiers exposed by provider accounts.
    #[derive(Hash)]
    pub enum CapabilityId {
        /// Text or multimodal generation.
        ModelGenerate = "model.generate",
        /// Classification or lightweight analysis.
        ModelClassify = "model.classify",
        /// Hosted or public web search.
        WebSearch = "web.search",
        /// Direct web content fetch.
        WebFetch = "web.fetch",
        /// Interactive JavaScript-rendered web browsing.
        WebBrowse = "web.browse",
    }
}

metadata_vocabulary! {
    /// Operational trust contract for a capability provider.
    pub enum ReliabilityContract {
        /// First-party Noema-owned behavior.
        FirstParty = "first_party",
        /// Hosted model or tool behavior.
        HostedProvider = "hosted_provider",
        /// Best-effort public integration.
        BestEffortPublic = "best_effort_public",
    }
}

metadata_vocabulary! {
    /// Data handling class for an invocation.
    pub enum DataFlowClass {
        /// Prompt data flows to a model provider.
        ModelProviderPrompt = "model_provider_prompt",
        /// Prompt data remains on-device.
        LocalInference = "local_inference",
        /// Query data flows to a search backend.
        TrustedExternalSearchQuery = "trusted_external_search_query",
        /// A remote web page is fetched directly.
        ExternalWebFetch = "external_web_fetch",
        /// An interactive browser exchanges data with public web pages.
        ExternalWebBrowse = "external_web_browse",
    }
}

metadata_vocabulary! {
    /// Persistence policy for capability results.
    pub enum ResultPersistencePolicy {
        /// Persist compact metadata only.
        CompactMetadata = "compact_metadata",
        /// Persist compact content payloads.
        CompactContent = "compact_content",
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
