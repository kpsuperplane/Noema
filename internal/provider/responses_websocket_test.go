package provider

import (
	"context"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"net/http/httptest"
	"sync/atomic"
	"testing"

	"github.com/coder/websocket"
	"github.com/coder/websocket/wsjson"
)

func TestOpenAIResponsesWebSocketSessionReusesIncrementalConnection(t *testing.T) {
	requests := make(chan map[string]any, 2)
	fallbacks := make(chan map[string]any, 1)
	var connections atomic.Int32
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, request *http.Request) {
		if request.Header.Get("Upgrade") != "websocket" {
			var wire map[string]any
			_ = json.NewDecoder(request.Body).Decode(&wire)
			fallbacks <- wire
			_, _ = io.WriteString(w, `data: {"type":"response.completed","response":{"id":"resp_3","status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":"third"}]}]}}`+"\n\n")
			return
		}
		connections.Add(1)
		connection, err := websocket.Accept(w, request, nil)
		if err != nil {
			t.Errorf("accept WebSocket: %v", err)
			return
		}
		defer connection.CloseNow()
		for index, id := range []string{"resp_1", "resp_2"} {
			var wire map[string]any
			if err := wsjson.Read(request.Context(), connection, &wire); err != nil {
				t.Errorf("read request: %v", err)
				return
			}
			requests <- wire
			text := []string{"first", "second"}[index]
			terminal := map[string]any{"type": "response.completed", "response": map[string]any{
				"id": id, "status": "completed", "output": []any{map[string]any{
					"type": "message", "content": []any{map[string]any{"type": "output_text", "text": text}},
				}},
			}}
			if err := wsjson.Write(request.Context(), connection, terminal); err != nil {
				t.Errorf("write response: %v", err)
				return
			}
		}
	}))
	t.Cleanup(remote.Close)
	generator, _ := openAIGenerationFixture(t, remote.URL, "session-key")
	session := generator.OpenGenerationSession()
	t.Cleanup(func() { _ = session.Close() })
	firstResult, err := session.Generate(context.Background(), basicOpenAIGenerationRequest(), nil)
	if err != nil || firstResult.ID != "resp_1" {
		t.Fatalf("initial response = %#v, %v", firstResult, err)
	}
	second := basicOpenAIGenerationRequest()
	second.Messages = []GenerationMessage{{Role: "user", Content: "again"}}
	second.ReplayMessages = []GenerationMessage{
		{Role: "user", Content: "hello"}, {Role: "assistant", Content: "first"},
		{Role: "user", Content: "again"},
	}
	second.PreviousResponseID = "caller-response-id"
	secondResult, err := session.Generate(context.Background(), second, nil)
	if err != nil || secondResult.ID != "resp_2" || connections.Load() != 1 {
		t.Fatalf("continued response = %#v, connections = %d, %v", secondResult, connections.Load(), err)
	}
	initial, continued := <-requests, <-requests
	if initial["type"] != "response.create" || initial["store"] != true ||
		continued["previous_response_id"] != "resp_1" || continued["store"] != true ||
		len(continued["input"].([]any)) != 1 {
		t.Fatalf("session requests = %#v, %#v", initial, continued)
	}
	third := second
	third.Messages = []GenerationMessage{{Role: "user", Content: "third"}}
	third.ReplayMessages = append(second.ReplayMessages, third.Messages...)
	if result, err := session.Generate(context.Background(), third, nil); err != nil || result.ID != "resp_3" {
		t.Fatalf("HTTP continuation = %#v, %v", result, err)
	}
	fallback := <-fallbacks
	if fallback["previous_response_id"] != "resp_2" || len(fallback["input"].([]any)) != 1 {
		t.Fatalf("HTTP continuation request = %#v", fallback)
	}
}
func TestCodexResponsesWebSocketFallbackUsesFullReplay(t *testing.T) {
	httpBodies := make(chan map[string]any, 1)
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, request *http.Request) {
		if request.Header.Get("Upgrade") == "websocket" {
			connection, err := websocket.Accept(w, request, nil)
			if err != nil {
				t.Errorf("accept WebSocket: %v", err)
				return
			}
			var wire map[string]any
			if err := wsjson.Read(request.Context(), connection, &wire); err != nil {
				t.Errorf("read request: %v", err)
				return
			}
			_ = wsjson.Write(request.Context(), connection, map[string]any{
				"type": "response.completed", "response": map[string]any{
					"id": "resp_1", "status": "completed", "output": []any{map[string]any{
						"type": "message", "content": []any{map[string]any{"type": "output_text", "text": "done"}},
					}},
				},
			})
			_ = connection.Close(websocket.StatusInternalError, "closed")
			return
		}
		var body map[string]any
		if err := json.NewDecoder(request.Body).Decode(&body); err != nil {
			t.Errorf("decode HTTP fallback: %v", err)
			return
		}
		httpBodies <- body
		_, _ = io.WriteString(w, `data: {"type":"response.completed","response":{"id":"resp_http","status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":"done"}]}]}}`+"\n\n")
	}))
	t.Cleanup(remote.Close)
	session := codexGenerationFixture(t, remote.URL).OpenGenerationSession()
	t.Cleanup(func() { _ = session.Close() })
	first := basicCodexGenerationRequest()
	if _, err := session.Generate(context.Background(), first, nil); err != nil {
		t.Fatal(err)
	}
	second := basicCodexGenerationRequest()
	second.Messages = []GenerationMessage{{Role: "user", Content: "again"}}
	second.ReplayMessages = []GenerationMessage{
		{Role: "user", Content: "hello"}, {Role: "assistant", Content: "done"},
		{Role: "user", Content: "again"},
	}
	second.PreviousResponseID = "resp_1"
	result, err := session.Generate(context.Background(), second, nil)
	if err != nil || result.ID != "resp_http" {
		t.Fatalf("fallback response = %#v, %v", result, err)
	}
	body := <-httpBodies
	if body["previous_response_id"] != nil || body["store"] != false || len(body["input"].([]any)) != 3 {
		t.Fatalf("fallback request = %#v", body)
	}
}
func TestResponsesWebSocketDoesNotReplayUncertainOrCancelledOutput(t *testing.T) {
	for _, test := range []struct {
		name, event string
		cancel      bool
	}{
		{name: "output then close", event: `{"type":"response.output_text.delta","output_index":0,"delta":"partial"}`},
		{name: "cancel", cancel: true},
	} {
		t.Run(test.name, func(t *testing.T) {
			started := make(chan struct{})
			var httpCalls atomic.Int32
			remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, request *http.Request) {
				if request.Header.Get("Upgrade") != "websocket" {
					httpCalls.Add(1)
					return
				}
				connection, err := websocket.Accept(w, request, nil)
				if err != nil {
					return
				}
				defer connection.CloseNow()
				var wire map[string]any
				if wsjson.Read(request.Context(), connection, &wire) != nil {
					return
				}
				close(started)
				if test.event != "" {
					_ = connection.Write(request.Context(), websocket.MessageText, []byte(test.event))
					_ = connection.Close(websocket.StatusInternalError, "closed")
					return
				}
				<-request.Context().Done()
			}))
			generator, _ := openAIGenerationFixture(t, remote.URL, "session-key")
			session := generator.OpenGenerationSession()
			ctx, cancel := context.WithCancel(context.Background())
			result := make(chan error, 1)
			go func() {
				_, err := session.Generate(ctx, basicOpenAIGenerationRequest(), nil)
				result <- err
			}()
			<-started
			if test.cancel {
				cancel()
			}
			err := <-result
			cancel()
			_ = session.Close()
			remote.Close()
			if err == nil || httpCalls.Load() != 0 || (test.cancel && !errors.Is(err, context.Canceled)) {
				t.Fatalf("result error = %v, HTTP calls = %d", err, httpCalls.Load())
			}
		})
	}
}
