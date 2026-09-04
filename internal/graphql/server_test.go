package graphql

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io/fs"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/coder/websocket"
	"github.com/coder/websocket/wsjson"
	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestCaptureAndReadTask(t *testing.T) {
	resolver := openTestResolver(t)
	server := httptest.NewServer(NewHandler(resolver))
	t.Cleanup(server.Close)
	document := "## Objective\n\nAudit every server dependency.\n"
	digest := sha256.Sum256([]byte(document))
	wantDigest := hex.EncodeToString(digest[:])

	mutation := postGraphQL(t, server.URL, `
mutation Capture($input: CaptureTaskInput!) {
  captureTask(input: $input) {
    clientMutationId
    eventCursor
    task { taskId title taskDocument taskDocumentDigest revision stage { key behavior } }
  }
}`, map[string]any{
		"input": map[string]any{
			"workspaceId":      "workspace:personal",
			"title":            "Audit dependencies",
			"taskDocument":     document,
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
	if task["taskDocument"] != document || task["taskDocumentDigest"] != wantDigest {
		t.Fatalf("unexpected captured Task document: %#v", task)
	}
	stored, err := resolver.home.ReadFile(filepath.Join(
		"tasks",
		strings.TrimPrefix(taskID, "task:"),
		"TASK.md",
	))
	if err != nil {
		t.Fatalf("read stored TASK.md: %v", err)
	}
	if string(stored) != document {
		t.Fatalf("stored TASK.md = %q, want %q", stored, document)
	}
	stage := task["stage"].(map[string]any)
	if stage["key"] != "inbox" || stage["behavior"] != "INTAKE" {
		t.Fatalf("unexpected captured stage: %#v", stage)
	}

	query := postGraphQL(t, server.URL, `
query Task($taskId: String!) {
  task(taskId: $taskId) {
    taskId title taskDocument taskDocumentDigest revision stage { key behavior }
  }
}`, map[string]any{"taskId": taskID})
	readTask := query.Data["task"].(map[string]any)
	if readTask["taskId"] != taskID || readTask["title"] != "Audit dependencies" {
		t.Fatalf("unexpected Task read: %#v", readTask)
	}
	if readTask["taskDocument"] != document || readTask["taskDocumentDigest"] != wantDigest {
		t.Fatalf("unexpected read Task document: %#v", readTask)
	}
	if got := taskDocumentPreview(strings.Repeat("é", 281)); got != strings.Repeat("é", 280) {
		t.Fatalf("Unicode Task preview has %d characters, want 280", len([]rune(got)))
	}
}

func TestCaptureDoesNotExposeTaskWhenDocumentFails(t *testing.T) {
	resolver := openTestResolver(t)
	if err := resolver.home.WriteFile("tasks", []byte("blocks the Task root"), 0o600); err != nil {
		t.Fatalf("block Task root: %v", err)
	}
	_, err := resolver.captureTask(context.Background(), model.CaptureTaskInput{
		WorkspaceID:      "workspace:personal",
		Title:            "Documented Task",
		TaskDocument:     "This must not get a Task row.",
		ClientMutationID: "capture-document",
	})
	if err == nil || !strings.Contains(err.Error(), "task directory") {
		t.Fatalf("unexpected capture error: %v", err)
	}
	if err := resolver.home.Remove("tasks"); err != nil {
		t.Fatalf("remove Task root blocker: %v", err)
	}
	_, err = resolver.captureTask(context.Background(), model.CaptureTaskInput{
		WorkspaceID:      "workspace:personal",
		Title:            "Invalid document",
		TaskDocument:     string([]byte{0xff}),
		ClientMutationID: "capture-invalid-document",
	})
	if err == nil || !strings.Contains(err.Error(), "not valid UTF-8") {
		t.Fatalf("unexpected invalid document error: %v", err)
	}
	_, err = resolver.captureTask(context.Background(), model.CaptureTaskInput{
		WorkspaceID:      "workspace:personal",
		Title:            "Oversized document",
		TaskDocument:     strings.Repeat("x", 64*1024+1),
		ClientMutationID: "capture-oversized-document",
	})
	if err == nil || !strings.Contains(err.Error(), "64 KiB") {
		t.Fatalf("unexpected oversized document error: %v", err)
	}
	_, err = resolver.captureTask(context.Background(), model.CaptureTaskInput{
		WorkspaceID:      "workspace:personal",
		Title:            " ",
		TaskDocument:     "The store must reject this title.",
		ClientMutationID: "capture-invalid-title",
	})
	if err == nil || !strings.Contains(err.Error(), "task title cannot be empty") {
		t.Fatalf("unexpected invalid title error: %v", err)
	}
	entries, err := fs.ReadDir(resolver.home.FS(), filepath.Join("tasks", ".pending"))
	if err != nil {
		t.Fatalf("read pending Task root after store failure: %v", err)
	}
	if len(entries) != 0 {
		t.Fatalf("store failure left pending Task directories: %#v", entries)
	}

	payload, err := resolver.captureTask(context.Background(), model.CaptureTaskInput{
		WorkspaceID:      "workspace:personal",
		Title:            "First stored Task",
		TaskDocument:     "Stored after the failed capture.",
		ClientMutationID: "capture-after-failure",
	})
	if err != nil {
		t.Fatalf("capture after document failure: %v", err)
	}
	if payload.EventCursor != "1" {
		t.Fatalf("event cursor = %q, want first stored event", payload.EventCursor)
	}

	orphanID, err := store.NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	if _, err := home.CreatePendingTaskDocument(resolver.home, orphanID, "Remove this orphan."); err != nil {
		t.Fatalf("stage orphan Task document: %v", err)
	}
	if err := home.RecoverTaskDocuments(resolver.home, func(taskID string) (bool, error) {
		return resolver.Store.TaskExists(context.Background(), taskID)
	}); err != nil {
		t.Fatalf("remove orphan Task document: %v", err)
	}
	if _, err := home.ReadTaskDocument(resolver.home, orphanID); !errors.Is(err, fs.ErrNotExist) {
		t.Fatalf("orphan Task document remains: %v", err)
	}

	recoveryID, err := store.NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	const recoveryDocument = "Promote this committed Task document."
	if _, err := home.CreatePendingTaskDocument(resolver.home, recoveryID, recoveryDocument); err != nil {
		t.Fatalf("stage recoverable Task document: %v", err)
	}
	if _, err := resolver.Store.CreateTask(
		context.Background(),
		recoveryID,
		"Recover document",
		time.Now(),
	); err != nil {
		t.Fatalf("create recoverable Task row: %v", err)
	}
	if err := home.RecoverTaskDocuments(resolver.home, func(taskID string) (bool, error) {
		return resolver.Store.TaskExists(context.Background(), taskID)
	}); err != nil {
		t.Fatalf("promote recoverable Task document: %v", err)
	}
	recovered, err := home.ReadTaskDocument(resolver.home, recoveryID)
	if err != nil || recovered.Content != recoveryDocument {
		t.Fatalf("recovered Task document = %#v, %v", recovered, err)
	}
}

func TestTaskEventsUseGraphQLTransportWS(t *testing.T) {
	resolver := openTestResolver(t)
	taskID, err := store.NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	const document = "Stream this document preview."
	if _, err := home.CreatePendingTaskDocument(resolver.home, taskID, document); err != nil {
		t.Fatal(err)
	}
	task, err := resolver.Store.CreateTask(context.Background(), taskID, "Stream events", time.Now())
	if err != nil {
		t.Fatal(err)
	}
	if err := home.CommitTaskDocument(resolver.home, taskID); err != nil {
		t.Fatal(err)
	}
	server := httptest.NewServer(NewHandler(resolver))
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
  taskEvents(taskId: $taskId, after: "0") {
    cursor kind taskId task { title taskDocumentPreview }
  }
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
	eventTask := event["task"].(map[string]any)
	if eventTask["taskDocumentPreview"] != document {
		t.Fatalf("unexpected Task document preview: %#v", eventTask)
	}
}

func TestUnimplementedFieldReturnsClearError(t *testing.T) {
	server := newTestServer(t)
	body, err := json.Marshal(map[string]any{"query": `{ agents { agentId } }`})
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
	if len(result.Errors) != 1 || !strings.Contains(result.Errors[0].Message, "not implemented: Agents") {
		t.Fatalf("unexpected GraphQL errors: %#v", result.Errors)
	}
}

func TestOpenRouterGraphQLStartQueryAndSubscriptionKeepClientShape(t *testing.T) {
	resolver := openProviderTestResolver(t)
	server := httptest.NewServer(NewHandler(resolver))
	t.Cleanup(server.Close)
	root := postGraphQL(t, server.URL, `{
  providerAccountCatalog { providerKind preferredAuthMethod supportedAuthMethods capabilities { capabilityId status } }
  providerAccounts { providerAccountId providerKind authMethod status capabilities { capabilityId status } }
}`, nil)
	catalog := root.Data["providerAccountCatalog"].([]any)
	accounts := root.Data["providerAccounts"].([]any)
	if len(catalog) != 6 || len(accounts) != 8 {
		t.Fatalf("provider root sizes = %d catalog, %d accounts", len(catalog), len(accounts))
	}
	openRouterCatalog := catalog[1].(map[string]any)
	if openRouterCatalog["providerKind"] != "openrouter" ||
		openRouterCatalog["preferredAuthMethod"] != "OAUTH_PKCE" {
		t.Fatalf("OpenRouter catalog = %#v", openRouterCatalog)
	}

	started := postGraphQL(t, server.URL, `mutation Start($input: StartProviderAuthAttemptInput!) {
  startProviderAuthAttempt(input: $input) {
    attemptId providerKind providerAccountId method status verificationUrl instructions errorCode errorMessage
  }
}`, map[string]any{"input": map[string]any{
		"providerKind": "openrouter", "providerAccountId": "client-value-is-ignored", "method": "OAUTH_PKCE",
	}}).Data["startProviderAuthAttempt"].(map[string]any)
	attemptID := started["attemptId"].(string)
	if started["providerAccountId"] != "provider_account:openrouter:default" ||
		started["status"] != "WAITING_FOR_USER" || started["method"] != "OAUTH_PKCE" {
		t.Fatalf("start response = %#v", started)
	}

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	events, err := resolver.SubscriptionRoot().ProviderAuthAttemptEvents(ctx, attemptID)
	if err != nil || (<-events).Status != model.ProviderAuthAttemptStatusWaitingForUser {
		t.Fatalf("subscription initial state is unavailable: %v", err)
	}
	cancelled := postGraphQL(t, server.URL, `mutation Cancel($input: CancelProviderAuthAttemptInput!) {
  cancelProviderAuthAttempt(input: $input) { attemptId providerAccountId status errorCode errorMessage }
}`, map[string]any{"input": map[string]any{"attemptId": attemptID}}).
		Data["cancelProviderAuthAttempt"].(map[string]any)
	if cancelled["status"] != "CANCELLED" || cancelled["providerAccountId"] != "provider_account:openrouter:default" {
		t.Fatalf("cancel response = %#v", cancelled)
	}
	if event, open := <-events; !open || event.Status != model.ProviderAuthAttemptStatusCancelled {
		t.Fatalf("subscription terminal event = %#v, open %v", event, open)
	}
	queried := postGraphQL(t, server.URL, `query Attempt($id: String!) {
  providerAuthAttempt(attemptId: $id) { attemptId status }
}`, map[string]any{"id": attemptID}).Data["providerAuthAttempt"].(map[string]any)
	if queried["status"] != "CANCELLED" {
		t.Fatalf("attempt query = %#v", queried)
	}
}

func TestCodexGraphQLAuthUsesExistingClientShape(t *testing.T) {
	resolver := openProviderTestResolver(t)
	service := &fakeProviderAuthService{}
	resolver.providerAuth["codex"] = service
	openRouterAttempt, err := resolver.OpenRouter.StartAuth(
		context.Background(), "openrouter", "provider_account:openrouter:default",
		provider.AuthOAuthPKCE,
	)
	if err != nil {
		t.Fatal(err)
	}
	server := httptest.NewServer(NewHandler(resolver))
	t.Cleanup(server.Close)

	started := postGraphQL(t, server.URL, `mutation Start($input: StartProviderAuthAttemptInput!) {
  startProviderAuthAttempt(input: $input) {
    attemptId providerKind providerAccountId method status verificationUrl userCode instructions
  }
}`, map[string]any{"input": map[string]any{
		"providerKind": "codex", "method": "OAUTH_DEVICE_CODE",
	}}).Data["startProviderAuthAttempt"].(map[string]any)
	if started["providerKind"] != "codex" || started["method"] != "OAUTH_DEVICE_CODE" ||
		started["providerAccountId"] != "provider_account:codex:default" ||
		started["userCode"] != "ABCD-EFGH" {
		t.Fatalf("Codex start response = %#v", started)
	}
	attemptID := started["attemptId"].(string)
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	events, err := resolver.SubscriptionRoot().ProviderAuthAttemptEvents(ctx, attemptID)
	if err != nil || (<-events).ProviderKind != "codex" {
		t.Fatalf("Codex subscription start: %v", err)
	}
	cancelled := postGraphQL(t, server.URL, `mutation Cancel($input: CancelProviderAuthAttemptInput!) {
  cancelProviderAuthAttempt(input: $input) { attemptId providerKind status }
}`, map[string]any{"input": map[string]any{"attemptId": attemptID}}).
		Data["cancelProviderAuthAttempt"].(map[string]any)
	if cancelled["providerKind"] != "codex" || cancelled["status"] != "CANCELLED" {
		t.Fatalf("Codex cancellation = %#v", cancelled)
	}
	if terminal, open := <-events; !open || terminal.Status != model.ProviderAuthAttemptStatusCancelled {
		t.Fatalf("Codex terminal event = %#v, open %v", terminal, open)
	}
	queried := postGraphQL(t, server.URL, `query Attempt($id: String!) {
  providerAuthAttempt(attemptId: $id) { attemptId providerKind status }
}`, map[string]any{"id": attemptID}).Data["providerAuthAttempt"].(map[string]any)
	if queried["providerKind"] != "codex" || queried["status"] != "CANCELLED" {
		t.Fatalf("Codex attempt query = %#v", queried)
	}
	openRouter := resolver.providerAuthAttempt(openRouterAttempt.ID)
	if openRouter == nil || openRouter.ProviderKind != "openrouter" ||
		openRouter.Status != model.ProviderAuthAttemptStatusWaitingForUser {
		t.Fatalf("coexisting OpenRouter attempt = %#v", openRouter)
	}
}

type fakeProviderAuthService struct {
	attempt provider.AuthAttempt
	events  chan provider.AuthAttempt
}

func (s *fakeProviderAuthService) StartAuth(
	_ context.Context,
	providerKind string,
	accountID string,
	method provider.AuthMethod,
) (provider.AuthAttempt, error) {
	if providerKind != "codex" || accountID != "provider_account:codex:default" ||
		method != provider.AuthOAuthDeviceCode {
		return provider.AuthAttempt{}, errors.New("unexpected fake provider authentication input")
	}
	s.attempt = provider.AuthAttempt{
		ID: "codex-attempt", ProviderKind: providerKind, ProviderAccountID: accountID,
		Method: method, Status: provider.AuthAttemptWaiting,
		VerificationURL: "https://auth.openai.com/codex/device", UserCode: "ABCD-EFGH",
		Instructions: "Complete the login in your browser.",
	}
	s.events = make(chan provider.AuthAttempt, 2)
	s.events <- s.attempt
	return s.attempt, nil
}

func (s *fakeProviderAuthService) Attempt(attemptID string) (provider.AuthAttempt, bool) {
	return s.attempt, s.attempt.ID != "" && attemptID == s.attempt.ID
}

func (s *fakeProviderAuthService) Cancel(attemptID string) (provider.AuthAttempt, bool) {
	if s.attempt.ID == "" || attemptID != s.attempt.ID {
		return provider.AuthAttempt{}, false
	}
	if s.attempt.Status == provider.AuthAttemptWaiting {
		s.attempt.Status = provider.AuthAttemptCancelled
		s.events <- s.attempt
		close(s.events)
	}
	return s.attempt, true
}

func (s *fakeProviderAuthService) Subscribe(
	_ context.Context,
	attemptID string,
) (<-chan provider.AuthAttempt, error) {
	if s.attempt.ID == "" || attemptID != s.attempt.ID {
		return nil, provider.ErrAuthAttemptNotCurrent
	}
	return s.events, nil
}

type graphQLResponse struct {
	Data   map[string]any `json:"data"`
	Errors []struct {
		Message string `json:"message"`
	} `json:"errors"`
}

func newTestServer(t *testing.T) *httptest.Server {
	t.Helper()
	server := httptest.NewServer(NewHandler(openTestResolver(t)))
	t.Cleanup(server.Close)
	return server
}

func openTestResolver(t *testing.T) *Resolver {
	t.Helper()
	paths, err := home.FromRoot(filepath.Join(t.TempDir(), "home"))
	if err != nil {
		t.Fatal(err)
	}
	root, err := paths.Open()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = root.Close() })
	taskStore, err := store.Open(context.Background(), paths.Database())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = taskStore.Close() })
	return NewResolver(taskStore, root, nil, nil, nil, nil, nil)
}

func openProviderTestResolver(t *testing.T) *Resolver {
	t.Helper()
	paths, err := home.FromRoot(filepath.Join(t.TempDir(), "home"))
	if err != nil {
		t.Fatal(err)
	}
	root, err := paths.Open()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = root.Close() })
	taskStore, err := store.Open(context.Background(), paths.Database())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = taskStore.Close() })
	accounts, err := provider.NewAccountService(paths.Root(), taskStore)
	if err != nil {
		t.Fatal(err)
	}
	if err := accounts.Initialize(context.Background(), time.Now()); err != nil {
		t.Fatal(err)
	}
	openRouter, err := provider.NewOpenRouterService(
		accounts, "http://localhost:3737/provider/oauth/callback",
	)
	if err != nil {
		t.Fatal(err)
	}
	return NewResolver(taskStore, root, nil, accounts, openRouter, nil, nil)
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
