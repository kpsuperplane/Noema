use serde::Serialize;

/// Programmatic configuration overrides supplied by local entrypoints.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ConfigOverrides {
    /// Provider override, such as `openai` or `codex`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// Model override for the selected provider.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// OpenAI-specific overrides.
    #[serde(skip_serializing_if = "Option::is_none", rename = "openai")]
    pub openai_overrides: Option<ConfigOpenAiOverrides>,
}

impl ConfigOverrides {
    /// Build overrides from parsed entrypoint options.
    #[must_use]
    pub fn new(provider: Option<String>, model: Option<String>, base_url: Option<String>) -> Self {
        Self {
            provider,
            model,
            openai_overrides: base_url.map(|base_url| ConfigOpenAiOverrides {
                base_url: Some(base_url),
            }),
        }
    }

    /// Return the `OpenAI` base URL override, if present.
    #[must_use]
    pub fn base_url(&self) -> Option<&str> {
        self.openai_overrides
            .as_ref()
            .and_then(|openai| openai.base_url.as_deref())
    }
}

/// OpenAI-specific entrypoint overrides.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ConfigOpenAiOverrides {
    /// OpenAI-compatible API base URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}
