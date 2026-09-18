package provider

import (
	"context"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestOpenAIGeneratorPreservesResponsesWireAndProtectedSetup(t *testing.T) {
	requestSeen := make(chan map[string]any, 1)
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, request *http.Request) {
		if request.URL.Path != "/v1/responses" || request.Header.Get("Authorization") != "Bearer platform-key" ||
			request.Header.Get("OpenAI-Organization") != "org_test" || request.Header.Get("OpenAI-Project") != "proj_test" ||
			request.Header.Get("Accept") != "text/event-stream" {
			t.Error("OpenAI request headers are invalid")
		}
		var body map[string]any
		if err := json.NewDecoder(request.Body).Decode(&body); err != nil {
			t.Errorf("decode request: %v", err)
		}
		requestSeen <- body
		_, _ = io.WriteString(w, `data: {"type":"response.output_text.delta","output_index":0,"delta":"Found it."}`+"\n\n")
		_, _ = io.WriteString(w, `data: {"type":"response.output_item.done","output_index":0,"item":{"type":"message","content":[{"type":"output_text","text":"stale","annotations":[{"type":"url_citation","title":"Source","url":"https://example.test/source","start_index":0,"end_index":5}]}]}}`+"\n\n")
		_, _ = io.WriteString(w, `data: {"type":"response.output_item.done","output_index":1,"item":{"type":"reasoning","id":"rs_1","encrypted_content":"opaque","summary":[{"type":"summary_text","text":"Searched"}]}}`+"\n\n")
		_, _ = io.WriteString(w, `data: {"type":"response.output_item.done","output_index":2,"item":{"type":"web_search_call","id":"ws_1","status":"completed","action":{"type":"search","query":"current","sources":[{"title":"Source","url":"https://example.test/source"}]}}}`+"\n\n")
		_, _ = io.WriteString(w, `data: {"type":"response.completed","response":{"id":"resp_1","model":"gpt-5.6-terra","status":"completed","output":null,"usage":{"input_tokens":8,"output_tokens":3,"total_tokens":11,"input_tokens_details":{"cached_tokens":2}}}}`+"\n\n")
	}))
	t.Cleanup(remote.Close)
	generator, account := openAIGenerationFixture(t, remote.URL+"/v1/responses", "platform-key")
	maxTokens := uint32(256)
	var deltas []string
	result, err := generator.Generate(context.Background(), GenerateRequest{
		AccountID: openAIDefaultAccountID, Model: "gpt-5.6-terra",
		Messages: []GenerationMessage{
			{Role: "developer", Content: "Use stable Noema context."},
			{Role: "user", Content: "Find it."},
		},
		ReasoningEffort: "high", MaxOutputTokens: &maxTokens, ConversationID: "conversation:one",
		ToolTransport: ToolTransportNative, ToolChoice: ToolChoiceAuto,
		Tools:           []GenerationTool{{Name: "docs.read", Description: "Read a document.", InputSchema: json.RawMessage(`{"type":"object","properties":{"id":{"type":"string"}},"required":["id"]}`)}},
		HostedWebSearch: true, FastMode: true,
	}, func(event StreamEvent) {
		if event.Kind == TextDelta {
			deltas = append(deltas, event.Delta)
		}
	})
	if err != nil {
		t.Fatal(err)
	}
	profiles, profileErr := account.Metadata.ModelProfiles()
	if profileErr != nil || len(profiles) != 3 || profiles[0].ID != "gpt-5.6-terra" || account.AuthMethod != AuthExternalManual {
		t.Fatalf("protected OpenAI setup = %#v, %v", account, profileErr)
	}
	if result.ID != "resp_1" || result.Text != "Found it." || result.Usage.TotalTokens != 11 ||
		len(result.Citations) != 1 || len(result.Reasoning) != 1 || len(result.Searches) != 1 ||
		strings.Join(deltas, "") != "Found it." {
		t.Fatalf("OpenAI result = %#v, deltas = %#v", result, deltas)
	}
	body := <-requestSeen
	include := body["include"].([]any)
	if body["model"] != "gpt-5.6-terra" || body["max_output_tokens"] != float64(256) ||
		body["prompt_cache_retention"] != nil || body["prompt_cache_key"] != "conversation:one" ||
		body["store"] != false || body["stream"] != true || body["service_tier"] != "priority" ||
		len(include) != 2 || include[0] != "reasoning.encrypted_content" || include[1] != "web_search_call.action.sources" {
		t.Fatalf("OpenAI request controls = %#v", body)
	}
	cache := body["prompt_cache_options"].(map[string]any)
	input := body["input"].([]any)
	developer := input[0].(map[string]any)["content"].([]any)[0].(map[string]any)
	breakpoint := developer["prompt_cache_breakpoint"].(map[string]any)
	if cache["mode"] != "explicit" || cache["ttl"] != "30m" ||
		developer["type"] != "input_text" || developer["text"] != "Use stable Noema context." ||
		breakpoint["mode"] != "explicit" {
		t.Fatalf("OpenAI explicit prompt cache = %#v, input %#v", cache, input)
	}
	tools := body["tools"].([]any)
	if len(tools) != 2 || tools[0].(map[string]any)["description"] != "Read a document." ||
		tools[0].(map[string]any)["parameters"] == nil || tools[1].(map[string]any)["external_web_access"] != true {
		t.Fatalf("OpenAI hosted tool = %#v", tools)
	}
}

func TestOpenAIBackgroundRequestKeepsNonGPT56CacheWithoutStorage(t *testing.T) {
	request := basicOpenAIGenerationRequest()
	request.Model = "gpt-5.5"
	request.Messages = []GenerationMessage{{Role: "developer", Content: "Stable context."}}
	body, _, err := prepareResponsesGeneration(request, responsesGenerationProfile{
		accountID: openAIDefaultAccountID, providerName: "OpenAI",
		promptCacheRetention: "24h", forwardMaxOutput: true,
		includeEncryptedReasoning: true, promptCacheOptions: true, stream: true,
	})
	if err != nil {
		t.Fatal(err)
	}
	var wire map[string]any
	if json.Unmarshal(body, &wire) != nil || wire["store"] != false ||
		wire["prompt_cache_retention"] != "24h" || wire["prompt_cache_options"] != nil {
		t.Fatalf("OpenAI legacy cache request = %#v", wire)
	}
	input := wire["input"].([]any)
	if input[0].(map[string]any)["content"] != "Stable context." {
		t.Fatalf("OpenAI legacy cache input = %#v", input)
	}
}

func TestOpenAIGeneratorMapsSafeStatusErrors(t *testing.T) {
	for _, test := range []struct {
		status int
		want   error
	}{
		{http.StatusUnauthorized, ErrAuthenticationRejected},
		{http.StatusTooManyRequests, ErrProviderRateLimited},
		{http.StatusPaymentRequired, ErrProviderPaymentRequired},
		{http.StatusBadRequest, ErrProviderRequestRejected},
		{http.StatusInternalServerError, ErrProviderUnavailable},
	} {
		remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
			w.WriteHeader(test.status)
			_, _ = io.WriteString(w, "remote-private-body")
		}))
		generator, _ := openAIGenerationFixture(t, remote.URL, "status-key")
		_, err := generator.Generate(context.Background(), basicOpenAIGenerationRequest(), nil)
		remote.Close()
		if !errors.Is(err, test.want) || strings.Contains(err.Error(), "remote-private-body") || strings.Contains(err.Error(), "status-key") {
			t.Fatalf("OpenAI status %d error = %v", test.status, err)
		}
	}
}

func TestOpenAIGeneratorCancelsAndBoundsResponses(t *testing.T) {
	t.Run("cancel", func(t *testing.T) {
		started := make(chan struct{})
		remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, request *http.Request) {
			if flusher, ok := w.(http.Flusher); ok {
				_, _ = io.WriteString(w, `data: {"type":"response.output_text.delta","delta":"started"}`+"\n\n")
				flusher.Flush()
			}
			close(started)
			<-request.Context().Done()
		}))
		defer remote.Close()
		generator, _ := openAIGenerationFixture(t, remote.URL, "cancel-key")
		ctx, cancel := context.WithCancel(context.Background())
		result := make(chan error, 1)
		go func() {
			_, err := generator.Generate(ctx, basicOpenAIGenerationRequest(), nil)
			result <- err
		}()
		<-started
		cancel()
		if err := <-result; !errors.Is(err, context.Canceled) {
			t.Fatalf("OpenAI cancellation error = %v", err)
		}
	})
	t.Run("bound", func(t *testing.T) {
		remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
			_, _ = io.WriteString(w, strings.Repeat(":padding\n", codexGenerationResponseLimit/9+1))
		}))
		defer remote.Close()
		generator, _ := openAIGenerationFixture(t, remote.URL, "bound-key")
		_, err := generator.Generate(context.Background(), basicOpenAIGenerationRequest(), nil)
		if !errors.Is(err, errOpenAIGenerationTooLarge) {
			t.Fatalf("OpenAI response bound error = %v", err)
		}
	})
}

func TestOpenAIGeneratorLimitsCredentialHeaderLifetime(t *testing.T) {
	generator, _ := openAIGenerationFixture(t, "http://generation.invalid", "short-lived-key")
	var captured *http.Request
	generator.client.Transport = generationRoundTripFunc(func(request *http.Request) (*http.Response, error) {
		if request.Header.Get("Authorization") != "Bearer short-lived-key" {
			t.Fatal("OpenAI transport authorization is invalid")
		}
		captured = request
		return &http.Response{StatusCode: http.StatusOK, Body: io.NopCloser(strings.NewReader(
			`data: {"type":"response.completed","response":{"status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":"ok"}]}]}}` + "\n\n",
		)), Request: request}, nil
	})
	if _, err := generator.Generate(context.Background(), basicOpenAIGenerationRequest(), nil); err != nil {
		t.Fatal(err)
	}
	if captured == nil || captured.Header.Get("Authorization") != "" ||
		captured.Header.Get("OpenAI-Organization") != "org_test" {
		t.Fatalf("retained OpenAI headers = %#v", captured)
	}
}

func TestOpenAIGeneratorRejectsUnsafeInputBeforeSecretAccess(t *testing.T) {
	generator := &OpenAIGenerator{}
	request := basicOpenAIGenerationRequest()
	request.AccountID = "provider_account:openai:other"
	if _, err := generator.Generate(context.Background(), request, nil); err == nil {
		t.Fatal("invalid OpenAI account succeeded")
	}
	request = basicOpenAIGenerationRequest()
	request.Messages[0].Role = "operation"
	if _, err := generator.Generate(context.Background(), request, nil); err == nil {
		t.Fatal("invalid OpenAI message succeeded")
	}
}

func openAIGenerationFixture(t *testing.T, responsesURL, key string) (*OpenAIGenerator, Account) {
	t.Helper()
	now := time.Now().UTC()
	persistence := &memoryAccountPersistence{accounts: map[string]Account{
		openAIDefaultAccountID: {
			ID: openAIDefaultAccountID, ProviderKind: "openai", AccountKey: "default", DisplayName: "OpenAI",
			AuthMethod: AuthExternalManual, IsActive: true, IsDefault: true, Status: StatusUnknown,
			Metadata: AccountMetadata{
				"credentialRevision": json.RawMessage("0"), "secretConfigured": json.RawMessage("false"),
				"organization_id": json.RawMessage(`"org_test"`), "project_id": json.RawMessage(`"proj_test"`),
			}, CreatedAt: now, UpdatedAt: now,
		},
	}}
	accounts, err := NewAccountService(filepath.Join(t.TempDir(), "home"), persistence)
	if err != nil {
		t.Fatal(err)
	}
	secret, _ := NewSecret(key)
	account, err := accounts.SaveSecret(context.Background(), openAIDefaultAccountID, secret, now)
	if err != nil {
		t.Fatal(err)
	}
	generator, err := newOpenAIGenerator(accounts, &http.Client{Timeout: 10 * time.Second}, responsesURL)
	if err != nil {
		t.Fatal(err)
	}
	return generator, account
}

func basicOpenAIGenerationRequest() GenerateRequest {
	return GenerateRequest{
		AccountID: openAIDefaultAccountID, Model: "gpt-5.6-terra",
		Messages: []GenerationMessage{{Role: "user", Content: "hello"}},
	}
}
