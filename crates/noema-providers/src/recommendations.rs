//! Product-owned model recommendations for Noema workloads.

use crate::{ProviderKind, ReasoningEffort};

/// One semantic workload whose model choice can be delegated to Noema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NoemaModelUseCase {
    /// Foreground conversation.
    Primary,
    /// Routine task execution.
    TaskSimple,
    /// General task execution.
    TaskMedium,
    /// Complex task execution.
    TaskDifficult,
    /// Review of completed task work.
    TaskReviewer,
    /// Summarization of fetched web pages.
    WebFetchSummarizer,
    /// Audit of long-running tool progress.
    ToolProgressAudit,
    /// Review of governed actions.
    ActionReviewer,
    /// Consolidation of native memory.
    MemoryConsolidation,
}

impl NoemaModelUseCase {
    /// Every workload with a configurable model preference.
    pub const ALL: [Self; 9] = [
        Self::Primary,
        Self::TaskSimple,
        Self::TaskMedium,
        Self::TaskDifficult,
        Self::TaskReviewer,
        Self::WebFetchSummarizer,
        Self::ToolProgressAudit,
        Self::ActionReviewer,
        Self::MemoryConsolidation,
    ];
}

/// Concrete model and effort currently recommended for one workload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoemaModelRecommendation {
    /// Provider-specific model profile.
    pub model_profile: &'static str,
    /// Explicit effort chosen by Noema, when the model supports one.
    pub reasoning_effort: Option<ReasoningEffort>,
}

/// One shipped provider/workload recommendation cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoemaModelRecommendationCell {
    /// Provider whose profile id is stored in this cell.
    pub provider_kind: ProviderKind,
    /// Semantic workload governed by the cell.
    pub use_case: NoemaModelUseCase,
    /// Shipped model and effort.
    pub recommendation: NoemaModelRecommendation,
}

macro_rules! recommendations {
    ($(($provider:ident, $use_case:ident, $model:literal, $effort:ident)),* $(,)?) => {
        &[$(
            cell(
                ProviderKind::$provider,
                NoemaModelUseCase::$use_case,
                $model,
                recommendations!(@effort $effort),
            ),
        )*]
    };
    (@effort none) => { None };
    (@effort off) => { Some(ReasoningEffort::None) };
    (@effort minimal) => { Some(ReasoningEffort::Minimal) };
    (@effort low) => { Some(ReasoningEffort::Low) };
    (@effort medium) => { Some(ReasoningEffort::Medium) };
    (@effort high) => { Some(ReasoningEffort::High) };
    (@effort xhigh) => { Some(ReasoningEffort::XHigh) };
}

/// Stable shipped recommendation table used by both runtime lookup and eval verification.
#[rustfmt::skip]
pub static NOEMA_MODEL_RECOMMENDATIONS: &[NoemaModelRecommendationCell] = recommendations![
    (Codex, Primary, "gpt-5.6-luna", high),
    (Codex, TaskSimple, "gpt-5.6-luna", low),
    (Codex, TaskMedium, "gpt-5.6-luna", low),
    (Codex, TaskDifficult, "gpt-5.6-sol", medium),
    (Codex, TaskReviewer, "gpt-5.6-luna", low),
    (Codex, WebFetchSummarizer, "gpt-5.6-luna", low),
    (Codex, ToolProgressAudit, "gpt-5.6-luna", low),
    (Codex, ActionReviewer, "gpt-5.6-luna", low),
    (Codex, MemoryConsolidation, "gpt-5.6-luna", low),
    (OpenAi, Primary, "gpt-5.6-luna", high),
    (OpenAi, TaskSimple, "gpt-5.6-luna", low),
    (OpenAi, TaskMedium, "gpt-5.6-luna", low),
    (OpenAi, TaskDifficult, "gpt-5.6-sol", medium),
    (OpenAi, TaskReviewer, "gpt-5.6-luna", low),
    (OpenAi, WebFetchSummarizer, "gpt-5.6-luna", low),
    (OpenAi, ToolProgressAudit, "gpt-5.6-luna", low),
    (OpenAi, ActionReviewer, "gpt-5.6-luna", low),
    (OpenAi, MemoryConsolidation, "gpt-5.6-luna", low),
    (OpenRouter, Primary, "openai/gpt-5.6-luna", high),
    (OpenRouter, TaskSimple, "openai/gpt-5.6-luna", low),
    (OpenRouter, TaskMedium, "openai/gpt-5.6-luna", low),
    (OpenRouter, TaskDifficult, "openai/gpt-5.6-sol", medium),
    (OpenRouter, TaskReviewer, "openai/gpt-5.6-luna", low),
    (OpenRouter, WebFetchSummarizer, "openai/gpt-5.6-luna", low),
    (OpenRouter, ToolProgressAudit, "openai/gpt-5.6-luna", low),
    (OpenRouter, ActionReviewer, "openai/gpt-5.6-luna", low),
    (OpenRouter, MemoryConsolidation, "openai/gpt-5.6-luna", low),
];

/// Return Noema's current recommendation for one provider and workload.
#[must_use]
pub fn noema_model_recommendation(
    provider_kind: ProviderKind,
    use_case: NoemaModelUseCase,
) -> Option<NoemaModelRecommendation> {
    NOEMA_MODEL_RECOMMENDATIONS
        .iter()
        .find(|cell| cell.provider_kind == provider_kind && cell.use_case == use_case)
        .map(|cell| cell.recommendation)
}

const fn cell(
    provider_kind: ProviderKind,
    use_case: NoemaModelUseCase,
    model_profile: &'static str,
    reasoning_effort: Option<ReasoningEffort>,
) -> NoemaModelRecommendationCell {
    NoemaModelRecommendationCell {
        provider_kind,
        use_case,
        recommendation: recommendation(model_profile, reasoning_effort),
    }
}

const fn recommendation(
    model_profile: &'static str,
    reasoning_effort: Option<ReasoningEffort>,
) -> NoemaModelRecommendation {
    NoemaModelRecommendation {
        model_profile,
        reasoning_effort,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recommendation_matrix_matches_the_evaluated_defaults() {
        for provider in [ProviderKind::Codex, ProviderKind::OpenAi] {
            for use_case in NoemaModelUseCase::ALL {
                let expected = match use_case {
                    NoemaModelUseCase::Primary => {
                        recommendation("gpt-5.6-luna", Some(ReasoningEffort::High))
                    }
                    NoemaModelUseCase::TaskDifficult => {
                        recommendation("gpt-5.6-sol", Some(ReasoningEffort::Medium))
                    }
                    _ => recommendation("gpt-5.6-luna", Some(ReasoningEffort::Low)),
                };
                assert_eq!(
                    noema_model_recommendation(provider.clone(), use_case),
                    Some(expected)
                );
            }
        }

        for use_case in NoemaModelUseCase::ALL {
            let expected = match use_case {
                NoemaModelUseCase::Primary => {
                    recommendation("openai/gpt-5.6-luna", Some(ReasoningEffort::High))
                }
                NoemaModelUseCase::TaskDifficult => {
                    recommendation("openai/gpt-5.6-sol", Some(ReasoningEffort::Medium))
                }
                _ => recommendation("openai/gpt-5.6-luna", Some(ReasoningEffort::Low)),
            };
            assert_eq!(
                noema_model_recommendation(ProviderKind::OpenRouter, use_case),
                Some(expected)
            );
        }

        for provider in [ProviderKind::FoundationLocal, ProviderKind::LocalModels] {
            assert!(NoemaModelUseCase::ALL.into_iter().all(|use_case| {
                noema_model_recommendation(provider.clone(), use_case).is_none()
            }));
        }
    }
}
