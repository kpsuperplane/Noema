//! A2UI v0.9.1 validation and deterministic surface reduction.

#![allow(missing_docs)]

mod authority;
mod component_validation;
mod reducer;
mod validation;

use std::collections::{BTreeMap, BTreeSet};

pub use authority::{
    A2UI_AUTHORITY_COMMIT, A2UI_AUTHORITY_SOURCE, A2UI_PROTOCOL_VERSION, A2UICatalogDescriptor,
    NOEMA_A2UI_CATALOG_ID, NOEMA_A2UI_COMPONENTS, advertised_catalog,
};
pub use reducer::{A2UIAction, A2UISurface};
pub use validation::{
    A2UIValidationCode, A2UIValidationError, A2UIValidationLimits, A2UIValidator, parse_and_reduce,
};

pub const A2UI_VALIDATION_FAILED: &str = "VALIDATION_FAILED";

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct A2UIValidatedBatch {
    pub messages: Vec<serde_json::Value>,
    pub surfaces: BTreeMap<String, A2UISurface>,
    pub deleted_surface_ids: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct A2UIRepairResult {
    pub status: String,
    pub code: A2UIValidationCode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub message: String,
}

impl A2UIRepairResult {
    pub(crate) fn from_error(error: &A2UIValidationError) -> Self {
        Self {
            status: A2UI_VALIDATION_FAILED.to_string(),
            code: error.code,
            line: error.line,
            path: error.path.clone(),
            message: error.message.clone(),
        }
    }
}
