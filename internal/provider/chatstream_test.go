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
	if len(result.Reasoning) != 2 || !strings.Contains(string(result.Reasoning[0]), `"data":"opaque"`) || len(result.Searches) != 1 {
		t.Fatalf("unexpected reasoning or searches: %#v", result)
	}
	if len(result.Citations) != 1 || result.Citations[0].URL != "https://example.test/source" {
		t.Fatalf("unexpected citations: %#v", result.Citations)
	}
	if result.Usage.CachedInputTokens != 96 || result.Usage.TotalTokens != 108 || result.Usage.WebSearchRequests != 1 {
		t.Fatalf("unexpected usage: %#v", result.Usage)
	}
	if len(events) != 4 || events[0].Kind != TextDelta || events[2].Kind != ToolCallStarted || events[3].Kind != HostedSearchStarted {
		t.Fatalf("unexpected events: %#v", events)
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
	stream := `data: {"choices":[{"index":0,"delta":{"tool_calls":[{"id":"one","function":{"name":"first","arguments":"{}"}},{"id":"two","function":{"name":"second","arguments":"{}"}}]}}]}` + "\n\ndata: [DONE]\n\n"
	reader := newBlockingReadCloser([]byte(stream))
	result, err := ParseChatStream(context.Background(), reader, nil)
	if err != nil {
		t.Fatalf("parse stream: %v", err)
	}
	if len(result.ToolCalls) != 2 || result.ToolCalls[0].ID != "one" || result.ToolCalls[1].ID != "two" {
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
