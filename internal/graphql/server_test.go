package graphql

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/coder/websocket"
	"github.com/coder/websocket/wsjson"
	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestCaptureAndReadTask(t *testing.T) {
	server := newTestServer(t)

	mutation := postGraphQL(t, server.URL, `
mutation Capture($input: CaptureTaskInput!) {
  captureTask(input: $input) {
    clientMutationId
    eventCursor
    task { taskId title revision stage { key behavior } }
  }
}`, map[string]any{
		"input": map[string]any{
			"workspaceId":      "workspace:personal",
			"title":            "Audit dependencies",
			"executorAgentId":  "agent:task-executor",
			"clientMutationId": "capture-1",
		},
	})
	capture := mutation.Data["captureTask"].(map[string]any)
	task := capture["task"].(map[string]any)
	taskID := task["taskId"].(string)
	if capture["clientMutationId"] != "capture-1" || capture["eventCursor"] != "1" {
		t.Fatalf("unexpected mutation payload: %#v", capture)
	}
	if task["title"] != "Audit dependencies" || task["revision"] != float64(1) {
		t.Fatalf("unexpected captured Task: %#v", task)
	}
	stage := task["stage"].(map[string]any)
	if stage["key"] != "inbox" || stage["behavior"] != "INTAKE" {
		t.Fatalf("unexpected captured stage: %#v", stage)
	}

	query := postGraphQL(t, server.URL, `
query Task($taskId: String!) {
  task(taskId: $taskId) { taskId title revision stage { key behavior } }
}`, map[string]any{"taskId": taskID})
	readTask := query.Data["task"].(map[string]any)
	if readTask["taskId"] != taskID || readTask["title"] != "Audit dependencies" {
		t.Fatalf("unexpected Task read: %#v", readTask)
	}
}

func TestCaptureRejectsUnsupportedTaskDocument(t *testing.T) {
	taskStore := openTestStore(t)
	_, err := NewResolver(taskStore).captureTask(context.Background(), model.CaptureTaskInput{
		WorkspaceID:      "workspace:personal",
		Title:            "Documented Task",
		TaskDocument:     "This slice does not persist this text.",
		ClientMutationID: "capture-document",
	})
	if err == nil || !strings.Contains(err.Error(), "title-only Task capture") {
		t.Fatalf("unexpected capture error: %v", err)
	}
}

func TestTaskEventsUseGraphQLTransportWS(t *testing.T) {
	taskStore := openTestStore(t)
	task, err := taskStore.CreateTask(context.Background(), "Stream events", time.Now())
	if err != nil {
		t.Fatal(err)
	}
	server := httptest.NewServer(NewHandler(NewResolver(taskStore)))
	t.Cleanup(server.Close)

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	connection, response, err := websocket.Dial(
		ctx,
		"ws"+strings.TrimPrefix(server.URL, "http"),
		&websocket.DialOptions{Subprotocols: []string{"graphql-transport-ws"}},
	)
	if err != nil {
		if response != nil {
			t.Fatalf("dial websocket: %v (%s)", err, response.Status)
		}
		t.Fatalf("dial websocket: %v", err)
	}
	t.Cleanup(func() { _ = connection.CloseNow() })
	if connection.Subprotocol() != "graphql-transport-ws" {
		t.Fatalf("unexpected websocket protocol %q", connection.Subprotocol())
	}

	writeWS(t, ctx, connection, map[string]any{"type": "connection_init"})
	if message := readWS(t, ctx, connection); message["type"] != "connection_ack" {
		t.Fatalf("unexpected connection response: %#v", message)
	}
	writeWS(t, ctx, connection, map[string]any{
		"id":   "task-events",
		"type": "subscribe",
		"payload": map[string]any{
			"query": `subscription TaskEvents($taskId: String!) {
  taskEvents(taskId: $taskId, after: "0") { cursor kind taskId task { title } }
}`,
			"variables": map[string]any{"taskId": task.ID},
		},
	})
	message := readWS(t, ctx, connection)
	if message["type"] != "next" || message["id"] != "task-events" {
		t.Fatalf("unexpected subscription response: %#v", message)
	}
	payload := message["payload"].(map[string]any)
	data := payload["data"].(map[string]any)
	event := data["taskEvents"].(map[string]any)
	if event["kind"] != "task.captured" || event["taskId"] != task.ID {
		t.Fatalf("unexpected Task event: %#v", event)
	}
}

func TestUnimplementedFieldReturnsClearError(t *testing.T) {
	server := newTestServer(t)
	body, err := json.Marshal(map[string]any{"query": `{ localStatus { primaryAgentDisplayName } }`})
	if err != nil {
		t.Fatal(err)
	}
	response, err := http.Post(server.URL, "application/json", bytes.NewReader(body))
	if err != nil {
		t.Fatal(err)
	}
	defer response.Body.Close()
	var result graphQLResponse
	if err := json.NewDecoder(response.Body).Decode(&result); err != nil {
		t.Fatal(err)
	}
	if len(result.Errors) != 1 || !strings.Contains(result.Errors[0].Message, "not implemented: LocalStatus") {
		t.Fatalf("unexpected GraphQL errors: %#v", result.Errors)
	}
}

type graphQLResponse struct {
	Data   map[string]any `json:"data"`
	Errors []struct {
		Message string `json:"message"`
	} `json:"errors"`
}

func newTestServer(t *testing.T) *httptest.Server {
	t.Helper()
	taskStore := openTestStore(t)
	server := httptest.NewServer(NewHandler(NewResolver(taskStore)))
	t.Cleanup(server.Close)
	return server
}

func openTestStore(t *testing.T) *store.Store {
	t.Helper()
	taskStore, err := store.Open(context.Background(), filepath.Join(t.TempDir(), "noema.db"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = taskStore.Close() })
	return taskStore
}

func postGraphQL(
	t *testing.T,
	url string,
	query string,
	variables map[string]any,
) graphQLResponse {
	t.Helper()
	body, err := json.Marshal(map[string]any{"query": query, "variables": variables})
	if err != nil {
		t.Fatal(err)
	}
	response, err := http.Post(url, "application/json", bytes.NewReader(body))
	if err != nil {
		t.Fatal(err)
	}
	defer response.Body.Close()
	if response.StatusCode != http.StatusOK {
		t.Fatalf("GraphQL status: %s", response.Status)
	}
	var result graphQLResponse
	if err := json.NewDecoder(response.Body).Decode(&result); err != nil {
		t.Fatal(err)
	}
	if len(result.Errors) > 0 {
		t.Fatalf("GraphQL error: %s", result.Errors[0].Message)
	}
	return result
}

func writeWS(t *testing.T, ctx context.Context, connection *websocket.Conn, message any) {
	t.Helper()
	if err := wsjson.Write(ctx, connection, message); err != nil {
		t.Fatal(err)
	}
}

func readWS(t *testing.T, ctx context.Context, connection *websocket.Conn) map[string]any {
	t.Helper()
	var message map[string]any
	if err := wsjson.Read(ctx, connection, &message); err != nil {
		t.Fatal(fmt.Errorf("read websocket message: %w", err))
	}
	return message
}
