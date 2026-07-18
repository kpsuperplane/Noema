use std::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize};

use crate::WorkDomainError;

const MAX_ID_BYTES: usize = 255;

fn validate_id(
    value: String,
    kind: &'static str,
    prefix: &'static str,
) -> Result<String, WorkDomainError> {
    if value.trim().is_empty() {
        return Err(WorkDomainError::InvalidInput {
            field: kind,
            message: "identifier cannot be blank".to_string(),
        });
    }
    if value.len() > MAX_ID_BYTES {
        return Err(WorkDomainError::InvalidInput {
            field: kind,
            message: "identifier exceeds 255 UTF-8 bytes".to_string(),
        });
    }
    if value.chars().any(char::is_control) {
        return Err(WorkDomainError::InvalidInput {
            field: kind,
            message: "identifier cannot contain control characters".to_string(),
        });
    }
    if !value.starts_with(prefix) {
        return Err(WorkDomainError::InvalidInput {
            field: kind,
            message: format!("identifier must start with {prefix}"),
        });
    }
    if value[prefix.len()..].trim().is_empty() {
        return Err(WorkDomainError::InvalidInput {
            field: kind,
            message: "identifier must contain a value after its prefix".to_string(),
        });
    }
    Ok(value)
}

macro_rules! semantic_id {
    ($name:ident, $kind:literal, $prefix:literal) => {
        /// Validated opaque identifier owned by the Work domain.
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Construct and validate an identifier.
            pub fn new(value: impl Into<String>) -> Result<Self, WorkDomainError> {
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
            type Err = WorkDomainError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::new(value)
            }
        }

        impl TryFrom<String> for $name {
            type Error = WorkDomainError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl TryFrom<&str> for $name {
            type Error = WorkDomainError;

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

semantic_id!(WorkflowId, "workflow", "workflow:");
semantic_id!(WorkflowStageId, "workflow_stage", "stage:");
semantic_id!(TaskId, "task", "task:");
semantic_id!(TaskContractId, "task_contract", "contract:");
semantic_id!(TaskGateId, "task_gate", "gate:");
semantic_id!(TaskMessageId, "task_message", "task_message:");
semantic_id!(WorkEventId, "work_event", "event:");

#[cfg(test)]
mod tests {
    use super::{TaskId, WorkflowId};

    #[test]
    fn task_ids_reject_wrong_prefix_controls_and_oversize_values() {
        assert!(TaskId::new("workflow:one").is_err());
        assert!(TaskId::new(" ").is_err());
        assert!(TaskId::new("task:bad\nvalue").is_err());
        assert!(TaskId::new(format!("task:{}", "x".repeat(251))).is_err());
        assert_eq!(TaskId::new("task:one").unwrap().as_str(), "task:one");
    }

    #[test]
    fn ids_serialize_as_opaque_wire_strings_and_fail_closed() {
        let id = WorkflowId::new("workflow:personal:default").unwrap();
        assert_eq!(
            serde_json::to_string(&id).unwrap(),
            "\"workflow:personal:default\""
        );
        assert!(serde_json::from_str::<WorkflowId>("\"task:one\"").is_err());
    }
}
