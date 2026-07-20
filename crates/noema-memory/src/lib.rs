//! Native Markdown memory storage and model-facing page/search contracts.

mod native;

pub use native::{
    MEMORY_MAX_WORDS, MEMORY_OWNER, MEMORY_SCOPE, MemoryChangeSet, MemoryPage, MemoryPageChange,
    MemoryPageRef, MemorySearchResult, MemoryState, NATIVE_SEARCH_MEMORY_TOOL_NAME, NativeMemory,
    NativeMemoryError, READ_MEMORY_PAGE_TOOL_NAME, ROOT_PAGE_PATH, native_search_memory_tool_spec,
    read_memory_page_tool_spec,
};
