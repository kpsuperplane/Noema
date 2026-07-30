//! Durable intent for one configurable model preference.

use crate::{NoemaModelUseCase, ProviderKind, ReasoningEffort, noema_model_recommendation};

/// Whether Noema or the human chooses the concrete model profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelPreferenceSelection {
    /// Resolve the current product recommendation for this preference's use case.
    NoemaRecommended,
    /// Always use the exact provider model and effort selected by the human.
    ExplicitProfile {
        /// Provider-specific model profile.
        model_profile: String,
        /// Explicit reasoning effort, when supported.
        reasoning_effort: Option<ReasoningEffort>,
    },
}

impl ModelPreferenceSelection {
    /// Stable SQLite and GraphQL-adjacent representation.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::NoemaRecommended => "noema_recommended",
            Self::ExplicitProfile { .. } => "explicit_profile",
        }
    }

    /// Parse the stable persistence representation.
    #[must_use]
    pub fn from_persisted_parts(
        mode: &str,
        model_profile: Option<String>,
        reasoning_effort: Option<ReasoningEffort>,
    ) -> Option<Self> {
        match mode {
            "noema_recommended" if model_profile.is_none() && reasoning_effort.is_none() => {
                Some(Self::NoemaRecommended)
            }
            "explicit_profile" => model_profile.map(|model_profile| Self::ExplicitProfile {
                model_profile,
                reasoning_effort,
            }),
            _ => None,
        }
    }

    /// Return the explicit model profile, when the human selected one.
    #[must_use]
    pub fn model_profile(&self) -> Option<&str> {
        match self {
            Self::NoemaRecommended => None,
            Self::ExplicitProfile { model_profile, .. } => Some(model_profile),
        }
    }

    /// Return the explicit reasoning effort, when present.
    #[must_use]
    pub const fn reasoning_effort(&self) -> Option<ReasoningEffort> {
        match self {
            Self::NoemaRecommended => None,
            Self::ExplicitProfile {
                reasoning_effort, ..
            } => *reasoning_effort,
        }
    }

    /// Resolve this intent to the concrete model shipped for the use case.
    #[must_use]
    pub fn resolve(
        &self,
        provider_kind: ProviderKind,
        use_case: NoemaModelUseCase,
    ) -> Option<(String, Option<ReasoningEffort>)> {
        match self {
            Self::NoemaRecommended => {
                noema_model_recommendation(provider_kind, use_case).map(|recommendation| {
                    (
                        recommendation.model_profile.to_string(),
                        recommendation.reasoning_effort,
                    )
                })
            }
            Self::ExplicitProfile {
                model_profile,
                reasoning_effort,
            } => Some((model_profile.clone(), *reasoning_effort)),
        }
    }
}
