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

/// Define an opaque semantic identifier with shared wire and trait behavior.
///
/// The caller supplies its domain error and validator so error variants and
/// messages remain owned by that domain.
#[macro_export]
macro_rules! semantic_id {
    ($error:ty, $validator:path; $( $name:ident, $kind:literal, $prefix:literal );+ $(;)?) => {
        $(
            $crate::semantic_id!($name, $error, $validator, $kind, $prefix);
        )+
    };
    ($name:ident, $error:ty, $validator:path, $kind:literal, $prefix:literal) => {
        /// Validated opaque semantic identifier.
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Construct and validate an identifier.
            /// # Errors
            /// Returns the domain input error when validation fails.
            pub fn new(value: impl Into<String>) -> Result<Self, $error> {
                Ok(Self($validator(value.into(), $kind, $prefix)?))
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

        impl ::std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl ::std::str::FromStr for $name {
            type Err = $error;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::new(value)
            }
        }

        impl TryFrom<String> for $name {
            type Error = $error;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl TryFrom<&str> for $name {
            type Error = $error;

            fn try_from(value: &str) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let value = <String as serde::Deserialize>::deserialize(deserializer)?;
                Self::new(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

semantic_id!(WorkspaceInputError, validate_id;
    WorkspaceId, "workspace", "workspace:";
    ProjectId, "project", "project:";
);
