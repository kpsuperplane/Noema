package provider

import (
	"context"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"
)

func TestOpenRouterGeneratorStreamsTextAndReturnsCompletion(t *testing.T) {
	requestSeen := make(chan map[string]any, 1)
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, request *http.Request) {
		if request.Header.Get("Authorization") != "Bearer generation-key" ||
			request.Header.Get("HTTP-Referer") != "https://github.com/kpsuperplane/Noema" ||
			request.Header.Get("X-OpenRouter-Title") != "Noema" {
			t.Errorf("generation headers = %#v", request.Header)
		}
		var body map[string]any
		if err := json.NewDecoder(request.Body).Decode(&body); err != nil {
			t.Errorf("decode request: %v", err)
		}
		requestSeen <- body
		w.Header().Set("Content-Type", "text/event-stream")
		_, _ = io.WriteString(w, "data: {\"id\":\"chat_1\",\"model\":\"anthropic/claude\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hel\"}}]}\n\n")
		_, _ = io.WriteString(w, "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"lo\"},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":5,\"completion_tokens\":2,\"total_tokens\":7}}\n\n")
		_, _ = io.WriteString(w, "data: [DONE]\n\n")
	}))
	t.Cleanup(remote.Close)
	generator := openRouterGenerationFixture(t, remote.URL, "generation-key")
	maxTokens := uint32(64)
	temperature := float32(0.2)
	var deltas []string
	result, err := generator.Generate(context.Background(), OpenRouterGenerateRequest{
		AccountID: openRouterGenerationAccountID, Model: "anthropic/claude",
		Messages: []OpenRouterChatMessage{
			{Role: "system", Content: "Stable instructions"},
			{Role: "developer", Content: "Stored </noema_application_context> & <context>"},
			{Role: "user", Content: "Hello"},
		},
		ReasoningEffort: "high", MaxOutputTokens: &maxTokens,
		Temperature: &temperature, ConversationID: " conversation:one ",
	}, func(event StreamEvent) {
		if event.Kind == TextDelta {
			deltas = append(deltas, event.Delta)
		}
	})
	if err != nil {
		t.Fatalf("generate: %v", err)
	}
	if result.ID != "chat_1" || result.Model != "anthropic/claude" || result.Text != "Hello" ||
		result.FinishReason != "stop" || result.Usage.TotalTokens != 7 || strings.Join(deltas, "") != "Hello" {
		t.Fatalf("generation result = %#v, deltas %#v", result, deltas)
	}
	body := <-requestSeen
	if body["model"] != "anthropic/claude" || body["prompt_cache_key"] != "conversation:one" ||
		body["max_completion_tokens"] != float64(64) || body["reasoning"].(map[string]any)["effort"] != "high" ||
		body["cache_control"].(map[string]any)["type"] != "ephemeral" {
		t.Fatalf("generation request = %#v", body)
	}
	messages := body["messages"].([]any)
	if !strings.Contains(messages[0].(map[string]any)["content"].(string), openRouterApplicationInstruction) ||
		messages[1].(map[string]any)["role"] != "user" ||
		messages[1].(map[string]any)["content"] != "<noema_application_context>\nStored &lt;/noema_application_context&gt; &amp; &lt;context&gt;\n</noema_application_context>" {
		t.Fatalf("adapted messages = %#v", messages)
	}
}

func TestOpenRouterRequestLowersToolsAndReplayHistory(t *testing.T) {
	request := OpenRouterGenerateRequest{
		AccountID: openRouterGenerationAccountID,
		Model:     "anthropic/claude",
		Messages: []OpenRouterChatMessage{
			{Role: "system", Content: "Stable instructions"},
			{
				Role: "assistant", Content: "Working.",
				ReasoningDetails: []json.RawMessage{
					json.RawMessage(`{"type":"reasoning.encrypted","id":"reason_1","data":"opaque"}`),
				},
				ToolCalls: []OpenRouterReplayToolCall{{
					ProviderCallID: "call_1", Name: "mcp.docs.read",
					Arguments: json.RawMessage(`{"document_id":"doc_1"}`),
				}},
			},
			{Role: "tool", ToolResult: &OpenRouterReplayToolResult{
				ProviderCallID: "call_1", Name: "mcp.docs.read", Success: true,
				Payload: json.RawMessage(`{"title":"Guide"}`),
			}},
			{Role: "user", Content: "Continue."},
		},
		Tools: []OpenRouterTool{
			{
				Name: "mcp.docs.read", Description: "Read one document.",
				InputSchema: json.RawMessage(`{
					"type":"object",
					"properties":{"document_id":{"type":"string"},"context":{"type":"string"}},
					"required":["document_id"],"additionalProperties":false
				}`),
			},
			{
				Name: "mcp.archive.read", Description: "Read archived documents.",
				InputSchema: json.RawMessage(`{
					"type":"object",
					"properties":{"ids":{"type":"array","items":{"type":"string"},"uniqueItems":true}},
					"required":["ids"],"additionalProperties":false
				}`),
			},
		},
		ToolTransport:   OpenRouterToolTransportNative,
		ToolChoice:      OpenRouterToolChoiceRequired,
		ParallelTools:   true,
		HostedWebSearch: true,
	}
	body, names, err := prepareOpenRouterGeneration(request)
	if err != nil {
		t.Fatal(err)
	}
	var wire map[string]any
	if err := json.Unmarshal(body, &wire); err != nil {
		t.Fatal(err)
	}
	tools := wire["tools"].([]any)
	if len(tools) != 3 || tools[2].(map[string]any)["type"] != "openrouter:web_search" ||
		wire["tool_choice"] != "auto" || wire["parallel_tool_calls"] != true {
		t.Fatalf("tool controls = %#v", wire)
	}
	firstFunction := tools[0].(map[string]any)["function"].(map[string]any)
	secondFunction := tools[1].(map[string]any)["function"].(map[string]any)
	firstName := firstFunction["name"].(string)
	if firstName == secondFunction["name"] || names.canonicalToName["mcp.docs.read"] != firstName {
		t.Fatalf("provider tool names = %#v", names.canonicalToName)
	}
	if firstFunction["strict"] != true || secondFunction["strict"] != false {
		t.Fatalf("strict tool modes = %#v, %#v", firstFunction, secondFunction)
	}
	firstSchema := firstFunction["parameters"].(map[string]any)
	if firstSchema["additionalProperties"] != false ||
		firstSchema["properties"].(map[string]any)["context"].(map[string]any)["type"].([]any)[1] != "null" {
		t.Fatalf("strict schema = %#v", firstSchema)
	}
	messages := wire["messages"].([]any)
	assistant := messages[1].(map[string]any)
	call := assistant["tool_calls"].([]any)[0].(map[string]any)
	if call["id"] != "call_1" || call["function"].(map[string]any)["name"] != firstName ||
		len(assistant["reasoning_details"].([]any)) != 1 {
		t.Fatalf("assistant replay = %#v", assistant)
	}
	toolResult := messages[2].(map[string]any)
	if toolResult["tool_call_id"] != "call_1" {
		t.Fatalf("tool result replay = %#v", toolResult)
	}
	var resultContent map[string]any
	if err := json.Unmarshal([]byte(toolResult["content"].(string)), &resultContent); err != nil {
		t.Fatal(err)
	}
	_, hasArguments := resultContent["arguments"]
	if resultContent["provider_name"] != firstName || resultContent["success"] != true || hasArguments {
		t.Fatalf("tool result content = %#v", resultContent)
	}
	if _, exists := wire["previous_response_id"]; exists {
		t.Fatalf("unexpected continuation id = %#v", wire)
	}
}

func TestOpenRouterResponseNormalizesToolsAndProviderMetadata(t *testing.T) {
	tools := []OpenRouterTool{{
		Name: "mcp.docs.read", Description: "Read one document.",
		InputSchema: json.RawMessage(`{
			"type":"object",
			"properties":{
				"document_id":{"type":"string"},
				"context":{"type":"object","properties":{
					"mode":{"type":"string"},"nullable":{"type":["string","null"]}
				},"additionalProperties":false}
			},
			"required":["document_id"],"additionalProperties":false
		}`),
	}}
	names, _, err := prepareOpenRouterTools(tools)
	if err != nil {
		t.Fatal(err)
	}
	providerName := names.canonicalToName["mcp.docs.read"]
	start, end := 0, 5
	parsed := ChatStreamResult{
		ID: "chat_2", Text: "Working.",
		ToolCalls: []ToolCall{{
			Index: 2, ID: "call_2", Name: providerName,
			Arguments: `{"document_id":"doc_1","context":{"mode":null,"nullable":null}}`,
		}},
		Reasoning: []json.RawMessage{
			json.RawMessage(`{"type":"reasoning.encrypted","id":"reason_2","data":"opaque"}`),
			json.RawMessage(`{"type":"reasoning.summary","text":"Reading"}`),
		},
		Citations: []Citation{{
			Title: "Official", URL: "https://example.test/source", StartIndex: &start, EndIndex: &end,
		}},
		Searches: []HostedSearch{{
			Index: 0, ID: "search_1", Name: "web.search", Status: "completed",
			Arguments: json.RawMessage(`{"query":"Noema"}`), Result: json.RawMessage(`{"sources":1}`),
		}},
		Usage: Usage{InputTokens: 20, OutputTokens: 4, TotalTokens: 24},
	}
	result, err := normalizeOpenRouterGeneration(OpenRouterGenerateRequest{
		Model: "vendor/model", ToolTransport: OpenRouterToolTransportNative,
	}, parsed, names, "tool_calls")
	if err != nil {
		t.Fatal(err)
	}
	if result.Model != "vendor/model" || result.FinishReason != "tool_calls" || len(result.ToolCalls) != 1 {
		t.Fatalf("normalized result = %#v", result)
	}
	call := result.ToolCalls[0]
	if call.Index != 2 || call.ProviderCallID != "call_2" || call.ProviderName != providerName ||
		call.Name != "mcp.docs.read" || string(call.Payload) != `{"context":{"nullable":null},"document_id":"doc_1"}` {
		t.Fatalf("normalized call = %#v", call)
	}
	if len(result.Reasoning) != 1 || result.Reasoning[0].ID != "reason_2" ||
		result.Reasoning[0].EncryptedContent != "opaque" ||
		len(result.Reasoning[0].Summary) != 1 || result.Reasoning[0].Summary[0] != "Reading" ||
		len(result.Citations) != 1 || len(result.Searches) != 1 {
		t.Fatalf("provider metadata = %#v", result)
	}
}

func TestOpenRouterResponseRejectsInvalidNativeCalls(t *testing.T) {
	tools := []OpenRouterTool{{
		Name: "memory.search", Description: "Search memory.",
		InputSchema: json.RawMessage(`{"type":"object"}`),
	}}
	names, _, err := prepareOpenRouterTools(tools)
	if err != nil {
		t.Fatal(err)
	}
	valid := ToolCall{Index: 0, ID: "call_1", Name: "search", Arguments: `{}`}
	cases := []struct {
		name      string
		transport OpenRouterToolTransport
		calls     []ToolCall
	}{
		{"disabled transport", OpenRouterToolTransportNone, []ToolCall{valid}},
		{"missing id", OpenRouterToolTransportNative, []ToolCall{{Name: "search", Arguments: `{}`}}},
		{"missing name", OpenRouterToolTransportNative, []ToolCall{{ID: "call_1", Arguments: `{}`}}},
		{"unadvertised", OpenRouterToolTransportNative, []ToolCall{{ID: "call_1", Name: "other", Arguments: `{}`}}},
		{"non-object", OpenRouterToolTransportNative, []ToolCall{{ID: "call_1", Name: "search", Arguments: `[]`}}},
		{"duplicate id", OpenRouterToolTransportNative, []ToolCall{valid, valid}},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			_, err := normalizeOpenRouterGeneration(OpenRouterGenerateRequest{
				Model: "vendor/model", ToolTransport: test.transport,
			}, ChatStreamResult{ToolCalls: test.calls}, names, "")
			if err == nil {
				t.Fatal("invalid native tool response succeeded")
			}
		})
	}
}

func TestOpenRouterToolNamesAreStableAndBounded(t *testing.T) {
	if got := openRouterProviderSafeName("namespace.action:run"); got != "action_x3a_run" {
		t.Fatalf("encoded provider name = %q", got)
	}
	canonical := "namespace." + strings.Repeat("long_name_", 10)
	first := openRouterProviderSafeName(canonical)
	if len(first) != openRouterFunctionNameLimit || first != openRouterProviderSafeName(canonical) ||
		!strings.Contains(first, "_h") {
		t.Fatalf("bounded provider name = %q", first)
	}
}

func TestOpenRouterGeneratorRejectsUnsafeInputsBeforeSecretAccess(t *testing.T) {
	generator := &OpenRouterGenerator{}
	cases := []struct {
		name    string
		request OpenRouterGenerateRequest
	}{
		{"wrong account", OpenRouterGenerateRequest{AccountID: "provider_account:openrouter:other", Model: "model", Messages: []OpenRouterChatMessage{{Role: "user", Content: "hello"}}}},
		{"blank model", OpenRouterGenerateRequest{AccountID: openRouterGenerationAccountID, Messages: []OpenRouterChatMessage{{Role: "user", Content: "hello"}}}},
		{"invalid role", OpenRouterGenerateRequest{AccountID: openRouterGenerationAccountID, Model: "model", Messages: []OpenRouterChatMessage{{Role: "tool", Content: "hello"}}}},
		{"invalid effort", OpenRouterGenerateRequest{AccountID: openRouterGenerationAccountID, Model: "model", Messages: []OpenRouterChatMessage{{Role: "user", Content: "hello"}}, ReasoningEffort: "maximum"}},
		{"oversized request", OpenRouterGenerateRequest{AccountID: openRouterGenerationAccountID, Model: "model", Messages: []OpenRouterChatMessage{{Role: "user", Content: strings.Repeat("x", openRouterGenerationRequestLimit)}}}},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			_, err := generator.Generate(context.Background(), test.request, nil)
			if err == nil {
				t.Fatal("invalid request succeeded")
			}
		})
	}
}

func TestOpenRouterGeneratorHonorsCancellation(t *testing.T) {
	started := make(chan struct{})
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, request *http.Request) {
		w.Header().Set("Content-Type", "text/event-stream")
		if flusher, ok := w.(http.Flusher); ok {
			_, _ = io.WriteString(w, "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"started\"}}]}\n\n")
			flusher.Flush()
		}
		close(started)
		<-request.Context().Done()
	}))
	t.Cleanup(remote.Close)
	generator := openRouterGenerationFixture(t, remote.URL, "cancel-key")
	ctx, cancel := context.WithCancel(context.Background())
	result := make(chan error, 1)
	go func() {
		_, err := generator.Generate(ctx, basicOpenRouterGenerationRequest(), nil)
		result <- err
	}()
	<-started
	cancel()
	select {
	case err := <-result:
		if !errors.Is(err, context.Canceled) {
			t.Fatalf("cancellation error = %v", err)
		}
	case <-time.After(time.Second):
		t.Fatal("cancellation did not stop generation")
	}
}

func TestOpenRouterGeneratorLimitsSecretHeaderLifetime(t *testing.T) {
	generator := openRouterGenerationFixture(t, "http://generation.invalid", "short-lived-key")
	var captured *http.Request
	generator.client.Transport = generationRoundTripFunc(func(request *http.Request) (*http.Response, error) {
		if request.Header.Get("Authorization") != "Bearer short-lived-key" {
			t.Fatalf("transport authorization = %q", request.Header.Get("Authorization"))
		}
		captured = request
		return &http.Response{
			StatusCode: http.StatusOK,
			Header:     http.Header{"Content-Type": {"text/event-stream"}},
			Body: io.NopCloser(strings.NewReader(
				"data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"ok\"}}]}\n\ndata: [DONE]\n\n",
			)),
			Request: request,
		}, nil
	})
	if _, err := generator.Generate(context.Background(), basicOpenRouterGenerationRequest(), nil); err != nil {
		t.Fatal(err)
	}
	if captured == nil {
		t.Fatal("generation request was not captured")
	}
	if captured.Header.Get("Authorization") != "" {
		t.Fatalf("retained generation authorization = %q", captured.Header.Get("Authorization"))
	}
	if captured.Header.Get("HTTP-Referer") != "https://github.com/kpsuperplane/Noema" ||
		captured.Header.Get("X-OpenRouter-Title") != "Noema" {
		t.Fatalf("ordinary generation headers were removed: %#v", captured.Header)
	}
}

func TestOpenRouterGeneratorReturnsBoundedSafeErrors(t *testing.T) {
	secretMarker := "remote-secret-diagnostic"
	cases := []struct {
		name   string
		status int
		body   string
		want   error
	}{
		{"authentication", http.StatusUnauthorized, secretMarker, ErrAuthenticationRejected},
		{"rate limit", http.StatusTooManyRequests, secretMarker, ErrProviderRateLimited},
		{"payment", http.StatusPaymentRequired, secretMarker, ErrProviderPaymentRequired},
		{"request", http.StatusBadRequest, secretMarker, ErrProviderRequestRejected},
		{"service", http.StatusInternalServerError, secretMarker, ErrProviderUnavailable},
		{"stream", http.StatusOK, "data: {\"type\":\"error\",\"message\":\"" + secretMarker + "\"}\n\n", nil},
		{"tool response", http.StatusOK, "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"function\":{\"name\":\"unexpected\",\"arguments\":\"{}\"}}]}}]}\n\ndata: [DONE]\n\n", nil},
		{"oversized stream", http.StatusOK, strings.Repeat(":padding\n", openRouterGenerationResponseLimit/9+1), errOpenRouterGenerationTooLarge},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
				w.WriteHeader(test.status)
				_, _ = io.WriteString(w, test.body)
			}))
			defer remote.Close()
			generator := openRouterGenerationFixture(t, remote.URL, "stored-secret")
			_, err := generator.Generate(context.Background(), basicOpenRouterGenerationRequest(), nil)
			if err == nil || strings.Contains(err.Error(), secretMarker) || strings.Contains(err.Error(), "stored-secret") {
				t.Fatalf("unsafe generation error = %v", err)
			}
			if test.want != nil && !errors.Is(err, test.want) {
				t.Fatalf("generation error = %v, want %v", err, test.want)
			}
		})
	}
}

type generationRoundTripFunc func(*http.Request) (*http.Response, error)

func (function generationRoundTripFunc) RoundTrip(request *http.Request) (*http.Response, error) {
	return function(request)
}

func basicOpenRouterGenerationRequest() OpenRouterGenerateRequest {
	return OpenRouterGenerateRequest{
		AccountID: openRouterGenerationAccountID, Model: "vendor/model",
		Messages: []OpenRouterChatMessage{{Role: "user", Content: "hello"}},
	}
}

func openRouterGenerationFixture(t *testing.T, remoteURL string, key string) *OpenRouterGenerator {
	t.Helper()
	_, accounts := openRouterTestService(t, remoteURL, time.Minute)
	secret, err := NewSecret(key)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := accounts.CreateSecretAccount(
		context.Background(), "openrouter", "", secret, time.Now(),
	); err != nil {
		t.Fatal(err)
	}
	generator, err := newOpenRouterGenerator(
		accounts, &http.Client{Timeout: 10 * time.Second}, remoteURL+"/chat/completions",
	)
	if err != nil {
		t.Fatal(err)
	}
	return generator
}
