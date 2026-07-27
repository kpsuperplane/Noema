//! Bounded restricted-data projections and eligibility gates.

#[cfg(test)]
mod tests;

use crate::{ModelPayload, ModelRoute, PersistenceMode, ResultClassification, ResultDefinition};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;
use thiserror::Error;

const MAX_POINTERS: usize = 64;
const MAX_POINTER_BYTES: usize = 512;
const MAX_DEPTH: u8 = 32;
const MAX_NODES: u32 = 65_536;
const MAX_STRING_BYTES: usize = 64 * 1024;
const MAX_SCOPES: usize = 128;

/// Explicit field-pointer policy for restricted result views.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestrictedDataPolicy {
    /// JSON pointers omitted from the model view.
    #[serde(default)]
    pub model_omit_pointers: Vec<String>,
    /// JSON pointers omitted from the durable local view.
    #[serde(default)]
    pub persistence_omit_pointers: Vec<String>,
    /// Maximum recursive value depth.
    pub max_depth: u8,
    /// Maximum recursive value nodes.
    pub max_nodes: u32,
    /// Maximum one-string byte length.
    pub max_string_bytes: usize,
}

/// Result projection produced without retaining raw restricted bytes in metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectedResult {
    /// Route-authorized model view, if continuation is allowed.
    pub model: Option<Value>,
    /// Durable local view, if the result policy permits persistence.
    pub persisted: Option<Value>,
    /// Number of values omitted by explicit pointer policy.
    pub omitted_paths: u32,
    /// Number of values traversed.
    pub node_count: u32,
}

/// Restricted-data eligibility state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestrictedDataEligibility {
    /// Exact reviewed account surface.
    pub account_kind: String,
    /// Exact scopes required by the reviewed operation.
    #[serde(default)]
    pub required_scopes: Vec<String>,
    /// Synthetic or live grant subset supplied by connection authority.
    #[serde(default)]
    pub granted_scopes: Vec<String>,
    /// Whether provider testing/verification must be complete.
    #[serde(default)]
    pub testing_required: bool,
    /// Whether the reviewed testing state is complete.
    #[serde(default)]
    pub testing_complete: bool,
    /// Whether a security or access assessment must be complete.
    #[serde(default)]
    pub assessment_required: bool,
    /// Whether the reviewed assessment is complete.
    #[serde(default)]
    pub assessment_complete: bool,
    /// Whether this connection is on the reviewed allowlist.
    #[serde(default = "default_true")]
    pub allowlisted: bool,
}

/// A safe eligibility blocker.
#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum ProjectionError {
    /// The account surface differs from reviewed data.
    #[error("restricted data account kind does not match")]
    AccountKindMismatch,
    /// A required scope is not granted.
    #[error("restricted data scope is not granted")]
    ScopeMissing,
    /// Provider testing or verification is incomplete.
    #[error("restricted data testing is incomplete")]
    TestingIncomplete,
    /// Provider security/access assessment is incomplete.
    #[error("restricted data assessment is incomplete")]
    AssessmentIncomplete,
    /// The connection is not on the reviewed allowlist.
    #[error("restricted data connection is not allowlisted")]
    NotAllowlisted,
    /// The result cannot be sent to this model route.
    #[error("restricted result route is not permitted")]
    RouteDenied,
    /// The explicit projection policy is invalid.
    #[error("restricted projection policy is invalid")]
    PolicyInvalid,
    /// The payload exceeds recursive bounds.
    #[error("restricted result payload is oversized")]
    Oversized,
}

impl RestrictedDataPolicy {
    /// Validate pointer, recursion, and size bounds before a result is projected.
    ///
    /// # Errors
    ///
    /// Returns ProjectionError::PolicyInvalid for invalid or ambiguous policy.
    pub fn validate(&self) -> Result<(), ProjectionError> {
        if self.max_depth == 0
            || self.max_depth > MAX_DEPTH
            || self.max_nodes == 0
            || self.max_nodes > MAX_NODES
            || self.max_string_bytes == 0
            || self.max_string_bytes > MAX_STRING_BYTES
            || self.model_omit_pointers.len() > MAX_POINTERS
            || self.persistence_omit_pointers.len() > MAX_POINTERS
        {
            return Err(ProjectionError::PolicyInvalid);
        }
        let mut pointers = BTreeSet::new();
        for pointer in self
            .model_omit_pointers
            .iter()
            .chain(&self.persistence_omit_pointers)
        {
            if pointer.len() > MAX_POINTER_BYTES
                || !pointer.starts_with('/')
                || !pointers.insert(pointer)
                || pointer.bytes().any(|byte| byte.is_ascii_control())
            {
                return Err(ProjectionError::PolicyInvalid);
            }
            parse_pointer(pointer)?;
        }
        Ok(())
    }
}

impl RestrictedDataEligibility {
    /// Check an exact account, grant, review, and allowlist state.
    ///
    /// # Errors
    ///
    /// Returns the first typed blocker; no partial restricted result is
    /// produced while a gate is unresolved.
    pub fn check(&self, actual_account_kind: &str) -> Result<(), ProjectionError> {
        if self.account_kind != actual_account_kind {
            return Err(ProjectionError::AccountKindMismatch);
        }
        if self.required_scopes.len() > MAX_SCOPES || self.granted_scopes.len() > MAX_SCOPES {
            return Err(ProjectionError::PolicyInvalid);
        }
        let granted = self.granted_scopes.iter().collect::<BTreeSet<_>>();
        if self
            .required_scopes
            .iter()
            .any(|scope| !granted.contains(scope))
        {
            return Err(ProjectionError::ScopeMissing);
        }
        if self.testing_required && !self.testing_complete {
            return Err(ProjectionError::TestingIncomplete);
        }
        if self.assessment_required && !self.assessment_complete {
            return Err(ProjectionError::AssessmentIncomplete);
        }
        if !self.allowlisted {
            return Err(ProjectionError::NotAllowlisted);
        }
        Ok(())
    }
}

/// Project one result according to the reviewed route and explicit pointers.
///
/// # Errors
///
/// Returns ProjectionError before any route or persistence consumer receives
/// bytes when the policy, route, or recursive payload is not safe.
pub fn project_result(
    value: &Value,
    result: &ResultDefinition,
    policy: &RestrictedDataPolicy,
    local_model_route: bool,
) -> Result<ProjectedResult, ProjectionError> {
    policy.validate()?;
    if result.classification == ResultClassification::Private
        && result.model_route == ModelRoute::LocalOnly
        && !local_model_route
    {
        return Err(ProjectionError::RouteDenied);
    }
    let mut model_counter = Counter::default();
    let mut persisted_counter = Counter::default();
    let model = match result.model_payload {
        ModelPayload::Omit => None,
        ModelPayload::MetadataOnly => Some(metadata(value, policy, &mut model_counter)?),
        ModelPayload::Full => Some(project_value(
            value,
            &[],
            &parse_pointers(&policy.model_omit_pointers)?,
            policy,
            &mut model_counter,
        )?),
    };
    let persisted = match result.persistence {
        PersistenceMode::Omit => None,
        PersistenceMode::MetadataOnly => Some(metadata(value, policy, &mut persisted_counter)?),
        PersistenceMode::Redacted => Some(project_value(
            value,
            &[],
            &parse_pointers(&policy.persistence_omit_pointers)?,
            policy,
            &mut persisted_counter,
        )?),
    };
    Ok(ProjectedResult {
        model,
        persisted,
        omitted_paths: model_counter.omitted + persisted_counter.omitted,
        node_count: model_counter.nodes.max(persisted_counter.nodes),
    })
}

#[derive(Default)]
struct Counter {
    nodes: u32,
    omitted: u32,
}

fn metadata(
    value: &Value,
    policy: &RestrictedDataPolicy,
    counter: &mut Counter,
) -> Result<Value, ProjectionError> {
    count_value(value, 0, policy, counter)?;
    Ok(json!({
        "kind": "restricted_result_metadata",
        "node_count": counter.nodes,
        "json_bytes": serde_json::to_vec(value).map_err(|_| ProjectionError::Oversized)?.len(),
    }))
}

fn project_value(
    value: &Value,
    path: &[String],
    omitted: &BTreeSet<Vec<String>>,
    policy: &RestrictedDataPolicy,
    counter: &mut Counter,
) -> Result<Value, ProjectionError> {
    if path.len() > policy.max_depth as usize {
        return Err(ProjectionError::Oversized);
    }
    count_node(value, path, policy, counter)?;
    if omitted.contains(path) {
        counter.omitted = counter.omitted.saturating_add(1);
        return Ok(Value::Null);
    }
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) => Ok(value.clone()),
        Value::String(text) => {
            if text.len() > policy.max_string_bytes {
                return Err(ProjectionError::Oversized);
            }
            Ok(Value::String(text.clone()))
        }
        Value::Array(values) => {
            let mut output = Vec::with_capacity(values.len());
            for (index, child) in values.iter().enumerate() {
                let mut child_path = path.to_vec();
                child_path.push(index.to_string());
                if omitted.contains(&child_path) {
                    counter.omitted = counter.omitted.saturating_add(1);
                    continue;
                }
                output.push(project_value(child, &child_path, omitted, policy, counter)?);
            }
            Ok(Value::Array(output))
        }
        Value::Object(values) => {
            let mut output = Map::new();
            for (key, child) in values {
                if key.len() > policy.max_string_bytes
                    || key.bytes().any(|byte| byte.is_ascii_control())
                {
                    return Err(ProjectionError::Oversized);
                }
                let mut child_path = path.to_vec();
                child_path.push(key.clone());
                if omitted.contains(&child_path) {
                    counter.omitted = counter.omitted.saturating_add(1);
                    continue;
                }
                output.insert(
                    key.clone(),
                    project_value(child, &child_path, omitted, policy, counter)?,
                );
            }
            Ok(Value::Object(output))
        }
    }
}

fn count_value(
    value: &Value,
    depth: u8,
    policy: &RestrictedDataPolicy,
    counter: &mut Counter,
) -> Result<(), ProjectionError> {
    count_node(value, &[], policy, counter)?;
    if let Value::String(text) = value
        && text.len() > policy.max_string_bytes
    {
        return Err(ProjectionError::Oversized);
    }
    match value {
        Value::Array(values) => {
            if depth >= policy.max_depth {
                return Err(ProjectionError::Oversized);
            }
            for child in values {
                count_value_at_depth(child, depth + 1, policy, counter)?;
            }
        }
        Value::Object(values) => {
            if depth >= policy.max_depth {
                return Err(ProjectionError::Oversized);
            }
            for (key, child) in values {
                if key.len() > policy.max_string_bytes
                    || key.bytes().any(|byte| byte.is_ascii_control())
                {
                    return Err(ProjectionError::Oversized);
                }
                count_value_at_depth(child, depth + 1, policy, counter)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn count_value_at_depth(
    value: &Value,
    depth: u8,
    policy: &RestrictedDataPolicy,
    counter: &mut Counter,
) -> Result<(), ProjectionError> {
    count_node(value, &[], policy, counter)?;
    if let Value::String(text) = value
        && text.len() > policy.max_string_bytes
    {
        return Err(ProjectionError::Oversized);
    }
    match value {
        Value::Array(values) => {
            if depth >= policy.max_depth {
                return Err(ProjectionError::Oversized);
            }
            for child in values {
                count_value_at_depth(child, depth + 1, policy, counter)?;
            }
        }
        Value::Object(values) => {
            if depth >= policy.max_depth {
                return Err(ProjectionError::Oversized);
            }
            for (key, child) in values {
                if key.len() > policy.max_string_bytes
                    || key.bytes().any(|byte| byte.is_ascii_control())
                {
                    return Err(ProjectionError::Oversized);
                }
                count_value_at_depth(child, depth + 1, policy, counter)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn count_node(
    value: &Value,
    _path: &[String],
    policy: &RestrictedDataPolicy,
    counter: &mut Counter,
) -> Result<(), ProjectionError> {
    let _ = value;
    counter.nodes = counter
        .nodes
        .checked_add(1)
        .ok_or(ProjectionError::Oversized)?;
    if counter.nodes > policy.max_nodes {
        return Err(ProjectionError::Oversized);
    }
    Ok(())
}

fn parse_pointers(values: &[String]) -> Result<BTreeSet<Vec<String>>, ProjectionError> {
    values.iter().map(|value| parse_pointer(value)).collect()
}

fn parse_pointer(value: &str) -> Result<Vec<String>, ProjectionError> {
    if !value.starts_with('/') {
        return Err(ProjectionError::PolicyInvalid);
    }
    value
        .split('/')
        .skip(1)
        .map(|segment| {
            let mut decoded = String::new();
            let mut chars = segment.chars();
            while let Some(character) = chars.next() {
                if character == '~' {
                    match chars.next() {
                        Some('0') => decoded.push('~'),
                        Some('1') => decoded.push('/'),
                        _ => return Err(ProjectionError::PolicyInvalid),
                    }
                } else {
                    decoded.push(character);
                }
            }
            Ok(decoded)
        })
        .collect()
}

const fn default_true() -> bool {
    true
}
