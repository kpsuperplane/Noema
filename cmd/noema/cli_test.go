//go:build !windows

package main

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"io"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"sync/atomic"
	"testing"
	"time"

	"github.com/coder/websocket"
	"github.com/coder/websocket/wsjson"
	"github.com/vektah/gqlparser/v2"
	"github.com/vektah/gqlparser/v2/ast"
)

func cliTestSocket(t *testing.T, handler http.Handler) string {
	t.Helper()
	path := filepath.Join(t.TempDir(), "api.sock")
	listener, err := net.Listen("unix", path)
	if err != nil {
		t.Fatal(err)
	}
	server := &http.Server{Handler: handler}
	go func() { _ = server.Serve(listener) }()
	t.Cleanup(func() { _ = server.Close() })
	return path
}
func runCLI(t *testing.T, input string, args ...string) (string, error) {
	t.Helper()
	var output, diagnostics bytes.Buffer
	ctx, cancel := context.WithTimeout(context.Background(), 3*time.Second)
	defer cancel()
	err := cliCommand(strings.NewReader(input), &output, &diagnostics).Run(ctx, append([]string{"noema"}, args...))
	return output.String(), err
}

func TestCLIUsesSelectedSocketWithoutOpeningHome(t *testing.T) {
	root := filepath.Join(t.TempDir(), "absent")
	t.Setenv("NOEMA_HOME", root)
	path := cliTestSocket(t, http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Header.Get("Authorization") != "" || r.Header.Get("Cookie") != "" {
			t.Error("client sent credentials")
		}
		_, _ = io.WriteString(w, `{"data":{"localStatus":{"primaryAgentDisplayName":"Agent / ordinary:authorization"}}}`)
	}))
	t.Setenv("NOEMA_SOCKET", path)
	for _, args := range [][]string{{"status"}, {"--socket", path, "status"}} {
		output, err := runCLI(t, "", args...)
		if err != nil || !strings.Contains(output, "ordinary:authorization") {
			t.Fatalf("%v: %s %v", args, output, err)
		}
	}
	if _, err := os.Stat(root); !errors.Is(err, os.ErrNotExist) {
		t.Fatal("client initialized home")
	}
	_, err := runCLI(t, "", "--socket", filepath.Join(root, "missing"), "status")
	if err == nil || !strings.Contains(err.Error(), "permissions") {
		t.Fatalf("connection error: %v", err)
	}
}

func TestCLIInputAndExitBehavior(t *testing.T) {
	t.Setenv("NOEMA_SOCKET", "/unused")
	for _, args := range [][]string{{"unknown"}, {"status", "extra"}, {"api", "--unknown"}, {"api", "--file", "-", "{x}"}, {"api", "--file", "-", "--variables-file", "-"}, {"api", "{x}", "--variables", "[]"}, {"api", "{x}", "--variables", "{} {}"}, {"--desktop-sidecar"}, {"serve", "--desktop-sidecar"}} {
		_, err := runCLI(t, "", args...)
		if err == nil || commandExitCode(context.Background(), err) != 2 {
			t.Fatalf("%v: %v", args, err)
		}
	}
	output, err := runCLI(t, "", "--help")
	if err != nil || !strings.Contains(output, "tasks") {
		t.Fatalf("help: %s %v", output, err)
	}
	path := cliTestSocket(t, http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		var request apiRequest
		if err := json.NewDecoder(r.Body).Decode(&request); err != nil {
			t.Error(err)
		}
		if request.Variables["text"] != "exact\n日本語" {
			t.Errorf("variables: %#v", request.Variables)
		}
		_, _ = io.WriteString(w, `{"data":{"value":"kept"},"errors":[{"message":"denied"}]}`)
	}))
	output, err = runCLI(t, "query Test($text:String!){value(text:$text)}", "--socket", path, "api", "--file", "-", "--variables", `{"text":"exact\n日本語"}`, "--operation-name", "Test")
	if err == nil || commandExitCode(context.Background(), err) != 1 || !strings.Contains(output, `"value":"kept"`) {
		t.Fatalf("partial result: %s %v", output, err)
	}
}

func TestCLIOperationsMatchServerSchema(t *testing.T) {
	source, err := os.ReadFile("../../graphql/schema.graphql")
	if err != nil {
		t.Fatal(err)
	}
	schema, err := gqlparser.LoadSchema(&ast.Source{Input: string(source)})
	if err != nil {
		t.Fatal(err)
	}
	queries := []string{taskListQuery, taskReadQuery, taskWatchQuery, chatReadQuery, chatSendQuery, chatWatchQuery, interventionQuery}
	for name, input := range map[string]string{"captureTask": "CaptureTaskInput", "queueTask": "QueueTaskInput", "runScheduledTaskNow": "RunScheduledTaskNowInput", "cancelTask": "CancelTaskInput"} {
		queries = append(queries, taskMutationQuery(name, input))
	}
	for _, query := range queries {
		if _, err := gqlparser.LoadQuery(schema, query); err != nil {
			t.Errorf("%s: %v", query, err)
		}
	}
}

func TestCLITaskConflictsNeverRepeatWrites(t *testing.T) {
	var writes atomic.Int32
	path := cliTestSocket(t, http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		var request apiRequest
		_ = json.NewDecoder(r.Body).Decode(&request)
		if strings.HasPrefix(request.Query, "mutation") {
			writes.Add(1)
			v := request.Variables["input"].(map[string]any)
			if v["expectedRevision"] != float64(7) || v["expectedGeneration"] != float64(3) || v["clientMutationId"] != "fixed" {
				t.Errorf("lost conflict checks: %#v", v)
			}
			if !strings.Contains(request.Query, "runScheduledTaskNow") {
				t.Error("scheduled task used queue")
			}
			_, _ = io.WriteString(w, `{"errors":[{"message":"revision conflict"}]}`)
			return
		}
		_, _ = io.WriteString(w, `{"data":{"task":{"revision":7,"generation":3,"validActions":["RUN_NOW"]}}}`)
	}))
	_, err := runCLI(t, "", "--socket", path, "tasks", "run", "task:one", "--request-id", "fixed")
	if err == nil || writes.Load() != 1 {
		t.Fatalf("writes=%d error=%v", writes.Load(), err)
	}
}

func TestCLIChatSubscribesBeforeSendingAndMatchesCompletion(t *testing.T) {
	for _, mode := range []string{"complete", "failed", "needs_input", "old_input", "disconnect", "no_wait"} {
		t.Run(mode, func(t *testing.T) {
			var ready, sent atomic.Bool
			accepted := make(chan struct{})
			path := cliTestSocket(t, http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				if r.URL.Path != "/graphql/ws" {
					var request apiRequest
					_ = json.NewDecoder(r.Body).Decode(&request)
					if strings.Contains(request.Query, "pendingHumanInterventions") {
						items := `[]`
						if mode == "old_input" || sent.Load() && mode == "needs_input" {
							items = `[{"__typename":"GovernedAction","actionId":"action:one"}]`
						}
						_, _ = io.WriteString(w, `{"data":{"pendingHumanInterventions":`+items+`}}`)
						return
					}
					if !ready.Load() && mode != "no_wait" {
						t.Error("message sent before readiness")
					}
					if sent.Swap(true) {
						t.Error("message sent twice")
					}
					_, _ = io.WriteString(w, `{"data":{"sendConversationTurn":{"conversationId":"chat:one","clientMessageId":"mine"}}}`)
					close(accepted)
					return
				}
				connection, err := websocket.Accept(w, r, &websocket.AcceptOptions{Subprotocols: []string{"graphql-transport-ws"}})
				if err != nil {
					t.Error(err)
					return
				}
				defer connection.CloseNow()
				ctx, cancel := context.WithTimeout(r.Context(), 2*time.Second)
				defer cancel()
				var frame socketFrame
				if wsjson.Read(ctx, connection, &frame) != nil {
					return
				}
				_ = wsjson.Write(ctx, connection, socketFrame{Type: "connection_ack"})
				if wsjson.Read(ctx, connection, &frame) != nil {
					return
				}
				emit := func(event string) {
					_ = wsjson.Write(ctx, connection, socketFrame{ID: "1", Type: "next", Payload: json.RawMessage(`{"data":{"conversationEvents":` + event + `}}`)})
				}
				ready.Store(true)
				emit(`{"__typename":"SubscriptionReadyEvent"}`)
				select {
				case <-accepted:
				case <-ctx.Done():
					return
				}
				emit(`{"__typename":"TurnCompletedEvent","clientMessageId":"another"}`)
				emit(`{"__typename":"ConversationItemEvent","clientMessageId":"mine","item":{"__typename":"UserText","text":"hi"}}`)
				if mode == "disconnect" {
					return
				}
				if mode == "needs_input" {
					emit(`{"__typename":"HumanInterventionsChangedEvent"}`)
				} else {
					if mode == "old_input" {
						emit(`{"__typename":"HumanInterventionsChangedEvent"}`)
					}
					if mode == "failed" {
						emit(`{"__typename":"ConversationItemEvent","clientMessageId":"mine","item":{"__typename":"ErrorNotice"}}`)
					}
					emit(`{"__typename":"AssistantTextDeltaEvent","delta":"hello"}`)
					emit(`{"__typename":"TurnCompletedEvent","clientMessageId":"mine"}`)
				}
				_ = wsjson.Read(ctx, connection, &frame)
			}))
			args := []string{"--socket", path, "chat", "--conversation", "chat:one", "send", "hi", "--request-id", "mine"}
			if mode == "no_wait" {
				args = append(args, "--no-wait")
			}
			output, err := runCLI(t, "", args...)
			wantError := mode == "failed" || mode == "disconnect"
			if (err != nil) != wantError || !sent.Load() {
				t.Fatalf("%s %v", output, err)
			}
			if (mode == "complete" || mode == "old_input") && !strings.Contains(output, "hello") {
				t.Fatal("stopped on another turn")
			}
			if mode == "needs_input" && !strings.Contains(output, "needs_input") {
				t.Fatal("missing human input status")
			}
		})
	}
}

func TestCLIStreamCancellation(t *testing.T) {
	ready := make(chan struct{})
	path := cliTestSocket(t, http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		connection, err := websocket.Accept(w, r, &websocket.AcceptOptions{Subprotocols: []string{"graphql-transport-ws"}})
		if err != nil {
			return
		}
		defer connection.CloseNow()
		var frame socketFrame
		_ = wsjson.Read(r.Context(), connection, &frame)
		_ = wsjson.Write(r.Context(), connection, socketFrame{Type: "connection_ack"})
		_ = wsjson.Read(r.Context(), connection, &frame)
		close(ready)
		_ = wsjson.Read(r.Context(), connection, &frame)
	}))
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	go func() { <-ready; cancel() }()
	client := newLocalClient(path, time.Second, io.Discard)
	defer client.http.CloseIdleConnections()
	err := client.stream(ctx, apiRequest{Query: taskWatchQuery}, nil)
	if err == nil || commandExitCode(ctx, err) != 130 {
		t.Fatalf("cancel: %v", err)
	}
}

func TestCLIStreamCompletionAndErrors(t *testing.T) {
	for _, mode := range []string{"complete", "error", "invalid"} {
		t.Run(mode, func(t *testing.T) {
			path := cliTestSocket(t, http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				connection, err := websocket.Accept(w, r, &websocket.AcceptOptions{Subprotocols: []string{"graphql-transport-ws"}})
				if err != nil {
					return
				}
				defer connection.CloseNow()
				var frame socketFrame
				_ = wsjson.Read(r.Context(), connection, &frame)
				_ = wsjson.Write(r.Context(), connection, socketFrame{Type: "ping", Payload: json.RawMessage(`{"value":7}`)})
				_ = wsjson.Read(r.Context(), connection, &frame)
				if frame.Type != "pong" || string(frame.Payload) != `{"value":7}` {
					t.Error("ping payload was not preserved")
				}
				_ = wsjson.Write(r.Context(), connection, socketFrame{Type: "connection_ack"})
				_ = wsjson.Read(r.Context(), connection, &frame)
				if mode == "error" {
					_ = wsjson.Write(r.Context(), connection, socketFrame{ID: "1", Type: "error", Payload: json.RawMessage(`[{"message":"denied"}]`)})
				} else if mode == "invalid" {
					_ = wsjson.Write(r.Context(), connection, socketFrame{Type: "unexpected"})
				} else {
					_ = wsjson.Write(r.Context(), connection, socketFrame{ID: "1", Type: "next", Payload: json.RawMessage(`{"data":{"value":"kept"}}`)})
					_ = wsjson.Write(r.Context(), connection, socketFrame{ID: "1", Type: "complete"})
				}
				_ = wsjson.Read(r.Context(), connection, &frame)
			}))
			output, err := runCLI(t, "", "--socket", path, "api", "--subscribe", "subscription{value}")
			if (err != nil) != (mode != "complete") {
				t.Fatalf("%s %v", output, err)
			}
			if mode == "complete" && !strings.Contains(output, "kept") || mode == "error" && !strings.Contains(output, "denied") {
				t.Fatalf("lost result: %s", output)
			}
		})
	}
}
