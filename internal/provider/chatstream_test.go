package provider

import (
	"context"
	"errors"
	"io"
	"strings"
	"sync"
	"testing"
	"time"
)

func TestParseChatStreamPreservesFragmentedOutput(t *testing.T) {
	stream := strings.Join([]string{
		": keepalive\n\n",
		`data: {"reasoning_details":[42,{"type":"opaque","count":9007199254740993}]}` + "\n\n",
		`data: {"id":"chat_1","model":"model_1","choices":[{"index":0,"delta":{"content":"Hel","reasoning_details":[{"type":"reasoning.encrypted","id":"rs_1","data":"opa"}]}}]}` + "\n\n",
		`data: {"choices":[{"index":0,"delta":{"content":"lo","tool_calls":[{"index":0,"type":"function","function":{"name":"search_memory","arguments":"{\"query\":\""}}]}}]}` + "\n\n",
		`data: {"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"arguments":"trains\"}"}}],"reasoning_details":[{"type":"reasoning.encrypted","id":"rs_1","data":"que"},{"type":"reasoning.server_tool_call","id":"ws_1","name":"web.search","status":"completed","arguments":{"query":"trains"},"result":{"sources":1}}],"annotations":[{"type":"url_citation","url_citation":{"title":"Official","url":"https://example.test/source","start_index":0,"end_index":5}}]}}]}` + "\n\n",
		`data: {"choices":[],"usage":{"prompt_tokens":100,"completion_tokens":8,"total_tokens":108,"prompt_tokens_details":{"cached_tokens":96},"server_tool_use":{"web_search_requests":1}}}` + "\n\n",
		"data: [DONE]\n\n",
	}, "")
	reader := &fragmentReader{data: []byte(stream), size: 7}
	var events []StreamEvent

	result, err := ParseChatStream(context.Background(), io.NopCloser(reader), func(event StreamEvent) {
		events = append(events, event)
	})
	if err != nil {
		t.Fatalf("parse stream: %v", err)
	}
	if result.ID != "chat_1" || result.Model != "model_1" || result.Text != "Hello" {
		t.Fatalf("unexpected result header: %#v", result)
	}
	if len(result.ToolCalls) != 1 || result.ToolCalls[0].ID != "call_1" ||
		result.ToolCalls[0].Name != "search_memory" || result.ToolCalls[0].Arguments != `{"query":"trains"}` {
		t.Fatalf("unexpected tool calls: %#v", result.ToolCalls)
	}
	if len(result.Reasoning) != 4 || string(result.Reasoning[0]) != "42" ||
		!strings.Contains(string(result.Reasoning[1]), `9007199254740993`) ||
		!strings.Contains(string(result.Reasoning[2]), `"data":"opaque"`) || len(result.Searches) != 1 {
		t.Fatalf("unexpected reasoning or searches: %#v", result)
	}
	if len(result.Citations) != 1 || result.Citations[0].URL != "https://example.test/source" {
		t.Fatalf("unexpected citations: %#v", result.Citations)
	}
	if result.Usage.CachedInputTokens != 96 || result.Usage.TotalTokens != 108 || result.Usage.WebSearchRequests != 1 {
		t.Fatalf("unexpected usage: %#v", result.Usage)
	}
	if len(events) != 6 || events[0].Kind != MessageStarted || events[1].Kind != TextDelta || events[3].Kind != HostedSearchStarted || events[4].Kind != ToolCallStarted || events[5].Kind != MessageCompleted {
		t.Fatalf("unexpected events: %#v", events)
	}
}

func TestParseChatStreamNormalizesHostedSearchAndRecursiveCitations(t *testing.T) {
	stream := strings.Join([]string{
		`data: {"id":"chat_search","reasoning_details":[{"type":"reasoning.server_tool_call","id":"search_1","input":{"query":"trains"}}]}` + "\n\n",
		`data: {"reasoning_details":[{"type":"reasoning.server_tool_call","id":"search_1","status":"completed","result":{"sources":1}}]}` + "\n\n",
		`data: {"choices":[{"index":0,"delta":{"annotations":{"group":[{"type":"url_citation","url":" https://example.test/source ","title":" ","start_index":1,"end_index":4},{"url_citation":{"url":"https://example.test/source","start_index":1,"end_index":4}}]}}}],"usage":{"input_tokens":10,"output_tokens":2,"total_tokens":12,"server_tool_use":{"web_search_requests":3}}}` + "\n\n",
		"data: [DONE]\n\n",
	}, "")
	result, err := ParseChatStream(context.Background(), io.NopCloser(strings.NewReader(stream)), nil)
	if err != nil {
		t.Fatal(err)
	}
	if result.Usage.InputTokens != 10 || result.Usage.OutputTokens != 2 || len(result.Citations) != 1 ||
		result.Citations[0].Title != "https://example.test/source" {
		t.Fatalf("normalized usage and citations = %#v, %#v", result.Usage, result.Citations)
	}
	if len(result.Searches) != 3 || result.Searches[0].ID != "search_1" ||
		result.Searches[0].Name != "web.search" || result.Searches[0].Status != "completed" ||
		string(result.Searches[0].Arguments) != `{"query":"trains"}` ||
		result.Searches[1].ID != "chat_search:web_search:1" {
		t.Fatalf("normalized hosted searches = %#v", result.Searches)
	}
}

func TestParseChatStreamRejectsDuplicateJSONKeys(t *testing.T) {
	cases := []struct {
		name   string
		stream string
		want   string
	}{
		{
			name:   "duplicate key",
			stream: "data: {\"id\":\"one\",\"id\":\"two\",\"choices\":[]}\n\n",
			want:   "duplicate JSON key",
		},
		{
			name:   "typed error",
			stream: "data: {\"type\":\"error\",\"message\":\"failed\"}\n\n",
			want:   "stream error",
		},
		{
			name:   "unsafe citation",
			stream: "data: {\"choices\":[{\"index\":0,\"delta\":{\"annotations\":[{\"type\":\"url_citation\",\"url_citation\":{\"url\":\"javascript:alert(1)\"}}]}}]}\n\n",
			want:   "must use HTTP or HTTPS",
		},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			_, err := ParseChatStream(context.Background(), io.NopCloser(strings.NewReader(test.stream)), nil)
			if err == nil || !strings.Contains(err.Error(), test.want) {
				t.Fatalf("stream error = %v, want %q", err, test.want)
			}
		})
	}
}

func TestParseChatStreamPreservesHostedSearchFragmentsAndSeparatesAnonymousIDs(t *testing.T) {
	stream := strings.Join([]string{
		`data: {"reasoning_details":[{"type":"reasoning.server_tool_call","name":"custom.search","result":{"sources":1}}]}` + "\n\n",
		`data: {"reasoning_details":[{"type":"reasoning.server_tool_call","id":"0:0","name":"first.search","result":{"sources":2}}]}` + "\n\n",
		`data: {"reasoning_details":[{"type":"reasoning.server_tool_call","id":"0:0","status":"failed"}]}` + "\n\n",
		`data: {"reasoning_details":[{"type":"reasoning.server_tool_call","id":"search_2","status":"in_progress"}]}` + "\n\n",
		`data: {"reasoning_details":[{"type":"reasoning.server_tool_call","id":"search_2","status":"completed"}]}` + "\n\n",
		`data: {"reasoning_details":[{"type":"reasoning.server_tool_call","id":"search_3","result":{"sources":3}}]}` + "\n\n",
		`data: {"reasoning_details":[{"type":"reasoning.server_tool_call","id":"search_3","result":null}]}` + "\n\n",
		"data: [DONE]\n\n",
	}, "")
	var events []StreamEvent
	result, err := ParseChatStream(context.Background(), io.NopCloser(strings.NewReader(stream)), func(event StreamEvent) {
		events = append(events, event)
	})
	if err != nil {
		t.Fatal(err)
	}
	if len(result.Searches) != 4 || result.Searches[0].Name != "custom.search" ||
		result.Searches[1].ID != "0:0" || result.Searches[1].Name != "first.search" ||
		result.Searches[1].Status != "failed" || string(result.Searches[1].Result) != `{"sources":2}` ||
		result.Searches[2].Status != "completed" || string(result.Searches[2].Result) != `{"status":"completed"}` ||
		string(result.Searches[3].Result) != "null" {
		t.Fatalf("hosted searches = %#v", result.Searches)
	}
	if len(events) != 4 || events[0].ID != "anonymous:0" || events[1].ID != "0:0" ||
		events[2].ID != "search_2" || events[3].ID != "search_3" {
		t.Fatalf("hosted search events = %#v", events)
	}
}

func TestParseChatStreamIgnoresMalformedUsage(t *testing.T) {
	for _, usage := range []string{
		`{"prompt_tokens":1,"input_tokens":1}`,
		`{"prompt_tokens":null,"input_tokens":1}`,
		`{"prompt_tokens_details":{"cached_tokens":null}}`,
	} {
		stream := "data: {\"usage\":{\"prompt_tokens\":5}}\n\n" +
			"data: {\"usage\":" + usage + "}\n\ndata: [DONE]\n\n"
		result, err := ParseChatStream(context.Background(), io.NopCloser(strings.NewReader(stream)), nil)
		if err != nil {
			t.Fatalf("parse malformed usage %s: %v", usage, err)
		}
		if result.Usage != (Usage{}) {
			t.Fatalf("usage %s produced %#v", usage, result.Usage)
		}
	}
}

func TestParseChatStreamHonorsCancellation(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	reader := newBlockingReadCloser(nil)
	result := make(chan error, 1)
	go func() {
		_, err := ParseChatStream(ctx, reader, nil)
		result <- err
	}()
	cancel()
	select {
	case err := <-result:
		if !errors.Is(err, context.Canceled) {
			t.Fatalf("cancellation error = %v", err)
		}
	case <-time.After(time.Second):
		t.Fatal("cancellation did not interrupt the active read")
	}
}

func TestParseChatStreamKeepsUnindexedCallsSeparateAndStopsAtDone(t *testing.T) {
	stream := `data: {"choices":[{"index":0,"delta":{"tool_calls":[{"index":-1,"id":"one","function":{"name":"first","arguments":"{}"}},{"id":"two","function":{"name":"second","arguments":"{}"}}]}}]}` + "\n\ndata: [DONE]\n\n"
	reader := newBlockingReadCloser([]byte(stream))
	result, err := ParseChatStream(context.Background(), reader, nil)
	if err != nil {
		t.Fatalf("parse stream: %v", err)
	}
	if len(result.ToolCalls) != 2 || result.ToolCalls[0].Index != 0 || result.ToolCalls[0].ID != "one" ||
		result.ToolCalls[1].Index != 1 || result.ToolCalls[1].ID != "two" {
		t.Fatalf("unexpected tool calls: %#v", result.ToolCalls)
	}
}

type fragmentReader struct {
	data []byte
	size int
}

type blockingReadCloser struct {
	mu     sync.Mutex
	first  []byte
	closed chan struct{}
	once   sync.Once
}

func newBlockingReadCloser(first []byte) *blockingReadCloser {
	return &blockingReadCloser{first: first, closed: make(chan struct{})}
}

func (r *blockingReadCloser) Read(buffer []byte) (int, error) {
	r.mu.Lock()
	if len(r.first) > 0 {
		size := copy(buffer, r.first)
		r.first = r.first[size:]
		r.mu.Unlock()
		return size, nil
	}
	r.mu.Unlock()
	<-r.closed
	return 0, errors.New("reader closed")
}

func (r *blockingReadCloser) Close() error {
	r.once.Do(func() { close(r.closed) })
	return nil
}

func (r *fragmentReader) Read(buffer []byte) (int, error) {
	if len(r.data) == 0 {
		return 0, io.EOF
	}
	size := min(r.size, len(buffer), len(r.data))
	copy(buffer, r.data[:size])
	r.data = r.data[size:]
	if len(r.data) == 0 {
		return size, nil
	}
	return size, nil
}

func TestChatReasoningReadableFieldsAndDuplicateRepresentations(t *testing.T) {
	for _, field := range []string{"reasoning", "reasoning_content"} {
		for _, details := range []string{"", `,"reasoning_details":[{"type":"reasoning.text","text":"Check calendar"}]`, `,"reasoning_details":[{"type":"reasoning.encrypted","data":"opaque"}]`} {
			stream := `data: {"choices":[{"delta":{"` + field + `":"Check "}}]}` + "\n\n" +
				`data: {"choices":[{"delta":{"` + field + `":"calendar","content":"Reply"` + details + `}}]}` + "\n\ndata: [DONE]\n\n"
			result, err := ParseChatStream(t.Context(), io.NopCloser(strings.NewReader(stream)), nil)
			if err != nil {
				t.Fatal(err)
			}
			items := normalizeOpenRouterReasoning(result.Reasoning)
			if len(items) != 1 || len(items[0].Summary) != 1 || items[0].Summary[0] != "Check calendar" || result.Text != "Reply" {
				t.Fatalf("readable reasoning duplicated or lost: %#v", items)
			}
			if strings.Contains(details, "encrypted") && items[0].EncryptedContent != "opaque" {
				t.Fatal("replay data lost")
			}
		}
	}
}
