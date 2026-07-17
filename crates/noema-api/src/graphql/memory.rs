use async_graphql::{Enum, InputObject, Json, Result, SimpleObject};
use std::collections::{BTreeMap, HashSet};
use time::{Duration as TimeDuration, OffsetDateTime, format_description::well_known::Rfc3339};

use noema_memory::{
    HUMAN_MEMORY_SCOPE_ID, ListMemoriesRequest, MemoryArticleCacheRecord, MemoryOperationError,
    MemoryRecord, MemoryServiceMode, MemoryServiceSettingsRecord, MemoryServiceSnapshot,
    SaveMemoryArticleCache, SaveMemoryServiceSettings,
};

const DEFAULT_MEMORY_GRAPH_PAGE: i32 = 1;
const DEFAULT_MEMORY_GRAPH_LIMIT: i32 = 25;
const MAX_MEMORY_GRAPH_LIMIT: i32 = 100;
const MEMORY_ARTICLE_CACHE_MIN_AGE: TimeDuration = TimeDuration::hours(4);
const MEMORY_ARTICLE_FORMAT_VERSION: &str = "v2";

use super::{
    agents::{
        GraphqlAgentModelPreference, GraphqlAgentModelProviderOption, GraphqlReasoningEffort,
        active_default_model_accounts, model_options_from_accounts, provider_disabled_reason,
        require_selectable_profile, selectable_model_account, selectable_profiles_from_account,
        validate_reasoning_effort_for_profile,
    },
    errors::graphql_error,
    schema::GraphqlState,
};

mod article;
mod graph;
mod status;

use article::*;
use graph::*;
use status::*;

pub(super) use article::memory_citation_key;
pub(super) use status::check_memory_service;

/// Memory service operating mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "MemoryServiceMode")]
pub enum GraphqlMemoryServiceMode {
    /// Noema manages a local memory service.
    Managed,
    /// Noema connects to an externally managed memory service.
    External,
}

impl From<MemoryServiceMode> for GraphqlMemoryServiceMode {
    fn from(value: MemoryServiceMode) -> Self {
        match value {
            MemoryServiceMode::Managed => Self::Managed,
            MemoryServiceMode::External => Self::External,
        }
    }
}

impl From<GraphqlMemoryServiceMode> for MemoryServiceMode {
    fn from(value: GraphqlMemoryServiceMode) -> Self {
        match value {
            GraphqlMemoryServiceMode::Managed => Self::Managed,
            GraphqlMemoryServiceMode::External => Self::External,
        }
    }
}

/// Memory service readiness status kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "MemoryServiceStatusKind")]
pub enum GraphqlMemoryServiceStatusKind {
    /// Service configuration has not been checked.
    NotConfigured,
    /// Managed service startup is in progress.
    Starting,
    /// Service is reachable and ready.
    Ready,
    /// Service is not reachable.
    Unavailable,
    /// Service authentication failed.
    AuthError,
}

/// Memory service readiness exposed to Settings.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "MemoryServiceStatus")]
pub struct GraphqlMemoryServiceStatus {
    /// Current readiness status.
    pub status: GraphqlMemoryServiceStatusKind,
    /// Last readiness check timestamp.
    pub checked_at: Option<String>,
    /// Sanitized last error code.
    pub last_error_code: Option<String>,
    /// Sanitized last error message.
    pub last_error_message: Option<String>,
}

/// Memory service settings exposed to Settings.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "MemorySettings")]
pub struct GraphqlMemorySettings {
    /// Memory service mode.
    pub mode: GraphqlMemoryServiceMode,
    /// External memory service base URL.
    pub base_url: Option<String>,
    /// External service port, when configured.
    pub port: Option<i32>,
    /// Current readiness status.
    pub status: GraphqlMemoryServiceStatus,
    /// Current persisted extraction model preference, when configured.
    pub model_preference: Option<GraphqlAgentModelPreference>,
    /// Provider/profile options available for memory extraction.
    pub model_options: Vec<GraphqlAgentModelProviderOption>,
}

/// Input for reading the human memory graph.
#[derive(Clone, Debug, Default, InputObject)]
#[graphql(name = "MemoryGraphInput")]
pub struct GraphqlMemoryGraphInput {
    /// One-based document page.
    pub page: Option<i32>,
    /// Maximum document count.
    pub limit: Option<i32>,
}

/// Memory graph response for the local human.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "MemoryGraph")]
pub struct GraphqlMemoryGraph {
    /// Memory service availability for this request.
    pub status: GraphqlMemoryServiceStatus,
    /// AI-written article describing the local human from memory facts.
    pub article: GraphqlMemoryArticle,
    /// Documents and memory entries in the graph.
    pub documents: Vec<GraphqlMemoryGraphDocument>,
    /// Pagination metadata.
    pub page_info: GraphqlMemoryGraphPageInfo,
}

/// AI-written article describing the local human memory record.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "MemoryArticle")]
pub struct GraphqlMemoryArticle {
    /// Article title.
    pub title: String,
    /// Article subtitle.
    pub subtitle: String,
    /// Markdown body for the article.
    pub markdown: String,
    /// Whether a model generated this article.
    pub is_generated: bool,
    /// Article generation timestamp.
    pub generated_at: Option<String>,
}

/// Memory graph pagination metadata.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "MemoryGraphPageInfo")]
pub struct GraphqlMemoryGraphPageInfo {
    /// One-based document page.
    pub page: i32,
    /// Requested document limit.
    pub limit: i32,
    /// Whether more documents are available.
    pub has_more: bool,
    /// Total memory count when returned by the memory service.
    pub total: Option<i32>,
}

/// One document displayed in the memory graph.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "MemoryGraphDocument")]
pub struct GraphqlMemoryGraphDocument {
    /// Memory document/group id.
    pub id: String,
    /// Caller supplied document id.
    pub custom_id: Option<String>,
    /// Document title.
    pub title: Option<String>,
    /// Document content.
    pub content: Option<String>,
    /// Document summary.
    pub summary: Option<String>,
    /// Source URL.
    pub url: Option<String>,
    /// Source label.
    pub source: Option<String>,
    /// Memory document/group type.
    pub r#type: Option<String>,
    /// Memory document/group status.
    pub status: String,
    /// Document metadata.
    pub metadata: Option<Json<serde_json::Value>>,
    /// Creation timestamp.
    pub created_at: String,
    /// Update timestamp.
    pub updated_at: String,
    /// Memory entries extracted from the document.
    pub memory_entries: Vec<GraphqlMemoryGraphMemoryEntry>,
}

/// One memory entry displayed in the memory graph.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "MemoryGraphMemoryEntry")]
pub struct GraphqlMemoryGraphMemoryEntry {
    /// Memory entry id.
    pub id: String,
    /// Stable key used by generated article footnotes to cite this memory.
    pub citation_key: String,
    /// Source document id.
    pub document_id: String,
    /// Memory content.
    pub content: Option<String>,
    /// Memory summary.
    pub summary: Option<String>,
    /// Memory title.
    pub title: Option<String>,
    /// Memory type.
    pub r#type: Option<String>,
    /// Noema source observation that produced this memory, when known.
    pub source: Option<GraphqlMemoryGraphMemorySource>,
    /// Memory metadata.
    pub metadata: Option<Json<serde_json::Value>>,
    /// Creation timestamp.
    pub created_at: String,
    /// Update timestamp.
    pub updated_at: String,
    /// Memory scope tag.
    pub space_container_tag: Option<String>,
    /// Relationship to another memory.
    pub relation: Option<String>,
    /// Parent memory id when available.
    pub parent_memory_id: Option<String>,
    /// Root memory id for the memory graph branch.
    pub root_memory_id: Option<String>,
    /// Memory relation map keyed by target memory id.
    pub memory_relations: Option<Json<serde_json::Value>>,
    /// Whether this is the latest memory.
    pub is_latest: Option<bool>,
    /// Memory space id.
    pub space_id: Option<String>,
}

/// Noema provenance for a memory entry.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "MemoryGraphMemorySource")]
pub struct GraphqlMemoryGraphMemorySource {
    /// Source kind recorded by Noema.
    pub kind: Option<String>,
    /// Durable Noema conversation id, when known.
    pub conversation_id: Option<String>,
    /// Durable Noema turn id, when known.
    pub turn_id: Option<String>,
    /// Durable Noema conversation item id, when known.
    pub item_id: Option<String>,
    /// Exact persisted user message text when available, otherwise the stored source observation.
    pub message_text: Option<String>,
}

/// Input for saving memory service settings.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "SaveMemoryServiceSettingsInput")]
pub struct GraphqlSaveMemoryServiceSettingsInput {
    /// Memory service mode.
    pub mode: GraphqlMemoryServiceMode,
    /// External memory service base URL.
    pub base_url: Option<String>,
    /// External service port, when configured.
    pub port: Option<i32>,
    /// Provider account id to use for memory extraction.
    pub provider_account_id: Option<String>,
    /// Provider-specific model id or profile id.
    pub model_profile: Option<String>,
    /// Optional explicit reasoning effort for reasoning-capable model profiles.
    pub reasoning_effort: Option<GraphqlReasoningEffort>,
}

pub(super) async fn memory_settings(state: &GraphqlState) -> Result<GraphqlMemorySettings> {
    memory_settings_from_store(state).await
}

pub(super) async fn memory_graph(
    state: &GraphqlState,
    input: GraphqlMemoryGraphInput,
) -> Result<GraphqlMemoryGraph> {
    let (settings, memory_operations) = resolve_memory_service(state).await?.into_parts();
    let page = input.page.unwrap_or(DEFAULT_MEMORY_GRAPH_PAGE).max(1);
    let limit = input
        .limit
        .unwrap_or(DEFAULT_MEMORY_GRAPH_LIMIT)
        .clamp(1, MAX_MEMORY_GRAPH_LIMIT);
    let page_info = GraphqlMemoryGraphPageInfo {
        page,
        limit,
        has_more: false,
        total: None,
    };
    let Some(memory_operations) = memory_operations else {
        let status = missing_memory_operations_status(state, &settings).await?;
        return Ok(GraphqlMemoryGraph {
            status,
            article: fallback_memory_article(&[], Some(now_rfc3339()?)),
            documents: Vec::new(),
            page_info,
        });
    };

    let request_limit = u16::try_from(limit).expect("memory graph limit is clamped positive");
    let response = match memory_operations
        .list_memories(ListMemoriesRequest {
            user_id: HUMAN_MEMORY_SCOPE_ID.to_string(),
            limit: request_limit,
        })
        .await
    {
        Ok(response) => response,
        Err(error) => {
            let status = if settings.mode == MemoryServiceMode::Managed
                && error == MemoryOperationError::ServiceUnavailable
            {
                managed_memory_unavailable_status(state).await?
            } else {
                memory_graph_error_status(&error)?
            };
            return Ok(GraphqlMemoryGraph {
                status,
                article: fallback_memory_article(&[], Some(now_rfc3339()?)),
                documents: Vec::new(),
                page_info,
            });
        }
    };
    let page_info = GraphqlMemoryGraphPageInfo {
        page,
        limit,
        has_more: false,
        total: Some(i32::try_from(response.results.len()).unwrap_or(i32::MAX)),
    };

    let article = memory_article_for_facts(state, &response.results, false).await?;

    Ok(GraphqlMemoryGraph {
        status: GraphqlMemoryServiceStatus {
            status: GraphqlMemoryServiceStatusKind::Ready,
            checked_at: Some(now_rfc3339()?),
            last_error_code: None,
            last_error_message: None,
        },
        documents: mnemosyne_memories_to_graph_documents(state, response.results).await?,
        article,
        page_info,
    })
}

pub(super) async fn regenerate_memory_article(
    state: &GraphqlState,
) -> Result<GraphqlMemoryArticle> {
    let (_, memory_operations) = resolve_memory_service(state).await?.into_parts();
    let Some(memory_operations) = memory_operations else {
        return Ok(fallback_memory_article(&[], Some(now_rfc3339()?)));
    };

    let response = memory_operations
        .list_memories(ListMemoriesRequest {
            user_id: HUMAN_MEMORY_SCOPE_ID.to_string(),
            limit: u16::try_from(MAX_MEMORY_GRAPH_LIMIT)
                .expect("memory graph max limit fits in u16"),
        })
        .await
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;

    memory_article_for_facts(state, &response.results, true).await
}

pub(super) async fn save_memory_service_settings(
    state: &GraphqlState,
    input: GraphqlSaveMemoryServiceSettingsInput,
) -> Result<GraphqlMemorySettings> {
    let store = state.store()?;
    let mode: MemoryServiceMode = input.mode.into();
    let (base_url, port) = match mode {
        MemoryServiceMode::External => {
            let base_url = input.base_url.as_deref().unwrap_or("").trim();
            if base_url.is_empty() {
                return Err(async_graphql::Error::new(
                    "memory service base URL is required",
                ));
            }
            (
                Some(base_url.to_string()),
                input.port.map(parse_memory_service_port).transpose()?,
            )
        }
        MemoryServiceMode::Managed => (None, None),
    };
    let (provider_account_id, provider_kind, model_profile, reasoning_effort) =
        match (input.provider_account_id, input.model_profile) {
            (Some(provider_account_id), Some(model_profile)) => {
                let model_profile = model_profile.trim().to_string();
                if model_profile.is_empty() {
                    return Err(async_graphql::Error::new("model profile is required"));
                }
                let account = selectable_model_account(state, &provider_account_id).await?;
                if let Some(reason) = provider_disabled_reason(&account) {
                    return Err(async_graphql::Error::new(reason));
                }
                let profiles = selectable_profiles_from_account(store, &account).await?;
                let profile = require_selectable_profile(&profiles, &model_profile)?;
                let reasoning_effort =
                    validate_reasoning_effort_for_profile(profile, input.reasoning_effort)?;
                (
                    Some(account.provider_account_id),
                    Some(account.provider_kind),
                    Some(model_profile),
                    reasoning_effort,
                )
            }
            (None, None) => {
                if input.reasoning_effort.is_some() {
                    return Err(async_graphql::Error::new(
                        "reasoning effort requires a provider account and model profile",
                    ));
                }
                (None, None, None, None)
            }
            (Some(_), None) => {
                return Err(async_graphql::Error::new("model profile is required"));
            }
            (None, Some(_)) => {
                return Err(async_graphql::Error::new("provider account is required"));
            }
        };

    let ready_selection = match (
        provider_kind.as_deref(),
        provider_account_id.as_deref(),
        model_profile.as_deref(),
    ) {
        (Some(provider_kind), Some(provider_account_id), Some(model_profile)) => Some(
            super::provider_selection::prove_ready_selection(
                state,
                provider_kind,
                provider_account_id,
                model_profile,
                reasoning_effort,
                "graphql_memory_service_settings",
            )
            .await?,
        ),
        _ => None,
    };
    let settings = SaveMemoryServiceSettings {
        mode,
        base_url,
        port,
        provider_account_id,
        provider_kind,
        model_profile,
        reasoning_effort,
    };
    match ready_selection.as_ref() {
        Some(ready_selection) => {
            store
                .save_memory_service_settings_with_ready_selection(settings, ready_selection)
                .await
        }
        None => store.save_memory_service_settings(settings).await,
    }
    .map_err(graphql_error)?;

    memory_settings_from_store(state).await
}
