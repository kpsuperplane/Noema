use std::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize};

use crate::WorkspaceInputError;

const MAX_ID_BYTES: usize = 255;

fn validate_id(
    value: String,
    kind: &'static str,
    prefix: &'static str,
) -> Result<String, WorkspaceInputError> {
    if value.trim().is_empty() {
        return Err(WorkspaceInputError::InvalidId {
            kind,
            reason: "identifier cannot be blank",
        });
    }
    if value.len() > MAX_ID_BYTES {
        return Err(WorkspaceInputError::InvalidId {
            kind,
            reason: "identifier exceeds 255 UTF-8 bytes",
        });
    }
    if value.chars().any(char::is_control) {
        return Err(WorkspaceInputError::InvalidId {
            kind,
            reason: "identifier cannot contain control characters",
        });
    }
    if !value.starts_with(prefix) {
        return Err(WorkspaceInputError::InvalidId {
            kind,
            reason: "identifier has the wrong semantic prefix",
        });
    }
    if value[prefix.len()..].trim().is_empty() {
        return Err(WorkspaceInputError::InvalidId {
            kind,
            reason: "identifier must contain a value after its prefix",
        });
    }
    Ok(value)
}

macro_rules! semantic_id {
    ($name:ident, $kind:literal, $prefix:literal) => {
        /// Validated opaque identifier owned by the workspace domain.
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Construct and validate an identifier.
            ///
            /// # Errors
            ///
            /// Returns [`WorkspaceInputError`] when the value is blank, too
            /// long, contains control characters, or has the wrong semantic
            /// prefix.
            pub fn new(value: impl Into<String>) -> Result<Self, WorkspaceInputError> {
                Ok(Self(validate_id(value.into(), $kind, $prefix)?))
            }

            /// Borrow the canonical persisted value.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Consume the wrapper and return its canonical value.
            #[must_use]
            pub fn into_string(self) -> String {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = WorkspaceInputError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::new(value)
            }
        }

        impl TryFrom<String> for $name {
            type Error = WorkspaceInputError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl TryFrom<&str> for $name {
            type Error = WorkspaceInputError;

            fn try_from(value: &str) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::new(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

semantic_id!(WorkspaceId, "workspace", "workspace:");
semantic_id!(ProjectId, "project", "project:");

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::{ProjectId, WorkspaceId};

    #[test]
    fn ids_normalize_only_outer_whitespace_is_not_accepted_as_identity() {
        let id = WorkspaceId::new("workspace:personal").expect("valid workspace id");
        assert_eq!(id.as_str(), "workspace:personal");
        assert_eq!(
            ProjectId::from_str("project:alpha").unwrap().to_string(),
            "project:alpha"
        );
        assert!(WorkspaceId::new(" workspace:personal ").is_err());
    }

    #[test]
    fn ids_reject_prefix_blanks_controls_and_oversize_values() {
        for value in ["", "   ", "project:wrong", "workspace:\nlocal"] {
            assert!(WorkspaceId::new(value).is_err(), "{value:?}");
        }
        assert!(ProjectId::new("project:").is_err());
        assert!(ProjectId::new(format!("project:{}", "x".repeat(250))).is_err());
    }

    #[test]
    fn ids_fail_closed_during_json_deserialization() {
        assert!(serde_json::from_str::<WorkspaceId>("\"task:1\"").is_err());
        let value =
            serde_json::to_string(&WorkspaceId::new("workspace:personal").unwrap()).unwrap();
        assert_eq!(value, "\"workspace:personal\"");
    }
}
