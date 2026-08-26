use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct ChatUsage {
    #[serde(default, alias = "input_tokens")]
    pub(crate) prompt_tokens: u64,
    #[serde(default, alias = "output_tokens")]
    pub(crate) completion_tokens: u64,
    #[serde(default)]
    pub(crate) total_tokens: u64,
    #[serde(default)]
    pub(crate) prompt_tokens_details: Option<ChatPromptTokenDetails>,
    #[serde(default)]
    pub(crate) server_tool_use: Option<ChatServerToolUse>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct ChatPromptTokenDetails {
    #[serde(default)]
    pub(crate) cached_tokens: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct ChatServerToolUse {
    #[serde(default)]
    pub(crate) web_search_requests: u64,
}
