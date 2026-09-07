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

// ModelRecommendation is one shipped provider choice for a workload.
type ModelRecommendation struct {
	UseCase         ModelUseCase
	ModelProfile    string
	ReasoningEffort string
}

// ModelRecommendations returns shipped choices in stable workload order.
func ModelRecommendations(providerKind string) []ModelRecommendation {
	switch providerKind {
	case "codex":
		return []ModelRecommendation{
			{UseCase: ModelUseCase("primary"), ModelProfile: "gpt-5.6-terra", ReasoningEffort: "medium"},
			{UseCase: ModelUseCase("task_simple"), ModelProfile: "gpt-5.6-luna", ReasoningEffort: "medium"},
			{UseCase: ModelUseCase("task_medium"), ModelProfile: "gpt-5.6-luna", ReasoningEffort: "xhigh"},
			{UseCase: ModelUseCase("task_difficult"), ModelProfile: "gpt-5.6-sol", ReasoningEffort: "medium"},
			{UseCase: ModelUseCase("task_reviewer"), ModelProfile: "gpt-5.6-luna", ReasoningEffort: "medium"},
			{UseCase: ModelUseCase("web_fetch_summarizer"), ModelProfile: "gpt-5.6-luna", ReasoningEffort: "low"},
			{UseCase: ModelUseCase("tool_progress_audit"), ModelProfile: "gpt-5.6-luna", ReasoningEffort: "low"},
			{UseCase: ModelUseCase("action_reviewer"), ModelProfile: "gpt-5.6-luna", ReasoningEffort: "low"},
			{UseCase: ModelUseCase("memory_consolidation"), ModelProfile: "gpt-5.6-luna", ReasoningEffort: "medium"},
		}
	case "openai":
		return []ModelRecommendation{
			{UseCase: ModelUseCase("primary"), ModelProfile: "gpt-5.6-terra", ReasoningEffort: "medium"},
			{UseCase: ModelUseCase("task_simple"), ModelProfile: "gpt-5.6-luna", ReasoningEffort: "medium"},
			{UseCase: ModelUseCase("task_medium"), ModelProfile: "gpt-5.6-luna", ReasoningEffort: "xhigh"},
			{UseCase: ModelUseCase("task_difficult"), ModelProfile: "gpt-5.6-sol", ReasoningEffort: "medium"},
			{UseCase: ModelUseCase("task_reviewer"), ModelProfile: "gpt-5.6-luna", ReasoningEffort: "medium"},
			{UseCase: ModelUseCase("web_fetch_summarizer"), ModelProfile: "gpt-5.6-luna", ReasoningEffort: "low"},
			{UseCase: ModelUseCase("tool_progress_audit"), ModelProfile: "gpt-5.6-luna", ReasoningEffort: "low"},
			{UseCase: ModelUseCase("action_reviewer"), ModelProfile: "gpt-5.6-luna", ReasoningEffort: "low"},
			{UseCase: ModelUseCase("memory_consolidation"), ModelProfile: "gpt-5.6-luna", ReasoningEffort: "medium"},
		}
	case "openrouter":
		return []ModelRecommendation{
			{UseCase: ModelUseCase("primary"), ModelProfile: "openai/gpt-5.6-luna", ReasoningEffort: "high"},
			{UseCase: ModelUseCase("task_simple"), ModelProfile: "openai/gpt-5.6-luna", ReasoningEffort: "low"},
			{UseCase: ModelUseCase("task_medium"), ModelProfile: "openai/gpt-5.6-luna", ReasoningEffort: "low"},
			{UseCase: ModelUseCase("task_difficult"), ModelProfile: "openai/gpt-5.6-sol", ReasoningEffort: "medium"},
			{UseCase: ModelUseCase("task_reviewer"), ModelProfile: "openai/gpt-5.6-luna", ReasoningEffort: "low"},
			{UseCase: ModelUseCase("web_fetch_summarizer"), ModelProfile: "openai/gpt-5.6-luna", ReasoningEffort: "low"},
			{UseCase: ModelUseCase("tool_progress_audit"), ModelProfile: "openai/gpt-5.6-luna", ReasoningEffort: "low"},
			{UseCase: ModelUseCase("action_reviewer"), ModelProfile: "openai/gpt-5.6-luna", ReasoningEffort: "low"},
			{UseCase: ModelUseCase("memory_consolidation"), ModelProfile: "openai/gpt-5.6-luna", ReasoningEffort: "low"},
		}
	default:
		return nil
	}
}
