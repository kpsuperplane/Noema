//! Provider model profile metadata.

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

use crate::ReasoningEffort;

/// One provider model/profile exposed through account metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderModelProfile {
    /// Stable provider-facing profile id.
    pub id: String,
    /// Human-readable profile label.
    pub label: String,
    /// Reasoning efforts supported by this profile.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reasoning_efforts: Vec<ReasoningEffort>,
    /// Provider-recommended default reasoning effort.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_reasoning_effort: Option<ReasoningEffort>,
    /// Maximum context window reported for this profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window_tokens: Option<u32>,
}

impl ProviderModelProfile {
    /// Decode valid profile objects from provider account metadata.
    ///
    /// Invalid entries are ignored independently. Unknown reasoning-effort
    /// strings are dropped, and a blank label falls back to the profile id.
    #[must_use]
    pub fn from_account_metadata(metadata: &Value) -> Vec<Self> {
        metadata
            .get("profiles")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Self::from_metadata_value)
            .collect()
    }

    /// Decode one profile using the tolerant legacy metadata rules.
    ///
    /// Invalid optional values are ignored independently. A missing, non-string,
    /// or blank label falls back to the normalized profile id.
    #[must_use]
    fn from_metadata_value(value: &Value) -> Option<Self> {
        let id = value.get("id")?.as_str()?.trim();
        if id.is_empty() {
            return None;
        }
        let label = value
            .get("label")
            .and_then(Value::as_str)
            .filter(|label| !label.trim().is_empty())
            .unwrap_or(id);
        let reasoning_efforts = value
            .get("reasoning_efforts")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .filter_map(parse_metadata_reasoning_effort)
            .collect();
        let default_reasoning_effort = value
            .get("default_reasoning_effort")
            .and_then(Value::as_str)
            .and_then(parse_metadata_reasoning_effort);
        let context_window_tokens = value
            .get("context_window_tokens")
            .and_then(Value::as_u64)
            .and_then(|tokens| u32::try_from(tokens).ok())
            .filter(|tokens| *tokens > 0);
        Some(Self {
            id: id.to_string(),
            label: label.to_string(),
            reasoning_efforts,
            default_reasoning_effort,
            context_window_tokens,
        })
    }

    /// Replace the typed profile list while preserving all unrelated metadata.
    ///
    /// # Errors
    ///
    /// Returns a serialization error if the typed profiles cannot be encoded.
    pub fn write_account_metadata(
        metadata: &mut Value,
        profiles: &[Self],
    ) -> Result<(), serde_json::Error> {
        let profiles = serde_json::to_value(profiles)?;
        if !metadata.is_object() {
            *metadata = Value::Object(Map::new());
        }
        let object = metadata
            .as_object_mut()
            .expect("metadata was normalized to an object");
        object.insert("profiles".to_string(), profiles);
        Ok(())
    }
}

impl<'de> Deserialize<'de> for ProviderModelProfile {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        Self::from_metadata_value(&value)
            .ok_or_else(|| serde::de::Error::custom("provider model profile id is invalid"))
    }
}

fn parse_metadata_reasoning_effort(value: &str) -> Option<ReasoningEffort> {
    ReasoningEffort::from_persistence_str(&value.trim().to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn profile_json_preserves_minimal_shape_and_metadata_field_order() {
        let minimal = ProviderModelProfile {
            id: "default".to_string(),
            label: "Default on-device".to_string(),
            reasoning_efforts: Vec::new(),
            default_reasoning_effort: None,
            context_window_tokens: None,
        };
        assert_eq!(
            serde_json::to_string(&minimal).unwrap(),
            r#"{"id":"default","label":"Default on-device"}"#,
            "minimal profile shape"
        );

        let complete = ProviderModelProfile {
            id: "gpt-5.5".to_string(),
            label: "GPT-5.5".to_string(),
            reasoning_efforts: vec![ReasoningEffort::Low, ReasoningEffort::High],
            default_reasoning_effort: Some(ReasoningEffort::High),
            context_window_tokens: Some(128_000),
        };
        assert_eq!(
            serde_json::to_string(&serde_json::to_value(complete).unwrap()).unwrap(),
            r#"{"id":"gpt-5.5","label":"GPT-5.5","reasoning_efforts":["low","high"],"default_reasoning_effort":"high","context_window_tokens":128000}"#,
            "complete metadata field order"
        );
    }

    #[test]
    fn tolerant_metadata_read_filters_unknown_efforts_and_falls_back_blank_labels() {
        let profiles = ProviderModelProfile::from_account_metadata(&json!({
            "profiles": [
                {
                    "id": " gpt ",
                    "label": " ",
                    "reasoning_efforts": [" low ", 3, "future", "HIGH"],
                    "default_reasoning_effort": {"invalid": true}
                },
                {"id": "second"},
                {"id": " ", "label": "Invalid"}
            ]
        }));

        assert_eq!(profiles.len(), 2);
        assert_eq!(profiles[0].id, "gpt");
        assert_eq!(profiles[0].label, "gpt");
        assert_eq!(
            profiles[0].reasoning_efforts,
            vec![ReasoningEffort::Low, ReasoningEffort::High]
        );
        assert_eq!(profiles[0].default_reasoning_effort, None);
        assert_eq!(profiles[1].label, "second");
    }

    #[test]
    fn replacing_profiles_preserves_unknown_account_metadata() {
        let mut metadata = json!({
            "tenant": {"region": "west"},
            "profiles": [{"id": "old", "label": "Old"}]
        });
        ProviderModelProfile::write_account_metadata(
            &mut metadata,
            &[ProviderModelProfile {
                id: "new".to_string(),
                label: "New".to_string(),
                reasoning_efforts: vec![ReasoningEffort::Medium],
                default_reasoning_effort: Some(ReasoningEffort::Medium),
                context_window_tokens: Some(64_000),
            }],
        )
        .unwrap();

        assert_eq!(metadata["tenant"], json!({"region": "west"}));
        assert_eq!(
            metadata["profiles"],
            json!([{
                "id": "new",
                "label": "New",
                "reasoning_efforts": ["medium"],
                "default_reasoning_effort": "medium",
                "context_window_tokens": 64000
            }])
        );
    }
}
