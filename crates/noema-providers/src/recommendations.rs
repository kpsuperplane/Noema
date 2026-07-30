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

/// Return Noema's current recommendation for one provider and workload.
#[must_use]
pub const fn noema_model_recommendation(
    provider_kind: ProviderKind,
    use_case: NoemaModelUseCase,
) -> Option<NoemaModelRecommendation> {
    use NoemaModelUseCase::{ActionReviewer, MemoryConsolidation, TaskDifficult};

    match provider_kind {
        ProviderKind::Codex | ProviderKind::OpenAi => Some(match use_case {
            TaskDifficult => recommendation("gpt-5.6-terra", Some(ReasoningEffort::Medium)),
            _ => recommendation("gpt-5.6-luna", Some(ReasoningEffort::Low)),
        }),
        ProviderKind::OpenRouter => Some(match use_case {
            TaskDifficult => recommendation("openai/gpt-5.6-terra", Some(ReasoningEffort::Medium)),
            ActionReviewer | MemoryConsolidation => {
                recommendation("deepseek/deepseek-v4-flash", None)
            }
            _ => recommendation("openai/gpt-5.6-luna", Some(ReasoningEffort::Low)),
        }),
        ProviderKind::FoundationLocal | ProviderKind::LocalModels => None,
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
    fn recommendation_matrix_matches_the_evaluated_initial_set() {
        for provider in [ProviderKind::Codex, ProviderKind::OpenAi] {
            for use_case in NoemaModelUseCase::ALL {
                let expected = if use_case == NoemaModelUseCase::TaskDifficult {
                    recommendation("gpt-5.6-terra", Some(ReasoningEffort::Medium))
                } else {
                    recommendation("gpt-5.6-luna", Some(ReasoningEffort::Low))
                };
                assert_eq!(
                    noema_model_recommendation(provider.clone(), use_case),
                    Some(expected)
                );
            }
        }

        for use_case in NoemaModelUseCase::ALL {
            let expected = match use_case {
                NoemaModelUseCase::TaskDifficult => {
                    recommendation("openai/gpt-5.6-terra", Some(ReasoningEffort::Medium))
                }
                NoemaModelUseCase::ActionReviewer | NoemaModelUseCase::MemoryConsolidation => {
                    recommendation("deepseek/deepseek-v4-flash", None)
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
