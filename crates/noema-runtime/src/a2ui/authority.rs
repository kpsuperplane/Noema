//! Pinned A2UI authority metadata and the Noema catalog.

use serde::{Deserialize, Serialize};

pub const A2UI_AUTHORITY_SOURCE: &str = "https://github.com/a2ui-project/a2ui";

pub const A2UI_AUTHORITY_COMMIT: &str = "ef941afd93267a2218f5aaca1fcc27da87f0e464";

pub const A2UI_PROTOCOL_VERSION: &str = "v0.9.1";

pub const NOEMA_A2UI_CATALOG_ID: &str = "com.noema.a2ui/catalog/v0.9.1";

pub const NOEMA_A2UI_COMPONENTS: &[&str] = &[
    "Text",
    "Row",
    "Column",
    "Card",
    "Divider",
    "Button",
    "TextField",
    "CheckBox",
    "ChoicePicker",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct A2UICatalogDescriptor {
    pub catalog_id: String,
    pub protocol_version: String,
    pub components: Vec<String>,
}

#[must_use]
pub fn advertised_catalog() -> A2UICatalogDescriptor {
    A2UICatalogDescriptor {
        catalog_id: NOEMA_A2UI_CATALOG_ID.to_string(),
        protocol_version: A2UI_PROTOCOL_VERSION.to_string(),
        components: NOEMA_A2UI_COMPONENTS
            .iter()
            .map(|component| (*component).to_string())
            .collect(),
    }
}

pub(crate) fn supports_component(name: &str) -> bool {
    NOEMA_A2UI_COMPONENTS.contains(&name)
}
