//! Source-neutral integration management policy and tool classification.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Invalid human-visible label for a configured connection.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum CapabilityConnectionLabelError {
    /// The normalized label exceeds its storage and model-context bound.
    #[error("connection label is too long")]
    TooLong,
    /// The label contains control characters.
    #[error("connection label contains control characters")]
    Invalid,
}

/// Normalize one source-neutral, non-authoritative connection label.
///
/// Outer whitespace is removed, empty input becomes `None`, and retained
/// values are bounded to 256 UTF-8 bytes without control characters.
///
/// # Errors
/// Returns a bounded error when a retained label is too long or contains controls.
pub fn normalize_capability_connection_label(
    value: Option<String>,
) -> Result<Option<String>, CapabilityConnectionLabelError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.len() > 256 {
        return Err(CapabilityConnectionLabelError::TooLong);
    }
    if value.chars().any(char::is_control) {
        return Err(CapabilityConnectionLabelError::Invalid);
    }
    Ok(Some(value.to_owned()))
}

use crate::{CapabilityExecutionDecision, CapabilityToolBehavior};

/// Error returned when persisted integration policy contains an unknown value.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("invalid {kind}: {value}")]
pub struct CapabilityPolicyValueError {
    kind: &'static str,
    value: String,
}

impl CapabilityPolicyValueError {
    fn new(kind: &'static str, value: impl Into<String>) -> Self {
        Self {
            kind,
            value: value.into(),
        }
    }
}

macro_rules! persisted_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident {
            $($(#[$variant_meta:meta])* $variant:ident => $wire:literal),+ $(,)?
        }
        kind = $kind:literal
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub enum $name {
            $(
                $(#[$variant_meta])*
                #[serde(rename = $wire)]
                $variant,
            )+
        }

        impl $name {
            /// Return the stable persisted snake_case representation.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $wire,)+
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = CapabilityPolicyValueError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                match value {
                    $($wire => Ok(Self::$variant),)+
                    _ => Err(CapabilityPolicyValueError::new($kind, value)),
                }
            }
        }
    };
}

persisted_enum! {
    /// Source kind for one managed capability integration.
    pub enum CapabilityIntegrationKind {
        /// Reviewed API definition and one of its authenticated connections.
        Api => "api",
        /// MCP transport definition and one of its concrete connections.
        Mcp => "mcp",
    }
    kind = "capability_integration_kind"
}

persisted_enum! {
    /// Whether ordinary calls may share context with an integration directly.
    pub enum CapabilityDataSharingPolicy {
        /// Otherwise-safe calls may execute without approval.
        AllowAutomatically => "allow_automatically",
        /// Every call is routed through the unsafe-action policy.
        ReviewEveryCall => "review_every_call",
    }
    kind = "capability_data_sharing_policy"
}

persisted_enum! {
    /// How unsafe calls are reviewed.
    pub enum CapabilityUnsafeActionPolicy {
        /// Require human review.
        AlwaysAsk => "always_ask",
        /// Let the configured reviewer approve or escalate the call.
        ReviewerMayApprove => "reviewer_may_approve",
        /// Execute without review.
        NeverAsk => "never_ask",
    }
    kind = "capability_unsafe_action_policy"
}

persisted_enum! {
    /// Durable readiness state for one effective tool policy.
    pub enum CapabilityToolPolicyStatus {
        /// One or more behavior hints still require classification.
        Pending => "pending",
        /// All behavior hints are available from source metadata, inference, or a human.
        Ready => "ready",
        /// Missing hints were filled with pessimistic defaults.
        Defaulted => "defaulted",
        /// The user intentionally disabled this tool.
        Disabled => "disabled",
    }
    kind = "capability_tool_policy_status"
}

persisted_enum! {
    /// Authority that supplied one effective tool hint.
    pub enum CapabilityToolHintSource {
        /// Supplied by source-owned tool annotations.
        Annotation => "annotation",
        /// Inferred by the classification model.
        Model => "model",
        /// Filled with pessimistic defaults after classification failed.
        SafeDefault => "safe_default",
        /// Supplied as a complete human override.
        Human => "human",
    }
    kind = "capability_tool_hint_source"
}

/// Structured identity for one integration definition.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityDefinitionKey {
    /// Source kind that owns the definition.
    pub kind: CapabilityIntegrationKind,
    /// Stable identifier interpreted only by that source.
    pub definition_id: String,
}

/// Structured identity for one concrete integration connection.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityConnectionKey {
    /// Source kind that owns the connection.
    pub kind: CapabilityIntegrationKind,
    /// Stable identifier interpreted only by that source.
    pub connection_id: String,
}

/// Structured identity for one connection-owned tool.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityToolKey {
    /// Source kind that owns the tool.
    pub kind: CapabilityIntegrationKind,
    /// Owning source connection.
    pub connection_id: String,
    /// Stable tool identifier interpreted only by that source.
    pub tool_id: String,
}

/// Complete trust policy for one concrete connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityConnectionPolicy {
    /// Whether otherwise-safe calls may share context automatically.
    pub data_sharing: CapabilityDataSharingPolicy,
    /// How unsafe calls are reviewed.
    pub unsafe_actions: CapabilityUnsafeActionPolicy,
    /// Monotonic revision fencing catalog and invocation authority.
    pub revision: u64,
}

/// One effective behavior hint and its durable provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityToolHint {
    /// Effective value, or `None` while classification is pending.
    pub value: Option<bool>,
    /// Authority that supplied the effective value.
    pub source: Option<CapabilityToolHintSource>,
}

/// Source-neutral effective policy for one exact tool revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityToolPolicy {
    /// Stable source-owned tool identifier.
    pub tool_id: String,
    /// Effective read-only hint.
    pub read_only: CapabilityToolHint,
    /// Effective idempotency hint.
    pub idempotent: CapabilityToolHint,
    /// Effective destructive hint.
    pub destructive: CapabilityToolHint,
    /// Effective open-world hint.
    pub open_world: CapabilityToolHint,
    /// Durable classification and enabled state.
    pub status: CapabilityToolPolicyStatus,
    /// Monotonic revision fencing classification and execution authority.
    pub policy_revision: u64,
    /// Exact source metadata revision covered by this policy.
    pub source_revision: String,
}

impl CapabilityToolPolicy {
    /// Return complete behavior when this policy may be advertised.
    #[must_use]
    pub const fn behavior(&self) -> Option<CapabilityToolBehavior> {
        if !matches!(
            self.status,
            CapabilityToolPolicyStatus::Ready | CapabilityToolPolicyStatus::Defaulted
        ) {
            return None;
        }
        let (Some(read_only), Some(idempotent), Some(destructive), Some(open_world)) = (
            self.read_only.value,
            self.idempotent.value,
            self.destructive.value,
            self.open_world.value,
        ) else {
            return None;
        };
        Some(CapabilityToolBehavior {
            read_only,
            idempotent,
            destructive,
            open_world,
        })
    }

    /// Whether all behavior values are known and this tool may be advertised.
    #[must_use]
    pub const fn is_callable(&self) -> bool {
        self.behavior().is_some()
    }

    /// Whether the effective tool behavior is potentially risky.
    #[must_use]
    pub const fn is_risky(&self) -> bool {
        match self.behavior() {
            Some(behavior) => behavior.is_risky(),
            None => true,
        }
    }
}

/// Complete human override for one exact tool revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityToolPolicyOverride {
    /// Stable source-owned tool identifier.
    pub tool_id: String,
    /// Human-selected read-only value.
    pub read_only: bool,
    /// Human-selected idempotency value.
    pub idempotent: bool,
    /// Human-selected destructive value.
    pub destructive: bool,
    /// Human-selected open-world value.
    pub open_world: bool,
    /// Exact source metadata revision being overridden.
    pub source_revision: String,
}

/// Strict model output for only the missing behavior hints.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilityToolHintCompletion {
    /// Inferred read-only value when that hint was missing.
    pub read_only: Option<bool>,
    /// Inferred idempotency value when that hint was missing.
    pub idempotent: Option<bool>,
    /// Inferred destructive value when that hint was missing.
    pub destructive: Option<bool>,
    /// Inferred open-world value when that hint was missing.
    pub open_world: Option<bool>,
}

/// Validation error for one metadata-only classification response.
#[derive(Debug, Error)]
pub enum CapabilityToolClassificationError {
    /// The response was not strict JSON.
    #[error("invalid tool-hint JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// The response did not contain exactly the missing fields.
    #[error("tool-hint response did not match the missing fields")]
    FieldMismatch,
}

/// Resolve the original three-way trust decision for one complete policy.
#[must_use]
pub const fn resolve_capability_execution_decision(
    connection: CapabilityConnectionPolicy,
    behavior: CapabilityToolBehavior,
) -> CapabilityExecutionDecision {
    let unsafe_call = behavior.is_risky()
        || matches!(
            connection.data_sharing,
            CapabilityDataSharingPolicy::ReviewEveryCall
        );
    if !unsafe_call {
        return CapabilityExecutionDecision::ExecuteImmediately;
    }
    match connection.unsafe_actions {
        CapabilityUnsafeActionPolicy::AlwaysAsk => CapabilityExecutionDecision::HumanReview,
        CapabilityUnsafeActionPolicy::ReviewerMayApprove => CapabilityExecutionDecision::LlmReview,
        CapabilityUnsafeActionPolicy::NeverAsk => CapabilityExecutionDecision::ExecuteImmediately,
    }
}

/// Validate one connection policy pair.
///
/// # Errors
///
/// Returns an error when review-every-call is combined with never-ask.
pub fn validate_capability_connection_policy(
    data_sharing: CapabilityDataSharingPolicy,
    unsafe_actions: CapabilityUnsafeActionPolicy,
) -> Result<(), CapabilityPolicyValueError> {
    if data_sharing == CapabilityDataSharingPolicy::ReviewEveryCall
        && unsafe_actions == CapabilityUnsafeActionPolicy::NeverAsk
    {
        return Err(CapabilityPolicyValueError::new(
            "capability_connection_policy",
            "review_every_call+never_ask",
        ));
    }
    Ok(())
}

/// Build a bounded metadata-only classification prompt for one incomplete tool.
#[must_use]
pub fn build_tool_classification_prompt(
    name: &str,
    description: Option<&str>,
    input_schema: &Value,
    output_schema: Option<&Value>,
    policy: &CapabilityToolPolicy,
) -> String {
    let missing = missing_fields(policy).join(",");
    let description = description.unwrap_or("-");
    let input = schema_fields(input_schema);
    let output = output_schema.map_or_else(|| "-".to_string(), schema_fields);
    format!(
        "Classify only these missing tool behavior hints: {missing}.\nReturn one strict JSON object using only the corresponding camelCase keys and boolean values.\nreadOnly=no environment mutation; idempotent=repeating identical arguments adds no effect; destructive=may overwrite/delete; openWorld=may interact with external entities.\ntool={}\ndescription={}\ninput_fields={}\noutput_fields={}",
        sanitize(name, 128),
        sanitize(description, 256),
        input,
        output,
    )
}

/// Parse a strict response that fills exactly the currently missing hints.
///
/// # Errors
///
/// Returns an error when the response is not strict JSON or does not contain
/// exactly the fields missing from the captured policy.
pub fn parse_tool_classification_response(
    text: &str,
    policy: &CapabilityToolPolicy,
) -> Result<CapabilityToolHintCompletion, CapabilityToolClassificationError> {
    let completion: CapabilityToolHintCompletion = serde_json::from_str(text.trim())?;
    let expected = [
        policy.read_only.value.is_none(),
        policy.idempotent.value.is_none(),
        policy.destructive.value.is_none(),
        policy.open_world.value.is_none(),
    ];
    let actual = [
        completion.read_only.is_some(),
        completion.idempotent.is_some(),
        completion.destructive.is_some(),
        completion.open_world.is_some(),
    ];
    if expected != actual {
        return Err(CapabilityToolClassificationError::FieldMismatch);
    }
    Ok(completion)
}

/// Merge a validated completion without replacing source-owned fields.
#[must_use]
pub fn apply_tool_classification(
    mut policy: CapabilityToolPolicy,
    completion: CapabilityToolHintCompletion,
) -> CapabilityToolPolicy {
    fill(
        &mut policy.read_only,
        completion.read_only,
        false,
        CapabilityToolHintSource::Model,
    );
    fill(
        &mut policy.idempotent,
        completion.idempotent,
        false,
        CapabilityToolHintSource::Model,
    );
    fill(
        &mut policy.destructive,
        completion.destructive,
        true,
        CapabilityToolHintSource::Model,
    );
    fill(
        &mut policy.open_world,
        completion.open_world,
        true,
        CapabilityToolHintSource::Model,
    );
    policy.status = CapabilityToolPolicyStatus::Ready;
    policy
}

/// Fill missing hints with pessimistic defaults after model failure.
#[must_use]
pub fn apply_tool_safe_defaults(mut policy: CapabilityToolPolicy) -> CapabilityToolPolicy {
    fill(
        &mut policy.read_only,
        None,
        false,
        CapabilityToolHintSource::SafeDefault,
    );
    fill(
        &mut policy.idempotent,
        None,
        false,
        CapabilityToolHintSource::SafeDefault,
    );
    fill(
        &mut policy.destructive,
        None,
        true,
        CapabilityToolHintSource::SafeDefault,
    );
    fill(
        &mut policy.open_world,
        None,
        true,
        CapabilityToolHintSource::SafeDefault,
    );
    policy.status = CapabilityToolPolicyStatus::Defaulted;
    policy
}

fn fill(
    hint: &mut CapabilityToolHint,
    inferred: Option<bool>,
    default: bool,
    source: CapabilityToolHintSource,
) {
    if hint.value.is_none() {
        hint.value = Some(inferred.unwrap_or(default));
        hint.source = Some(source);
    }
}

fn missing_fields(policy: &CapabilityToolPolicy) -> Vec<&'static str> {
    [
        ("readOnly", policy.read_only.value),
        ("idempotent", policy.idempotent.value),
        ("destructive", policy.destructive.value),
        ("openWorld", policy.open_world.value),
    ]
    .into_iter()
    .filter_map(|(name, value)| value.is_none().then_some(name))
    .collect()
}

fn schema_fields(schema: &Value) -> String {
    schema
        .get("properties")
        .and_then(Value::as_object)
        .map(|properties| {
            properties
                .keys()
                .map(|key| sanitize(key, 64))
                .collect::<Vec<_>>()
                .join(",")
        })
        .filter(|fields| !fields.is_empty())
        .unwrap_or_else(|| "-".to_string())
}

fn sanitize(value: &str, max_chars: usize) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .take(max_chars)
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn policy() -> CapabilityToolPolicy {
        CapabilityToolPolicy {
            tool_id: "tool".into(),
            read_only: CapabilityToolHint {
                value: Some(true),
                source: Some(CapabilityToolHintSource::Annotation),
            },
            idempotent: CapabilityToolHint {
                value: None,
                source: None,
            },
            destructive: CapabilityToolHint {
                value: Some(false),
                source: Some(CapabilityToolHintSource::Annotation),
            },
            open_world: CapabilityToolHint {
                value: None,
                source: None,
            },
            status: CapabilityToolPolicyStatus::Pending,
            policy_revision: 1,
            source_revision: "revision".into(),
        }
    }

    #[test]
    fn resolver_preserves_original_safe_risky_and_review_matrix() {
        let safe = CapabilityToolBehavior {
            read_only: true,
            idempotent: false,
            destructive: false,
            open_world: true,
        };
        let risky = CapabilityToolBehavior {
            read_only: false,
            idempotent: false,
            destructive: true,
            open_world: false,
        };
        for unsafe_actions in [
            CapabilityUnsafeActionPolicy::AlwaysAsk,
            CapabilityUnsafeActionPolicy::ReviewerMayApprove,
            CapabilityUnsafeActionPolicy::NeverAsk,
        ] {
            let connection = CapabilityConnectionPolicy {
                data_sharing: CapabilityDataSharingPolicy::AllowAutomatically,
                unsafe_actions,
                revision: 1,
            };
            assert_eq!(
                resolve_capability_execution_decision(connection, safe),
                CapabilityExecutionDecision::ExecuteImmediately
            );
            let expected = match unsafe_actions {
                CapabilityUnsafeActionPolicy::AlwaysAsk => CapabilityExecutionDecision::HumanReview,
                CapabilityUnsafeActionPolicy::ReviewerMayApprove => {
                    CapabilityExecutionDecision::LlmReview
                }
                CapabilityUnsafeActionPolicy::NeverAsk => {
                    CapabilityExecutionDecision::ExecuteImmediately
                }
            };
            assert_eq!(
                resolve_capability_execution_decision(connection, risky),
                expected
            );
        }
    }

    #[test]
    fn classification_fills_only_missing_hints_and_defaults_fail_closed() {
        let policy = policy();
        let completion = parse_tool_classification_response(
            &json!({"idempotent": true, "openWorld": false}).to_string(),
            &policy,
        )
        .expect("completion");
        let merged = apply_tool_classification(policy, completion);
        assert_eq!(
            merged.read_only.source,
            Some(CapabilityToolHintSource::Annotation)
        );
        assert_eq!(
            merged.idempotent.source,
            Some(CapabilityToolHintSource::Model)
        );
        assert!(merged.is_callable());

        let defaulted = apply_tool_safe_defaults(CapabilityToolPolicy {
            idempotent: CapabilityToolHint {
                value: None,
                source: None,
            },
            open_world: CapabilityToolHint {
                value: None,
                source: None,
            },
            status: CapabilityToolPolicyStatus::Pending,
            ..merged
        });
        assert_eq!(defaulted.status, CapabilityToolPolicyStatus::Defaulted);
        assert_eq!(defaulted.idempotent.value, Some(false));
        assert_eq!(defaulted.open_world.value, Some(true));
    }
}
