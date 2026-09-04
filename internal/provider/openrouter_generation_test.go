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
		{"invalid usage", http.StatusOK, "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"bad\"}}],\"usage\":{\"prompt_tokens\":-1}}\n\ndata: [DONE]\n\n", nil},
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
