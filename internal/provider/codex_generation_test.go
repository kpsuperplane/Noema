package provider

import (
	"context"
	"encoding/base64"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"net/http/httptest"
	"net/url"
	"strconv"
	"strings"
	"testing"
	"time"
)

func TestCodexGeneratorPreservesResponsesWireAndOutput(t *testing.T) {
	requestSeen := make(chan map[string]any, 1)
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, request *http.Request) {
		if request.Header.Get("Authorization") != "Bearer "+codexGenerationAccessToken() ||
			request.Header.Get("ChatGPT-Account-ID") != "workspace-test" ||
			request.Header.Get("originator") != codexGenerationOriginator ||
			request.Header.Get("version") != "0.144.1" ||
			request.Header.Get("User-Agent") != "codex_cli_rs/0.144.1 (Noema)" ||
			request.Header.Get("session-id") != "conversation:one" ||
			request.Header.Get("Accept") != "text/event-stream" {
			t.Error("Codex generation headers are invalid")
		}
		var body map[string]any
		if err := json.NewDecoder(request.Body).Decode(&body); err != nil {
			t.Errorf("decode request: %v", err)
		}
		requestSeen <- body
		w.Header().Set("Content-Type", "text/event-stream")
		_, _ = io.WriteString(w, "event: response.output_text.delta\n")
		_, _ = io.WriteString(w, `data: {"type":"response.output_text.delta","output_index":0,"delta":"Checking."}`+"\n\n")
		_, _ = io.WriteString(w, "event: response.output_item.done\n")
		_, _ = io.WriteString(w, `data: {"type":"response.output_item.done","output_index":0,"item":{"type":"message","content":[{"type":"output_text","text":"stale","annotations":[{"type":"url_citation","title":"Official","url":"https://example.test/current","start_index":0,"end_index":8}]}]}}`+"\n\n")
		_, _ = io.WriteString(w, "event: response.output_item.done\n")
		_, _ = io.WriteString(w, `data: {"type":"response.output_item.done","output_index":1,"item":{"type":"reasoning","id":"rs_2","encrypted_content":"opaque-new","summary":[{"type":"summary_text","text":"Checked the Task"}]}}`+"\n\n")
		_, _ = io.WriteString(w, "event: response.output_item.added\n")
		_, _ = io.WriteString(w, `data: {"type":"response.output_item.added","output_index":2,"item":{"type":"web_search_call","id":"ws_new","status":"in_progress"}}`+"\n\n")
		_, _ = io.WriteString(w, `data: {"type":"response.web_search_call.searching","output_index":2,"item_id":"ws_new"}`+"\n\n")
		_, _ = io.WriteString(w, "event: response.output_item.done\n")
		_, _ = io.WriteString(w, `data: {"type":"response.output_item.done","output_index":2,"item":{"type":"web_search_call","id":"ws_new","status":"completed","action":{"type":"search","query":"current trains","sources":[{"title":"Rail","url":"https://rail.example/times"},{"title":"Unsafe","url":"file:///private"}]}}}`+"\n\n")
		_, _ = io.WriteString(w, "event: response.output_item.added\n")
		_, _ = io.WriteString(w, `data: {"type":"response.output_item.added","output_index":3,"item":{"type":"function_call","call_id":"call_2","name":"inspect"}}`+"\n\n")
		_, _ = io.WriteString(w, "event: response.output_item.done\n")
		_, _ = io.WriteString(w, `data: {"type":"response.output_item.done","output_index":3,"item":{"type":"function_call","id":"item_2","call_id":"call_2","name":"inspect","arguments":"{\"task_id\":\"task:two\"}"}}`+"\n\n")
		_, _ = io.WriteString(w, "event: response.completed\n")
		_, _ = io.WriteString(w, `data: {"type":"response.completed","response":{"id":"resp_2","model":"gpt-5.6-codex","status":"completed","output":null,"usage":{"input_tokens":20,"output_tokens":4,"total_tokens":24,"input_tokens_details":{"cached_tokens":7}}}}`+"\n\n")
	}))
	t.Cleanup(remote.Close)
	generator := codexGenerationFixture(t, remote.URL)
	maxTokens := uint32(128)
	providerReasoning := json.RawMessage(`{"type":"reasoning","id":"rs_1","encrypted_content":"opaque-old","summary":[]}`)
	request := GenerateRequest{
		AccountID: codexGenerationAccountID, Model: "gpt-5.6-codex",
		Messages: []GenerationMessage{
			{Role: "developer", Content: "Use the Task context."},
			{Role: "user", Content: "Inspect it."},
			{Role: "assistant", Phase: "commentary", Content: "I will inspect it.", ReasoningDetails: []json.RawMessage{providerReasoning}, ToolCalls: []ReplayToolCall{{
				ProviderItemID: "item_1", ProviderCallID: "call_1",
				ProviderName: "inspect", Name: "task.inspect",
				Arguments: json.RawMessage(`{"task_id":"task:one"}`),
			}}},
			{Role: "tool", ToolResult: &ReplayToolResult{
				ProviderCallID: "call_1", ProviderName: "inspect", Name: "task.inspect", Success: true,
				Payload: json.RawMessage(`{"title":"One"}`),
			}},
			{Role: "hosted_web_search", HostedSearch: &HostedSearch{
				Index: 2, ID: "ws_old", Name: "web.search", Status: "completed",
				Arguments:      json.RawMessage(`{"query":"old trains"}`),
				Result:         json.RawMessage(`{"status":"completed"}`),
				ProviderAction: json.RawMessage(`{"type":"search","query":"old trains"}`),
			}},
			{Role: "user", Content: "Inspect the other Task."},
		},
		ReasoningEffort: "high", MaxOutputTokens: &maxTokens,
		ConversationID: "conversation:one", FastMode: true,
		Tools: []GenerationTool{{
			Name: "task.inspect", Description: "Inspect one Task.",
			InputSchema: json.RawMessage(`{"type":"object","properties":{"task_id":{"type":"string"},"detail":{"type":"string"}},"required":["task_id"],"additionalProperties":false}`),
		}},
		ToolTransport: ToolTransportNative, ToolChoice: ToolChoiceRequired, HostedWebSearch: true,
	}
	var events []StreamEvent
	result, err := generator.Generate(context.Background(), request, func(event StreamEvent) {
		events = append(events, event)
	})
	if err != nil {
		t.Fatal(err)
	}
	if result.ID != "resp_2" || result.Model != "gpt-5.6-codex" || result.Text != "Checking." ||
		result.FinishReason != "tool_calls" || result.Usage.TotalTokens != 24 ||
		result.Usage.CachedInputTokens != 7 || len(result.ToolCalls) != 1 || len(result.Reasoning) != 1 ||
		len(result.Searches) != 1 || len(result.Citations) != 1 {
		t.Fatalf("Codex result = %#v", result)
	}
	if call := result.ToolCalls[0]; call.Index != 3 || call.ProviderItemID != "item_2" ||
		call.ProviderCallID != "call_2" || call.ProviderName != "inspect" ||
		call.Name != "task.inspect" || string(call.Payload) != `{"task_id":"task:two"}` {
		t.Fatalf("Codex tool call = %#v", call)
	}
	if search := result.Searches[0]; search.Index != 2 || search.ID != "ws_new" ||
		search.Name != "web.search" || string(search.Arguments) != `{"query":"current trains"}` ||
		len(search.Sources) != 1 || search.Sources[0].URL != "https://rail.example/times" ||
		string(search.ProviderAction) == "" {
		t.Fatalf("Codex hosted search = %#v", search)
	}
	if citation := result.Citations[0]; citation.Title != "Official" ||
		citation.URL != "https://example.test/current" || citation.EndIndex == nil || *citation.EndIndex != 8 {
		t.Fatalf("Codex citation = %#v", citation)
	}
	if reasoning := result.Reasoning[0]; reasoning.ID != "rs_2" || reasoning.EncryptedContent != "opaque-new" ||
		len(reasoning.Summary) != 1 || len(reasoning.ProviderDetails) != 1 {
		t.Fatalf("Codex reasoning = %#v", reasoning)
	}
	if len(events) != 7 || events[0].Kind != TextDelta || events[0].Delta != "Checking." ||
		events[3].Kind != HostedSearchStarted || events[3].ID != "ws_new" ||
		events[4].Kind != ToolCallStarted || events[4].ID != "call_2" {
		t.Fatalf("Codex events = %#v", events)
	}

	body := <-requestSeen
	if body["model"] != "gpt-5.6-codex" || body["stream"] != true || body["store"] != false ||
		body["service_tier"] != "priority" || body["prompt_cache_key"] != "conversation:one" ||
		body["tool_choice"] != "auto" || body["parallel_tool_calls"] != false ||
		body["max_output_tokens"] != nil {
		t.Fatalf("Codex request controls = %#v", body)
	}
	if reasoning := body["reasoning"].(map[string]any); reasoning["effort"] != "high" || reasoning["summary"] != "auto" {
		t.Fatalf("Codex request reasoning = %#v", reasoning)
	}
	if include := body["include"].([]any); len(include) != 1 || include[0] != "web_search_call.action.sources" {
		t.Fatalf("Codex hosted include = %#v", include)
	}
	tools := body["tools"].([]any)
	tool := tools[0].(map[string]any)
	if len(tools) != 2 || tool["type"] != "function" || tool["name"] != "inspect" ||
		tool["description"] != request.Tools[0].Description || tool["parameters"] == nil ||
		tool["strict"] != true || tools[1].(map[string]any)["type"] != "web_search" {
		t.Fatalf("Codex tool wire = %#v", tool)
	}
	input := body["input"].([]any)
	if len(input) != 8 || input[0].(map[string]any)["role"] != "developer" ||
		input[2].(map[string]any)["type"] != "reasoning" ||
		len(input[2].(map[string]any)["summary"].([]any)) != 0 ||
		input[3].(map[string]any)["phase"] != "commentary" ||
		input[3].(map[string]any)["content"].([]any)[0].(map[string]any)["type"] != "output_text" ||
		input[4].(map[string]any)["type"] != "function_call" || input[4].(map[string]any)["id"] != "item_1" ||
		input[5].(map[string]any)["type"] != "function_call_output" ||
		input[6].(map[string]any)["type"] != "web_search_call" ||
		input[6].(map[string]any)["action"].(map[string]any)["query"] != "old trains" {
		t.Fatalf("Codex replay input = %#v", input)
	}
	var output map[string]any
	if err := json.Unmarshal([]byte(input[5].(map[string]any)["output"].(string)), &output); err != nil ||
		output["success"] != true || output["provider_name"] != "inspect" {
		t.Fatalf("Codex tool result replay = %#v, %v", output, err)
	}
}

func TestCodexGeneratorUsesTerminalTextWithoutDeltas(t *testing.T) {
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		_, _ = io.WriteString(w, "event: response.completed\n")
		_, _ = io.WriteString(w, `data: {"type":"response.completed","response":{"id":"resp_1","status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":"Done"}]}]}}`+"\n\n")
	}))
	t.Cleanup(remote.Close)
	result, err := codexGenerationFixture(t, remote.URL).Generate(
		context.Background(), basicCodexGenerationRequest(), nil,
	)
	if err != nil || result.Text != "Done" || result.FinishReason != "stop" {
		t.Fatalf("Codex terminal result = %#v, %v", result, err)
	}
}

func TestCodexContinuationAndHostedURLCredentialRules(t *testing.T) {
	request := basicCodexGenerationRequest()
	request.PreviousResponseID = "resp_previous"
	request.StoreResponse = true
	request.Messages = []GenerationMessage{{Role: "tool", ToolResult: &ReplayToolResult{
		ProviderCallID: "call_1", ProviderName: "inspect", Name: "task.inspect",
		Success: true, Payload: json.RawMessage(`{"title":"One"}`),
	}}}
	body, _, err := prepareCodexGeneration(request)
	if err != nil {
		t.Fatal(err)
	}
	var wire map[string]any
	if json.Unmarshal(body, &wire) != nil || wire["previous_response_id"] != "resp_previous" ||
		wire["store"] != true || len(wire["input"].([]any)) != 1 {
		t.Fatalf("Codex continuation wire = %#v", wire)
	}
	for _, test := range []struct {
		url     string
		blocked bool
	}{
		{"https://person:password@example.test/page", true},
		{"https://person@example.test/page", true},
		{"https://example.test/page", false},
	} {
		action, _ := json.Marshal(map[string]any{"type": "open_page", "url": test.url})
		_, err := normalizeCodexHostedSearch(0, map[string]json.RawMessage{
			"status": json.RawMessage(`"completed"`), "action": action,
		})
		if (err != nil) != test.blocked {
			t.Fatalf("open_page URL %q error = %v", test.url, err)
		}
	}
}

func TestCodexGeneratorReconcilesStreamedAndTerminalMessages(t *testing.T) {
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		_, _ = io.WriteString(w, `data: {"type":"response.output_text.delta","output_index":0,"delta":"First"}`+"\n\n")
		_, _ = io.WriteString(w, `data: {"type":"response.output_item.done","output_index":0,"item":{"type":"message","content":[{"type":"output_text","text":"stale"}]}}`+"\n\n")
		_, _ = io.WriteString(w, `data: {"type":"response.output_item.done","output_index":2,"item":{"type":"reasoning","id":"rs_1","encrypted_content":"opaque","summary":[]}}`+"\n\n")
		_, _ = io.WriteString(w, `data: {"type":"response.completed","response":{"status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":"First"}]},{"type":"message","content":[{"type":"output_text","text":" second"}]},{"type":"reasoning","id":"rs_1","encrypted_content":"opaque","summary":[]}]}}`+"\n\n")
	}))
	t.Cleanup(remote.Close)
	result, err := codexGenerationFixture(t, remote.URL).Generate(
		context.Background(), basicCodexGenerationRequest(), nil,
	)
	if err != nil || result.Text != "First second" || len(result.Reasoning) != 1 {
		t.Fatalf("reconciled Codex result = %#v, %v", result, err)
	}
}

func TestCodexGeneratorRejectsUnsafeInputsBeforeCredentialAccess(t *testing.T) {
	generator := &CodexGenerator{}
	cases := []struct {
		name    string
		request GenerateRequest
		want    error
	}{
		{"wrong account", GenerateRequest{AccountID: "provider_account:codex:other", Model: "model", Messages: []GenerationMessage{{Role: "user", Content: "hello"}}}, nil},
		{"blank model", GenerateRequest{AccountID: codexGenerationAccountID, Messages: []GenerationMessage{{Role: "user", Content: "hello"}}}, nil},
		{"invalid role", GenerateRequest{AccountID: codexGenerationAccountID, Model: "model", Messages: []GenerationMessage{{Role: "operation", Content: "hello"}}}, nil},
		{"invalid effort", GenerateRequest{AccountID: codexGenerationAccountID, Model: "model", Messages: []GenerationMessage{{Role: "user", Content: "hello"}}, ReasoningEffort: "maximum"}, nil},
		{"hosted search", GenerateRequest{AccountID: codexGenerationAccountID, Model: "model", Messages: []GenerationMessage{{Role: "user", Content: "hello"}}, HostedWebSearch: true}, nil},
		{"oversized request", GenerateRequest{AccountID: codexGenerationAccountID, Model: "model", Messages: []GenerationMessage{{Role: "user", Content: strings.Repeat("x", codexGenerationRequestLimit)}}}, ErrGenerationRequestTooLarge},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			_, err := generator.Generate(context.Background(), test.request, nil)
			if err == nil {
				t.Fatal("invalid request succeeded")
			}
			if test.want != nil && !errors.Is(err, test.want) {
				t.Fatalf("Codex generation error = %v, want %v", err, test.want)
			}
		})
	}
}

func TestCodexGeneratorRejectsUnsafeWorkspaceHeader(t *testing.T) {
	claims := base64.RawURLEncoding.EncodeToString([]byte(`{"chatgpt_account_id":"workspace\nforged"}`))
	if _, err := codexChatGPTAccountID("header." + claims + ".signature"); err == nil {
		t.Fatal("unsafe ChatGPT workspace header was accepted")
	}
}

func TestCodexGeneratorRejectsUnadvertisedAndMultipleTools(t *testing.T) {
	responses := []string{
		`{"type":"response.completed","response":{"status":"completed","output":[{"type":"function_call","call_id":"call_1","name":"other","arguments":"{}"}]}}`,
		`{"type":"response.completed","response":{"status":"completed","output":[{"type":"function_call","call_id":"call_1","name":"inspect","arguments":"{}"},{"type":"function_call","call_id":"call_2","name":"inspect","arguments":"{}"}]}}`,
	}
	for index, event := range responses {
		t.Run([]string{"unadvertised", "multiple"}[index], func(t *testing.T) {
			remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
				_, _ = io.WriteString(w, "event: response.completed\ndata: "+event+"\n\n")
			}))
			defer remote.Close()
			request := basicCodexGenerationRequest()
			request.Tools = []GenerationTool{{
				Name: "task.inspect", Description: "Inspect one Task.",
				InputSchema: json.RawMessage(`{"type":"object","properties":{},"additionalProperties":false}`),
			}}
			request.ToolTransport = ToolTransportNative
			if _, err := codexGenerationFixture(t, remote.URL).Generate(context.Background(), request, nil); err == nil {
				t.Fatal("invalid Codex tool response succeeded")
			}
		})
	}
}

func TestCodexGeneratorRejectsTwoStreamedCallsWithoutIndexes(t *testing.T) {
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		for _, call := range []string{
			`{"type":"function_call","call_id":"call_1","name":"inspect","arguments":"{}"}`,
			`{"type":"function_call","call_id":"call_2","name":"inspect","arguments":"{}"}`,
		} {
			_, _ = io.WriteString(w, "event: response.output_item.done\n")
			_, _ = io.WriteString(w, `data: {"type":"response.output_item.done","item":`+call+"}\n\n")
		}
		_, _ = io.WriteString(w, `data: {"type":"response.completed","response":{"status":"completed","output":null}}`+"\n\n")
	}))
	t.Cleanup(remote.Close)
	request := basicCodexGenerationRequest()
	request.Tools = []GenerationTool{{
		Name: "task.inspect", Description: "Inspect one Task.",
		InputSchema: json.RawMessage(`{"type":"object","properties":{},"additionalProperties":false}`),
	}}
	request.ToolTransport = ToolTransportNative
	if _, err := codexGenerationFixture(t, remote.URL).Generate(context.Background(), request, nil); err == nil {
		t.Fatal("two streamed Codex tool calls succeeded")
	}
}

func TestCodexGeneratorHonorsCancellation(t *testing.T) {
	started := make(chan struct{})
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, request *http.Request) {
		w.Header().Set("Content-Type", "text/event-stream")
		if flusher, ok := w.(http.Flusher); ok {
			_, _ = io.WriteString(w, `data: {"type":"response.output_text.delta","delta":"started"}`+"\n\n")
			flusher.Flush()
		}
		close(started)
		<-request.Context().Done()
	}))
	t.Cleanup(remote.Close)
	ctx, cancel := context.WithCancel(context.Background())
	result := make(chan error, 1)
	go func() {
		_, err := codexGenerationFixture(t, remote.URL).Generate(ctx, basicCodexGenerationRequest(), nil)
		result <- err
	}()
	<-started
	cancel()
	select {
	case err := <-result:
		if !errors.Is(err, context.Canceled) {
			t.Fatalf("Codex cancellation error = %v", err)
		}
	case <-time.After(time.Second):
		t.Fatal("Codex cancellation did not stop generation")
	}
}

func TestCodexGeneratorLimitsCredentialHeaderLifetime(t *testing.T) {
	generator := codexGenerationFixture(t, "http://generation.invalid")
	var captured *http.Request
	generator.client.Transport = generationRoundTripFunc(func(request *http.Request) (*http.Response, error) {
		if request.Header.Get("Authorization") != "Bearer "+codexGenerationAccessToken() {
			t.Fatal("Codex transport authorization is invalid")
		}
		captured = request
		return &http.Response{
			StatusCode: http.StatusOK,
			Header:     http.Header{"Content-Type": {"text/event-stream"}},
			Body: io.NopCloser(strings.NewReader(
				`data: {"type":"response.completed","response":{"status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":"ok"}]}]}}` + "\n\n",
			)),
			Request: request,
		}, nil
	})
	if _, err := generator.Generate(context.Background(), basicCodexGenerationRequest(), nil); err != nil {
		t.Fatal(err)
	}
	if captured == nil {
		t.Fatal("Codex generation request was not captured")
	}
	if captured.Header.Get("Authorization") != "" {
		t.Fatalf("retained Codex authorization = %q", captured.Header.Get("Authorization"))
	}
	if captured.Header.Get("originator") != codexGenerationOriginator ||
		captured.Header.Get("ChatGPT-Account-ID") != "workspace-test" {
		t.Fatalf("ordinary Codex headers were removed: %#v", captured.Header)
	}
}

func TestCodexGeneratorReturnsBoundedSafeErrors(t *testing.T) {
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
		{"stream", http.StatusOK, `data: {"type":"error","message":"` + secretMarker + `"}` + "\n\n", nil},
		{"oversized stream", http.StatusOK, strings.Repeat(":padding\n", codexGenerationResponseLimit/9+1), errCodexGenerationTooLarge},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, request *http.Request) {
				if request.URL.Path == "/oauth/token" {
					_, _ = io.WriteString(w, `{"error":"invalid_grant"}`)
					return
				}
				w.WriteHeader(test.status)
				_, _ = io.WriteString(w, test.body)
			}))
			defer remote.Close()
			_, err := codexGenerationFixture(t, remote.URL).Generate(
				context.Background(), basicCodexGenerationRequest(), nil,
			)
			if err == nil || strings.Contains(err.Error(), secretMarker) ||
				strings.Contains(err.Error(), codexGenerationAccessToken()) {
				t.Fatalf("unsafe Codex generation error = %v", err)
			}
			if test.want != nil && !errors.Is(err, test.want) {
				t.Fatalf("Codex generation error = %v, want %v", err, test.want)
			}
		})
	}
}

func TestCodexGeneratorRefreshesAfterRejectionAndRetriesOnce(t *testing.T) {
	for _, test := range []struct {
		name       string
		secondCode int
		want       error
	}{
		{name: "success", secondCode: http.StatusOK},
		{name: "second rejection", secondCode: http.StatusUnauthorized, want: ErrAuthenticationRejected},
	} {
		t.Run(test.name, func(t *testing.T) {
			var responseCalls, refreshCalls int
			remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, request *http.Request) {
				switch request.URL.Path {
				case "/oauth/token":
					refreshCalls++
					form, err := url.ParseQuery(mustReadCodexBody(t, request))
					if err != nil || form.Get("grant_type") != "refresh_token" ||
						form.Get("client_id") != codexOAuthClientID || form.Get("refresh_token") != "protected-refresh-token" {
						t.Error("Codex refresh form is invalid")
					}
					_, _ = io.WriteString(w, `{"access_token":"`+codexGenerationRefreshedAccessToken()+`"}`)
				case "/responses":
					responseCalls++
					if responseCalls == 1 {
						w.WriteHeader(http.StatusUnauthorized)
						return
					}
					if request.Header.Get("Authorization") != "Bearer "+codexGenerationRefreshedAccessToken() {
						t.Error("retry did not use refreshed access token")
					}
					w.WriteHeader(test.secondCode)
					if test.secondCode == http.StatusOK {
						_, _ = io.WriteString(w, `data: {"type":"response.completed","response":{"status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":"ok"}]}]}}`+"\n\n")
					}
				default:
					http.NotFound(w, request)
				}
			}))
			t.Cleanup(remote.Close)
			generator := codexGenerationFixture(t, remote.URL)
			_, err := generator.Generate(context.Background(), basicCodexGenerationRequest(), nil)
			if !errors.Is(err, test.want) {
				t.Fatalf("Codex generation error = %v, want %v", err, test.want)
			}
			if responseCalls != 2 || refreshCalls != 1 {
				t.Fatalf("Codex calls = responses %d, refresh %d", responseCalls, refreshCalls)
			}
			stored, err := generator.accounts.LoadCodexTokens(context.Background())
			if err != nil {
				t.Fatal(err)
			}
			_ = stored.Use(func(accessToken string, refreshToken string, _ uint64) error {
				if accessToken != codexGenerationRefreshedAccessToken() || refreshToken != "protected-refresh-token" {
					t.Fatal("Codex refresh did not retain the missing rotated token")
				}
				return nil
			})
		})
	}
}

func TestCodexGeneratorRefreshesNearJWTExpiry(t *testing.T) {
	var refreshCalls int
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, request *http.Request) {
		if request.URL.Path == "/oauth/token" {
			refreshCalls++
			_, _ = io.WriteString(w, `{"access_token":"`+codexGenerationRefreshedAccessToken()+`"}`)
			return
		}
		if request.Header.Get("Authorization") != "Bearer "+codexGenerationRefreshedAccessToken() {
			t.Error("Codex generation used an access token close to expiry")
		}
		_, _ = io.WriteString(w, `data: {"type":"response.completed","response":{"status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":"ok"}]}]}}`+"\n\n")
	}))
	t.Cleanup(remote.Close)
	generator := codexGenerationFixture(t, remote.URL)
	_, err := generator.accounts.publishCodexTokens(context.Background(), 1, CodexTokens{
		accessToken: codexGenerationAccessTokenWithExpiry(time.Now().Add(119 * time.Second)), refreshToken: "protected-refresh-token",
	}, codexModelCatalog{Profiles: []ModelProfile{{ID: "gpt-5.6-codex", Label: "GPT 5.6 Codex"}}, ClientVersion: "0.144.1"}, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	if _, err := generator.Generate(context.Background(), basicCodexGenerationRequest(), nil); err != nil {
		t.Fatal(err)
	}
	if refreshCalls != 1 {
		t.Fatalf("near-expiry token refresh count = %d", refreshCalls)
	}
}

func TestCodexRefreshRejectsStaleAccountPublication(t *testing.T) {
	generator := codexGenerationFixture(t, "https://codex.invalid")
	stored, err := generator.accounts.LoadCodexTokens(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	var oldAccessDigest codexAccessTokenDigest
	_ = stored.Use(func(accessToken string, _ string, _ uint64) error {
		oldAccessDigest = codexAccessTokenDigestFor(accessToken)
		return nil
	})
	persistence := generator.accounts.persistence.(*memoryAccountPersistence)
	_, err = generator.accounts.RefreshCodexTokens(
		context.Background(), &oldAccessDigest, time.Now(), func(context.Context, CodexTokens) (CodexTokens, error) {
			persistence.mu.Lock()
			account := persistence.accounts[codexGenerationAccountID]
			account.Metadata = credentialMetadata(2, true)
			persistence.accounts[codexGenerationAccountID] = account
			persistence.mu.Unlock()
			return CodexTokens{accessToken: "replacement", refreshToken: "replacement"}, nil
		},
	)
	if !errors.Is(err, ErrAccountConflict) {
		t.Fatalf("stale Codex refresh error = %v", err)
	}
}

func TestCodexRefreshWaitRespectsCancellation(t *testing.T) {
	generator := codexGenerationFixture(t, "https://codex.invalid")
	stored, err := generator.accounts.LoadCodexTokens(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	var digest codexAccessTokenDigest
	_ = stored.Use(func(accessToken string, _ string, _ uint64) error {
		digest = codexAccessTokenDigestFor(accessToken)
		return nil
	})
	started := make(chan struct{})
	release := make(chan struct{})
	firstDone := make(chan error, 1)
	go func() {
		_, err := generator.accounts.RefreshCodexTokens(
			context.Background(), &digest, time.Now(), func(context.Context, CodexTokens) (CodexTokens, error) {
				close(started)
				<-release
				return CodexTokens{accessToken: "replacement", refreshToken: "replacement"}, nil
			},
		)
		firstDone <- err
	}()
	<-started
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	if _, err := generator.accounts.RefreshCodexTokens(ctx, &digest, time.Now(), func(context.Context, CodexTokens) (CodexTokens, error) {
		t.Fatal("cancelled Codex refresh started another remote request")
		return CodexTokens{}, nil
	}); !errors.Is(err, context.Canceled) {
		t.Fatalf("cancelled Codex refresh error = %v", err)
	}
	close(release)
	if err := <-firstDone; err != nil {
		t.Fatalf("first Codex refresh error = %v", err)
	}
}

func TestCodexGeneratorPreservesRefreshRateLimit(t *testing.T) {
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, request *http.Request) {
		if request.URL.Path == "/oauth/token" {
			w.WriteHeader(http.StatusTooManyRequests)
			return
		}
		w.WriteHeader(http.StatusUnauthorized)
	}))
	t.Cleanup(remote.Close)
	_, err := codexGenerationFixture(t, remote.URL).Generate(context.Background(), basicCodexGenerationRequest(), nil)
	if !errors.Is(err, ErrProviderRateLimited) {
		t.Fatalf("Codex forced refresh error = %v", err)
	}
}

func TestCodexRefreshMapsOAuthRejections(t *testing.T) {
	for _, test := range []struct {
		name    string
		status  int
		body    string
		wantErr error
	}{
		{name: "string error", status: http.StatusBadRequest, body: `{"error":"invalid_grant"}`, wantErr: ErrAuthenticationRejected},
		{name: "error code", status: http.StatusBadRequest, body: `{"error":{"code":"invalid_token"}}`, wantErr: ErrAuthenticationRejected},
		{name: "error message", status: http.StatusBadRequest, body: `{"error":{"message":"Remote refresh Refresh_Token_Reused detected"}}`, wantErr: ErrAuthenticationRejected},
		{name: "status authentication", status: http.StatusUnauthorized, body: "not JSON", wantErr: ErrAuthenticationRejected},
		{name: "status rate limit", status: http.StatusTooManyRequests, body: "not JSON", wantErr: ErrProviderRateLimited},
	} {
		t.Run(test.name, func(t *testing.T) {
			remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
				w.WriteHeader(test.status)
				_, _ = io.WriteString(w, test.body)
			}))
			t.Cleanup(remote.Close)
			generator := codexGenerationFixture(t, remote.URL)
			tokens, err := generator.accounts.LoadCodexTokens(context.Background())
			if err != nil {
				t.Fatal(err)
			}
			if _, err := generator.refreshCodexTokens(context.Background(), tokens); !errors.Is(err, test.wantErr) {
				t.Fatalf("Codex refresh error = %v, want %v", err, test.wantErr)
			}
		})
	}
}

func basicCodexGenerationRequest() GenerateRequest {
	return GenerateRequest{
		AccountID: codexGenerationAccountID, Model: "gpt-5.6-codex",
		Messages: []GenerationMessage{{Role: "user", Content: "hello"}},
	}
}

func codexGenerationFixture(t *testing.T, baseURL string) *CodexGenerator {
	t.Helper()
	_, accounts, _ := codexTestService(t, baseURL, time.Millisecond, time.Second)
	_, err := accounts.publishCodexTokens(context.Background(), 0, CodexTokens{
		accessToken: codexGenerationAccessToken(), refreshToken: "protected-refresh-token",
	}, codexModelCatalog{
		Profiles:      []ModelProfile{{ID: "gpt-5.6-codex", Label: "GPT 5.6 Codex"}},
		ClientVersion: "0.144.1",
	}, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	generator, err := newCodexGenerator(
		accounts, &http.Client{Timeout: 10 * time.Second}, baseURL+"/responses",
	)
	if err != nil {
		t.Fatal(err)
	}
	generator.tokenURL = baseURL + "/oauth/token"
	return generator
}

func codexGenerationAccessToken() string {
	claims := base64.RawURLEncoding.EncodeToString([]byte(`{"https://api.openai.com/auth":{"chatgpt_account_id":"workspace-test"}}`))
	return "header." + claims + ".signature"
}

func codexGenerationRefreshedAccessToken() string {
	return strings.TrimSuffix(codexGenerationAccessToken(), ".signature") + ".refreshed"
}

func codexGenerationAccessTokenWithExpiry(expiresAt time.Time) string {
	claims := base64.RawURLEncoding.EncodeToString([]byte(`{"exp":` + strconv.FormatInt(expiresAt.Unix(), 10) + `,"https://api.openai.com/auth":{"chatgpt_account_id":"workspace-test"}}`))
	return "header." + claims + ".signature"
}

func mustReadCodexBody(t *testing.T, request *http.Request) string {
	t.Helper()
	data, err := io.ReadAll(request.Body)
	if err != nil {
		t.Fatal(err)
	}
	return string(data)
}

func TestResponsesReasoningKeepsReadableContent(t *testing.T) {
	for _, raw := range []string{
		`{"type":"reasoning","summary":[{"type":"summary_text","text":"Check calendar"}]}`,
		`{"type":"reasoning","content":[{"type":"reasoning_text","text":"Check calendar"}]}`,
	} {
		var fields map[string]json.RawMessage
		if err := json.Unmarshal([]byte(raw), &fields); err != nil {
			t.Fatal(err)
		}
		result, keep, err := normalizeCodexReasoning(json.RawMessage(raw), fields)
		if err != nil || !keep || len(result.ProviderDetails) != 1 || string(result.ProviderDetails[0]) != raw {
			t.Fatalf("readable response lost: %#v, %v", result, err)
		}
	}
}
