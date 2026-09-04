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

// OpenRouterChatMessage is one text-only Chat Completions message.
type OpenRouterChatMessage struct {
	Role    string
	Content string
}

// OpenRouterGenerateRequest is one text-only OpenRouter generation request.
type OpenRouterGenerateRequest struct {
	AccountID       string
	Model           string
	Messages        []OpenRouterChatMessage
	ReasoningEffort string
	MaxOutputTokens *uint32
	Temperature     *float32
	ConversationID  string
}

// OpenRouterGenerationResult is one completed text generation.
type OpenRouterGenerationResult struct {
	ID           string
	Model        string
	Text         string
	FinishReason string
	Usage        Usage
}

// OpenRouterGenerator sends text-only Chat Completions requests.
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
	return &OpenRouterGenerator{
		accounts: accounts, client: &boundedClient, chatURL: parsed.String(),
	}, nil
}

// Generate streams text deltas and returns the completed response.
func (g *OpenRouterGenerator) Generate(
	ctx context.Context,
	request OpenRouterGenerateRequest,
	onEvent func(StreamEvent),
) (OpenRouterGenerationResult, error) {
	body, err := openRouterGenerationBody(request)
	if err != nil {
		return OpenRouterGenerationResult{}, err
	}
	secret, err := g.accounts.LoadSecret(ctx, request.AccountID)
	if err != nil {
		return OpenRouterGenerationResult{}, err
	}
	httpRequest, err := http.NewRequestWithContext(ctx, http.MethodPost, g.chatURL, bytes.NewReader(body))
	if err != nil {
		return OpenRouterGenerationResult{}, errors.New("OpenRouter generation request is invalid")
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
			return OpenRouterGenerationResult{}, ctx.Err()
		}
		return OpenRouterGenerationResult{}, ErrProviderUnavailable
	}
	if response == nil {
		return OpenRouterGenerationResult{}, ErrProviderUnavailable
	}
	if response.StatusCode < http.StatusOK || response.StatusCode >= http.StatusMultipleChoices {
		_ = response.Body.Close()
		switch response.StatusCode {
		case http.StatusUnauthorized, http.StatusForbidden:
			return OpenRouterGenerationResult{}, ErrAuthenticationRejected
		case http.StatusTooManyRequests:
			return OpenRouterGenerationResult{}, ErrProviderRateLimited
		case http.StatusPaymentRequired:
			return OpenRouterGenerationResult{}, ErrProviderPaymentRequired
		case http.StatusBadRequest, http.StatusNotFound, http.StatusConflict,
			http.StatusUnprocessableEntity:
			return OpenRouterGenerationResult{}, ErrProviderRequestRejected
		default:
			return OpenRouterGenerationResult{}, ErrProviderUnavailable
		}
	}

	stream := newOpenRouterGenerationStream(response.Body)
	parsed, err := ParseChatStream(ctx, stream, onEvent)
	if err != nil {
		return OpenRouterGenerationResult{}, safeOpenRouterGenerationError(ctx, err)
	}
	if len(parsed.ToolCalls) != 0 || parsed.Text == "" {
		return OpenRouterGenerationResult{}, errors.New("OpenRouter response did not contain text")
	}
	if !validOpenRouterUsage(parsed.Usage) {
		return OpenRouterGenerationResult{}, errors.New("OpenRouter returned invalid token usage")
	}
	return OpenRouterGenerationResult{
		ID: parsed.ID, Model: parsed.Model, Text: parsed.Text,
		FinishReason: openRouterFinishReason(stream.captured.Bytes()), Usage: parsed.Usage,
	}, nil
}

func validOpenRouterUsage(usage Usage) bool {
	if usage.InputTokens < 0 || usage.CachedInputTokens < 0 || usage.OutputTokens < 0 ||
		usage.TotalTokens < 0 || usage.WebSearchRequests < 0 ||
		usage.CachedInputTokens > usage.InputTokens {
		return false
	}
	return usage.TotalTokens == 0 || usage.TotalTokens == usage.InputTokens+usage.OutputTokens
}

type openRouterGenerationPayload struct {
	Model               string                     `json:"model"`
	Messages            []openRouterMessagePayload `json:"messages"`
	MaxCompletionTokens *uint32                    `json:"max_completion_tokens,omitempty"`
	Temperature         *float32                   `json:"temperature,omitempty"`
	Reasoning           *openRouterReasoning       `json:"reasoning,omitempty"`
	PromptCacheKey      string                     `json:"prompt_cache_key,omitempty"`
	CacheControl        map[string]string          `json:"cache_control,omitempty"`
	Stream              bool                       `json:"stream"`
	StreamOptions       openRouterStreamOptions    `json:"stream_options"`
}

type openRouterMessagePayload struct {
	Role    string `json:"role"`
	Content string `json:"content"`
}

type openRouterReasoning struct {
	Effort string `json:"effort"`
}

type openRouterStreamOptions struct {
	IncludeUsage bool `json:"include_usage"`
}

func openRouterGenerationBody(request OpenRouterGenerateRequest) ([]byte, error) {
	if request.AccountID != openRouterGenerationAccountID {
		return nil, errors.New("OpenRouter generation requires the default account")
	}
	model := strings.TrimSpace(request.Model)
	if model == "" {
		return nil, errors.New("OpenRouter generation model is required")
	}
	messages := make([]openRouterMessagePayload, 0, len(request.Messages)+1)
	for _, message := range request.Messages {
		if strings.TrimSpace(message.Content) == "" {
			continue
		}
		switch message.Role {
		case "system", "user", "assistant":
			messages = append(messages, openRouterMessagePayload{Role: message.Role, Content: message.Content})
		case "developer":
			messages = append(messages, openRouterMessagePayload{
				Role: "user", Content: wrapOpenRouterApplicationContext(message.Content),
			})
		default:
			return nil, errors.New("OpenRouter generation message role is invalid")
		}
	}
	if len(messages) == 0 {
		return nil, errors.New("OpenRouter generation messages are required")
	}
	if !appendOpenRouterApplicationInstruction(messages) {
		messages = append([]openRouterMessagePayload{{
			Role: "system", Content: openRouterApplicationInstruction,
		}}, messages...)
	}
	payload := openRouterGenerationPayload{
		Model: model, Messages: messages,
		MaxCompletionTokens: request.MaxOutputTokens, Temperature: request.Temperature,
		PromptCacheKey: strings.TrimSpace(request.ConversationID), Stream: true,
		StreamOptions: openRouterStreamOptions{IncludeUsage: true},
	}
	if request.ReasoningEffort != "" {
		switch request.ReasoningEffort {
		case "none", "minimal", "low", "medium", "high", "xhigh":
			payload.Reasoning = &openRouterReasoning{Effort: request.ReasoningEffort}
		default:
			return nil, errors.New("OpenRouter generation reasoning effort is invalid")
		}
	}
	if strings.HasPrefix(strings.TrimPrefix(model, "~"), "anthropic/") {
		payload.CacheControl = map[string]string{"type": "ephemeral"}
	}
	body, err := json.Marshal(payload)
	if err != nil {
		return nil, errors.New("OpenRouter generation request is invalid")
	}
	if len(body) > openRouterGenerationRequestLimit {
		return nil, errors.New("OpenRouter generation request is too large")
	}
	return body, nil
}

func appendOpenRouterApplicationInstruction(messages []openRouterMessagePayload) bool {
	for index := range messages {
		if messages[index].Role != "system" {
			continue
		}
		messages[index].Content += "\n\n" + openRouterApplicationInstruction
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
