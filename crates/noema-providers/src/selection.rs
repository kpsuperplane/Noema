//! Immutable provider selection provenance.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

use crate::ReasoningEffort;

/// Stable key for one concrete provider instance.
#[derive(Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct ProviderInstanceKey(String);

impl ProviderInstanceKey {
    /// Construct a validated provider instance key.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderSelectionError::EmptyField`] for a blank key.
    pub fn new(value: impl Into<String>) -> Result<Self, ProviderSelectionError> {
        let value = value.into();
        let value = value.trim();
        if value.is_empty() {
            return Err(ProviderSelectionError::EmptyField("provider_instance_key"));
        }
        Ok(Self(value.to_string()))
    }

    /// Return the stable string value.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ProviderInstanceKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("ProviderInstanceKey")
            .field(&self.0)
            .finish()
    }
}

impl fmt::Display for ProviderInstanceKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ProviderInstanceKey {
    type Err = ProviderSelectionError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl<'de> Deserialize<'de> for ProviderInstanceKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// Whether a model profile is explicit or delegated to the provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderSelectionMode {
    /// A concrete provider profile is pinned in the snapshot.
    ExplicitProfile,
    /// The provider chooses its current default model/profile.
    ProviderDefault,
}

impl ProviderSelectionMode {
    /// Return the stable SQLite/API representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExplicitProfile => "explicit_profile",
            Self::ProviderDefault => "provider_default",
        }
    }
}

impl fmt::Display for ProviderSelectionMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ProviderSelectionMode {
    type Err = ProviderSelectionError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "explicit_profile" => Ok(Self::ExplicitProfile),
            "provider_default" => Ok(Self::ProviderDefault),
            other => Err(ProviderSelectionError::InvalidSelectionMode {
                value: other.to_string(),
            }),
        }
    }
}

/// Immutable provider-request provenance persisted on tasks and runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderSelectionSnapshot {
    /// Provider family, such as `codex`, `openai`, or `local_models`.
    pub provider_kind: String,
    /// Concrete provider account selected for this request.
    pub provider_account_id: String,
    /// Exact provider instance selected when one has already been resolved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_instance_key: Option<ProviderInstanceKey>,
    /// Explicit profile versus provider-owned default semantics.
    pub selection_mode: ProviderSelectionMode,
    /// Provider-specific model/profile, when explicitly selected.
    pub model_profile: Option<String>,
    /// Explicit reasoning effort, when requested.
    pub reasoning_effort: Option<ReasoningEffort>,
    /// Human/system source of the selection, retained for auditability.
    pub selection_source: Option<String>,
}

impl ProviderSelectionSnapshot {
    /// Construct an explicit model-profile snapshot without resolving an instance.
    #[must_use]
    pub fn explicit(
        provider_kind: impl Into<String>,
        provider_account_id: impl Into<String>,
        model_profile: impl Into<String>,
        reasoning_effort: Option<ReasoningEffort>,
        selection_source: Option<String>,
    ) -> Self {
        Self {
            provider_kind: provider_kind.into(),
            provider_account_id: provider_account_id.into(),
            provider_instance_key: None,
            selection_mode: ProviderSelectionMode::ExplicitProfile,
            model_profile: Some(model_profile.into()),
            reasoning_effort,
            selection_source,
        }
    }

    /// Construct a provider-default snapshot without resolving an instance.
    #[must_use]
    pub fn provider_default(
        provider_kind: impl Into<String>,
        provider_account_id: impl Into<String>,
        reasoning_effort: Option<ReasoningEffort>,
        selection_source: Option<String>,
    ) -> Self {
        Self {
            provider_kind: provider_kind.into(),
            provider_account_id: provider_account_id.into(),
            provider_instance_key: None,
            selection_mode: ProviderSelectionMode::ProviderDefault,
            model_profile: None,
            reasoning_effort,
            selection_source,
        }
    }

    /// Validate and normalize user/model input at the persistence boundary.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderSelectionError`] when the provider, account, instance,
    /// or selection mode is malformed.
    pub fn normalized(&self) -> Result<Self, ProviderSelectionError> {
        let provider_kind = self.provider_kind.trim().to_ascii_lowercase();
        if !matches!(
            provider_kind.as_str(),
            "codex" | "openai" | "foundation_local" | "local_models"
        ) {
            return Err(ProviderSelectionError::UnsupportedProvider { provider_kind });
        }
        let provider_account_id = self.provider_account_id.trim();
        if provider_account_id.is_empty() {
            return Err(ProviderSelectionError::EmptyField("provider_account_id"));
        }
        let provider_instance_key = self
            .provider_instance_key
            .as_ref()
            .map(|key| ProviderInstanceKey::new(key.as_str()))
            .transpose()?;
        let selection_source = self
            .selection_source
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        let model_profile = self
            .model_profile
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);

        match (self.selection_mode, model_profile.as_deref()) {
            (ProviderSelectionMode::ExplicitProfile, None) => {
                Err(ProviderSelectionError::ProfileRequired)
            }
            (ProviderSelectionMode::ProviderDefault, Some(_)) => {
                Err(ProviderSelectionError::ProfileForbidden)
            }
            _ => Ok(Self {
                provider_kind,
                provider_account_id: provider_account_id.to_string(),
                provider_instance_key,
                selection_mode: self.selection_mode,
                model_profile,
                reasoning_effort: self.reasoning_effort,
                selection_source,
            }),
        }
    }

    /// Normalize a snapshot before writing it to durable task storage.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderSelectionError::DurableInstanceKeyRequired`] when the
    /// snapshot has not been resolved to one exact provider instance.
    pub fn normalized_for_persistence(&self) -> Result<Self, ProviderSelectionError> {
        let normalized = self.normalized()?;
        if normalized.provider_instance_key.is_none() {
            return Err(ProviderSelectionError::DurableInstanceKeyRequired);
        }
        Ok(normalized)
    }
}

/// Validation failure for a provider selection snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProviderSelectionError {
    /// A provider family is not supported by Noema's model plane.
    #[error("unsupported model provider: {provider_kind}")]
    UnsupportedProvider {
        /// Provider family that was rejected.
        provider_kind: String,
    },
    /// A required snapshot field was blank.
    #[error("provider selection field cannot be empty: {0}")]
    EmptyField(&'static str),
    /// A persisted provider selection mode was invalid.
    #[error("invalid provider selection mode: {value}")]
    InvalidSelectionMode {
        /// Rejected persistence value.
        value: String,
    },
    /// Durable provider selections must resolve one exact provider instance.
    #[error("durable provider selection requires an exact provider instance key")]
    DurableInstanceKeyRequired,
    /// Explicit profile selection omitted its profile.
    #[error("explicit model selection requires a model profile")]
    ProfileRequired,
    /// Provider-default selection must not pin a profile.
    #[error("provider-default model selection cannot include a model profile")]
    ProfileForbidden,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn snapshots_preserve_explicit_and_provider_default_semantics() {
        let explicit = ProviderSelectionSnapshot::explicit(
            "Codex",
            "provider_account:codex:default",
            " gpt-5.5 ",
            Some(ReasoningEffort::High),
            Some("pool:simple".to_string()),
        )
        .normalized()
        .expect("explicit snapshot");
        assert_eq!(explicit.provider_kind, "codex");
        assert_eq!(explicit.model_profile.as_deref(), Some("gpt-5.5"));

        let local = ProviderSelectionSnapshot::explicit(
            "local_models",
            "provider_account:local_models:default",
            "ternary-bonsai-8b",
            None,
            Some("pool:simple".to_string()),
        )
        .normalized()
        .expect("local model snapshot");
        assert_eq!(local.provider_kind, "local_models");

        let inherited = ProviderSelectionSnapshot::provider_default(
            "openai",
            "provider_account:openai:default",
            None,
            Some("primary:effective".to_string()),
        );
        assert_eq!(
            inherited
                .normalized()
                .expect("default snapshot")
                .model_profile,
            None
        );
    }

    #[test]
    fn resolved_instance_key_serializes_exactly_when_present() {
        let mut snapshot = ProviderSelectionSnapshot::explicit(
            "openai",
            "provider_account:openai:default",
            "gpt-5.5",
            None,
            None,
        );
        snapshot.provider_instance_key =
            Some(ProviderInstanceKey::new("openai:default:1").unwrap());

        assert_eq!(
            serde_json::to_value(snapshot).unwrap()["provider_instance_key"],
            json!("openai:default:1")
        );
    }

    #[test]
    fn persistence_requires_and_retains_a_resolved_instance_key() {
        let unresolved = ProviderSelectionSnapshot::explicit(
            "openai",
            "provider_account:openai:default",
            "gpt-5.5",
            None,
            None,
        );
        assert_eq!(
            unresolved.normalized_for_persistence(),
            Err(ProviderSelectionError::DurableInstanceKeyRequired)
        );

        let mut snapshot = ProviderSelectionSnapshot::explicit(
            "openai",
            "provider_account:openai:default",
            "gpt-5.5",
            None,
            None,
        );
        snapshot.provider_instance_key =
            Some(ProviderInstanceKey::new("openai:default:1").unwrap());

        let persisted = snapshot
            .normalized_for_persistence()
            .expect("resolved durable selection");
        assert_eq!(
            persisted
                .provider_instance_key
                .as_ref()
                .map(ProviderInstanceKey::as_str),
            Some("openai:default:1")
        );
    }
}
