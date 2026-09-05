package runtime

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"slices"
	"strings"
	"time"

	noemamcp "github.com/kpsuperplane/noema/internal/mcp"
	"github.com/kpsuperplane/noema/internal/provider"
)

// EvaluationSuiteVersion changes when qualification uses a new runtime contract.
const EvaluationSuiteVersion = 10
const EvaluationProtocolCategory = "openrouter_protocol"

// EvaluationCase describes one bounded production-runtime qualification case.
type EvaluationCase struct {
	ID                   string `json:"case_id"`
	Role                 string `json:"role"`
	Category             string `json:"category"`
	Critical             bool   `json:"critical"`
	MaximumOutputTokens  uint32 `json:"maximum_output_tokens"`
	MaximumProviderCalls int    `json:"maximum_provider_calls"`
	JudgeRubric          string `json:"judge_rubric,omitempty"`
	request              provider.GenerateRequest
	expectation          string
	steps                []evaluationStep
}

// EvaluationResult retains bounded evidence for one qualification case.
type EvaluationResult struct {
	CaseID              string               `json:"case_id"`
	Role                string               `json:"role"`
	Category            string               `json:"category"`
	Critical            bool                 `json:"critical"`
	Passed              bool                 `json:"passed"`
	JudgeRubric         string               `json:"judge_rubric,omitempty"`
	ResponseProvider    string               `json:"response_provider,omitempty"`
	ResponseModel       string               `json:"response_model,omitempty"`
	LatencyMS           int64                `json:"latency_ms"`
	FirstVisibleDeltaMS *int64               `json:"first_visible_delta_ms,omitempty"`
	StreamedChars       int                  `json:"streamed_chars"`
	InputTokens         *int                 `json:"input_tokens,omitempty"`
	CachedInputTokens   *int                 `json:"cached_input_tokens,omitempty"`
	OutputTokens        *int                 `json:"output_tokens,omitempty"`
	AssistantText       string               `json:"assistant_text"`
	ToolCalls           []EvaluationToolCall `json:"tool_calls"`
	Failure             string               `json:"failure,omitempty"`
}

type EvaluationToolCall struct {
	Name    string          `json:"name"`
	Payload json.RawMessage `json:"payload"`
}

// EvaluationCases selects the role cases and shared provider protocol cases.
func EvaluationCases(roles []string) []EvaluationCase {
	cases := evaluationCases()
	return slices.DeleteFunc(cases, func(c EvaluationCase) bool {
		return c.Category != EvaluationProtocolCategory && !slices.Contains(roles, c.Role)
	})
}

// RunEvaluationCase uses production provider conversion, source checks, and replay.
func RunEvaluationCase(ctx context.Context, generator provider.Generator, providerKind string,
	base provider.GenerateRequest, caseID string, contextTokens uint32,
) (EvaluationResult, error) {
	var selected *EvaluationCase
	for _, candidate := range evaluationCases() {
		if candidate.ID == caseID {
			selected = &candidate
			break
		}
	}
	if selected == nil {
		return EvaluationResult{}, fmt.Errorf("unknown evaluation case %s", caseID)
	}
	c := *selected
	result := EvaluationResult{CaseID: c.ID, Role: c.Role, Category: c.Category, Critical: c.Critical,
		JudgeRubric: c.JudgeRubric, ToolCalls: []EvaluationToolCall{}}
	request := c.request
	request.AccountID, request.Model, request.ReasoningEffort = base.AccountID, base.Model, base.ReasoningEffort
	request.MaxOutputTokens = &c.MaximumOutputTokens
	request.ConversationID = "conversation:evaluation"
	zero := float32(0)
	request.Temperature = &zero
	request.ParallelTools = false
	started := time.Now()
	inputTokens, cachedTokens, outputTokens := 0, 0, 0
	for step := 0; step < c.MaximumProviderCalls; step++ {
		if CountModelContext(ctx, generator, request.Messages, request.Tools, false) > contextTokens {
			return result, errors.New("evaluation input exceeds the planned context budget")
		}
		var streamed strings.Builder
		response, err := generator.Generate(ctx, request, func(event provider.StreamEvent) {
			if event.Kind != provider.TextDelta {
				return
			}
			if result.FirstVisibleDeltaMS == nil {
				elapsed := time.Since(started).Milliseconds()
				result.FirstVisibleDeltaMS = &elapsed
			}
			streamed.WriteString(event.Delta)
		})
		result.LatencyMS = time.Since(started).Milliseconds()
		result.StreamedChars += len([]rune(streamed.String()))
		if err != nil {
			if ctx.Err() != nil {
				return result, ctx.Err()
			}
			if errors.Is(err, provider.ErrAuthenticationRejected) || errors.Is(err, provider.ErrProviderRateLimited) || errors.Is(err, provider.ErrProviderPaymentRequired) {
				return result, err
			}
			result.Failure = err.Error()
			return result, nil
		}
		if result.ResponseModel != "" && result.ResponseModel != response.Model {
			result.Failure = "provider changed model identity during the case"
			return result, nil
		}
		result.ResponseProvider, result.ResponseModel = providerKind, response.Model
		inputTokens += response.Usage.InputTokens
		cachedTokens += response.Usage.CachedInputTokens
		outputTokens += response.Usage.OutputTokens
		if response.Usage.TotalTokens > 0 || inputTokens > 0 || outputTokens > 0 {
			result.InputTokens, result.CachedInputTokens, result.OutputTokens = &inputTokens, &cachedTokens, &outputTokens
		}
		result.AssistantText = boundedRunes(response.Text, 12000)
		for _, call := range response.ToolCalls {
			result.ToolCalls = append(result.ToolCalls, EvaluationToolCall{Name: call.Name, Payload: call.Payload})
			index := slices.IndexFunc(request.Tools, func(tool provider.GenerationTool) bool { return tool.Name == call.Name })
			if index < 0 {
				result.Failure = "provider returned an unavailable tool"
				return result, nil
			}
			if err := noemamcp.ValidateArguments(request.Tools[index].InputSchema, call.Payload); err != nil {
				result.Failure = err.Error()
				return result, nil
			}
		}
		var output json.RawMessage
		if len(c.steps) != 0 {
			output, err = gradeEvaluationStep(c, step, response)
		} else {
			err = gradeEvaluationResponse(c.expectation, response, streamed.String())
		}
		if err != nil {
			result.Failure = err.Error()
			return result, nil
		}
		if output == nil {
			result.Passed = true
			return result, nil
		}
		call := response.ToolCalls[0]
		request.Messages = append(request.Messages,
			provider.GenerationMessage{Role: "assistant", Content: response.Text, ReasoningDetails: generationReasoning(response), ToolCalls: []provider.ReplayToolCall{{
				ProviderItemID: call.ProviderItemID, ProviderCallID: call.ProviderCallID, Name: call.Name, ProviderName: call.ProviderName, Arguments: call.Payload}}},
			provider.GenerationMessage{Role: "tool", ToolResult: &provider.ReplayToolResult{
				ProviderCallID: call.ProviderCallID, Name: call.Name, ProviderName: call.ProviderName, Arguments: call.Payload, Success: true, Payload: output}})
	}
	result.Failure = "case exceeded its provider call budget"
	return result, nil
}
