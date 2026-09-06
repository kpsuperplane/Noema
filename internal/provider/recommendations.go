package provider

// ModelUseCase identifies one product-owned model workload.
type ModelUseCase string

const (
	ModelUsePrimary             ModelUseCase = "primary"
	ModelUseTaskSimple          ModelUseCase = "task_simple"
	ModelUseTaskMedium          ModelUseCase = "task_medium"
	ModelUseTaskDifficult       ModelUseCase = "task_difficult"
	ModelUseTaskReviewer        ModelUseCase = "task_reviewer"
	ModelUseWebFetchSummarizer  ModelUseCase = "web_fetch_summarizer"
	ModelUseToolProgressAudit   ModelUseCase = "tool_progress_audit"
	ModelUseActionReviewer      ModelUseCase = "action_reviewer"
	ModelUseMemoryConsolidation ModelUseCase = "memory_consolidation"
)

var modelUseCases = []ModelUseCase{
	ModelUsePrimary,
	ModelUseTaskSimple,
	ModelUseTaskMedium,
	ModelUseTaskDifficult,
	ModelUseTaskReviewer,
	ModelUseWebFetchSummarizer,
	ModelUseToolProgressAudit,
	ModelUseActionReviewer,
	ModelUseMemoryConsolidation,
}

// ModelRecommendation is one shipped provider choice for a workload.
type ModelRecommendation struct {
	UseCase         ModelUseCase
	ModelProfile    string
	ReasoningEffort string
}

// ModelRecommendations returns shipped choices in stable workload order.
func ModelRecommendations(providerKind string) []ModelRecommendation {
	var primaryProfile, routineProfile, difficultProfile string
	var primaryEffort string
	switch providerKind {
	case "codex", "openai":
		primaryProfile = "gpt-5.6-terra"
		routineProfile = "gpt-5.6-luna"
		difficultProfile = "gpt-5.6-sol"
		primaryEffort = "medium"
	case "openrouter":
		primaryProfile = "openai/gpt-5.6-luna"
		routineProfile = primaryProfile
		difficultProfile = "openai/gpt-5.6-sol"
		primaryEffort = "high"
	default:
		return nil
	}

	recommendations := make([]ModelRecommendation, 0, len(modelUseCases))
	for _, useCase := range modelUseCases {
		profile, effort := routineProfile, "low"
		switch useCase {
		case ModelUsePrimary:
			profile, effort = primaryProfile, primaryEffort
		case ModelUseTaskDifficult:
			profile, effort = difficultProfile, "medium"
		case ModelUseTaskSimple, ModelUseTaskReviewer, ModelUseMemoryConsolidation:
			if providerKind != "openrouter" {
				effort = "medium"
			}
		case ModelUseTaskMedium:
			if providerKind != "openrouter" {
				effort = "xhigh"
			}
		}
		recommendations = append(recommendations, ModelRecommendation{
			UseCase: useCase, ModelProfile: profile, ReasoningEffort: effort,
		})
	}
	return recommendations
}
