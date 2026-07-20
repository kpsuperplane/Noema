//! GraphQL adapters for the native Markdown memory tree.

use async_graphql::{InputObject, Result, SimpleObject};
use futures_util::Stream;
use noema_memory::{MemoryPage, MemoryPageRef, NativeMemoryError};
use noema_store::AuxiliaryModelTask;

use super::{
    agents::{
        GraphqlAgentModelPreference, GraphqlReasoningEffort, auxiliary_model_settings,
        save_auxiliary_model_preference,
    },
    errors::graphql_error,
    schema::GraphqlState,
};

#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlNativeMemoryPageRef {
    pub id: String,
    pub path: String,
    pub title: String,
    pub excerpt: String,
    pub hash: String,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlNativeMemoryPage {
    pub id: String,
    pub path: String,
    pub title: String,
    pub body: String,
    pub hash: String,
    pub sources: Vec<String>,
    pub source_references: Vec<GraphqlNativeMemorySourceReference>,
    pub parent: Option<String>,
    pub ancestors: Vec<GraphqlNativeMemoryPageRef>,
    pub children: Vec<GraphqlNativeMemoryPageRef>,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlNativeMemorySourceReference {
    pub source: String,
    pub excerpt: Option<String>,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlNativeMemoryTree {
    pub root: Option<GraphqlNativeMemoryPage>,
    pub pages: Vec<GraphqlNativeMemoryPageRef>,
    pub pending_count: i32,
    pub update_status: GraphqlNativeMemoryUpdateStatus,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlNativeMemorySettings {
    pub model_preference: Option<GraphqlAgentModelPreference>,
    pub model_options: Vec<super::agents::GraphqlAgentModelProviderOption>,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlNativeMemoryUpdateStatus {
    pub state: String,
    pub active: bool,
    pub last_consolidated_sequence: i64,
    pub last_consolidated_item: Option<String>,
    pub error: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlNativeMemorySearchResult {
    pub id: String,
    pub path: String,
    pub title: String,
    pub snippet: String,
    pub hash: String,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlNativeMemoryUpdateResult {
    pub accepted: bool,
    pub status: GraphqlNativeMemoryUpdateStatus,
}

#[derive(Clone, Debug, InputObject)]
pub struct GraphqlSaveMemoryModelPreferenceInput {
    pub provider_account_id: String,
    pub model_profile: String,
    pub reasoning_effort: Option<GraphqlReasoningEffort>,
}

pub async fn memory_tree(state: &GraphqlState) -> Result<GraphqlNativeMemoryTree> {
    let memory = state
        .native_memory()
        .ok_or_else(|| async_graphql::Error::new("native memory is unavailable"))?;
    let root = memory.read_root().map_err(native_error)?;
    let pages = memory
        .list_pages()
        .map_err(native_error)?
        .iter()
        .map(MemoryPageRef::from)
        .map(child)
        .collect();
    Ok(GraphqlNativeMemoryTree {
        root: Some(page(state, root).await?),
        pages,
        pending_count: pending_count(state, memory).await,
        update_status: update_status(state, memory).await?,
    })
}

pub async fn memory_events(
    state: &GraphqlState,
    principal: &str,
) -> Result<impl Stream<Item = Result<GraphqlNativeMemoryTree>>> {
    require_memory_owner(principal)?;
    let mut receiver = state.subscriptions().subscribe_memory();
    let initial = memory_tree(state).await?;
    let state = state.clone();
    Ok(async_stream::stream! {
        yield Ok(initial);
        loop {
            match receiver.recv().await {
                Ok(noema_runtime::MemoryRuntimeEvent::Changed)
                | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
            yield memory_tree(&state).await;
        }
    })
}

pub async fn memory_page(
    state: &GraphqlState,
    page_id: String,
) -> Result<Option<GraphqlNativeMemoryPage>> {
    let Some(memory) = state.native_memory() else {
        return Ok(None);
    };
    match memory.read_page(&page_id) {
        Ok(value) => Ok(Some(page(state, value).await?)),
        Err(NativeMemoryError::InvalidPage(_)) => Ok(None),
        Err(error) => Err(native_error(error)),
    }
}

pub async fn search_memory(
    state: &GraphqlState,
    query: String,
    limit: Option<i32>,
) -> Result<Vec<GraphqlNativeMemorySearchResult>> {
    let memory = state
        .native_memory()
        .ok_or_else(|| async_graphql::Error::new("native memory is unavailable"))?;
    memory
        .search(&query, limit.unwrap_or(8).clamp(1, 16) as usize)
        .map(|results| {
            results
                .into_iter()
                .map(|result| GraphqlNativeMemorySearchResult {
                    id: result.id,
                    path: result.path,
                    title: result.title,
                    snippet: result.snippet,
                    hash: result.hash,
                })
                .collect()
        })
        .map_err(native_error)
}

pub async fn update_memory(
    state: &GraphqlState,
    principal: &str,
) -> Result<GraphqlNativeMemoryUpdateResult> {
    require_memory_owner(principal)?;
    let primary = state
        .store()?
        .primary_conversation_for_human(principal)
        .await
        .map_err(graphql_error)?
        .ok_or_else(|| async_graphql::Error::new("primary conversation is unavailable"))?;
    let accepted = state
        .runtime()?
        .trigger_native_memory_update(primary.conversation_id)
        .await
        .map_err(graphql_error)?;
    let memory = state
        .native_memory()
        .ok_or_else(|| async_graphql::Error::new("native memory is unavailable"))?;
    Ok(GraphqlNativeMemoryUpdateResult {
        accepted,
        status: update_status(state, memory).await?,
    })
}

pub async fn memory_settings(state: &GraphqlState) -> Result<GraphqlNativeMemorySettings> {
    let settings = auxiliary_model_settings(state, AuxiliaryModelTask::MemoryConsolidation).await?;
    Ok(GraphqlNativeMemorySettings {
        model_preference: settings.preference,
        model_options: settings.options,
    })
}

pub async fn save_memory_model_preference(
    state: &GraphqlState,
    input: GraphqlSaveMemoryModelPreferenceInput,
) -> Result<GraphqlAgentModelPreference> {
    save_auxiliary_model_preference(
        state,
        AuxiliaryModelTask::MemoryConsolidation,
        input.provider_account_id,
        input.model_profile,
        input.reasoning_effort,
        "graphql_memory_model_preference",
    )
    .await
}

async fn update_status(
    state: &GraphqlState,
    memory: &noema_memory::NativeMemory,
) -> Result<GraphqlNativeMemoryUpdateStatus> {
    let state_file = memory.state().map_err(native_error)?;
    let active = match state.runtime() {
        Ok(runtime) => runtime.native_memory_update_active().await.unwrap_or(false),
        Err(_) => false,
    };
    let error = match state.runtime() {
        Ok(runtime) => runtime.native_memory_update_error().await.unwrap_or(None),
        Err(_) => None,
    };
    Ok(GraphqlNativeMemoryUpdateStatus {
        state: if active {
            "running"
        } else if error.is_some() {
            "error"
        } else {
            "idle"
        }
        .to_string(),
        active,
        last_consolidated_sequence: state_file.last_consolidated_sequence,
        last_consolidated_item: state_file.last_consolidated_item,
        error,
        updated_at: (!state_file.updated_at.is_empty()).then_some(state_file.updated_at),
    })
}

async fn page(state: &GraphqlState, page: MemoryPage) -> Result<GraphqlNativeMemoryPage> {
    let source_references = source_references(state.store()?, &page.sources).await?;
    Ok(GraphqlNativeMemoryPage {
        id: page.id,
        path: page.path,
        title: page.title,
        body: page.body,
        hash: page.hash,
        sources: page.sources,
        source_references,
        parent: page.parent,
        ancestors: page.ancestors.into_iter().map(child).collect(),
        children: page.children.into_iter().map(child).collect(),
    })
}

async fn source_references(
    store: &noema_store::NoemaStore,
    sources: &[String],
) -> Result<Vec<GraphqlNativeMemorySourceReference>> {
    let mut references = Vec::with_capacity(sources.len());
    for source in sources {
        let excerpt = store
            .get_visible_conversation_item(source)
            .await
            .map_err(graphql_error)?
            .and_then(|item| item.content_text)
            .map(|text| bounded_reference_excerpt(&text));
        references.push(GraphqlNativeMemorySourceReference {
            source: source.clone(),
            excerpt,
        });
    }
    Ok(references)
}

fn bounded_reference_excerpt(text: &str) -> String {
    const LIMIT: usize = 360;
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let Some((boundary, _)) = normalized.char_indices().nth(LIMIT) else {
        return normalized;
    };
    format!("{}…", normalized[..boundary].trim_end())
}
fn child(child: MemoryPageRef) -> GraphqlNativeMemoryPageRef {
    GraphqlNativeMemoryPageRef {
        id: child.id,
        path: child.path,
        title: child.title,
        excerpt: child.excerpt,
        hash: child.hash,
    }
}
async fn pending_count(state: &GraphqlState, memory: &noema_memory::NativeMemory) -> i32 {
    let Ok(store) = state.store() else { return 0 };
    let Ok(Some(primary)) = store.primary_conversation_for_human("human:local").await else {
        return 0;
    };
    let Ok(checkpoint) = memory.state() else {
        return 0;
    };
    let cursor = if checkpoint.conversation_id.as_deref() == Some(primary.conversation_id.as_str())
    {
        checkpoint.last_consolidated_sequence
    } else {
        0
    };
    match store
        .capture_memory_source_range(&primary.conversation_id, cursor)
        .await
    {
        Ok(range) => range.items.len() as i32,
        Err(_) => 0,
    }
}
fn native_error(error: NativeMemoryError) -> async_graphql::Error {
    async_graphql::Error::new(error.to_string())
}

fn require_memory_owner(principal: &str) -> Result<()> {
    if principal == "human:local" {
        Ok(())
    } else {
        Err(async_graphql::Error::new(
            "memory belongs to the local human",
        ))
    }
}
