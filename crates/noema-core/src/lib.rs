//! Core Noema types and runtimes.
//!
//! This crate contains configuration loading, provider adapters, daemon
//! protocol support, home-directory setup, path resolution, and the V1 memory
//! retrieval model.

/// Configuration loading and provider selection.
pub mod config;
/// Local daemon protocol and client/server runtime.
pub mod daemon;
/// Noema home-directory initialization.
pub mod home;
/// V1 memory storage and retrieval policy model.
pub mod memory;
/// SQLite-backed durable memory repository.
pub mod memory_persistence;
/// Filesystem path resolution for Noema state.
pub mod paths;
/// Provider-neutral generation request and response types.
pub mod provider;
/// Concrete model provider adapters.
pub mod providers;

pub use config::{CliOverrides, Config, ConfigError, ProviderConfig, ProviderKind, ResolvedConfig};
pub use daemon::{
    DaemonClient, DaemonError, DaemonRequest, DaemonResponse, DaemonServerConfig,
    StartedConversation, default_socket_path, is_connection_refused, run_daemon,
    socket_path_for_home,
};
pub use home::{
    DEFAULT_NOEMA_CONFIG_YAML, NoemaHomeError, NoemaHomeInitOptions, NoemaHomeInitResult,
    init_noema_home,
};
pub use memory_persistence::{
    ChatMemorySource, MemoryAuthorityLevel, MemoryExtractionMethod, MemoryPersistenceError,
    MemorySummary, MemoryType, NewChatMemoryCandidate, NewMemoryParticipant,
    SqliteMemoryRepository,
};
pub use paths::{NOEMA_HOME_ENV, NoemaPathError, NoemaPaths};
pub use provider::{
    GenerateInput, GenerateOptions, GenerateRequest, GenerateResponse, ModelProvider,
    ProviderError, TokenUsage,
};
pub use providers::{
    codex::{CodexProvider, CodexProviderConfig},
    codex_app_server::{CodexAppServerConversation, CodexAppServerRuntime},
    openai::{OpenAiProvider, OpenAiProviderConfig},
};
