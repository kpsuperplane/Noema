pub mod config;
pub mod daemon;
pub mod home;
pub mod paths;
pub mod provider;
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
