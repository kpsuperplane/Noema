//! Memory contracts and the local Mnemosyne service adapter.

mod access;
mod model;
mod operations;
mod paths;
mod repository;
mod search;
mod selection;

#[cfg(feature = "service")]
mod mnemosyne;
#[cfg(feature = "service")]
mod model_proxy;

pub use access::{
    MemoryServiceAccess, MemoryServiceAccessFuture, MemoryServiceAccessHandle,
    MemoryServiceSnapshot,
};
pub use model::{
    AddMemoryRequest, HUMAN_MEMORY_SCOPE_ID, ListMemoriesRequest, ListMemoriesResponse,
    MemoryArticleCacheRecord, MemoryMessage, MemoryRecord, MemoryServiceMode,
    MemoryServiceSettingsRecord, MemorySettingsError, SaveMemoryArticleCache,
    SaveMemoryServiceSettings, SearchMemoriesRequest, SearchMemoriesResponse,
    conversation_scope_id,
};
pub use operations::{
    MemoryOperationError, MemoryOperationFuture, MemoryOperations, MemoryOperationsHandle,
    MemoryServiceReadiness,
};
pub use paths::MemoryServicePaths;
pub use repository::{
    MemoryRepository, MemoryRepositoryError, MemoryRepositoryErrorKind, MemoryRepositoryFuture,
    MemoryRepositoryHandle, MemoryRepositoryResult,
};
pub use search::{
    MemorySearchAuthority, MemoryToolResult, SEARCH_MEMORY_TOOL_NAME, SearchMemoryError,
    execute_search_memory, is_search_memory_tool, search_memory_tool_spec,
};
pub use selection::{memory_provider_selection, memory_provider_selection_loader};

#[cfg(feature = "service")]
pub use mnemosyne::*;
#[cfg(feature = "service")]
pub use model_proxy::{MemoryModelProxy, MemoryModelProxyConfig, MemoryModelProxyError};
