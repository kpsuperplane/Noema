use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct A2UIAction {
    pub source_component_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct A2UISurface {
    pub surface_id: String,
    pub namespaced_surface_id: String,
    pub version: String,
    pub catalog_id: String,
    pub send_data_model: bool,
    pub revision: u64,
    pub components: BTreeMap<String, Value>,
    pub data_model: Value,
    pub actions: Vec<A2UIAction>,
}

impl A2UISurface {
    pub(crate) fn new(
        surface_id: String,
        namespaced_surface_id: String,
        version: String,
        catalog_id: String,
        send_data_model: bool,
    ) -> Self {
        Self {
            surface_id,
            namespaced_surface_id,
            version,
            catalog_id,
            send_data_model,
            revision: 1,
            components: BTreeMap::new(),
            data_model: Value::Object(Map::new()),
            actions: Vec::new(),
        }
    }

    pub(crate) fn refresh_actions(&mut self) {
        let mut actions = self
            .components
            .iter()
            .filter_map(|(id, component)| {
                let object = component.as_object()?;
                if object.get("component")?.as_str()? != "Button" {
                    return None;
                }
                let event = object.get("action")?.get("event")?.as_object()?;
                let name = event.get("name")?.as_str()?.to_string();
                Some(A2UIAction {
                    source_component_id: id.clone(),
                    name,
                    context: event.get("context").cloned(),
                })
            })
            .collect::<Vec<_>>();
        actions.sort_by(|left, right| {
            left.source_component_id
                .cmp(&right.source_component_id)
                .then_with(|| left.name.cmp(&right.name))
        });
        self.actions = actions;
    }
}

#[derive(Debug, Clone)]
pub(crate) struct A2UIState {
    conversation_id: String,
    pub(crate) surfaces: BTreeMap<String, A2UISurface>,
    pub(crate) deleted_surface_ids: BTreeSet<String>,
}

impl A2UIState {
    pub(crate) fn new(conversation_id: String) -> Self {
        Self {
            conversation_id,
            surfaces: BTreeMap::new(),
            deleted_surface_ids: BTreeSet::new(),
        }
    }

    pub(crate) fn namespaced_surface_id(&self, surface_id: &str) -> String {
        format!(
            "a2ui/{}/{}",
            escape_namespace_segment(&self.conversation_id),
            escape_namespace_segment(surface_id)
        )
    }
}

fn escape_namespace_segment(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            output.push(byte as char);
        } else {
            output.push('%');
            output.push(hex_digit(byte >> 4));
            output.push(hex_digit(byte & 0x0f));
        }
    }
    output
}

fn hex_digit(value: u8) -> char {
    match value {
        0..=9 => (b'0' + value) as char,
        _ => (b'A' + value - 10) as char,
    }
}
