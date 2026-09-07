package modeleval

import "github.com/kpsuperplane/noema/internal/runtime"

type localCandidate struct {
	ID             string `json:"id" toml:"id"`
	Name           string `json:"name" toml:"name"`
	Repo           string `json:"repo" toml:"repo"`
	Revision       string `json:"revision" toml:"revision"`
	File           string `json:"file" toml:"file"`
	SHA256         string `json:"sha256" toml:"sha256"`
	Bytes          int64  `json:"bytes" toml:"bytes"`
	License        string `json:"license" toml:"license"`
	ContextTokens  int    `json:"context_tokens" toml:"context_tokens"`
	Source         string `json:"source" toml:"source"`
	ArtifactSource string `json:"artifact_source" toml:"artifact_source"`
	Notes          string `json:"notes" toml:"notes"`
}

type suiteConfig struct {
	ContextWindowTokens      int   `json:"context_window_tokens" toml:"context_window_tokens"`
	GenerationTimeoutSeconds int64 `json:"generation_timeout_seconds" toml:"generation_timeout_seconds"`
	StartupTimeoutSeconds    int64 `json:"startup_timeout_seconds" toml:"startup_timeout_seconds"`
	WorkerTimeoutSeconds     int64 `json:"worker_timeout_seconds" toml:"worker_timeout_seconds"`
	Repetitions              int   `json:"repetitions" toml:"repetitions"`
	DecisionRepetitions      *int  `json:"decision_repetitions" toml:"decision_repetitions"`
	ExplorationRepetitions   *int  `json:"exploration_repetitions" toml:"exploration_repetitions"`
}

type candidate struct {
	ID                     string                 `json:"id" toml:"id"`
	Name                   string                 `json:"name" toml:"name"`
	Model                  string                 `json:"model" toml:"model"`
	Roles                  []string               `json:"roles" toml:"roles"`
	ReasoningEffort        string                 `json:"reasoning_effort" toml:"reasoning_effort"`
	BaseURL                string                 `json:"-" toml:"base_url"`
	AcceptedResponseModels []string               `json:"accepted_response_models" toml:"accepted_response_models"`
	Targets                []recommendationTarget `json:"targets" toml:"targets"`
	Pricing                *modelPricing          `json:"pricing" toml:"pricing"`
	Enabled                *bool                  `json:"enabled" toml:"enabled"`
	Notes                  string                 `json:"notes" toml:"notes"`
}

type recommendationTarget struct {
	Provider        string `json:"provider" toml:"provider"`
	ModelProfile    string `json:"model_profile" toml:"model_profile"`
	ReasoningEffort string `json:"reasoning_effort" toml:"reasoning_effort"`
}

type modelPricing struct {
	InputUSDPerMillion       float64  `json:"input_usd_per_million" toml:"input_usd_per_million"`
	CachedInputUSDPerMillion *float64 `json:"cached_input_usd_per_million" toml:"cached_input_usd_per_million"`
	OutputUSDPerMillion      float64  `json:"output_usd_per_million" toml:"output_usd_per_million"`
}

type rolePolicies struct {
	SchemaVersion int          `json:"schema_version" toml:"schema_version"`
	Judge         judgePolicy  `json:"judge" toml:"judge"`
	Policies      []rolePolicy `json:"policies" toml:"policies"`
}

type judgePolicy struct {
	Model               string `json:"model" toml:"model"`
	ReasoningEffort     string `json:"reasoning_effort" toml:"reasoning_effort"`
	MaximumOutputTokens int    `json:"maximum_output_tokens" toml:"maximum_output_tokens"`
}

type rolePolicy struct {
	Role                     string   `json:"role" toml:"role"`
	IncumbentCandidateID     string   `json:"incumbent_candidate_id" toml:"incumbent_candidate_id"`
	MinimumCases             int      `json:"minimum_cases" toml:"minimum_cases"`
	MinimumQualityScore      float64  `json:"minimum_quality_score" toml:"minimum_quality_score"`
	MaximumErrorRate         float64  `json:"maximum_error_rate" toml:"maximum_error_rate"`
	MaximumP95LatencyMS      int64    `json:"maximum_p95_latency_ms" toml:"maximum_p95_latency_ms"`
	ReplacementQualityMargin float64  `json:"replacement_quality_margin" toml:"replacement_quality_margin"`
	DeterministicWeight      float64  `json:"deterministic_weight" toml:"deterministic_weight"`
	JudgeWeight              float64  `json:"judge_weight" toml:"judge_weight"`
	JudgeCaseIDs             []string `json:"judge_case_ids" toml:"judge_case_ids"`
}

type decisionPlan struct {
	SchemaVersion        int          `json:"schema_version" toml:"schema_version"`
	DecisionID           string       `json:"decision_id" toml:"decision_id"`
	CreatedAtUnixSeconds int64        `json:"created_at_unix_seconds" toml:"created_at_unix_seconds"`
	GitCommit            string       `json:"git_commit" toml:"git_commit"`
	GitDirty             bool         `json:"git_dirty" toml:"git_dirty"`
	RuntimeSuiteVersion  int          `json:"runtime_suite_version" toml:"runtime_suite_version"`
	Suite                suiteConfig  `json:"suite" toml:"suite"`
	Policies             rolePolicies `json:"policies" toml:"policies"`
	Candidates           []candidate  `json:"candidates" toml:"candidates"`
	EstimatedMaxCostUSD  float64      `json:"estimated_max_cost_usd" toml:"estimated_max_cost_usd"`
	SpendCeilingUSD      float64      `json:"spend_ceiling_usd" toml:"spend_ceiling_usd"`
	ContentFingerprint   string       `json:"content_fingerprint" toml:"content_fingerprint"`
}

type matrixReport struct {
	SchemaVersion       int                   `json:"schema_version" toml:"schema_version"`
	RunID               string                `json:"run_id" toml:"run_id"`
	Mode                string                `json:"mode" toml:"mode"`
	Status              string                `json:"status" toml:"status"`
	Failure             string                `json:"failure" toml:"failure"`
	Environment         evaluationEnvironment `json:"environment" toml:"environment"`
	RuntimeSuiteVersion int                   `json:"runtime_suite_version" toml:"runtime_suite_version"`
	DecisionFingerprint string                `json:"decision_fingerprint" toml:"decision_fingerprint"`
	Repetitions         int                   `json:"repetitions" toml:"repetitions"`
	Policies            rolePolicies          `json:"policies" toml:"policies"`
	Candidates          []candidate           `json:"candidates" toml:"candidates"`
	Entries             []matrixEntry         `json:"entries" toml:"entries"`
	Comparisons         []roleComparison      `json:"comparisons" toml:"comparisons"`
	Rankings            []roleRanking         `json:"rankings" toml:"rankings"`
}

type evaluationEnvironment struct {
	EvaluatorVersion string `json:"evaluator_version" toml:"evaluator_version"`
	TargetOS         string `json:"target_os" toml:"target_os"`
	TargetArch       string `json:"target_arch" toml:"target_arch"`
}

type matrixEntry struct {
	CandidateID string                     `json:"candidate_id" toml:"candidate_id"`
	Repetition  int                        `json:"repetition" toml:"repetition"`
	Cases       []runtime.EvaluationResult `json:"cases" toml:"cases"`
	Error       string                     `json:"error" toml:"error"`
}

type roleComparison struct {
	Role                  string `json:"role" toml:"role"`
	ChallengerCandidateID string `json:"challenger_candidate_id" toml:"challenger_candidate_id"`
	IncumbentCandidateID  string `json:"incumbent_candidate_id" toml:"incumbent_candidate_id"`
	JudgeModel            string `json:"judge_model" toml:"judge_model"`
	CandidateAID          string `json:"candidate_a_id" toml:"candidate_a_id"`
	CandidateBID          string `json:"candidate_b_id" toml:"candidate_b_id"`
	WinnerCandidateID     string `json:"winner_candidate_id" toml:"winner_candidate_id"`
	ChallengerScore       *int   `json:"challenger_score" toml:"challenger_score"`
	IncumbentScore        *int   `json:"incumbent_score" toml:"incumbent_score"`
	Rationale             string `json:"rationale" toml:"rationale"`
	ResponseProvider      string `json:"response_provider" toml:"response_provider"`
	ResponseModel         string `json:"response_model" toml:"response_model"`
	Error                 string `json:"error" toml:"error"`
}

type roleRanking struct {
	Role                   string           `json:"role" toml:"role"`
	RecommendedCandidateID string           `json:"recommended_candidate_id" toml:"recommended_candidate_id"`
	SelectionReason        string           `json:"selection_reason" toml:"selection_reason"`
	Candidates             []candidateScore `json:"candidates" toml:"candidates"`
}

type candidateScore struct {
	CandidateID          string   `json:"candidate_id" toml:"candidate_id"`
	CompletedRepetitions int      `json:"completed_repetitions" toml:"completed_repetitions"`
	Qualified            bool     `json:"qualified" toml:"qualified"`
	IdentityMatched      bool     `json:"identity_matched" toml:"identity_matched"`
	ObservedModels       []string `json:"observed_models" toml:"observed_models"`
	PassedCriticalCases  int      `json:"passed_critical_cases" toml:"passed_critical_cases"`
	TotalCriticalCases   int      `json:"total_critical_cases" toml:"total_critical_cases"`
	PassedCases          int      `json:"passed_cases" toml:"passed_cases"`
	TotalCases           int      `json:"total_cases" toml:"total_cases"`
	DeterministicScore   *float64 `json:"deterministic_score" toml:"deterministic_score"`
	JudgeScore           *float64 `json:"judge_score" toml:"judge_score"`
	QualityScore         *float64 `json:"quality_score" toml:"quality_score"`
	ErrorRate            *float64 `json:"error_rate" toml:"error_rate"`
	MedianLatencyMS      *int64   `json:"median_latency_ms" toml:"median_latency_ms"`
	P95LatencyMS         *int64   `json:"p95_latency_ms" toml:"p95_latency_ms"`
	EstimatedCostUSD     *float64 `json:"estimated_cost_usd" toml:"estimated_cost_usd"`
}

type runtimeMemory struct {
	Metric     string `json:"metric" toml:"metric"`
	ReadyBytes int64  `json:"ready_bytes" toml:"ready_bytes"`
	PeakBytes  int64  `json:"peak_bytes" toml:"peak_bytes"`
}

type resourceProbe struct {
	TargetInputTokens    int    `json:"target_input_tokens" toml:"target_input_tokens"`
	ObservedInputTokens  *int64 `json:"observed_input_tokens" toml:"observed_input_tokens"`
	NearContextLatencyMS *int64 `json:"near_context_latency_ms" toml:"near_context_latency_ms"`
	SteadyTurnsRequested int    `json:"steady_turns_requested" toml:"steady_turns_requested"`
	SteadyTurnsCompleted int    `json:"steady_turns_completed" toml:"steady_turns_completed"`
	PostTurnMinBytes     *int64 `json:"post_turn_min_bytes" toml:"post_turn_min_bytes"`
	PostTurnMaxBytes     *int64 `json:"post_turn_max_bytes" toml:"post_turn_max_bytes"`
	Failure              string `json:"failure" toml:"failure"`
}

type localReport struct {
	ModelID             string                     `json:"model_id" toml:"model_id"`
	LlamaCppRelease     string                     `json:"llama_cpp_release" toml:"llama_cpp_release"`
	LlamaCppCommit      string                     `json:"llama_cpp_commit" toml:"llama_cpp_commit"`
	Backend             string                     `json:"backend" toml:"backend"`
	RuntimeLoadMS       int64                      `json:"runtime_load_ms" toml:"runtime_load_ms"`
	RuntimeMemory       *runtimeMemory             `json:"runtime_memory" toml:"runtime_memory"`
	ResourceProbe       *resourceProbe             `json:"resource_probe" toml:"resource_probe"`
	RuntimeError        string                     `json:"runtime_error" toml:"runtime_error"`
	Cases               []runtime.EvaluationResult `json:"cases" toml:"cases"`
	PassedCases         int                        `json:"passed_cases" toml:"passed_cases"`
	TotalCases          int                        `json:"total_cases" toml:"total_cases"`
	PassedCriticalCases int                        `json:"passed_critical_cases" toml:"passed_critical_cases"`
	TotalCriticalCases  int                        `json:"total_critical_cases" toml:"total_critical_cases"`
}
