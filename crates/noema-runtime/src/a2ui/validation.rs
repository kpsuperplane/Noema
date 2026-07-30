use std::collections::{BTreeSet, HashSet};

use serde_json::{Map, Value};

use super::{A2UIRepairResult, A2UIValidatedBatch};
use super::{
    authority::*,
    component_validation::validate_component,
    reducer::{A2UIState, A2UISurface},
};

#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct A2UIValidationLimits {
    pub max_input_bytes: usize,
    pub max_messages: usize,
    pub max_surfaces: usize,
    pub max_components: usize,
    pub max_depth: usize,
    pub max_string_bytes: usize,
    pub max_data_model_bytes: usize,
    pub max_collection_items: usize,
}

impl Default for A2UIValidationLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: 256 * 1024,
            max_messages: 128,
            max_surfaces: 8,
            max_components: 256,
            max_depth: 32,
            max_string_bytes: 8 * 1024,
            max_data_model_bytes: 64 * 1024,
            max_collection_items: 256,
        }
    }
}

#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum A2UIValidationCode {
    InvalidJsonl,
    UnsupportedVersion,
    InvalidProtocol,
    UnsupportedCatalog,
    InvalidOrdering,
    InvalidLifecycle,
    InvalidComponent,
    InvalidReference,
    MissingRoot,
    InvalidDataPath,
    InvalidAction,
    UnsafeContent,
    BoundsExceeded,
}

#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct A2UIValidationError {
    pub code: A2UIValidationCode,
    pub line: Option<usize>,
    pub path: Option<String>,
    pub message: String,
}

pub(super) type Code = A2UIValidationCode;
pub(super) type Result<T> = std::result::Result<T, A2UIValidationError>;

fn error(
    code: Code,
    line: Option<usize>,
    path: Option<&str>,
    message: &str,
) -> A2UIValidationError {
    A2UIValidationError {
        code,
        line,
        path: path.map(str::to_string),
        message: message.to_string(),
    }
}

pub(super) fn at(
    code: Code,
    line: usize,
    path: Option<&str>,
    message: &str,
) -> A2UIValidationError {
    error(code, Some(line), path, message)
}

/// Parse, validate, and reduce one A2UI JSONL batch.
///
/// # Errors
/// Returns repair feedback when any protocol, safety, or resource-bound check fails.
pub fn parse_and_reduce(
    conversation_id: impl Into<String>,
    jsonl: &str,
) -> std::result::Result<A2UIValidatedBatch, A2UIRepairResult> {
    A2UIValidator::new(conversation_id).validate_jsonl(jsonl)
}

#[derive(Debug, Clone)]
pub struct A2UIValidator {
    conversation_id: String,
    limits: A2UIValidationLimits,
}

impl A2UIValidator {
    pub fn new(conversation_id: impl Into<String>) -> Self {
        Self {
            conversation_id: conversation_id.into(),
            limits: A2UIValidationLimits::default(),
        }
    }

    #[must_use]
    pub fn with_limits(mut self, limits: A2UIValidationLimits) -> Self {
        self.limits = limits;
        self
    }

    /// Validate and reduce one A2UI JSONL batch.
    ///
    /// # Errors
    /// Returns repair feedback when any protocol, safety, or resource-bound check fails.
    pub fn validate_jsonl(
        &self,
        jsonl: &str,
    ) -> std::result::Result<A2UIValidatedBatch, A2UIRepairResult> {
        self.validate(jsonl)
            .map_err(|error| A2UIRepairResult::from_error(&error))
    }

    fn validate(&self, jsonl: &str) -> Result<A2UIValidatedBatch> {
        if self.conversation_id.trim().is_empty() {
            return Err(error(Code::InvalidProtocol, None, None, "empty namespace"));
        }
        if jsonl.len() > self.limits.max_input_bytes {
            return Err(error(Code::BoundsExceeded, None, None, "input bound"));
        }
        let mut state = A2UIState::new(self.conversation_id.clone());
        let mut messages = Vec::new();
        for (index, raw) in jsonl.lines().enumerate() {
            let line = index + 1;
            if raw.trim().is_empty() || messages.len() >= self.limits.max_messages {
                return Err(at(Code::InvalidJsonl, line, None, "invalid JSONL line"));
            }
            let value: Value = serde_json::from_str(raw.trim_end_matches('\r'))
                .map_err(|_| at(Code::InvalidJsonl, line, None, "invalid JSON"))?;
            bounds(&value, 0, line, &self.limits)?;
            safe(&value, line)?;
            self.apply(&mut state, &value, line)?;
            messages.push(value);
        }
        if messages.is_empty() {
            return Err(error(Code::InvalidJsonl, None, None, "empty JSONL"));
        }
        for surface in state.surfaces.values_mut() {
            integrity(surface, &self.limits, None, true)?;
            surface.refresh_actions();
        }
        Ok(A2UIValidatedBatch {
            messages,
            surfaces: state.surfaces,
            deleted_surface_ids: state.deleted_surface_ids,
        })
    }

    fn apply(&self, state: &mut A2UIState, message: &Value, line: usize) -> Result<()> {
        let message = as_object(message, line, Code::InvalidJsonl)?;
        if message.get("version").and_then(Value::as_str) != Some(A2UI_PROTOCOL_VERSION) {
            return Err(at(
                Code::UnsupportedVersion,
                line,
                Some("version"),
                "unsupported version",
            ));
        }
        let names = [
            "createSurface",
            "updateComponents",
            "updateDataModel",
            "deleteSurface",
        ];
        let found = names
            .iter()
            .filter(|name| message.contains_key(**name))
            .copied()
            .collect::<Vec<_>>();
        if found.len() != 1
            || message
                .keys()
                .any(|key| key != "version" && !names.contains(&key.as_str()))
        {
            return Err(at(Code::InvalidProtocol, line, None, "invalid message"));
        }
        let body = as_object(&message[found[0]], line, Code::InvalidProtocol)?;
        match found[0] {
            "createSurface" => self.create(state, body, line),
            "updateComponents" => self.components(state, body, line),
            "updateDataModel" => self.data(state, body, line),
            "deleteSurface" => Self::delete(state, body, line),
            _ => unreachable!(),
        }
    }

    fn create(&self, state: &mut A2UIState, body: &Map<String, Value>, line: usize) -> Result<()> {
        only(body, &["surfaceId", "catalogId", "sendDataModel"], line)?;
        let id = identifier(body.get("surfaceId"), line, &self.limits)?;
        if body.get("catalogId").and_then(Value::as_str) != Some(NOEMA_A2UI_CATALOG_ID) {
            return Err(at(
                Code::UnsupportedCatalog,
                line,
                Some("catalogId"),
                "unsupported catalog",
            ));
        }
        let send_data_model = body.get("sendDataModel").map_or(Ok(false), |value| {
            value.as_bool().ok_or_else(|| {
                at(
                    Code::InvalidProtocol,
                    line,
                    Some("sendDataModel"),
                    "expected boolean",
                )
            })
        })?;
        if state.surfaces.len() >= self.limits.max_surfaces {
            return Err(at(
                Code::BoundsExceeded,
                line,
                Some("surfaceId"),
                "surface bound",
            ));
        }
        let key = state.namespaced_surface_id(&id);
        if state.surfaces.contains_key(&key) {
            return Err(at(
                Code::InvalidLifecycle,
                line,
                Some("surfaceId"),
                "surface exists",
            ));
        }
        state.surfaces.insert(
            key.clone(),
            A2UISurface::new(
                id,
                key.clone(),
                A2UI_PROTOCOL_VERSION.into(),
                NOEMA_A2UI_CATALOG_ID.into(),
                send_data_model,
            ),
        );
        state.deleted_surface_ids.remove(&key);
        Ok(())
    }

    fn components(
        &self,
        state: &mut A2UIState,
        body: &Map<String, Value>,
        line: usize,
    ) -> Result<()> {
        only(body, &["surfaceId", "components"], line)?;
        let key = state.namespaced_surface_id(text(body.get("surfaceId"), line)?);
        let values = body
            .get("components")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                at(
                    Code::InvalidComponent,
                    line,
                    Some("components"),
                    "expected array",
                )
            })?;
        if values.is_empty() || values.len() > self.limits.max_components {
            return Err(at(
                Code::BoundsExceeded,
                line,
                Some("components"),
                "component bound",
            ));
        }
        let surface = active(state, &key, line)?;
        let mut seen = HashSet::new();
        for value in values {
            let id = validate_component(value, line, &self.limits)?;
            if !seen.insert(id.clone()) {
                return Err(at(Code::InvalidComponent, line, Some("id"), "duplicate id"));
            }
            surface.components.insert(id, value.clone());
        }
        if surface.components.len() > self.limits.max_components {
            return Err(at(
                Code::BoundsExceeded,
                line,
                Some("components"),
                "component bound",
            ));
        }
        surface.revision = surface.revision.saturating_add(1);
        integrity(surface, &self.limits, Some(line), false)
    }

    fn data(&self, state: &mut A2UIState, body: &Map<String, Value>, line: usize) -> Result<()> {
        only(body, &["surfaceId", "path", "value"], line)?;
        let key = state.namespaced_surface_id(text(body.get("surfaceId"), line)?);
        let surface = active(state, &key, line)?;
        let path = pointer(
            body.get("path")
                .map_or("/", |value| value.as_str().unwrap_or("")),
            line,
        )?;
        let mut model = surface.data_model.clone();
        match body.get("value") {
            Some(value) => set(&mut model, &path, value.clone(), line)?,
            None => remove(&mut model, &path),
        }
        if serde_json::to_vec(&model).map_or(usize::MAX, |bytes| bytes.len())
            > self.limits.max_data_model_bytes
        {
            return Err(at(Code::BoundsExceeded, line, Some("value"), "data bound"));
        }
        surface.data_model = model;
        surface.revision = surface.revision.saturating_add(1);
        Ok(())
    }

    fn delete(state: &mut A2UIState, body: &Map<String, Value>, line: usize) -> Result<()> {
        only(body, &["surfaceId"], line)?;
        let key = state.namespaced_surface_id(text(body.get("surfaceId"), line)?);
        if state.surfaces.remove(&key).is_none() {
            return Err(at(
                Code::InvalidLifecycle,
                line,
                Some("surfaceId"),
                "surface inactive",
            ));
        }
        state.deleted_surface_ids.insert(key);
        Ok(())
    }
}

pub(super) fn as_object(value: &Value, line: usize, code: Code) -> Result<&Map<String, Value>> {
    value
        .as_object()
        .ok_or_else(|| at(code, line, None, "expected object"))
}

pub(super) fn only(value: &Map<String, Value>, allowed: &[&str], line: usize) -> Result<()> {
    if let Some(key) = value.keys().find(|key| !allowed.contains(&key.as_str())) {
        Err(at(
            Code::InvalidProtocol,
            line,
            Some(key),
            "unsupported field",
        ))
    } else {
        Ok(())
    }
}

pub(super) fn text(value: Option<&Value>, line: usize) -> Result<&str> {
    value
        .and_then(Value::as_str)
        .ok_or_else(|| at(Code::InvalidProtocol, line, None, "expected string"))
}

pub(super) fn identifier(
    value: Option<&Value>,
    line: usize,
    limits: &A2UIValidationLimits,
) -> Result<String> {
    let value = text(value, line)?;
    if value.is_empty()
        || value != value.trim()
        || value.len() > limits.max_string_bytes
        || value.chars().any(char::is_control)
    {
        Err(at(Code::InvalidProtocol, line, None, "invalid identifier"))
    } else {
        Ok(value.to_string())
    }
}

fn active<'a>(state: &'a mut A2UIState, key: &str, line: usize) -> Result<&'a mut A2UISurface> {
    if !state.surfaces.contains_key(key) {
        let code = if state.deleted_surface_ids.contains(key) {
            Code::InvalidLifecycle
        } else {
            Code::InvalidOrdering
        };
        return Err(at(code, line, Some("surfaceId"), "surface unavailable"));
    }
    Ok(state.surfaces.get_mut(key).expect("checked"))
}

pub(super) fn pointer(path: &str, line: usize) -> Result<Vec<String>> {
    if !path.starts_with('/') {
        return Err(at(
            Code::InvalidDataPath,
            line,
            Some("path"),
            "expected pointer",
        ));
    }
    if path == "/" {
        return Ok(Vec::new());
    }
    path[1..]
        .split('/')
        .map(|part| {
            if part.is_empty() {
                return Err(at(
                    Code::InvalidDataPath,
                    line,
                    Some("path"),
                    "empty segment",
                ));
            }
            let mut output = String::new();
            let mut chars = part.chars();
            while let Some(character) = chars.next() {
                if character != '~' {
                    output.push(character);
                    continue;
                }
                output.push(match chars.next() {
                    Some('0') => '~',
                    Some('1') => '/',
                    _ => {
                        return Err(at(
                            Code::InvalidDataPath,
                            line,
                            Some("path"),
                            "invalid escape",
                        ));
                    }
                });
            }
            Ok(output)
        })
        .collect()
}

fn set(root: &mut Value, path: &[String], value: Value, line: usize) -> Result<()> {
    if path.is_empty() {
        *root = value;
        return Ok(());
    }
    let object = root
        .as_object_mut()
        .ok_or_else(|| at(Code::InvalidDataPath, line, Some("path"), "cannot descend"))?;
    if path.len() == 1 {
        object.insert(path[0].clone(), value);
    } else {
        set(
            object
                .entry(path[0].clone())
                .or_insert_with(|| Value::Object(Map::new())),
            &path[1..],
            value,
            line,
        )?;
    }
    Ok(())
}

fn remove(root: &mut Value, path: &[String]) {
    if path.is_empty() {
        *root = Value::Object(Map::new());
        return;
    }
    let Some(object) = root.as_object_mut() else {
        return;
    };
    if path.len() == 1 {
        object.remove(&path[0]);
    } else if let Some(child) = object.get_mut(&path[0]) {
        remove(child, &path[1..]);
    }
}

fn integrity(
    surface: &A2UISurface,
    limits: &A2UIValidationLimits,
    line: Option<usize>,
    require_root: bool,
) -> Result<()> {
    let Some(root) = surface.components.get("root") else {
        return if require_root {
            Err(error(Code::MissingRoot, line, Some("root"), "missing root"))
        } else {
            Ok(())
        };
    };
    if root.get("id").and_then(Value::as_str) != Some("root") {
        return Err(error(Code::MissingRoot, line, Some("root"), "invalid root"));
    }
    let edges = surface
        .components
        .iter()
        .map(|(id, value)| (id.clone(), references(value)))
        .collect::<Vec<_>>();
    let ids = surface.components.keys().collect::<BTreeSet<_>>();
    for reference in edges.iter().flat_map(|(_, refs)| refs) {
        if !ids.contains(reference) {
            return Err(error(
                Code::InvalidReference,
                line,
                Some(reference),
                "missing reference",
            ));
        }
    }
    visit(
        "root",
        &edges,
        &mut HashSet::new(),
        &mut HashSet::new(),
        0,
        limits.max_depth,
        line,
    )
}

fn references(value: &Value) -> Vec<String> {
    let Some(value) = value.as_object() else {
        return Vec::new();
    };
    match value.get("component").and_then(Value::as_str) {
        Some("Row" | "Column") => value
            .get("children")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        Some("Card" | "Button") => value
            .get("child")
            .and_then(Value::as_str)
            .map(str::to_string)
            .into_iter()
            .collect(),
        _ => Vec::new(),
    }
}

fn visit(
    id: &str,
    edges: &[(String, Vec<String>)],
    visiting: &mut HashSet<String>,
    visited: &mut HashSet<String>,
    depth: usize,
    max: usize,
    line: Option<usize>,
) -> Result<()> {
    if depth > max {
        return Err(error(
            Code::BoundsExceeded,
            line,
            Some(id),
            "component depth",
        ));
    }
    if visited.contains(id) {
        return Ok(());
    }
    if !visiting.insert(id.to_string()) {
        return Err(error(
            Code::InvalidReference,
            line,
            Some(id),
            "component cycle",
        ));
    }
    if let Some((_, refs)) = edges.iter().find(|(name, _)| name == id) {
        for reference in refs {
            visit(reference, edges, visiting, visited, depth + 1, max, line)?;
        }
    }
    visiting.remove(id);
    visited.insert(id.to_string());
    Ok(())
}

fn bounds(value: &Value, depth: usize, line: usize, limits: &A2UIValidationLimits) -> Result<()> {
    if depth > limits.max_depth {
        return Err(at(Code::BoundsExceeded, line, None, "JSON depth"));
    }
    match value {
        Value::String(value) if value.len() > limits.max_string_bytes => {
            Err(at(Code::BoundsExceeded, line, None, "string bound"))
        }
        Value::Array(values) if values.len() > limits.max_collection_items => {
            Err(at(Code::BoundsExceeded, line, None, "array bound"))
        }
        Value::Object(values) if values.len() > limits.max_collection_items => {
            Err(at(Code::BoundsExceeded, line, None, "object bound"))
        }
        Value::Array(values) => {
            for value in values {
                bounds(value, depth + 1, line, limits)?;
            }
            Ok(())
        }
        Value::Object(values) => {
            for value in values.values() {
                bounds(value, depth + 1, line, limits)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn safe(value: &Value, line: usize) -> Result<()> {
    match value {
        Value::String(value) if has_html(value) => {
            Err(at(Code::UnsafeContent, line, None, "raw HTML rejected"))
        }
        Value::Array(values) => {
            for value in values {
                safe(value, line)?;
            }
            Ok(())
        }
        Value::Object(values) => {
            for (key, value) in values {
                let lower = key.to_ascii_lowercase();
                if matches!(
                    lower.as_str(),
                    "html" | "script" | "functioncall" | "clientfunction" | "call"
                ) {
                    return Err(at(Code::UnsafeContent, line, Some(key), "unsafe field"));
                }
                if lower == "url" && value.as_str().is_some_and(unsafe_url) {
                    return Err(at(Code::UnsafeContent, line, Some(key), "unsafe URL"));
                }
                safe(value, line)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn unsafe_url(value: &str) -> bool {
    let value = value.trim().to_ascii_lowercase();
    ["javascript:", "data:", "vbscript:", "file:", "blob:"]
        .iter()
        .any(|scheme| value.starts_with(scheme))
}

fn has_html(value: &str) -> bool {
    value
        .as_bytes()
        .windows(2)
        .any(|pair| pair[0] == b'<' && (pair[1].is_ascii_alphabetic() || b"/!?".contains(&pair[1])))
}
