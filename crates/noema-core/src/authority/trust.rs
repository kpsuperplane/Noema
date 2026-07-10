//! Structural content trust carried independently of content text.

use serde::Serialize;

/// Trust classification for model-visible or tool-provided content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    content = "components",
    rename_all = "SCREAMING_SNAKE_CASE"
)]
pub enum ContentTrust {
    /// Noema-created local content.
    TrustedLocal,
    /// Content directly supplied by the authenticated human.
    TrustedUser,
    /// Content returned by an external service or tool.
    UntrustedExternal,
    /// Content assembled from independently classified components.
    Mixed(Vec<ContentTrust>),
    /// Content withheld because its provenance or ownership could not be proven.
    Quarantined,
}

/// A value paired with structural trust metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TrustedEnvelope<T> {
    value: T,
    trust: ContentTrust,
}

impl<T> TrustedEnvelope<T> {
    /// Wrap a value with its server-derived trust classification.
    #[must_use]
    pub const fn new(value: T, trust: ContentTrust) -> Self {
        Self { value, trust }
    }

    /// Borrow the contained value.
    #[must_use]
    pub const fn value(&self) -> &T {
        &self.value
    }

    /// Borrow the structural trust classification.
    #[must_use]
    pub const fn trust(&self) -> &ContentTrust {
        &self.trust
    }

    /// Transform the value without inspecting or changing trust metadata.
    #[must_use]
    pub fn map<U>(self, transform: impl FnOnce(T) -> U) -> TrustedEnvelope<U> {
        TrustedEnvelope {
            value: transform(self.value),
            trust: self.trust,
        }
    }
}
