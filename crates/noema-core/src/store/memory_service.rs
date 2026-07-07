//! Memory service store records.

/// Saved memory service mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryServiceMode {
    /// Noema manages a local Supermemory child process.
    Managed,
    /// Noema connects to an externally managed Supermemory service.
    External,
}

/// Saved memory service readiness status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryServiceStatus {
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

/// Persisted memory service settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryServiceSettingsRecord {
    /// Stable singleton settings id.
    pub settings_id: String,
    /// Supermemory service mode.
    pub mode: MemoryServiceMode,
    /// Base URL for the Supermemory service.
    pub base_url: String,
    /// Managed service port, when configured.
    pub port: Option<u16>,
    /// Provider account used for memory extraction.
    pub provider_account_id: Option<String>,
    /// Provider kind used for memory extraction.
    pub provider_kind: Option<String>,
    /// Model profile used for memory extraction.
    pub model_profile: Option<String>,
    /// Reasoning effort used for memory extraction.
    pub reasoning_effort: Option<String>,
}

/// Input for saving memory service settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveMemoryServiceSettings {
    /// Supermemory service mode.
    pub mode: MemoryServiceMode,
    /// Base URL for the Supermemory service.
    pub base_url: String,
    /// Managed service port, when configured.
    pub port: Option<u16>,
    /// Provider account used for memory extraction.
    pub provider_account_id: Option<String>,
    /// Provider kind used for memory extraction.
    pub provider_kind: Option<String>,
    /// Model profile used for memory extraction.
    pub model_profile: Option<String>,
    /// Reasoning effort used for memory extraction.
    pub reasoning_effort: Option<String>,
}

/// Persisted memory service readiness status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryServiceStatusRecord {
    /// Stable singleton status id.
    pub status_id: String,
    /// Current readiness status.
    pub status: MemoryServiceStatus,
    /// Last readiness check timestamp.
    pub checked_at: Option<String>,
    /// Sanitized last error code.
    pub last_error_code: Option<String>,
    /// Sanitized last error message.
    pub last_error_message: Option<String>,
}

/// Input for creating a memory ingest job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewMemoryIngestJob {
    /// Durable ingest job id.
    pub job_id: String,
    /// Conversation submitted to Supermemory.
    pub conversation_id: String,
    /// Completed turn submitted to Supermemory.
    pub turn_id: String,
    /// Supermemory conversation/container id.
    pub supermemory_conversation_id: String,
}

/// Persisted memory ingest job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryIngestJobRecord {
    /// Durable ingest job id.
    pub job_id: String,
    /// Conversation submitted to Supermemory.
    pub conversation_id: String,
    /// Completed turn submitted to Supermemory.
    pub turn_id: String,
    /// Current submission status.
    pub status: String,
    /// Supermemory conversation/container id.
    pub supermemory_conversation_id: String,
    /// Sanitized error code.
    pub error_code: Option<String>,
    /// Sanitized error message.
    pub error_message: Option<String>,
}
