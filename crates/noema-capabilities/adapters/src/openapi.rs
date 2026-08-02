//! Bounded, provider-neutral OpenAPI 3.0/3.1 candidate importing.
//!
//! This module is a source-to-proposal boundary. It never chooses an origin,
//! credential mode, four-field behavior, retry rule,
//! quota, or account gate. A reviewed [`AdapterManifestV4`] remains the only
//! input that can be compiled and activated.

#[cfg(test)]
mod tests;

use crate::openapi_normalize::{
    diagnostic, inspect_raw, is_openapi_3_0, location, lower_parameters, lower_request_body,
    normalize_servers, normalized_operation_id, parse_source, path_arguments_match, sanitize_text,
    supported_method, validate_source_reference, within_depth,
};
use crate::{
    AdapterCompileError, AdapterCompiler, AdapterManifestV4, ArgumentDefinition,
    CompiledAdapterDefinition, HttpMethod, SemanticChange, SourceDigest,
    json_limits::validate_json_shape, openapi_schema::SchemaResolver,
};
use openapiv3::{OpenAPI, ReferenceOr};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub(crate) const MAX_OPENAPI_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_OPENAPI_DEPTH: usize = 24;
pub(crate) const MAX_OPENAPI_OPERATIONS: usize = 128;
pub(crate) const MAX_TEXT_BYTES: usize = 4_096;
pub(crate) const MAX_SOURCE_REFERENCE_BYTES: usize = 4_096;

/// The source encoding used for one immutable OpenAPI snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OpenApiSourceFormat {
    /// OpenAPI JSON source.
    Json,
    /// OpenAPI YAML source.
    Yaml,
}

/// Severity of a source diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OpenApiDiagnosticSeverity {
    /// The candidate cannot be activated until this is resolved.
    Error,
    /// The affected source feature is omitted from the proposal.
    Warning,
}

/// Bounded, non-secret source diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenApiDiagnostic {
    /// Error or warning severity.
    pub severity: OpenApiDiagnosticSeverity,
    /// Stable machine-readable category.
    pub code: String,
    /// Sanitized source location, never source prose.
    pub location: String,
}

/// Policy claims that OpenAPI cannot safely infer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenApiReviewClaim {
    /// A fixed reviewed HTTPS origin is required.
    Origin,
    /// Authentication mode and scopes are required.
    Authentication,
    /// Account kind and provider eligibility are required.
    AccountGates,
    /// Effect classification is required.
    Effects,
    /// Admission route is required.
    Admission,
    /// Retry behavior is required.
    Retry,
    /// Quota and economic metadata are required.
    Quota,
}

/// One operation proposed by the source normalizer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenApiOperationProposal {
    /// Stable normalized operation identity used by a candidate manifest.
    pub operation_id: String,
    /// Original source operation ID, when present, after bounded sanitation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_operation_id: Option<String>,
    /// Fixed HTTP method.
    pub method: HttpMethod,
    /// Fixed relative request path.
    pub path: String,
    /// Fixed non-secret JSON transport headers.
    #[serde(default)]
    pub fixed_headers: BTreeMap<String, String>,
    /// Model-supplied path/query/body arguments.
    #[serde(default)]
    pub arguments: Vec<ArgumentDefinition>,
    /// Bounded hostile-source prose retained only for human review.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_description: Option<String>,
}

/// Immutable result of parsing one exact source snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenApiCandidate {
    /// SHA-256 of the exact source bytes.
    pub source_digest: SourceDigest,
    /// Human-auditable source reference supplied by the caller.
    pub source_reference: String,
    /// Original source encoding.
    pub source_format: OpenApiSourceFormat,
    /// Bounded API title.
    pub title: String,
    /// OpenAPI document version, distinct from API version.
    pub version: String,
    /// Fixed source-declared servers for setup disclosure only.
    #[serde(default)]
    pub servers: Vec<String>,
    /// Supported operation proposals in deterministic order.
    #[serde(default)]
    pub operations: Vec<OpenApiOperationProposal>,
    /// Source and normalization diagnostics.
    #[serde(default)]
    pub diagnostics: Vec<OpenApiDiagnostic>,
    /// Policy claims that still require human/definition data.
    #[serde(default)]
    pub review_claims: Vec<OpenApiReviewClaim>,
}

impl OpenApiCandidate {
    /// Return whether any source diagnostic blocks activation.
    #[must_use]
    pub fn has_blocking_diagnostics(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == OpenApiDiagnosticSeverity::Error)
    }

    /// Build an explicit operation allowlist proposal.
    ///
    /// The returned value contains no policy authority and cannot activate a
    /// connection. A reviewed manifest must still be supplied to [`Self::activate`].
    ///
    /// # Errors
    ///
    /// Returns [`OpenApiSelectionError`] when the allowlist is empty, repeated,
    /// or names an operation absent from this candidate.
    pub fn select_operations<I, S>(
        &self,
        operation_ids: I,
    ) -> Result<OpenApiSelection, OpenApiSelectionError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let available = self
            .operations
            .iter()
            .map(|operation| operation.operation_id.as_str())
            .collect::<BTreeSet<_>>();
        let mut selected = operation_ids
            .into_iter()
            .map(Into::into)
            .collect::<Vec<_>>();
        selected.sort();
        if selected.is_empty() {
            return Err(OpenApiSelectionError::Empty);
        }
        if selected.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(OpenApiSelectionError::Duplicate);
        }
        if selected
            .iter()
            .any(|operation_id| !available.contains(operation_id.as_str()))
        {
            return Err(OpenApiSelectionError::UnknownOperation);
        }
        Ok(OpenApiSelection {
            source_digest: self.source_digest.clone(),
            operation_ids: selected,
        })
    }

    /// Activate an explicitly selected, reviewed manifest against this exact candidate.
    ///
    /// All execution and security fields come from `manifest` and are checked by
    /// [`AdapterCompiler`]. Source prose and operation method/path/argument
    /// mismatches cannot be smuggled into activation.
    ///
    /// # Errors
    ///
    /// Returns [`OpenApiActivationError`] when the candidate, selection, review
    /// state, source operation, or reviewed manifest compiler rejects activation.
    pub fn activate(
        &self,
        selection: &OpenApiSelection,
        manifest: &AdapterManifestV4,
    ) -> Result<OpenApiActivation, OpenApiActivationError> {
        if self.has_blocking_diagnostics() {
            return Err(OpenApiActivationError::CandidateBlocked);
        }
        if selection.source_digest != self.source_digest {
            return Err(OpenApiActivationError::StaleSelection);
        }
        if !manifest.reviewed {
            return Err(OpenApiActivationError::NotReviewed);
        }
        let manifest_ids = manifest
            .operations
            .iter()
            .map(|operation| operation.operation_id.as_str())
            .collect::<BTreeSet<_>>();
        if manifest_ids.len() != selection.operation_ids.len()
            || selection
                .operation_ids
                .iter()
                .any(|operation_id| !manifest_ids.contains(operation_id.as_str()))
        {
            return Err(OpenApiActivationError::OperationAllowlistMismatch);
        }
        for operation in &manifest.operations {
            let proposal = self
                .operations
                .iter()
                .find(|proposal| proposal.operation_id == operation.operation_id)
                .ok_or(OpenApiActivationError::UnknownOperation)?;
            if proposal.method != operation.method
                || proposal.path != operation.path
                || proposal.fixed_headers != operation.fixed_headers
                || proposal.arguments != operation.arguments
            {
                return Err(OpenApiActivationError::OperationSourceMismatch);
            }
        }
        let compiled = AdapterCompiler::compile(manifest)?;
        Ok(OpenApiActivation {
            source_digest: self.source_digest.clone(),
            semantic_digest: compiled.semantic_digest.clone(),
            operation_ids: selection.operation_ids.clone(),
            compiled,
        })
    }
}

/// An explicit, source-digest-bound operation allowlist proposal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenApiSelection {
    /// Candidate source digest this selection was made from.
    pub source_digest: SourceDigest,
    /// Sorted, unique normalized operation IDs.
    pub operation_ids: Vec<String>,
}

/// Immutable activation result retaining source and semantic identities.
#[derive(Debug, Clone)]
pub struct OpenApiActivation {
    /// Exact source digest selected for review.
    pub source_digest: SourceDigest,
    /// Compiled execution/security semantic digest.
    pub semantic_digest: crate::SemanticDigest,
    /// Exact reviewed operation allowlist.
    pub operation_ids: Vec<String>,
    /// Existing immutable compiler authority.
    pub compiled: CompiledAdapterDefinition,
}

impl OpenApiActivation {
    /// Compare the compiled semantic identity with a previous activation.
    #[must_use]
    pub fn semantic_change_from(&self, previous: &Self) -> SemanticChange {
        self.compiled.semantic_change_from(&previous.compiled)
    }
}

/// Source parsing and normalization failure.
#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum OpenApiImportError {
    /// Exact source bytes exceeded the bounded importer limit.
    #[error("OpenAPI source is oversized")]
    Oversized,
    /// Source encoding or reference metadata is invalid.
    #[error("OpenAPI source is invalid")]
    InvalidSource,
    /// Parsed source exceeded depth or node bounds.
    #[error("OpenAPI source shape is too deep or large")]
    InvalidShape,
    /// The bounded typed OpenAPI model could not parse the document.
    #[error("OpenAPI document is invalid")]
    InvalidDocument,
    /// The document version is outside the supported 3.0/3.1 subset.
    #[error("OpenAPI document version is unsupported")]
    UnsupportedVersion,
}

/// Explicit operation-selection failure.
#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum OpenApiSelectionError {
    /// No operation was selected.
    #[error("OpenAPI operation selection is empty")]
    Empty,
    /// An operation was selected more than once.
    #[error("OpenAPI operation selection contains a duplicate")]
    Duplicate,
    /// An operation is not present in the candidate.
    #[error("OpenAPI operation selection contains an unknown operation")]
    UnknownOperation,
}

/// Activation failure before or during the existing manifest compiler.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum OpenApiActivationError {
    /// A source diagnostic blocks all activation.
    #[error("OpenAPI candidate is blocked")]
    CandidateBlocked,
    /// The selection belongs to a different source snapshot.
    #[error("OpenAPI operation selection is stale")]
    StaleSelection,
    /// Only a reviewed manifest can activate.
    #[error("OpenAPI manifest is not reviewed")]
    NotReviewed,
    /// The reviewed manifest and explicit allowlist differ.
    #[error("OpenAPI operation allowlist does not match")]
    OperationAllowlistMismatch,
    /// A reviewed operation is absent from the candidate.
    #[error("OpenAPI operation is unknown")]
    UnknownOperation,
    /// A reviewed operation changed source method, path, headers, or arguments.
    #[error("OpenAPI operation does not match its source proposal")]
    OperationSourceMismatch,
    /// The existing manifest compiler rejected reviewed policy or schema data.
    #[error("reviewed adapter manifest is invalid: {0}")]
    Compile(#[from] AdapterCompileError),
}

/// Stateless bounded OpenAPI importer.
#[derive(Debug, Default)]
pub struct OpenApiImporter;

impl OpenApiImporter {
    /// Parse and normalize one exact JSON or YAML OpenAPI 3.0/3.1 snapshot.
    ///
    /// The source is never fetched, executed, or sent to a model.
    ///
    /// # Errors
    ///
    /// Returns [`OpenApiImportError`] for malformed, oversized, over-deep, or
    /// unsupported-version source input.
    pub fn import(
        source_reference: impl Into<String>,
        bytes: &[u8],
        format: OpenApiSourceFormat,
    ) -> Result<OpenApiCandidate, OpenApiImportError> {
        let source_reference = source_reference.into();
        validate_source_reference(&source_reference)?;
        if bytes.len() > MAX_OPENAPI_BYTES {
            return Err(OpenApiImportError::Oversized);
        }
        let raw = parse_source(bytes, format)?;
        if !validate_json_shape(&raw) || !within_depth(&raw, 0) {
            return Err(OpenApiImportError::InvalidShape);
        }
        let Some(root) = raw.as_object() else {
            return Err(OpenApiImportError::InvalidDocument);
        };
        let version = root
            .get("openapi")
            .and_then(Value::as_str)
            .ok_or(OpenApiImportError::InvalidDocument)?;
        if version.starts_with("3.1.") {
            let version = version.to_string();
            return crate::openapi31::import(source_reference, bytes, format, raw, &version);
        }
        if !is_openapi_3_0(version) {
            return Err(OpenApiImportError::UnsupportedVersion);
        }

        let mut diagnostics = Vec::new();
        inspect_raw(&raw, &mut diagnostics, "/".to_string());
        let document = serde_json::from_value::<OpenAPI>(raw)
            .map_err(|_| OpenApiImportError::InvalidDocument)?;
        let servers = normalize_servers(&document, &mut diagnostics);
        let mut operations = Vec::new();
        let mut operation_ids = BTreeSet::new();
        for (path, path_item) in document.paths.iter() {
            if operations.len() >= MAX_OPENAPI_OPERATIONS {
                diagnostic(
                    &mut diagnostics,
                    OpenApiDiagnosticSeverity::Error,
                    "operation_limit",
                    "/paths",
                );
                break;
            }
            let ReferenceOr::Item(path_item) = path_item else {
                diagnostic(
                    &mut diagnostics,
                    OpenApiDiagnosticSeverity::Warning,
                    "path_item_ref_unsupported",
                    &location(path, None),
                );
                continue;
            };
            for (method_name, operation) in path_item.iter() {
                if operations.len() >= MAX_OPENAPI_OPERATIONS {
                    diagnostic(
                        &mut diagnostics,
                        OpenApiDiagnosticSeverity::Error,
                        "operation_limit",
                        "/paths",
                    );
                    break;
                }
                let operation_location = location(path, Some(method_name));
                let Some(method) = supported_method(method_name) else {
                    diagnostic(
                        &mut diagnostics,
                        OpenApiDiagnosticSeverity::Warning,
                        "http_method_unsupported",
                        &operation_location,
                    );
                    continue;
                };
                if path_item
                    .servers
                    .iter()
                    .any(|server| server.variables.is_some())
                    || operation
                        .servers
                        .iter()
                        .any(|server| server.variables.is_some())
                    || !path_item.servers.is_empty()
                    || !operation.servers.is_empty()
                {
                    diagnostic(
                        &mut diagnostics,
                        OpenApiDiagnosticSeverity::Warning,
                        "operation_server_unsupported",
                        &operation_location,
                    );
                    continue;
                }
                if !operation.callbacks.is_empty() {
                    diagnostic(
                        &mut diagnostics,
                        OpenApiDiagnosticSeverity::Warning,
                        "callbacks_unsupported",
                        &operation_location,
                    );
                    continue;
                }
                let mut responses = operation
                    .responses
                    .responses
                    .values()
                    .chain(operation.responses.default.iter());
                if responses.any(|response| match response {
                    ReferenceOr::Reference { .. } => true,
                    ReferenceOr::Item(response) => response
                        .content
                        .keys()
                        .any(|media_type| media_type != "application/json"),
                }) {
                    diagnostic(
                        &mut diagnostics,
                        OpenApiDiagnosticSeverity::Warning,
                        "response_media_type_unsupported",
                        &operation_location,
                    );
                    continue;
                }
                let Some(operation_id) =
                    normalized_operation_id(operation.operation_id.as_deref(), method_name, path)
                else {
                    diagnostic(
                        &mut diagnostics,
                        OpenApiDiagnosticSeverity::Warning,
                        "operation_id_invalid",
                        &operation_location,
                    );
                    continue;
                };
                if !operation_ids.insert(operation_id.clone()) {
                    diagnostic(
                        &mut diagnostics,
                        OpenApiDiagnosticSeverity::Warning,
                        "duplicate_normalized_operation_id",
                        &operation_location,
                    );
                    continue;
                }
                let mut resolver = SchemaResolver::new(document.components.as_ref());
                let parameters = match lower_parameters(
                    &path_item.parameters,
                    &operation.parameters,
                    document.components.as_ref(),
                    &mut resolver,
                ) {
                    Ok(parameters) => parameters,
                    Err(failure) => {
                        diagnostic(
                            &mut diagnostics,
                            OpenApiDiagnosticSeverity::Warning,
                            failure.0,
                            &operation_location,
                        );
                        continue;
                    }
                };
                let (body_arguments, has_body) = match &operation.request_body {
                    Some(request_body) => match lower_request_body(
                        request_body,
                        document.components.as_ref(),
                        &mut resolver,
                    ) {
                        Ok(arguments) => (arguments, true),
                        Err(failure) => {
                            diagnostic(
                                &mut diagnostics,
                                OpenApiDiagnosticSeverity::Warning,
                                failure.0,
                                &operation_location,
                            );
                            continue;
                        }
                    },
                    None => (Vec::new(), false),
                };
                let mut arguments = parameters;
                arguments.extend(body_arguments);
                arguments.sort_by(|left, right| left.name.cmp(&right.name));
                if !path_arguments_match(path, &arguments) {
                    diagnostic(
                        &mut diagnostics,
                        OpenApiDiagnosticSeverity::Warning,
                        "path_parameter_mismatch",
                        &operation_location,
                    );
                    continue;
                }
                if arguments
                    .iter()
                    .map(|argument| argument.name.as_str())
                    .collect::<BTreeSet<_>>()
                    .len()
                    != arguments.len()
                {
                    diagnostic(
                        &mut diagnostics,
                        OpenApiDiagnosticSeverity::Warning,
                        "duplicate_argument_name",
                        &operation_location,
                    );
                    continue;
                }
                let mut fixed_headers =
                    BTreeMap::from([("accept".to_string(), "application/json".to_string())]);
                if has_body {
                    fixed_headers
                        .insert("content-type".to_string(), "application/json".to_string());
                }
                operations.push(OpenApiOperationProposal {
                    operation_id,
                    source_operation_id: operation.operation_id.as_deref().and_then(sanitize_text),
                    method,
                    path: path.clone(),
                    fixed_headers,
                    arguments,
                    source_description: operation
                        .summary
                        .as_deref()
                        .or(operation.description.as_deref())
                        .and_then(sanitize_text),
                });
            }
        }
        operations.sort_by(|left, right| left.operation_id.cmp(&right.operation_id));
        Ok(OpenApiCandidate {
            source_digest: SourceDigest::compute(bytes),
            source_reference,
            source_format: format,
            title: sanitize_text(&document.info.title)
                .unwrap_or_else(|| "Untitled API".to_string()),
            version: sanitize_text(&document.openapi).unwrap_or_else(|| "3.0".to_string()),
            servers,
            operations,
            diagnostics,
            review_claims: [
                OpenApiReviewClaim::Origin,
                OpenApiReviewClaim::Authentication,
                OpenApiReviewClaim::AccountGates,
                OpenApiReviewClaim::Effects,
                OpenApiReviewClaim::Admission,
                OpenApiReviewClaim::Retry,
                OpenApiReviewClaim::Quota,
            ]
            .into_iter()
            .collect(),
        })
    }

    /// Parse JSON source with duplicate-key rejection.
    ///
    /// # Errors
    ///
    /// Returns [`OpenApiImportError`] when the source is not a bounded JSON
    /// OpenAPI 3.0/3.1 document.
    pub fn import_json(
        source_reference: impl Into<String>,
        bytes: &[u8],
    ) -> Result<OpenApiCandidate, OpenApiImportError> {
        Self::import(source_reference, bytes, OpenApiSourceFormat::Json)
    }

    /// Parse YAML source with duplicate-key rejection.
    ///
    /// # Errors
    ///
    /// Returns [`OpenApiImportError`] when the source is not a bounded YAML
    /// OpenAPI 3.0/3.1 document.
    pub fn import_yaml(
        source_reference: impl Into<String>,
        bytes: &[u8],
    ) -> Result<OpenApiCandidate, OpenApiImportError> {
        Self::import(source_reference, bytes, OpenApiSourceFormat::Yaml)
    }
}
