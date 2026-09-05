package provider

import (
	"bufio"
	"bytes"
	"context"
	"encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strconv"
	"strings"
	"time"
)

const (
	codexGenerationAccountID     = "provider_account:codex:default"
	codexGenerationRequestLimit  = 8 << 20
	codexGenerationResponseLimit = 32 << 20
	codexGenerationTimeout       = 300 * time.Second
	codexGenerationOriginator    = "codex_cli_rs"
)

var errCodexGenerationTooLarge = errors.New("Codex generation response is too large")

// CodexGenerator sends direct Codex Responses requests.
type CodexGenerator struct {
	accounts     *AccountService
	client       *http.Client
	responsesURL string
	tokenURL     string
}

// NewCodexGenerator creates the production Codex generation transport.
func NewCodexGenerator(accounts *AccountService) (*CodexGenerator, error) {
	return newCodexGenerator(
		accounts, &http.Client{Timeout: codexGenerationTimeout},
		strings.TrimRight(codexModelsBaseURL, "/")+"/responses",
	)
}

func newCodexGenerator(
	accounts *AccountService,
	client *http.Client,
	responsesURL string,
) (*CodexGenerator, error) {
	if accounts == nil || client == nil {
		return nil, errors.New("Codex generation dependencies are unavailable")
	}
	parsed, err := exactHTTPURL(responsesURL)
	if err != nil || parsed.RawQuery != "" || parsed.Fragment != "" || parsed.User != nil {
		return nil, errors.New("Codex generation URL is invalid")
	}
	boundedClient := *client
	boundedClient.CheckRedirect = func(*http.Request, []*http.Request) error {
		return http.ErrUseLastResponse
	}
	return &CodexGenerator{
		accounts: accounts, client: &boundedClient, responsesURL: parsed.String(), tokenURL: codexOAuthTokenURL,
	}, nil
}

// Generate streams text deltas and returns one completed Codex response.
func (g *CodexGenerator) Generate(
	ctx context.Context,
	request GenerateRequest,
	onEvent func(StreamEvent),
) (GenerationResult, error) {
	body, toolNames, err := prepareCodexGeneration(request)
	if err != nil {
		return GenerationResult{}, err
	}
	account, err := g.accounts.LoadAccount(ctx, request.AccountID)
	if err != nil {
		return GenerationResult{}, err
	}
	if request.ExpectedCredentialRevision != nil &&
		account.Metadata.CredentialRevision() != *request.ExpectedCredentialRevision {
		return GenerationResult{}, ErrAccountConflict
	}
	clientVersion := codexClientVersion(account.Metadata)
	tokens, err := g.accounts.RefreshCodexTokens(ctx, nil, time.Now(), g.refreshCodexTokens)
	if err != nil {
		if errors.Is(err, ErrAuthenticationRejected) {
			return GenerationResult{}, ErrAuthenticationRejected
		}
		return GenerationResult{}, err
	}
	if err := g.validateCredentialRevision(ctx, request.ExpectedCredentialRevision); err != nil {
		return GenerationResult{}, err
	}
	response, accessDigest, err := g.sendCodexGeneration(ctx, body, request, clientVersion, tokens)
	if err != nil {
		if ctx.Err() != nil {
			return GenerationResult{}, ctx.Err()
		}
		return GenerationResult{}, ErrProviderUnavailable
	}
	if response == nil {
		return GenerationResult{}, ErrProviderUnavailable
	}
	if response.StatusCode == http.StatusUnauthorized || response.StatusCode == http.StatusForbidden {
		_ = response.Body.Close()
		tokens, err = g.accounts.RefreshCodexTokens(ctx, &accessDigest, time.Now(), g.refreshCodexTokens)
		if err != nil {
			if errors.Is(err, ErrAuthenticationRejected) {
				return GenerationResult{}, ErrAuthenticationRejected
			}
			if errors.Is(err, ErrProviderRateLimited) {
				return GenerationResult{}, ErrProviderRateLimited
			}
			if ctx.Err() != nil {
				return GenerationResult{}, ctx.Err()
			}
			return GenerationResult{}, ErrProviderUnavailable
		}
		if err := g.validateCredentialRevision(ctx, request.ExpectedCredentialRevision); err != nil {
			return GenerationResult{}, err
		}
		response, _, err = g.sendCodexGeneration(ctx, body, request, clientVersion, tokens)
		if err != nil {
			if ctx.Err() != nil {
				return GenerationResult{}, ctx.Err()
			}
			return GenerationResult{}, ErrProviderUnavailable
		}
		if response == nil {
			return GenerationResult{}, ErrProviderUnavailable
		}
	}
	if response.StatusCode < http.StatusOK || response.StatusCode >= http.StatusMultipleChoices {
		_ = response.Body.Close()
		return GenerationResult{}, codexGenerationStatusError(response.StatusCode)
	}
	parsed, err := parseCodexGenerationStream(ctx, newCodexGenerationStream(response.Body), onEvent)
	if err != nil {
		if ctx.Err() != nil {
			return GenerationResult{}, ctx.Err()
		}
		if errors.Is(err, errCodexGenerationTooLarge) {
			return GenerationResult{}, errCodexGenerationTooLarge
		}
		return GenerationResult{}, errors.New("Codex returned an invalid generation stream")
	}
	return normalizeCodexGeneration(request, parsed, toolNames)
}

func (g *CodexGenerator) validateCredentialRevision(ctx context.Context, expected *uint64) error {
	if expected == nil {
		return nil
	}
	account, err := g.accounts.LoadAccount(ctx, codexGenerationAccountID)
	if err != nil {
		return err
	}
	if account.Metadata.CredentialRevision() != *expected {
		return ErrAccountConflict
	}
	return nil
}

func (g *CodexGenerator) sendCodexGeneration(
	ctx context.Context,
	body []byte,
	request GenerateRequest,
	clientVersion string,
	tokens CodexTokens,
) (*http.Response, codexAccessTokenDigest, error) {
	httpRequest, err := http.NewRequestWithContext(ctx, http.MethodPost, g.responsesURL, bytes.NewReader(body))
	if err != nil {
		return nil, codexAccessTokenDigest{}, errors.New("Codex generation request is invalid")
	}
	httpRequest.Header.Set("Content-Type", "application/json")
	httpRequest.Header.Set("Accept", "text/event-stream")
	httpRequest.Header.Set("originator", codexGenerationOriginator)
	httpRequest.Header.Set("version", clientVersion)
	httpRequest.Header.Set("User-Agent", codexGenerationOriginator+"/"+clientVersion+" (Noema)")
	if sessionID := strings.TrimSpace(request.ConversationID); sessionID != "" {
		if !validCodexHeader(sessionID) {
			return nil, codexAccessTokenDigest{}, errors.New("Codex generation conversation id is invalid")
		}
		httpRequest.Header.Set("session-id", sessionID)
	}

	var response *http.Response
	var usedAccessDigest codexAccessTokenDigest
	err = tokens.Use(func(accessToken string, _ string, _ uint64) error {
		usedAccessDigest = codexAccessTokenDigestFor(accessToken)
		httpRequest.Header.Set("Authorization", "Bearer "+accessToken)
		accountID, err := codexChatGPTAccountID(accessToken)
		if err != nil {
			httpRequest.Header.Del("Authorization")
			return err
		}
		if accountID != "" {
			httpRequest.Header.Set("ChatGPT-Account-ID", accountID)
		}
		var sendErr error
		response, sendErr = g.client.Do(httpRequest)
		httpRequest.Header.Del("Authorization")
		if response != nil && response.Request != nil {
			response.Request.Header.Del("Authorization")
		}
		return sendErr
	})
	if err != nil {
		return nil, codexAccessTokenDigest{}, err
	}
	return response, usedAccessDigest, nil
}

func (g *CodexGenerator) refreshCodexTokens(ctx context.Context, tokens CodexTokens) (CodexTokens, error) {
	var response *http.Response
	err := tokens.Use(func(_ string, refreshToken string, _ uint64) error {
		values := url.Values{
			"grant_type":    {"refresh_token"},
			"refresh_token": {refreshToken},
			"client_id":     {codexOAuthClientID},
		}
		httpRequest, err := http.NewRequestWithContext(ctx, http.MethodPost, g.tokenURL, strings.NewReader(values.Encode()))
		if err != nil {
			return ErrProviderUnavailable
		}
		httpRequest.Header.Set("Content-Type", "application/x-www-form-urlencoded")
		response, err = g.client.Do(httpRequest)
		return err
	})
	if err != nil {
		if ctx.Err() != nil {
			return CodexTokens{}, ctx.Err()
		}
		return CodexTokens{}, ErrProviderUnavailable
	}
	defer response.Body.Close()
	if response.StatusCode == http.StatusUnauthorized || response.StatusCode == http.StatusForbidden {
		return CodexTokens{}, ErrAuthenticationRejected
	}
	if response.StatusCode == http.StatusTooManyRequests {
		return CodexTokens{}, ErrProviderRateLimited
	}
	data, err := readCodexResponse(response.Body)
	if err != nil {
		return CodexTokens{}, ErrProviderUnavailable
	}
	if codexRefreshRejected(data) {
		return CodexTokens{}, ErrAuthenticationRejected
	}
	if response.StatusCode < http.StatusOK || response.StatusCode >= http.StatusMultipleChoices {
		return CodexTokens{}, ErrProviderUnavailable
	}
	var payload struct {
		AccessToken  string `json:"access_token"`
		RefreshToken string `json:"refresh_token"`
	}
	if decodeCodexRemoteJSON(data, &payload) != nil {
		return CodexTokens{}, ErrProviderUnavailable
	}
	var refreshed CodexTokens
	if err := tokens.Use(func(_ string, priorRefreshToken string, _ uint64) error {
		refreshToken := strings.TrimSpace(payload.RefreshToken)
		if refreshToken == "" {
			refreshToken = priorRefreshToken
		}
		refreshed = CodexTokens{accessToken: strings.TrimSpace(payload.AccessToken), refreshToken: refreshToken}
		return nil
	}); err != nil {
		return CodexTokens{}, err
	}
	return refreshed, nil
}

func codexRefreshRejected(data []byte) bool {
	var payload struct {
		Error json.RawMessage `json:"error"`
	}
	if decodeCodexRemoteJSON(data, &payload) != nil {
		return false
	}
	var code string
	if json.Unmarshal(payload.Error, &code) == nil {
		return codexRefreshRejectionCode(code)
	}
	var detail struct {
		Code    string `json:"code"`
		Message string `json:"message"`
	}
	if json.Unmarshal(payload.Error, &detail) != nil {
		return false
	}
	return codexRefreshRejectionCode(detail.Code) || codexRefreshRejectionCode(detail.Message)
}

func codexRefreshRejectionCode(value string) bool {
	value = strings.ToLower(value)
	return strings.Contains(value, "invalid_grant") || strings.Contains(value, "invalid_token") ||
		strings.Contains(value, "refresh_token_reused")
}

func codexGenerationStatusError(status int) error {
	switch status {
	case http.StatusUnauthorized, http.StatusForbidden:
		return ErrAuthenticationRejected
	case http.StatusTooManyRequests:
		return ErrProviderRateLimited
	case http.StatusPaymentRequired:
		return ErrProviderPaymentRequired
	case http.StatusBadRequest, http.StatusNotFound, http.StatusConflict,
		http.StatusUnprocessableEntity:
		return ErrProviderRequestRejected
	default:
		return ErrProviderUnavailable
	}
}

type codexGenerationPayload struct {
	Model                string                       `json:"model"`
	Input                []any                        `json:"input"`
	MaxOutputTokens      *uint32                      `json:"max_output_tokens,omitempty"`
	PreviousResponseID   string                       `json:"previous_response_id,omitempty"`
	Temperature          *float32                     `json:"temperature,omitempty"`
	Reasoning            *codexReasoning              `json:"reasoning,omitempty"`
	ServiceTier          string                       `json:"service_tier,omitempty"`
	Tools                []codexToolPayload           `json:"tools,omitempty"`
	Include              []string                     `json:"include,omitempty"`
	ToolChoice           string                       `json:"tool_choice,omitempty"`
	ParallelToolCalls    *bool                        `json:"parallel_tool_calls,omitempty"`
	PromptCacheKey       string                       `json:"prompt_cache_key,omitempty"`
	PromptCacheOptions   *responsesPromptCacheOptions `json:"prompt_cache_options,omitempty"`
	PromptCacheRetention string                       `json:"prompt_cache_retention,omitempty"`
	Store                bool                         `json:"store"`
	Stream               bool                         `json:"stream"`
}

type responsesPromptCacheOptions struct {
	Mode string `json:"mode"`
	TTL  string `json:"ttl"`
}

type codexReasoning struct {
	Effort  string `json:"effort"`
	Summary string `json:"summary,omitempty"`
}

type codexToolPayload struct {
	Type              string `json:"type"`
	Name              string `json:"name,omitempty"`
	Description       string `json:"description,omitempty"`
	Parameters        any    `json:"parameters,omitempty"`
	Strict            *bool  `json:"strict,omitempty"`
	ExternalWebAccess *bool  `json:"external_web_access,omitempty"`
}

func prepareCodexGeneration(
	request GenerateRequest,
) ([]byte, openRouterToolNameMap, error) {
	return prepareResponsesGeneration(request, responsesGenerationProfile{
		accountID: codexGenerationAccountID, providerName: "Codex", stream: true,
	})
}

type responsesGenerationProfile struct {
	accountID, providerName, promptCacheRetention string
	forwardMaxOutput, includeEncryptedReasoning   bool
	promptCacheOptions, stream                    bool
}

func prepareResponsesGeneration(
	request GenerateRequest,
	profile responsesGenerationProfile,
) ([]byte, openRouterToolNameMap, error) {
	if request.AccountID != profile.accountID {
		return nil, openRouterToolNameMap{}, fmt.Errorf("%s generation requires the default account", profile.providerName)
	}
	model := strings.TrimSpace(request.Model)
	if model == "" {
		return nil, openRouterToolNameMap{}, fmt.Errorf("%s generation model is required", profile.providerName)
	}
	previousResponseID := strings.TrimSpace(request.PreviousResponseID)
	if previousResponseID != request.PreviousResponseID ||
		(previousResponseID != "" && (!validCodexHeader(previousResponseID) || !request.StoreResponse)) {
		return nil, openRouterToolNameMap{}, fmt.Errorf("%s previous response id is invalid", profile.providerName)
	}
	if request.HostedWebSearch && request.ToolTransport != ToolTransportNative {
		return nil, openRouterToolNameMap{}, fmt.Errorf("%s hosted web search is disabled", profile.providerName)
	}
	toolNames, chatTools, err := prepareOpenRouterTools(request.Tools)
	if err != nil {
		return nil, openRouterToolNameMap{}, fmt.Errorf("%s tool catalog is invalid", profile.providerName)
	}
	if len(chatTools) != 0 && request.ToolTransport != ToolTransportNative {
		return nil, openRouterToolNameMap{}, fmt.Errorf("%s tool transport is disabled", profile.providerName)
	}
	tools := make([]codexToolPayload, 0, len(chatTools))
	for _, tool := range chatTools {
		function := tool.Function
		tools = append(tools, codexToolPayload{
			Type: "function", Name: function.Name, Description: function.Description,
			Parameters: function.Parameters, Strict: boolPointer(function.Strict),
		})
	}
	if request.HostedWebSearch {
		tools = append(tools, codexToolPayload{Type: "web_search", ExternalWebAccess: boolPointer(true)})
	}
	explicitPromptCache := profile.promptCacheOptions && openAIGPT56Model(model)
	cacheBreakpoints := make(map[int]struct{}, 4)
	if explicitPromptCache {
		for index := len(request.Messages) - 1; index >= 0 && len(cacheBreakpoints) < 4; index-- {
			message := request.Messages[index]
			if message.Role == "developer" && strings.TrimSpace(message.Content) != "" {
				cacheBreakpoints[index] = struct{}{}
			}
		}
	}
	input := make([]any, 0, len(request.Messages)*2)
	for index, message := range request.Messages {
		lowered, err := lowerCodexMessage(message, toolNames)
		if err != nil {
			return nil, openRouterToolNameMap{}, fmt.Errorf("%s generation history is invalid", profile.providerName)
		}
		if _, ok := cacheBreakpoints[index]; ok {
			lowered = []any{map[string]any{
				"role": "developer",
				"content": []any{map[string]any{
					"type": "input_text", "text": message.Content,
					"prompt_cache_breakpoint": map[string]any{"mode": "explicit"},
				}},
			}}
		}
		input = append(input, lowered...)
	}
	if len(input) == 0 {
		return nil, openRouterToolNameMap{}, fmt.Errorf("%s generation messages are required", profile.providerName)
	}
	payload := codexGenerationPayload{
		Model: model, Input: input, Temperature: request.Temperature,
		PreviousResponseID:   previousResponseID,
		PromptCacheKey:       strings.TrimSpace(request.ConversationID),
		PromptCacheRetention: profile.promptCacheRetention,
		Store:                request.StoreResponse,
		Stream:               profile.stream,
		Tools:                tools,
	}
	if explicitPromptCache {
		payload.PromptCacheOptions = &responsesPromptCacheOptions{Mode: "explicit", TTL: "30m"}
		payload.PromptCacheRetention = ""
	}
	if profile.forwardMaxOutput {
		payload.MaxOutputTokens = request.MaxOutputTokens
	}
	if profile.includeEncryptedReasoning {
		payload.Include = append(payload.Include, "reasoning.encrypted_content")
	}
	if request.HostedWebSearch {
		payload.Include = append(payload.Include, "web_search_call.action.sources")
	}
	if request.ReasoningEffort != "" {
		switch request.ReasoningEffort {
		case "none", "minimal", "low", "medium", "high", "xhigh":
			payload.Reasoning = &codexReasoning{Effort: request.ReasoningEffort}
			if request.ReasoningEffort != "none" {
				payload.Reasoning.Summary = "auto"
			}
		default:
			return nil, openRouterToolNameMap{}, fmt.Errorf("%s generation reasoning effort is invalid", profile.providerName)
		}
	}
	choice, parallel, err := responsesToolControls(request, len(tools) != 0, profile.providerName)
	if err != nil {
		return nil, openRouterToolNameMap{}, err
	}
	payload.ToolChoice, payload.ParallelToolCalls = choice, parallel
	if request.HostedWebSearch {
		payload.ToolChoice = string(ToolChoiceAuto)
	}
	if request.FastMode {
		payload.ServiceTier = "priority"
	}
	body, err := json.Marshal(payload)
	if err != nil {
		return nil, openRouterToolNameMap{}, fmt.Errorf("%s generation request is invalid", profile.providerName)
	}
	if len(body) > codexGenerationRequestLimit {
		return nil, openRouterToolNameMap{}, ErrGenerationRequestTooLarge
	}
	return body, toolNames, nil
}

func openAIGPT56Model(model string) bool {
	return model == "gpt-5.6" || strings.HasPrefix(model, "gpt-5.6-")
}

func responsesToolControls(request GenerateRequest, hasTools bool, providerName string) (string, *bool, error) {
	choice := request.ToolChoice
	if choice == "" {
		choice = ToolChoiceAuto
	}
	if choice != ToolChoiceAuto && choice != ToolChoiceNone && choice != ToolChoiceRequired {
		return "", nil, fmt.Errorf("%s tool choice is invalid", providerName)
	}
	if !hasTools {
		return "", nil, nil
	}
	return string(choice), boolPointer(request.ParallelTools), nil
}

func lowerCodexMessage(message GenerationMessage, names openRouterToolNameMap) ([]any, error) {
	if message.Role == "hosted_web_search" {
		return lowerCodexHostedSearch(message)
	}
	if message.Role != "system" && message.Role != "developer" && message.Role != "user" &&
		message.Role != "assistant" && message.Role != "tool" {
		return nil, errors.New("Codex generation message role is invalid")
	}
	if message.Role == "tool" {
		return lowerCodexToolResult(message, names)
	}
	if (message.Role == "system" || message.Role == "developer" || message.Role == "user") &&
		(len(message.ToolCalls) != 0 || message.ToolResult != nil || len(message.ReasoningDetails) != 0 ||
			message.EncryptedReasoning != "") {
		return nil, errors.New("Codex generation message fields are invalid")
	}
	items := make([]any, 0, 2+len(message.ToolCalls))
	if message.Role == "assistant" {
		reasoning, err := lowerCodexReplayReasoning(message)
		if err != nil {
			return nil, err
		}
		items = append(items, reasoning...)
	}
	if strings.TrimSpace(message.Content) != "" {
		if message.Role == "assistant" {
			phase := "final_answer"
			if len(message.ToolCalls) != 0 {
				phase = "commentary"
			}
			items = append(items, map[string]any{
				"type": "message", "status": "completed", "role": "assistant", "phase": phase,
				"content": []any{map[string]any{
					"type": "output_text", "text": message.Content, "annotations": []any{},
				}},
			})
		} else {
			items = append(items, map[string]any{"role": message.Role, "content": message.Content})
		}
	}
	if message.Role == "assistant" {
		for _, call := range message.ToolCalls {
			item, err := lowerCodexReplayCall(call, names)
			if err != nil {
				return nil, err
			}
			items = append(items, item)
		}
	}
	return items, nil
}

func lowerCodexHostedSearch(message GenerationMessage) ([]any, error) {
	if message.HostedSearch == nil || message.Content != "" || len(message.ToolCalls) != 0 ||
		message.ToolResult != nil || len(message.ReasoningDetails) != 0 || message.EncryptedReasoning != "" {
		return nil, errors.New("Codex hosted web replay is invalid")
	}
	search := message.HostedSearch
	action, err := decodeOptionalOpenRouterJSON(search.ProviderAction)
	if err != nil {
		return nil, errors.New("Codex hosted web action is invalid")
	}
	if action == nil {
		action, err = decodeOptionalOpenRouterJSON(search.Arguments)
		if err != nil {
			return nil, errors.New("Codex hosted web arguments are invalid")
		}
		object := jsonObject(action)
		if object == nil {
			object = map[string]any{}
		}
		kind := "search"
		if search.Name == "web.fetch" {
			kind = "open_page"
		}
		object["type"] = kind
		action = object
	}
	if jsonObject(action) == nil {
		return nil, errors.New("Codex hosted web action must be an object")
	}
	item := map[string]any{
		"type": "web_search_call", "status": search.Status, "action": action,
	}
	if strings.TrimSpace(search.ID) != "" {
		item["id"] = search.ID
	}
	return []any{item}, nil
}

func lowerCodexReplayReasoning(message GenerationMessage) ([]any, error) {
	if len(message.ReasoningDetails) == 0 {
		if strings.TrimSpace(message.EncryptedReasoning) == "" {
			return nil, nil
		}
		item := map[string]any{
			"type": "reasoning", "summary": []any{}, "encrypted_content": message.EncryptedReasoning,
		}
		if strings.TrimSpace(message.ReasoningID) != "" {
			item["id"] = message.ReasoningID
		}
		return []any{item}, nil
	}
	items := make([]any, 0, len(message.ReasoningDetails))
	for _, raw := range message.ReasoningDetails {
		value, err := decodeOpenRouterJSON(raw)
		if err != nil {
			return nil, errors.New("Codex replay reasoning is invalid")
		}
		object := jsonObject(value)
		kind := jsonString(object["type"])
		encrypted := firstJSONText(object, "encrypted_content")
		if kind == "reasoning.encrypted" {
			encrypted = firstJSONText(object, "data", "encrypted_content")
		}
		if encrypted == "" {
			continue
		}
		item := map[string]any{
			"type": "reasoning", "summary": []any{}, "encrypted_content": encrypted,
		}
		if id := jsonString(object["id"]); id != "" {
			item["id"] = id
		}
		items = append(items, item)
	}
	return items, nil
}

func lowerCodexReplayCall(call ReplayToolCall, names openRouterToolNameMap) (any, error) {
	if strings.TrimSpace(call.ProviderCallID) == "" ||
		len(call.ProviderCallID) > openRouterProviderCallIDLimit || strings.TrimSpace(call.Name) == "" {
		return nil, errors.New("Codex replay tool call is invalid")
	}
	providerName := strings.TrimSpace(call.ProviderName)
	if providerName == "" {
		providerName = names.canonicalToName[call.Name]
		if providerName == "" {
			providerName = openRouterProviderSafeName(call.Name)
		}
	}
	arguments, err := openRouterJSONObject(call.Arguments)
	if err != nil {
		return nil, errors.New("Codex replay tool arguments must be an object")
	}
	encoded, err := json.Marshal(arguments)
	if err != nil {
		return nil, errors.New("Codex replay tool arguments are invalid")
	}
	item := map[string]any{
		"type": "function_call", "call_id": call.ProviderCallID,
		"name": providerName, "arguments": string(encoded),
	}
	if strings.TrimSpace(call.ProviderItemID) != "" {
		item["id"] = call.ProviderItemID
	}
	return item, nil
}

func lowerCodexToolResult(message GenerationMessage, names openRouterToolNameMap) ([]any, error) {
	if len(message.ToolCalls) != 0 || len(message.ReasoningDetails) != 0 ||
		message.EncryptedReasoning != "" || message.ToolResult == nil {
		return nil, errors.New("Codex tool result message is invalid")
	}
	result := message.ToolResult
	if strings.TrimSpace(result.ProviderCallID) == "" ||
		len(result.ProviderCallID) > openRouterProviderCallIDLimit || strings.TrimSpace(result.Name) == "" {
		return nil, errors.New("Codex tool result is invalid")
	}
	providerName := strings.TrimSpace(result.ProviderName)
	if providerName == "" {
		providerName = names.canonicalToName[result.Name]
	}
	payload, err := decodeOptionalOpenRouterJSON(result.Payload)
	if err != nil {
		return nil, errors.New("Codex tool result payload is invalid")
	}
	output, err := json.Marshal(map[string]any{
		"call_id": result.ProviderCallID, "name": result.Name,
		"provider_name": optionalString(providerName), "success": result.Success, "payload": payload,
	})
	if err != nil {
		return nil, errors.New("Codex tool result is invalid")
	}
	return []any{map[string]any{
		"type": "function_call_output", "call_id": result.ProviderCallID, "output": string(output),
	}}, nil
}

func optionalString(value string) any {
	if value == "" {
		return nil
	}
	return value
}

func codexClientVersion(metadata AccountMetadata) string {
	var version string
	if json.Unmarshal(metadata["models_client_version"], &version) == nil && validCodexClientVersion(version) {
		return strings.TrimSpace(version)
	}
	return codexFallbackClientVersion
}

func validCodexHeader(value string) bool {
	if len(value) > 1024 {
		return false
	}
	for _, character := range []byte(value) {
		if character < 32 || character > 126 {
			return false
		}
	}
	return true
}

func codexChatGPTAccountID(token string) (string, error) {
	parts := strings.Split(token, ".")
	if len(parts) < 2 {
		return "", nil
	}
	data, err := base64.RawURLEncoding.DecodeString(parts[1])
	if err != nil {
		data, err = base64.URLEncoding.DecodeString(parts[1])
	}
	if err != nil {
		return "", nil
	}
	var claims map[string]any
	if json.Unmarshal(data, &claims) != nil {
		return "", nil
	}
	if auth := jsonObject(claims["https://api.openai.com/auth"]); auth != nil {
		if id := strings.TrimSpace(jsonString(auth["chatgpt_account_id"])); id != "" {
			if !validCodexHeader(id) {
				return "", errors.New("Codex ChatGPT account id is invalid")
			}
			return id, nil
		}
	}
	id := strings.TrimSpace(jsonString(claims["chatgpt_account_id"]))
	if id != "" && !validCodexHeader(id) {
		return "", errors.New("Codex ChatGPT account id is invalid")
	}
	return id, nil
}

type codexGenerationStream struct {
	body      io.ReadCloser
	remaining int64
}

func newCodexGenerationStream(body io.ReadCloser) *codexGenerationStream {
	return &codexGenerationStream{body: body, remaining: codexGenerationResponseLimit}
}

func (stream *codexGenerationStream) Read(buffer []byte) (int, error) {
	if stream.remaining == 0 {
		var probe [1]byte
		if size, err := stream.body.Read(probe[:]); size != 0 {
			return 0, errCodexGenerationTooLarge
		} else {
			return 0, err
		}
	}
	limit := len(buffer)
	if int64(limit) > stream.remaining+1 {
		limit = int(stream.remaining + 1)
	}
	size, err := stream.body.Read(buffer[:limit])
	if int64(size) > stream.remaining {
		allowed := int(stream.remaining)
		stream.remaining = 0
		return allowed, errCodexGenerationTooLarge
	}
	stream.remaining -= int64(size)
	return size, err
}

func (stream *codexGenerationStream) Close() error { return stream.body.Close() }

type codexStreamResult struct {
	ID, Model, FinishReason string
	Usage                   Usage
	Output                  []codexOutputItem
}

type codexOutputItem struct {
	Index int
	Raw   json.RawMessage
}

func parseCodexGenerationStream(
	ctx context.Context,
	reader io.ReadCloser,
	onEvent func(StreamEvent),
) (codexStreamResult, error) {
	if onEvent == nil {
		onEvent = func(StreamEvent) {}
	}
	stopClose := context.AfterFunc(ctx, func() { _ = reader.Close() })
	defer stopClose()
	defer reader.Close()
	scanner := bufio.NewScanner(&contextReader{ctx: ctx, reader: reader})
	scanner.Buffer(make([]byte, 4096), maxSSELine)
	result := codexStreamResult{}
	output := make(map[int]json.RawMessage)
	text := make(map[int]string)
	hostedStarted := make(map[int]struct{})
	var eventType string
	var data strings.Builder
	dispatch := func() error {
		defer func() { eventType = ""; data.Reset() }()
		payload := strings.TrimSuffix(data.String(), "\n")
		if payload == "" || payload == "[DONE]" {
			return nil
		}
		if len(payload) > maxSSEEvent {
			return errors.New("Codex SSE event is too large")
		}
		return consumeCodexGenerationEvent(
			eventType, []byte(payload), &result, output, text, hostedStarted, onEvent,
		)
	}
	for scanner.Scan() {
		if err := ctx.Err(); err != nil {
			return codexStreamResult{}, err
		}
		line := strings.TrimSuffix(scanner.Text(), "\r")
		if line == "" {
			if err := dispatch(); err != nil {
				return codexStreamResult{}, err
			}
			continue
		}
		field, value, _ := strings.Cut(line, ":")
		value = strings.TrimPrefix(value, " ")
		switch field {
		case "event":
			eventType = value
		case "data":
			data.WriteString(value)
			data.WriteByte('\n')
		}
	}
	if err := scanner.Err(); err != nil {
		return codexStreamResult{}, err
	}
	if err := dispatch(); err != nil {
		return codexStreamResult{}, err
	}
	if result.FinishReason == "" {
		return codexStreamResult{}, errors.New("Codex stream did not complete")
	}
	if err := reconcileCodexStreamText(output, text); err != nil {
		return codexStreamResult{}, err
	}
	for index := 0; index < maxItems; index++ {
		if item, exists := output[index]; exists {
			result.Output = append(result.Output, codexOutputItem{Index: index, Raw: item})
		}
	}
	return result, nil
}

func reconcileCodexStreamText(output map[int]json.RawMessage, text map[int]string) error {
	for textIndex := 0; textIndex < maxItems; textIndex++ {
		value, exists := text[textIndex]
		if !exists {
			continue
		}
		if item, exists := output[textIndex]; exists {
			var object map[string]any
			if err := decodeUniqueJSON(item, &object); err != nil {
				return errors.New("Codex streamed message is invalid")
			}
			if object["type"] == "message" {
				if content, ok := object["content"].([]any); ok {
					for _, rawPart := range content {
						part, ok := rawPart.(map[string]any)
						if ok && part["type"] == "output_text" {
							part["text"] = value
							encoded, err := json.Marshal(object)
							if err != nil {
								return errors.New("Codex streamed message is invalid")
							}
							output[textIndex] = encoded
							value = ""
							break
						}
					}
				}
			}
		}
		if value == "" {
			continue
		}
		index := textIndex
		for index < maxItems {
			if _, exists := output[index]; !exists {
				break
			}
			index++
		}
		if index == maxItems {
			return errors.New("Codex response has too many output items")
		}
		encoded, _ := json.Marshal(map[string]any{
			"type": "message", "content": []any{map[string]any{
				"type": "output_text", "text": value,
			}},
		})
		output[index] = encoded
	}
	return nil
}

func consumeCodexGenerationEvent(
	framingType string,
	payload []byte,
	result *codexStreamResult,
	output map[int]json.RawMessage,
	text map[int]string,
	hostedStarted map[int]struct{},
	onEvent func(StreamEvent),
) error {
	var event map[string]json.RawMessage
	if decodeUniqueJSON(payload, &event) != nil {
		return errors.New("Codex SSE event is invalid")
	}
	eventType, _ := rawString(event["type"])
	if eventType == "" {
		eventType = framingType
	}
	switch eventType {
	case "response.output_text.delta":
		delta, _ := rawString(event["delta"])
		index := codexOutputIndex(event)
		text[index] += delta
		if delta != "" {
			onEvent(StreamEvent{Kind: TextDelta, Index: index, Delta: delta})
		}
	case "response.output_item.added":
		var item map[string]json.RawMessage
		if json.Unmarshal(event["item"], &item) == nil {
			kind, _ := rawString(item["type"])
			if kind == "function_call" {
				callID, _ := rawString(item["call_id"])
				name, _ := rawString(item["name"])
				if strings.TrimSpace(callID) != "" && strings.TrimSpace(name) != "" {
					onEvent(StreamEvent{
						Kind: ToolCallStarted, Index: codexOutputIndex(event), ID: callID, Name: name,
					})
				}
			} else if kind == "web_search_call" {
				id, _ := rawString(item["id"])
				emitCodexHostedSearchStart(codexOutputIndex(event), id, hostedStarted, onEvent)
			}
		}
	case "response.web_search_call.in_progress", "response.web_search_call.searching":
		id, _ := rawString(event["item_id"])
		emitCodexHostedSearchStart(codexOutputIndex(event), id, hostedStarted, onEvent)
	case "response.output_item.done":
		if len(output) >= maxItems {
			return errors.New("Codex response has too many output items")
		}
		if item := event["item"]; len(item) != 0 && string(item) != "null" {
			index, err := codexCompletedOutputIndex(event, output)
			if err != nil {
				return err
			}
			output[index] = append(json.RawMessage(nil), item...)
		}
	case "response.completed", "response.incomplete":
		if err := collectCodexTerminal(event["response"], result, output); err != nil {
			return err
		}
		if eventType == "response.completed" {
			result.FinishReason = "stop"
		} else {
			result.FinishReason = "incomplete"
		}
	case "response.failed", "error":
		return errors.New("Codex generation failed")
	}
	return nil
}

func emitCodexHostedSearchStart(
	index int,
	id string,
	seen map[int]struct{},
	onEvent func(StreamEvent),
) {
	if _, exists := seen[index]; exists {
		return
	}
	seen[index] = struct{}{}
	onEvent(StreamEvent{Kind: HostedSearchStarted, Index: index, ID: id, Name: "web.search"})
}

func codexOutputIndex(event map[string]json.RawMessage) int {
	var index int
	if json.Unmarshal(event["output_index"], &index) != nil || index < 0 || index >= maxItems {
		return 0
	}
	return index
}

func codexCompletedOutputIndex(
	event map[string]json.RawMessage,
	output map[int]json.RawMessage,
) (int, error) {
	raw := event["output_index"]
	if len(raw) == 0 || string(raw) == "null" {
		for index := 0; index < maxItems; index++ {
			if _, exists := output[index]; !exists {
				return index, nil
			}
		}
		return 0, errors.New("Codex response has too many output items")
	}
	var index int
	if json.Unmarshal(raw, &index) != nil || index < 0 || index >= maxItems {
		return 0, errors.New("Codex output item index is invalid")
	}
	if _, exists := output[index]; exists {
		return 0, errors.New("Codex output item index is duplicated")
	}
	return index, nil
}

func collectCodexTerminal(
	raw json.RawMessage,
	result *codexStreamResult,
	output map[int]json.RawMessage,
) error {
	var response struct {
		ID     string            `json:"id"`
		Model  string            `json:"model"`
		Output []json.RawMessage `json:"output"`
		Usage  struct {
			InputTokens  int `json:"input_tokens"`
			OutputTokens int `json:"output_tokens"`
			TotalTokens  int `json:"total_tokens"`
			InputDetails struct {
				CachedTokens int `json:"cached_tokens"`
			} `json:"input_tokens_details"`
		} `json:"usage"`
	}
	if decodeUniqueJSON(raw, &response) != nil {
		return errors.New("Codex terminal response is invalid")
	}
	result.ID, result.Model = response.ID, response.Model
	result.Usage = Usage{
		InputTokens: response.Usage.InputTokens, CachedInputTokens: response.Usage.InputDetails.CachedTokens,
		OutputTokens: response.Usage.OutputTokens, TotalTokens: response.Usage.TotalTokens,
	}
	if len(response.Output) > maxItems {
		return errors.New("Codex response has too many output items")
	}
	for index, item := range response.Output {
		if _, exists := output[index]; !exists {
			output[index] = append(json.RawMessage(nil), item...)
		}
	}
	return nil
}

func normalizeCodexGeneration(
	request GenerateRequest,
	parsed codexStreamResult,
	names openRouterToolNameMap,
) (GenerationResult, error) {
	result := GenerationResult{
		ID: parsed.ID, Model: strings.TrimSpace(parsed.Model),
		FinishReason: parsed.FinishReason, Usage: parsed.Usage,
	}
	if result.Model == "" {
		result.Model = strings.TrimSpace(request.Model)
	}
	for _, output := range parsed.Output {
		raw := output.Raw
		var item map[string]json.RawMessage
		if decodeUniqueJSON(raw, &item) != nil {
			return GenerationResult{}, errors.New("Codex output item is invalid")
		}
		kind, _ := rawString(item["type"])
		switch kind {
		case "message":
			value, citations, err := codexOutputText(item, utf16CodeUnits(result.Text))
			if err != nil {
				return GenerationResult{}, err
			}
			result.Text += value
			result.Citations = append(result.Citations, citations...)
		case "function_call":
			if len(result.ToolCalls) != 0 {
				return GenerationResult{}, errors.New("Codex returned more than one native tool call")
			}
			call, err := normalizeCodexToolCall(output.Index, item, request.ToolTransport, names)
			if err != nil {
				return GenerationResult{}, err
			}
			result.ToolCalls = append(result.ToolCalls, call)
		case "reasoning":
			reasoning, keep, err := normalizeCodexReasoning(raw, item)
			if err != nil {
				return GenerationResult{}, err
			}
			if keep {
				result.Reasoning = append(result.Reasoning, reasoning)
			}
		case "web_search_call":
			if len(result.Searches) >= maxHostedSearches {
				return GenerationResult{}, errors.New("Codex response has too many hosted web searches")
			}
			search, err := normalizeCodexHostedSearch(output.Index, item)
			if err != nil {
				return GenerationResult{}, err
			}
			result.Searches = append(result.Searches, search)
		}
	}
	if result.Text == "" && len(result.ToolCalls) == 0 {
		return GenerationResult{}, errors.New("Codex response did not contain assistant content or tool calls")
	}
	if len(result.ToolCalls) != 0 {
		result.FinishReason = "tool_calls"
	}
	return result, nil
}

func codexOutputText(item map[string]json.RawMessage, baseOffset int) (string, []Citation, error) {
	var content []struct {
		Type, Text, Refusal string
		Annotations         []struct {
			Type, Title, URL string
			StartIndex       *int `json:"start_index"`
			EndIndex         *int `json:"end_index"`
		} `json:"annotations"`
	}
	if json.Unmarshal(item["content"], &content) != nil {
		return "", nil, errors.New("Codex message output is invalid")
	}
	var result strings.Builder
	var citations []Citation
	seen := make(map[string]struct{})
	offset := baseOffset
	for _, part := range content {
		if part.Type == "output_text" {
			for _, annotation := range part.Annotations {
				if annotation.Type != "url_citation" {
					continue
				}
				url, err := safeCitationURL(annotation.URL)
				if err != nil {
					continue
				}
				start, end := addCitationOffset(annotation.StartIndex, offset), addCitationOffset(annotation.EndIndex, offset)
				key := url + "\x00" + pointerKey(start) + "\x00" + pointerKey(end)
				if _, exists := seen[key]; exists {
					continue
				}
				seen[key] = struct{}{}
				title := strings.TrimSpace(annotation.Title)
				if title == "" {
					title = url
				}
				citations = append(citations, Citation{Title: title, URL: url, StartIndex: start, EndIndex: end})
			}
			result.WriteString(part.Text)
			offset += utf16CodeUnits(part.Text)
		}
	}
	return result.String(), citations, nil
}

func addCitationOffset(value *int, offset int) *int {
	if value == nil || *value < 0 {
		return nil
	}
	result := *value + offset
	return &result
}

func pointerKey(value *int) string {
	if value == nil {
		return ""
	}
	return strconv.Itoa(*value)
}

func utf16CodeUnits(value string) int {
	count := 0
	for _, character := range value {
		count++
		if character > 0xffff {
			count++
		}
	}
	return count
}

func normalizeCodexHostedSearch(index int, item map[string]json.RawMessage) (HostedSearch, error) {
	id, _ := rawString(item["id"])
	status, _ := rawString(item["status"])
	action, err := decodeOptionalOpenRouterJSON(item["action"])
	if err != nil {
		return HostedSearch{}, errors.New("Codex hosted web action is invalid")
	}
	object := jsonObject(action)
	if object == nil {
		object = map[string]any{}
		action = object
	}
	name := "web.search"
	arguments := map[string]any{}
	kind := jsonString(object["type"])
	if kind == "open_page" {
		parsed, parseErr := url.Parse(jsonString(object["url"]))
		if parseErr != nil {
			return HostedSearch{}, errors.New("Codex hosted web URL is invalid")
		}
		if parsed.User != nil {
			if _, hasPassword := parsed.User.Password(); hasPassword {
				return HostedSearch{}, errors.New("Codex hosted web URL contains credentials")
			}
		}
	}
	if kind == "open_page" || kind == "find_in_page" {
		name = "web.fetch"
		if value, ok := object["url"].(string); ok {
			arguments["url"] = value
		}
	} else if query, ok := object["query"].(string); ok {
		arguments["query"] = query
	} else if queries, ok := object["queries"].([]any); ok {
		values := make([]string, 0, len(queries))
		for _, value := range queries {
			if text, ok := value.(string); ok {
				values = append(values, text)
			}
		}
		if joined := strings.Join(values, "; "); joined != "" {
			arguments["query"] = joined
		}
	}
	result := make(map[string]any, len(arguments)+2)
	for key, value := range arguments {
		result[key] = value
	}
	result["status"] = status
	if strings.EqualFold(status, "failed") {
		result["error"] = "provider-hosted web action failed"
	}
	sources := codexWebSources(kind, object)
	encodedArguments, _ := json.Marshal(arguments)
	encodedResult, _ := json.Marshal(result)
	encodedAction, _ := json.Marshal(action)
	return HostedSearch{
		Index: index, ID: id, Name: name, Status: status,
		Arguments: encodedArguments, Result: encodedResult,
		Sources: sources, ProviderAction: encodedAction,
	}, nil
}

func codexWebSources(kind string, action map[string]any) []WebSource {
	if kind == "open_page" || kind == "find_in_page" {
		url, ok := action["url"].(string)
		if safe, err := safeCitationURL(url); ok && err == nil {
			return []WebSource{{URL: safe}}
		}
		return nil
	}
	values, _ := action["sources"].([]any)
	sources := make([]WebSource, 0, len(values))
	for _, value := range values {
		source := jsonObject(value)
		url, err := safeCitationURL(jsonString(source["url"]))
		if err != nil {
			continue
		}
		sources = append(sources, WebSource{Title: strings.TrimSpace(jsonString(source["title"])), URL: url})
	}
	return sources
}

func normalizeCodexToolCall(
	index int,
	item map[string]json.RawMessage,
	transport ToolTransport,
	names openRouterToolNameMap,
) (GenerationToolCall, error) {
	if transport != ToolTransportNative {
		return GenerationToolCall{}, errors.New("Codex returned native tools when native tools were disabled")
	}
	callID, _ := rawString(item["call_id"])
	itemID, _ := rawString(item["id"])
	name, _ := rawString(item["name"])
	arguments, _ := rawString(item["arguments"])
	if strings.TrimSpace(callID) == "" || len(callID) > openRouterProviderCallIDLimit {
		return GenerationToolCall{}, errors.New("Codex native tool call id is invalid")
	}
	rule, exists := names.providerToRule[name]
	if !exists {
		return GenerationToolCall{}, errors.New("Codex returned an unadvertised tool name")
	}
	payload, err := openRouterJSONObject(json.RawMessage(arguments))
	if err != nil {
		return GenerationToolCall{}, errors.New("Codex native tool arguments must be a JSON object")
	}
	if rule.strict {
		restoreOpenRouterOptionalNulls(payload, rule.sourceSchema)
	}
	encoded, err := json.Marshal(payload)
	if err != nil {
		return GenerationToolCall{}, errors.New("Codex native tool arguments are invalid")
	}
	return GenerationToolCall{
		Index: index, ProviderItemID: itemID,
		ProviderCallID: callID, ProviderName: name,
		Name: rule.canonical, Payload: encoded,
	}, nil
}

func normalizeCodexReasoning(
	raw json.RawMessage,
	item map[string]json.RawMessage,
) (GenerationReasoning, bool, error) {
	id, _ := rawString(item["id"])
	encrypted, _ := rawString(item["encrypted_content"])
	var summary []struct {
		Type, Text string
	}
	if value := item["summary"]; len(value) != 0 && json.Unmarshal(value, &summary) != nil {
		return GenerationReasoning{}, false, errors.New("Codex reasoning output is invalid")
	}
	result := GenerationReasoning{
		ID: id, EncryptedContent: encrypted,
		ProviderDetails: []json.RawMessage{append(json.RawMessage(nil), raw...)},
	}
	for _, part := range summary {
		if part.Type == "summary_text" && strings.TrimSpace(part.Text) != "" {
			result.Summary = append(result.Summary, part.Text)
		}
	}
	return result, encrypted != "" || len(result.Summary) != 0, nil
}
