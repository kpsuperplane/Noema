//! Provider-neutral capability destination identity.

use serde::{Deserialize, Serialize};
use thiserror::Error;

const MAX_DESTINATION_COMPONENT_BYTES: usize = 256;

/// Exact non-secret destination captured by an immutable capability binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CapabilityDestination {
    service_id: String,
    connection_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    account_id: Option<String>,
    revision: String,
}

impl CapabilityDestination {
    /// Construct one exact destination identity and revision fence.
    ///
    /// # Errors
    ///
    /// Returns [`CapabilityDestinationError`] when a component is blank,
    /// oversized, or contains characters outside the stable identifier set.
    pub fn new(
        service_id: impl Into<String>,
        connection_id: impl Into<String>,
        account_id: Option<impl Into<String>>,
        revision: impl Into<String>,
    ) -> Result<Self, CapabilityDestinationError> {
        Ok(Self {
            service_id: validate_component("service_id", service_id.into())?,
            connection_id: validate_component("connection_id", connection_id.into())?,
            account_id: account_id
                .map(Into::into)
                .map(|value| validate_component("account_id", value))
                .transpose()?,
            revision: validate_component("revision", revision.into())?,
        })
    }

    /// Return the service or authority family.
    #[must_use]
    pub fn service_id(&self) -> &str {
        &self.service_id
    }

    /// Return the exact configured connection identity.
    #[must_use]
    pub fn connection_id(&self) -> &str {
        &self.connection_id
    }

    /// Return the exact external account identity when one is known.
    #[must_use]
    pub fn account_id(&self) -> Option<&str> {
        self.account_id.as_deref()
    }

    /// Return the revision fence captured for this destination.
    #[must_use]
    pub fn revision(&self) -> &str {
        &self.revision
    }
}

/// Invalid non-secret destination identity.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum CapabilityDestinationError {
    /// A required identity component is blank.
    #[error("capability destination component is blank: {0}")]
    Blank(&'static str),
    /// An identity component exceeds its bounded representation.
    #[error("capability destination component is too long: {0}")]
    TooLong(&'static str),
    /// An identity component contains an unsafe or unstable character.
    #[error("capability destination component is invalid: {0}")]
    Invalid(&'static str),
}

fn validate_component(
    field: &'static str,
    value: String,
) -> Result<String, CapabilityDestinationError> {
    if value.is_empty() || value.trim() != value {
        return Err(CapabilityDestinationError::Blank(field));
    }
    if value.len() > MAX_DESTINATION_COMPONENT_BYTES {
        return Err(CapabilityDestinationError::TooLong(field));
    }
    if !value.bytes().all(|byte| {
        byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-' | b'/')
    }) {
        return Err(CapabilityDestinationError::Invalid(field));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destination_rejects_display_text_and_control_characters() {
        assert_eq!(
            CapabilityDestination::new("gmail", "personal account", None::<String>, "1")
                .expect_err("display label must not become authority"),
            CapabilityDestinationError::Invalid("connection_id")
        );
        assert_eq!(
            CapabilityDestination::new("gmail", "personal\nother", None::<String>, "1")
                .expect_err("control character"),
            CapabilityDestinationError::Invalid("connection_id")
        );
    }

    #[test]
    fn destination_serializes_only_stable_non_secret_identity() {
        let destination = CapabilityDestination::new(
            "adapter:calendar",
            "connection:personal",
            Some("account:synthetic"),
            "definition:7/credential:2",
        )
        .expect("destination");
        assert_eq!(
            serde_json::to_value(destination).expect("serialize"),
            serde_json::json!({
                "service_id": "adapter:calendar",
                "connection_id": "connection:personal",
                "account_id": "account:synthetic",
                "revision": "definition:7/credential:2",
            })
        );
    }
}
