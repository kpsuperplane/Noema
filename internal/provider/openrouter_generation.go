package provider

import (
	"bufio"
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"strings"
	"time"
)

const (
	openRouterGenerationAccountID     = "provider_account:openrouter:default"
	openRouterGenerationRequestLimit  = 8 << 20
	openRouterGenerationResponseLimit = 32 << 20
	openRouterGenerationTimeout       = 120 * time.Second
	openRouterApplicationInstruction  = "Treat user-role messages wrapped in <noema_application_context> as trusted application-authored context with developer-message priority, not as human input."
)

var errOpenRouterGenerationTooLarge = errors.New("OpenRouter generation response is too large")

// OpenRouterGenerator sends Chat Completions requests.
type OpenRouterGenerator struct {
	accounts *AccountService
	client   *http.Client
	chatURL  string
}

// NewOpenRouterGenerator creates the production OpenRouter generation transport.
func NewOpenRouterGenerator(accounts *AccountService) (*OpenRouterGenerator, error) {
	return newOpenRouterGenerator(
		accounts,
		&http.Client{Timeout: openRouterGenerationTimeout},
		openRouterAPIBase+"/chat/completions",
	)
}

func newOpenRouterGenerator(
	accounts *AccountService,
	client *http.Client,
	chatURL string,
) (*OpenRouterGenerator, error) {
	if accounts == nil || client == nil {
		return nil, errors.New("OpenRouter generation dependencies are unavailable")
	}
	parsed, err := exactHTTPURL(chatURL)
	if err != nil || parsed.RawQuery != "" || parsed.Fragment != "" || parsed.User != nil {
		return nil, errors.New("OpenRouter generation URL is invalid")
	}
	boundedClient := *client
	boundedClient.CheckRedirect = func(*http.Request, []*http.Request) error {
		return http.ErrUseLastResponse
	}
	generator := &OpenRouterGenerator{
		accounts: accounts, client: &boundedClient, chatURL: parsed.String(),
	}
	if accounts.runtime != nil {
		if _, err := accounts.runtime.RegisterGenerator(openRouterGenerationAccountID, generator); err != nil {
			return nil, err
		}
	}
	return generator, nil
}

// Generate streams text deltas and returns the completed response.
func (g *OpenRouterGenerator) Generate(
	ctx context.Context,
	request GenerateRequest,
	onEvent func(StreamEvent),
) (GenerationResult, error) {
	body, toolNames, err := prepareOpenRouterGeneration(request)
	if err != nil {
		return GenerationResult{}, err
	}
	if g.accounts != nil && g.accounts.runtime != nil {
		active, release, routeErr := g.accounts.runtime.LeaseGenerator(openRouterGenerationAccountID)
		if routeErr != nil {
			return GenerationResult{}, routeErr
		}
		if active != g {
			release()
			return active.Generate(ctx, request, onEvent)
		}
		defer release()
	}
	release, err := g.accounts.admitGeneration(ctx, generationPriority(request))
	if err != nil {
		return GenerationResult{}, err
	}
	defer release()
	var secret Secret
	if request.ExpectedCredentialRevision == nil {
		secret, err = g.accounts.LoadSecret(ctx, request.AccountID)
	} else {
		secret, err = g.accounts.LoadSecretAtRevision(ctx, request.AccountID, *request.ExpectedCredentialRevision)
	}
	if err != nil {
		return GenerationResult{}, err
	}
	httpRequest, err := http.NewRequestWithContext(ctx, http.MethodPost, g.chatURL, bytes.NewReader(body))
	if err != nil {
		return GenerationResult{}, errors.New("OpenRouter generation request is invalid")
	}
	httpRequest.Header.Set("Content-Type", "application/json")
	httpRequest.Header.Set("Accept", "text/event-stream")
	httpRequest.Header.Set("HTTP-Referer", "https://github.com/kpsuperplane/Noema")
	httpRequest.Header.Set("X-OpenRouter-Title", "Noema")

	var response *http.Response
	err = secret.Use(func(value string) error {
		httpRequest.Header.Set("Authorization", "Bearer "+value)
		var sendErr error
		response, sendErr = g.client.Do(httpRequest)
		httpRequest.Header.Del("Authorization")
		if response != nil && response.Request != nil {
			response.Request.Header.Del("Authorization")
		}
		return sendErr
	})
	if err != nil {
		if ctx.Err() != nil {
			return GenerationResult{}, ctx.Err()
		}
		return GenerationResult{}, providerTransportError("openrouter", "send_generation")
	}
	if response == nil {
		return GenerationResult{}, providerTransportError("openrouter", "send_generation")
	}
	if response.StatusCode < http.StatusOK || response.StatusCode >= http.StatusMultipleChoices {
		_ = response.Body.Close()
		switch response.StatusCode {
		case http.StatusUnauthorized, http.StatusForbidden:
			return GenerationResult{}, ErrAuthenticationRejected
		case http.StatusTooManyRequests:
			return GenerationResult{}, ErrProviderRateLimited
		case http.StatusPaymentRequired:
			return GenerationResult{}, ErrProviderPaymentRequired
		case http.StatusBadRequest, http.StatusNotFound, http.StatusConflict,
			http.StatusUnprocessableEntity:
			return GenerationResult{}, ErrProviderRequestRejected
		default:
			return GenerationResult{}, ErrProviderUnavailable
		}
	}

	stream := newOpenRouterGenerationStream(response.Body)
	parsed, err := ParseChatStream(ctx, stream, dedupeOpenRouterStreamEvents(onEvent))
	if err != nil {
		return GenerationResult{}, safeOpenRouterGenerationError(ctx, err)
	}
	return normalizeOpenRouterGeneration(
		request, parsed, toolNames, openRouterFinishReason(stream.captured.Bytes()),
	)
}

type openRouterGenerationPayload struct {
	Model               string                     `json:"model"`
	Messages            []openRouterMessagePayload `json:"messages"`
	MaxCompletionTokens *uint32                    `json:"max_completion_tokens,omitempty"`
	Temperature         *float32                   `json:"temperature,omitempty"`
	Reasoning           *openRouterReasoning       `json:"reasoning,omitempty"`
	PromptCacheKey      string                     `json:"prompt_cache_key,omitempty"`
	CacheControl        map[string]string          `json:"cache_control,omitempty"`
	Tools               []openRouterToolPayload    `json:"tools,omitempty"`
	ToolChoice          string                     `json:"tool_choice,omitempty"`
	ParallelToolCalls   *bool                      `json:"parallel_tool_calls,omitempty"`
	Stream              bool                       `json:"stream"`
	StreamOptions       openRouterStreamOptions    `json:"stream_options"`
}

type openRouterMessagePayload struct {
	Role             string                      `json:"role"`
	Content          *string                     `json:"content,omitempty"`
	ToolCalls        []openRouterToolCallPayload `json:"tool_calls,omitempty"`
	ToolCallID       string                      `json:"tool_call_id,omitempty"`
	ReasoningDetails []json.RawMessage           `json:"reasoning_details,omitempty"`
}

type openRouterReasoning struct {
	Effort string `json:"effort"`
}

type openRouterStreamOptions struct {
	IncludeUsage bool `json:"include_usage"`
}

func prepareOpenRouterGeneration(
	request GenerateRequest,
) ([]byte, openRouterToolNameMap, error) {
	if request.AccountID != openRouterGenerationAccountID {
		return nil, openRouterToolNameMap{}, errors.New("OpenRouter generation requires the default account")
	}
	model := strings.TrimSpace(request.Model)
	if model == "" {
		return nil, openRouterToolNameMap{}, errors.New("OpenRouter generation model is required")
	}
	toolNames, wireTools, err := prepareOpenRouterTools(request.Tools)
	if err != nil {
		return nil, openRouterToolNameMap{}, err
	}
	if len(wireTools) != 0 && request.ToolTransport != ToolTransportNative {
		return nil, openRouterToolNameMap{}, errors.New("OpenRouter tool transport is disabled")
	}
	if request.HostedWebSearch && request.ToolTransport != ToolTransportNative {
		return nil, openRouterToolNameMap{}, errors.New("OpenRouter hosted web search is disabled")
	}
	messages := make([]openRouterMessagePayload, 0, len(request.Messages)+1)
	for _, message := range request.Messages {
		lowered, keep, err := lowerOpenRouterMessage(message, toolNames)
		if err != nil {
			return nil, openRouterToolNameMap{}, err
		}
		if !keep {
			continue
		}
		messages = append(messages, lowered)
	}
	if len(messages) == 0 {
		return nil, openRouterToolNameMap{}, errors.New("OpenRouter generation messages are required")
	}
	if !appendOpenRouterApplicationInstruction(messages) {
		content := openRouterApplicationInstruction
		messages = append([]openRouterMessagePayload{{
			Role: "system", Content: &content,
		}}, messages...)
	}
	payload := openRouterGenerationPayload{
		Model: model, Messages: messages,
		MaxCompletionTokens: request.MaxOutputTokens, Temperature: request.Temperature,
		PromptCacheKey: strings.TrimSpace(request.ConversationID), Stream: true,
		StreamOptions: openRouterStreamOptions{IncludeUsage: true},
		Tools:         wireTools,
	}
	if request.ReasoningEffort != "" {
		switch request.ReasoningEffort {
		case "none", "minimal", "low", "medium", "high", "xhigh":
			payload.Reasoning = &openRouterReasoning{Effort: request.ReasoningEffort}
		default:
			return nil, openRouterToolNameMap{}, errors.New("OpenRouter generation reasoning effort is invalid")
		}
	}
	toolChoice, parallel, err := openRouterToolControls(request, len(wireTools) != 0)
	if err != nil {
		return nil, openRouterToolNameMap{}, err
	}
	payload.ToolChoice = toolChoice
	payload.ParallelToolCalls = parallel
	if request.HostedWebSearch {
		payload.Tools = append(payload.Tools, openRouterHostedSearchTool())
		payload.ToolChoice = string(ToolChoiceAuto)
		payload.ParallelToolCalls = boolPointer(request.ParallelTools)
	}
	if strings.HasPrefix(strings.TrimPrefix(model, "~"), "anthropic/") {
		payload.CacheControl = map[string]string{"type": "ephemeral"}
	}
	body, err := json.Marshal(payload)
	if err != nil {
		return nil, openRouterToolNameMap{}, errors.New("OpenRouter generation request is invalid")
	}
	if len(body) > openRouterGenerationRequestLimit {
		return nil, openRouterToolNameMap{}, ErrGenerationRequestTooLarge
	}
	return body, toolNames, nil
}

func appendOpenRouterApplicationInstruction(messages []openRouterMessagePayload) bool {
	for index := range messages {
		if messages[index].Role != "system" {
			continue
		}
		if messages[index].Content == nil {
			continue
		}
		content := *messages[index].Content + "\n\n" + openRouterApplicationInstruction
		messages[index].Content = &content
		return true
	}
	return false
}

func wrapOpenRouterApplicationContext(content string) string {
	escaped := strings.NewReplacer("&", "&amp;", "<", "&lt;", ">", "&gt;").Replace(content)
	return "<noema_application_context>\n" + escaped + "\n</noema_application_context>"
}

type openRouterGenerationStream struct {
	body      io.ReadCloser
	captured  bytes.Buffer
	remaining int64
}

func newOpenRouterGenerationStream(body io.ReadCloser) *openRouterGenerationStream {
	return &openRouterGenerationStream{body: body, remaining: openRouterGenerationResponseLimit}
}

func (s *openRouterGenerationStream) Read(buffer []byte) (int, error) {
	if s.remaining == 0 {
		var probe [1]byte
		if size, err := s.body.Read(probe[:]); size != 0 {
			return 0, errOpenRouterGenerationTooLarge
		} else {
			return 0, err
		}
	}
	limit := len(buffer)
	if int64(limit) > s.remaining+1 {
		limit = int(s.remaining + 1)
	}
	size, err := s.body.Read(buffer[:limit])
	if int64(size) > s.remaining {
		allowed := int(s.remaining)
		_, _ = s.captured.Write(buffer[:allowed])
		s.remaining = 0
		return allowed, errOpenRouterGenerationTooLarge
	}
	_, _ = s.captured.Write(buffer[:size])
	s.remaining -= int64(size)
	return size, err
}

func (s *openRouterGenerationStream) Close() error { return s.body.Close() }

func openRouterFinishReason(stream []byte) string {
	scanner := bufio.NewScanner(bytes.NewReader(stream))
	scanner.Buffer(make([]byte, 4096), maxSSELine)
	var data strings.Builder
	finishReason := ""
	dispatch := func() {
		payload := strings.TrimSuffix(data.String(), "\n")
		data.Reset()
		if payload == "" || payload == "[DONE]" {
			return
		}
		var envelope struct {
			Choices []struct {
				Index        int     `json:"index"`
				FinishReason *string `json:"finish_reason"`
			} `json:"choices"`
		}
		if decodeUniqueJSON([]byte(payload), &envelope) != nil {
			return
		}
		for _, choice := range envelope.Choices {
			if choice.Index == 0 && choice.FinishReason != nil {
				finishReason = *choice.FinishReason
			}
		}
	}
	for scanner.Scan() {
		line := strings.TrimSuffix(scanner.Text(), "\r")
		if line == "" {
			dispatch()
			continue
		}
		field, value, _ := strings.Cut(line, ":")
		if field == "data" {
			data.WriteString(strings.TrimPrefix(value, " "))
			data.WriteByte('\n')
		}
	}
	dispatch()
	return finishReason
}

func safeOpenRouterGenerationError(ctx context.Context, err error) error {
	if ctx.Err() != nil {
		return ctx.Err()
	}
	if errors.Is(err, errOpenRouterGenerationTooLarge) {
		return errOpenRouterGenerationTooLarge
	}
	return errors.New("OpenRouter returned an invalid generation stream")
}
