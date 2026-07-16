use async_graphql::{Enum, InputObject, Json, Result, SimpleObject};
use std::collections::{BTreeMap, HashSet};
use std::time::Duration;
use time::{Duration as TimeDuration, OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{
    MemoryArticleCacheRecord, MemoryServiceMode, MemoryServiceSettingsRecord,
    SaveMemoryArticleCache, SaveMemoryServiceSettings,
};

const HUMAN_MEMORY_SCOPE_ID: &str = "human:local";
const DEFAULT_MEMORY_GRAPH_PAGE: i32 = 1;
const DEFAULT_MEMORY_GRAPH_LIMIT: i32 = 25;
const MAX_MEMORY_GRAPH_LIMIT: i32 = 100;
const MEMORY_SERVICE_READINESS_TIMEOUT: Duration = Duration::from_secs(2);
const MEMORY_ARTICLE_CACHE_MIN_AGE: TimeDuration = TimeDuration::hours(4);
const MEMORY_ARTICLE_FORMAT_VERSION: &str = "v2";

use super::{
    agents::{
        GraphqlAgentModelPreference, GraphqlAgentModelProviderOption, GraphqlReasoningEffort,
        model_options_from_accounts, provider_disabled_reason, refresh_missing_model_profiles,
        require_selectable_profile, selectable_profiles_from_account,
        validate_reasoning_effort_for_profile,
    },
    errors::graphql_error,
    schema::GraphqlState,
};

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
    let store = state.store()?;
    let settings = store
        .memory_service_settings()
        .await
        .map_err(graphql_error)?;
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
    let Some(connection) = memory_graph_connection(state, &settings).await? else {
        let status = match settings.mode {
            MemoryServiceMode::Managed => managed_memory_unavailable_status(state).await?,
            MemoryServiceMode::External => GraphqlMemoryServiceStatus {
                status: GraphqlMemoryServiceStatusKind::NotConfigured,
                checked_at: Some(now_rfc3339()?),
                last_error_code: Some("mnemosyne_not_configured".to_string()),
                last_error_message: Some("memory service base URL is required".to_string()),
            },
        };
        return Ok(GraphqlMemoryGraph {
            status,
            article: fallback_memory_article(&[], Some(now_rfc3339()?)),
            documents: Vec::new(),
            page_info,
        });
    };

    let request_limit = u16::try_from(limit).expect("memory graph limit is clamped positive");
    let client = crate::MnemosyneClient::new(connection.base_url, connection.api_key);
    let response = match client
        .list_memories(crate::MnemosyneListMemoriesRequest {
            user_id: HUMAN_MEMORY_SCOPE_ID.to_string(),
            limit: request_limit,
        })
        .await
    {
        Ok(response) => response,
        Err(error) => {
            return Ok(GraphqlMemoryGraph {
                status: memory_graph_error_status(&error)?,
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
    let store = state.store()?;
    let settings = store
        .memory_service_settings()
        .await
        .map_err(graphql_error)?;
    let Some(connection) = memory_graph_connection(state, &settings).await? else {
        return Ok(fallback_memory_article(&[], Some(now_rfc3339()?)));
    };

    let client = crate::MnemosyneClient::new(connection.base_url, connection.api_key);
    let response = client
        .list_memories(crate::MnemosyneListMemoriesRequest {
            user_id: HUMAN_MEMORY_SCOPE_ID.to_string(),
            limit: u16::try_from(MAX_MEMORY_GRAPH_LIMIT)
                .expect("memory graph max limit fits in u16"),
        })
        .await
        .map_err(|error| async_graphql::Error::new(error.sanitized_message().to_string()))?;

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
                let account = store
                    .get_provider_account(&provider_account_id)
                    .await
                    .map_err(graphql_error)?
                    .ok_or_else(|| async_graphql::Error::new("provider account not found"))?;
                if !account.is_active || !account.is_default {
                    return Err(async_graphql::Error::new(
                        "provider account is not selectable",
                    ));
                }
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

    store
        .save_memory_service_settings(SaveMemoryServiceSettings {
            mode,
            base_url,
            port,
            provider_account_id,
            provider_kind,
            model_profile,
            reasoning_effort,
        })
        .await
        .map_err(graphql_error)?;

    memory_settings_from_store(state).await
}

async fn memory_graph_connection(
    state: &GraphqlState,
    settings: &MemoryServiceSettingsRecord,
) -> Result<Option<crate::MnemosyneConnection>> {
    match settings.mode {
        MemoryServiceMode::Managed => Ok(state.memory_connection().cloned()),
        MemoryServiceMode::External => Ok(settings
            .base_url
            .clone()
            .map(|base_url| crate::MnemosyneConnection::new(base_url, None))),
    }
}

fn memory_graph_error_status(
    error: &crate::MnemosyneClientError,
) -> Result<GraphqlMemoryServiceStatus> {
    let status = match error.sanitized_code() {
        "auth_error" => GraphqlMemoryServiceStatusKind::AuthError,
        _ => GraphqlMemoryServiceStatusKind::Unavailable,
    };
    Ok(GraphqlMemoryServiceStatus {
        status,
        checked_at: Some(now_rfc3339()?),
        last_error_code: Some(error.sanitized_code().to_string()),
        last_error_message: Some(error.sanitized_message().to_string()),
    })
}

async fn mnemosyne_memories_to_graph_documents(
    state: &GraphqlState,
    memories: Vec<crate::MnemosyneMemory>,
) -> Result<Vec<GraphqlMemoryGraphDocument>> {
    let mut groups = BTreeMap::<String, Vec<crate::MnemosyneMemory>>::new();
    for memory in memories {
        let document_id = memory
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.get("noemaConversationId"))
            .and_then(serde_json::Value::as_str)
            .map(|conversation_id| format!("conversation:{conversation_id}"))
            .unwrap_or_else(|| format!("mnemosyne:{HUMAN_MEMORY_SCOPE_ID}"));
        groups.entry(document_id).or_default().push(memory);
    }

    let mut documents = Vec::new();
    for (document_id, memories) in groups {
        let title = if document_id == format!("mnemosyne:{HUMAN_MEMORY_SCOPE_ID}") {
            "Human memory".to_string()
        } else {
            "Conversation memory".to_string()
        };
        let created_at = memories
            .iter()
            .filter_map(|memory| memory.created_at.as_deref())
            .min()
            .unwrap_or("")
            .to_string();
        let updated_at = memories
            .iter()
            .filter_map(|memory| memory.updated_at.as_deref())
            .max()
            .unwrap_or("")
            .to_string();
        let mut memory_entries = Vec::new();
        for memory in memories {
            let source = memory_source_from_metadata(state, memory.metadata.as_ref()).await;
            let metadata = memory.metadata.map(Json);
            let citation_key = memory_citation_key(&memory.id);
            memory_entries.push(GraphqlMemoryGraphMemoryEntry {
                id: memory.id,
                citation_key,
                document_id: document_id.clone(),
                content: memory.memory,
                summary: None,
                title: None,
                r#type: Some("memory".to_string()),
                source,
                metadata,
                created_at: memory.created_at.unwrap_or_default(),
                updated_at: memory.updated_at.unwrap_or_default(),
                space_container_tag: Some(HUMAN_MEMORY_SCOPE_ID.to_string()),
                relation: None,
                parent_memory_id: None,
                root_memory_id: None,
                memory_relations: None,
                is_latest: None,
                space_id: None,
            });
        }

        documents.push(GraphqlMemoryGraphDocument {
            id: document_id,
            custom_id: None,
            title: Some(title),
            content: None,
            summary: None,
            url: None,
            source: Some("mnemosyne".to_string()),
            r#type: Some("memory_group".to_string()),
            status: "ready".to_string(),
            metadata: None,
            created_at,
            updated_at,
            memory_entries,
        });
    }
    Ok(documents)
}

async fn memory_source_from_metadata(
    state: &GraphqlState,
    metadata: Option<&serde_json::Value>,
) -> Option<GraphqlMemoryGraphMemorySource> {
    let metadata = metadata?;
    let kind = metadata_string(metadata, &["sourceKind", "source_kind"]);
    let conversation_id = metadata_string(metadata, &["noemaConversationId", "conversation_id"]);
    let turn_id = metadata_string(metadata, &["turnId", "turn_id"]);
    let item_id = metadata_string(metadata, &["userItemId", "source_item_id", "item_id"]);
    let source_observation =
        metadata_string(metadata, &["sourceObservation", "source_observation"]);

    let item = match (state.optional_store(), item_id.as_deref()) {
        (Some(store), Some(item_id)) => store
            .get_visible_conversation_item(item_id)
            .await
            .ok()
            .flatten(),
        _ => None,
    };
    let message_text = item
        .as_ref()
        .and_then(|item| item.content_text.clone())
        .filter(|text| !text.trim().is_empty())
        .or(source_observation);

    if kind.is_none()
        && conversation_id.is_none()
        && turn_id.is_none()
        && item_id.is_none()
        && message_text.is_none()
    {
        return None;
    }

    Some(GraphqlMemoryGraphMemorySource {
        kind,
        conversation_id: item
            .as_ref()
            .map(|item| item.conversation_id.clone())
            .or(conversation_id),
        turn_id: item
            .as_ref()
            .and_then(|item| item.turn_id.clone())
            .or(turn_id),
        item_id: item.as_ref().map(|item| item.item_id.clone()).or(item_id),
        message_text,
    })
}

fn metadata_string(metadata: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| metadata.get(*key).and_then(serde_json::Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

async fn memory_article_for_facts(
    state: &GraphqlState,
    memories: &[crate::MnemosyneMemory],
    force: bool,
) -> Result<GraphqlMemoryArticle> {
    let store = state.store()?;
    let fingerprint = memory_fact_fingerprint(memories);
    let now = OffsetDateTime::now_utc();

    if !force
        && let Some(cached) = store
            .memory_article_cache(HUMAN_MEMORY_SCOPE_ID)
            .await
            .map_err(graphql_error)?
        && (cached.fact_fingerprint == fingerprint
            || (memory_article_cache_has_current_format(&cached)
                && memory_article_cache_is_recent(&cached, now)))
        && let Some(article) = article_from_cache(cached, memories)
    {
        return Ok(article);
    }

    let generated_at = now_rfc3339()?;
    match generate_memory_article(state, memories, &generated_at).await {
        Ok(article) => {
            store
                .save_memory_article_cache(SaveMemoryArticleCache {
                    scope_id: HUMAN_MEMORY_SCOPE_ID.to_string(),
                    fact_fingerprint: fingerprint,
                    article_markdown: article.markdown.clone(),
                    generated_at,
                })
                .await
                .map_err(graphql_error)?;
            Ok(article)
        }
        Err(_) => Ok(fallback_memory_article(memories, Some(generated_at))),
    }
}

async fn generate_memory_article(
    state: &GraphqlState,
    memories: &[crate::MnemosyneMemory],
    generated_at: &str,
) -> Result<GraphqlMemoryArticle> {
    let runtime = state.runtime()?;
    let settings = state
        .store()?
        .memory_service_settings()
        .await
        .map_err(graphql_error)?;
    let mut request = noema_providers::GenerateRequest::text(memory_article_prompt(memories));
    request.model = settings.model_profile.clone();
    request.options.reasoning_effort = settings.reasoning_effort;
    request.instructions = Some(
        "Return Markdown only. Write a compact Wikipedia-style biographical article from the supplied memory facts. Do not invent facts. Preserve the supplied inline footnote markers exactly."
            .to_string(),
    );

    let response = runtime
        .generate_once_with_provider_kind(settings.provider_kind.clone(), request)
        .await
        .map_err(graphql_error)?;
    let markdown = response.assistant_text();
    validate_memory_article_citations(&markdown, memories)?;
    Ok(markdown_to_memory_article(
        &markdown,
        memories,
        true,
        Some(generated_at.to_string()),
    ))
}

fn memory_article_prompt(memories: &[crate::MnemosyneMemory]) -> String {
    let facts = if memories.is_empty() {
        "No extracted facts are currently available.".to_string()
    } else {
        memories
            .iter()
            .enumerate()
            .map(|(index, memory)| {
                let fact = memory.memory.as_deref().unwrap_or("").trim();
                let citation_key = memory_citation_key(&memory.id);
                let source = memory
                    .metadata
                    .as_ref()
                    .and_then(|metadata| metadata.get("sourceObservation"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("");
                if source.is_empty() {
                    format!(
                        "Fact {} (cite as [^{}]):\n- Extracted fact: {}",
                        index + 1,
                        citation_key,
                        fact
                    )
                } else {
                    format!(
                        "Fact {} (cite as [^{}]):\n- Extracted fact: {}\n- User-authored source observation: {}",
                        index + 1,
                        citation_key,
                        fact,
                        source
                    )
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    };

    format!(
        r#"Write the Memory page article for Noema's local human.

Use only these memory facts:
{facts}

Style:
- Wikipedia-like, biographical, compact, and factual.
- Lead with a concise identity sentence before expanding into details.
- If little is known, make that charming but honest, e.g. "Little is currently known about Kevin."
- Prefer the person's known name as the title when available; otherwise use "Local human".
- Facts and source observations are from the local human's perspective.
- First person ("I", "me", "my") refers to the local human.
- Second person ("you", "your") refers to Noema/the assistant, not to the local human.
- Preserve who said what: do not turn a preference about another speaker, tool, or assistant into a trait of the local human.
- Omit sparse or awkward meta-preferences when they would make the biography sound strange.
- Do not mention Noema, Mnemosyne, memory systems, records, extraction, or model state in the prose.
- Every factual sentence or clause must end with the exact inline footnote marker for the fact or facts that support it, for example [^m0123456789abcdef].
- Reuse a marker when the same fact supports multiple claims. Place multiple markers together when a claim combines facts.
- Never invent, alter, renumber, or define a citation marker.

Return Markdown only:
- Start with a single H1 title.
- Then write 1-3 compact lead paragraphs.
- When facts support them, include H2 sections such as "Early life and education", "Career", "Projects", "Personal interests", or similarly natural biography headings.
- Omit unsupported sections.
- Do not include a References section or footnote definitions; return inline footnote markers only."#
    )
}

fn article_from_cache(
    record: MemoryArticleCacheRecord,
    memories: &[crate::MnemosyneMemory],
) -> Option<GraphqlMemoryArticle> {
    if record.article_markdown.trim().is_empty() {
        return None;
    }
    if validate_memory_article_citations(&record.article_markdown, memories).is_err() {
        return None;
    }
    Some(markdown_to_memory_article(
        &record.article_markdown,
        &[],
        true,
        Some(record.generated_at.clone()),
    ))
}

fn memory_article_cache_is_recent(record: &MemoryArticleCacheRecord, now: OffsetDateTime) -> bool {
    OffsetDateTime::parse(&record.generated_at, &Rfc3339)
        .map(|generated_at| now - generated_at < MEMORY_ARTICLE_CACHE_MIN_AGE)
        .unwrap_or(false)
}

fn memory_article_cache_has_current_format(record: &MemoryArticleCacheRecord) -> bool {
    record
        .fact_fingerprint
        .starts_with(&format!("{MEMORY_ARTICLE_FORMAT_VERSION}:"))
}

fn fallback_memory_article(
    memories: &[crate::MnemosyneMemory],
    generated_at: Option<String>,
) -> GraphqlMemoryArticle {
    let title = infer_memory_subject_name(memories).unwrap_or_else(|| "Local human".to_string());
    let lead = if memories.is_empty() {
        format!("Little is currently known about {title}.")
    } else {
        format!("{title} is described by the currently available biographical facts.")
    };
    let mut markdown = format!("# {title}\n\n{lead}");
    for memory in memories {
        let Some(fact) = memory.memory.as_deref().map(str::trim) else {
            continue;
        };
        if fact.is_empty() {
            continue;
        }
        markdown.push_str("\n\n");
        markdown.push_str(&biographical_text_from_fact(fact));
        markdown.push_str(&format!(" [^{}]", memory_citation_key(&memory.id)));
    }
    markdown_to_memory_article(&markdown, memories, false, generated_at)
}

fn markdown_to_memory_article(
    markdown: &str,
    memories: &[crate::MnemosyneMemory],
    is_generated: bool,
    generated_at: Option<String>,
) -> GraphqlMemoryArticle {
    let markdown = normalize_article_markdown(markdown, memories);
    let title = infer_markdown_title(&markdown).unwrap_or_else(|| {
        infer_memory_subject_name(memories).unwrap_or_else(|| "Local human".to_string())
    });
    GraphqlMemoryArticle {
        title,
        subtitle: "From Noema, the private memory encyclopedia".to_string(),
        markdown,
        is_generated,
        generated_at,
    }
}

fn normalize_article_markdown(markdown: &str, memories: &[crate::MnemosyneMemory]) -> String {
    let trimmed = markdown.trim();
    if trimmed.is_empty() {
        let title =
            infer_memory_subject_name(memories).unwrap_or_else(|| "Local human".to_string());
        return format!("# {title}\n\nLittle is currently known about {title}.");
    }
    trimmed.to_string()
}

fn infer_markdown_title(markdown: &str) -> Option<String> {
    markdown.lines().find_map(|line| {
        let line = line.trim();
        line.strip_prefix("# ")
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(ToString::to_string)
    })
}

fn biographical_text_from_fact(fact: &str) -> String {
    let trimmed = fact.trim();
    if let Some(rest) = trimmed.strip_prefix("The user ") {
        format!("The local human {rest}")
    } else if let Some(rest) = trimmed.strip_prefix("User ") {
        format!("The local human {rest}")
    } else {
        trimmed.to_string()
    }
}

fn infer_memory_subject_name(memories: &[crate::MnemosyneMemory]) -> Option<String> {
    memories
        .iter()
        .filter_map(|memory| memory.memory.as_deref())
        .find_map(|fact| {
            let normalized = fact.trim().trim_end_matches(['.', '!']);
            normalized
                .strip_prefix("I'm ")
                .or_else(|| normalized.strip_prefix("I am "))
                .or_else(|| normalized.strip_prefix("My name is "))
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(ToString::to_string)
        })
}

fn memory_fact_fingerprint(memories: &[crate::MnemosyneMemory]) -> String {
    let mut facts = memories
        .iter()
        .map(|memory| {
            format!(
                "{}\u{1f}{}\u{1f}{}",
                memory.id,
                memory.memory.as_deref().unwrap_or(""),
                memory
                    .updated_at
                    .as_deref()
                    .or(memory.created_at.as_deref())
                    .unwrap_or("")
            )
        })
        .collect::<Vec<_>>();
    facts.sort();
    let digest = ring::digest::digest(&ring::digest::SHA256, facts.join("\u{1e}").as_bytes());
    format!(
        "{MEMORY_ARTICLE_FORMAT_VERSION}:{}",
        hex_digest(digest.as_ref())
    )
}

pub(super) fn memory_citation_key(memory_id: &str) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, memory_id.as_bytes());
    format!("m{}", &hex_digest(digest.as_ref())[..16])
}

fn validate_memory_article_citations(
    markdown: &str,
    memories: &[crate::MnemosyneMemory],
) -> Result<()> {
    let cited = article_citation_keys(markdown);
    if memories.is_empty() {
        return if cited.is_empty() {
            Ok(())
        } else {
            Err(async_graphql::Error::new(
                "generated memory article cited unavailable memories",
            ))
        };
    }
    let known = memories
        .iter()
        .map(|memory| memory_citation_key(&memory.id))
        .collect::<HashSet<_>>();
    if cited.is_empty() {
        return Err(async_graphql::Error::new(
            "generated memory article omitted required citations",
        ));
    }
    if let Some(unknown) = cited.iter().find(|key| !known.contains(*key)) {
        return Err(async_graphql::Error::new(format!(
            "generated memory article used unknown citation {unknown}"
        )));
    }
    Ok(())
}

fn article_citation_keys(markdown: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let mut remaining = markdown;
    while let Some(start) = remaining.find("[^") {
        let after_start = &remaining[start + 2..];
        let Some(end) = after_start.find(']') else {
            break;
        };
        let key = &after_start[..end];
        if !key.is_empty() {
            keys.push(key.to_string());
        }
        remaining = &after_start[end + 1..];
    }
    keys
}

fn hex_digest(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

pub(super) async fn check_memory_service(
    state: &GraphqlState,
) -> Result<GraphqlMemoryServiceStatus> {
    let store = state.store()?;
    let settings = store
        .memory_service_settings()
        .await
        .map_err(graphql_error)?;
    memory_service_status(state, &settings).await
}

async fn memory_service_status(
    state: &GraphqlState,
    settings: &MemoryServiceSettingsRecord,
) -> Result<GraphqlMemoryServiceStatus> {
    let base_url = match settings.mode {
        MemoryServiceMode::Managed => {
            let Some(connection) = state.memory_connection() else {
                return managed_memory_unavailable_status(state).await;
            };
            connection.base_url.as_str()
        }
        MemoryServiceMode::External => settings
            .base_url
            .as_deref()
            .ok_or_else(|| async_graphql::Error::new("memory service base URL is required"))?,
    };
    let checked_at = Some(now_rfc3339()?);
    let status = match memory_service_readiness_request(base_url)?.send().await {
        Ok(response) if response.status().is_success() => GraphqlMemoryServiceStatus {
            status: GraphqlMemoryServiceStatusKind::Ready,
            checked_at,
            last_error_code: None,
            last_error_message: None,
        },
        Ok(response) if response.status().as_u16() == 401 || response.status().as_u16() == 403 => {
            GraphqlMemoryServiceStatus {
                status: GraphqlMemoryServiceStatusKind::AuthError,
                checked_at,
                last_error_code: Some("auth_error".to_string()),
                last_error_message: Some("memory service rejected authentication".to_string()),
            }
        }
        Ok(response) => GraphqlMemoryServiceStatus {
            status: GraphqlMemoryServiceStatusKind::Unavailable,
            checked_at,
            last_error_code: Some(format!("http_{}", response.status().as_u16())),
            last_error_message: Some("memory service returned an unsuccessful status".to_string()),
        },
        Err(error) => GraphqlMemoryServiceStatus {
            status: GraphqlMemoryServiceStatusKind::Unavailable,
            checked_at,
            last_error_code: Some("request_failed".to_string()),
            last_error_message: Some(sanitize_error_message(&error.to_string())),
        },
    };

    Ok(status)
}

async fn managed_memory_unavailable_status(
    state: &GraphqlState,
) -> Result<GraphqlMemoryServiceStatus> {
    let last_error_message = state.memory_startup_error().map(sanitize_error_message);
    Ok(GraphqlMemoryServiceStatus {
        status: GraphqlMemoryServiceStatusKind::Unavailable,
        checked_at: Some(now_rfc3339()?),
        last_error_code: Some("mnemosyne_unavailable".to_string()),
        last_error_message: Some(
            last_error_message.unwrap_or_else(|| "Managed Mnemosyne is not running".to_string()),
        ),
    })
}

fn memory_service_readiness_request(base_url: &str) -> Result<reqwest::RequestBuilder> {
    let client = reqwest::Client::builder()
        .timeout(MEMORY_SERVICE_READINESS_TIMEOUT)
        .build()
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    Ok(client.get(format!("{}/health", base_url.trim_end_matches('/'))))
}

async fn memory_settings_from_store(state: &GraphqlState) -> Result<GraphqlMemorySettings> {
    let store = state.store()?;
    super::provider_accounts::refresh_foundation_local_availability(state).await;
    let mut accounts = store
        .active_default_provider_accounts()
        .await
        .map_err(graphql_error)?;
    refresh_missing_model_profiles(state, store, &accounts).await;
    accounts = store
        .active_default_provider_accounts()
        .await
        .map_err(graphql_error)?;
    let settings = store
        .memory_service_settings()
        .await
        .map_err(graphql_error)?;
    let status = memory_service_status(state, &settings).await?;
    let model_options = model_options_from_accounts(store, &accounts).await?;
    Ok(memory_settings_from_parts(settings, status, model_options))
}

fn memory_settings_from_parts(
    settings: MemoryServiceSettingsRecord,
    status: GraphqlMemoryServiceStatus,
    model_options: Vec<GraphqlAgentModelProviderOption>,
) -> GraphqlMemorySettings {
    GraphqlMemorySettings {
        mode: settings.mode.into(),
        base_url: settings.base_url,
        port: settings.port.map(i32::from),
        status,
        model_preference: match (
            settings.provider_kind,
            settings.provider_account_id,
            settings.model_profile,
        ) {
            (Some(provider_kind), Some(provider_account_id), Some(model_profile)) => {
                Some(GraphqlAgentModelPreference {
                    provider_kind,
                    provider_account_id,
                    model_profile,
                    reasoning_effort: settings.reasoning_effort.map(GraphqlReasoningEffort::from),
                })
            }
            _ => None,
        },
        model_options,
    }
}

fn parse_memory_service_port(port: i32) -> Result<u16> {
    u16::try_from(port)
        .ok()
        .filter(|port| *port > 0)
        .ok_or_else(|| async_graphql::Error::new("memory service port must be between 1 and 65535"))
}

fn now_rfc3339() -> Result<String> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| async_graphql::Error::new(error.to_string()))
}

fn sanitize_error_message(message: &str) -> String {
    let message = message.trim();
    if message.is_empty() {
        return "memory service request failed".to_string();
    }
    message.chars().take(240).collect()
}
