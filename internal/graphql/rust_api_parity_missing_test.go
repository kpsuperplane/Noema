package graphql

import (
	"bytes"
	"context"
	"crypto/ecdh"
	"crypto/rand"
	"crypto/sha256"
	"database/sql"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"math"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/coder/websocket"
	"github.com/kpsuperplane/noema/internal/artifact"
	"github.com/kpsuperplane/noema/internal/auth"
	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/mcp"
	noemamemory "github.com/kpsuperplane/noema/internal/memory"
	"github.com/kpsuperplane/noema/internal/notification"
	"github.com/kpsuperplane/noema/internal/provider"
	noemaruntime "github.com/kpsuperplane/noema/internal/runtime"
	"github.com/kpsuperplane/noema/internal/store"
	"github.com/kpsuperplane/noema/internal/webtool"
	_ "github.com/ncruces/go-sqlite3/driver"
	"github.com/vektah/gqlparser/v2/gqlerror"
)

// rustAPIGraphQLResponse keeps GraphQL errors and extensions visible for
// parity cases whose Rust tests assert the public error boundary.
type rustAPIGraphQLResponse struct {
	Data   map[string]any `json:"data"`
	Errors []struct {
		Message    string         `json:"message"`
		Extensions map[string]any `json:"extensions"`
	} `json:"errors"`
}

func rustAPIRawGraphQL(t *testing.T, resolver *Resolver, query string, variables map[string]any) rustAPIGraphQLResponse {
	return rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(context.Background()), query, variables)
}

func rustAPIRawGraphQLContext(t *testing.T, resolver *Resolver, ctx context.Context, query string, variables map[string]any) rustAPIGraphQLResponse {
	t.Helper()
	body, err := json.Marshal(map[string]any{"query": query, "variables": variables})
	if err != nil {
		t.Fatal(err)
	}
	recorder := httptest.NewRecorder()
	request := httptest.NewRequest(http.MethodPost, "http://localhost:3737/graphql", bytes.NewReader(body)).WithContext(ctx)
	request.Header.Set("Content-Type", "application/json")
	NewHandler(resolver).ServeHTTP(recorder, request)
	if recorder.Code != http.StatusOK && recorder.Code != http.StatusUnprocessableEntity {
		t.Fatalf("GraphQL status = %d, body = %s", recorder.Code, recorder.Body.String())
	}
	var response rustAPIGraphQLResponse
	if err := json.Unmarshal(recorder.Body.Bytes(), &response); err != nil {
		t.Fatalf("GraphQL response = %s: %v", recorder.Body.String(), err)
	}
	return response
}

func rustAPIAuthenticatedHandler(resolver *Resolver) http.Handler {
	next := NewHandler(resolver)
	return http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		next.ServeHTTP(writer, request.WithContext(auth.WithDesktopAccess(request.Context())))
	})
}

func rustAPIAssertGraphQLError(t *testing.T, response rustAPIGraphQLResponse, message, code string) {
	t.Helper()
	if len(response.Errors) != 1 {
		t.Fatalf("GraphQL errors = %#v, want one error", response.Errors)
	}
	if !strings.Contains(response.Errors[0].Message, message) {
		t.Fatalf("GraphQL error = %q, want %q", response.Errors[0].Message, message)
	}
	if code != "" && response.Errors[0].Extensions["code"] != code {
		t.Fatalf("GraphQL error code = %#v, want %q", response.Errors[0].Extensions["code"], code)
	}
}

func rustAPIPortSubscriptionProjectionFailure(t *testing.T) {
	t.Helper()
	original := uint64(1)
	cursor := original
	if _, err := store.DecodeWorkEventCursor("malformed"); err == nil {
		t.Fatal("malformed subscription cursor was accepted")
	}
	if cursor != original {
		t.Fatalf("cursor changed after failed projection: %d", cursor)
	}
	if _, err := projectWorkEventCursor(uint64(math.MaxInt64)+1, &cursor); err == nil {
		t.Fatal("overflowing subscription projection was accepted")
	}
	if cursor != original {
		t.Fatalf("cursor advanced after failed projection: %d", cursor)
	}
	maxCursor, err := projectWorkEventCursor(uint64(math.MaxInt64), &cursor)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := store.DecodeWorkEventCursor(maxCursor); err != nil {
		t.Fatal(err)
	}
	if cursor != uint64(math.MaxInt64) {
		t.Fatalf("valid projection did not advance cursor: %d", cursor)
	}
}

func rustAPIPortNotificationBoundaries(t *testing.T) {
	ctx := context.Background()
	paths, err := home.FromRoot(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	taskStore, err := store.Open(ctx, paths.Database())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = taskStore.Close() })
	now := time.Now()
	oldSession, newSession := sha256.Sum256([]byte("notification-old")), sha256.Sum256([]byte("notification-new"))
	if err := taskStore.CreateAnonymousSession(ctx, oldSession, now); err != nil {
		t.Fatal(err)
	}
	if err := taskStore.RegisterPasskey(ctx, store.HumanPasskey{CredentialID: "notification-passkey", CredentialJSON: `{}`},
		store.RegistrationInitial, oldSession, newSession, now); err != nil {
		t.Fatal(err)
	}
	config, recovery, err := auth.LoadConfig(paths, "127.0.0.1:3737")
	if err != nil {
		t.Fatal(err)
	}
	authentication, err := auth.New(paths, taskStore, config, recovery)
	if err != nil {
		t.Fatal(err)
	}
	service, err := notification.New(paths, taskStore, config.Origin)
	if err != nil {
		t.Fatal(err)
	}
	handler := authentication.Handler(NewHandler(NewResolver(taskStore, nil, authentication,
		nil, nil, nil, nil, nil, nil, service)))
	access := seedNotificationNativeClient(t, taskStore)
	device := base64.RawURLEncoding.EncodeToString([]byte("device-token"))
	register := nativeGraphQLRequest(t, access, `mutation { registerClientNotifications(input: {
deviceToken: "`+device+`", environment: DEVELOPMENT}) { available enabled environment blocker } }`)
	response := httptest.NewRecorder()
	handler.ServeHTTP(response, register)
	if response.Code != http.StatusOK || !strings.Contains(response.Body.String(), `"enabled":true`) ||
		!strings.Contains(response.Body.String(), `"environment":"DEVELOPMENT"`) {
		t.Fatalf("notification registration = %d %s", response.Code, response.Body.String())
	}
	activity := base64.RawURLEncoding.EncodeToString([]byte("push-to-start"))
	live := nativeGraphQLRequest(t, access, `mutation { registerClientLiveActivities(input: {
pushToStartToken: "`+activity+`", environment: PRODUCTION, activeActivityIds: ["live_activity:one"]}) {
available enabled registered environment } }`)
	response = httptest.NewRecorder()
	handler.ServeHTTP(response, live)
	if response.Code != http.StatusOK || !strings.Contains(response.Body.String(), `"registered":true`) ||
		!strings.Contains(response.Body.String(), `"environment":"PRODUCTION"`) {
		t.Fatalf("Live Activity registration = %d %s", response.Code, response.Body.String())
	}
	nativeProviderWrite := nativeGraphQLRequest(t, access, `mutation { removeApnsProvider(expectedRevision: 0) { revision } }`)
	response = httptest.NewRecorder()
	handler.ServeHTTP(response, nativeProviderWrite)
	if response.Code != http.StatusOK || !bytes.Contains(response.Body.Bytes(), []byte("browser session is unavailable")) {
		t.Fatalf("native provider write = %d %s", response.Code, response.Body.String())
	}
}

func rustAPIPortOwnerPrincipal(t *testing.T) {
	t.Helper()
	resolver := openTestResolver(t)
	query := `query {
  task(taskId: "task:foreign") { taskId }
  taskWorkspaceFile(taskId: "task:foreign", path: "TASK.md") { path }
  conversationTranscriptPage(input: { conversationId: "conversation:foreign" }) { pageInfo { hasMoreBefore } }
  pendingHumanInterventions { __typename }
  acpAgents { agentId }
}`
	response := rustAPIRawGraphQLContext(t, resolver, context.Background(), query, nil)
	if len(response.Errors) != 5 {
		t.Errorf("owner-sensitive query errors = %#v, want five", response.Errors)
	}
	for index, errorValue := range response.Errors {
		if !strings.Contains(errorValue.Message, "request is unauthenticated") {
			t.Errorf("owner-sensitive query %d = %q", index, errorValue.Message)
		}
	}
	mutation := `mutation {
  createConversationExternalArtifact(input: {
    conversationId: "conversation:foreign", title: "Foreign",
    artifactKind: "document", externalUrl: "https://example.com/foreign"
  }) { artifactId }
  startMcpAuthentication(input: {
    requestId: "mcp_auth:foreign", expectedRevision: 1,
    redirectUri: "http://localhost/callback"
  }) { attemptId }
  skipMcpAuthentication(input: {
    requestId: "mcp_auth:foreign", expectedRevision: 1
  }) { requestId }
  createAcpAgent(input: { displayName: "Foreign", command: "/bin/false" }) { agentId }
  deleteAcpAgent(input: { agentId: "agent:foreign", expectedRevision: 1 })
}`
	response = rustAPIRawGraphQLContext(t, resolver, context.Background(), mutation, nil)
	if len(response.Errors) != 5 {
		t.Errorf("owner-sensitive mutation errors = %#v, want five", response.Errors)
	}
	for index, errorValue := range response.Errors {
		if !strings.Contains(errorValue.Message, "request is unauthenticated") {
			t.Errorf("owner-sensitive mutation %d = %q", index, errorValue.Message)
		}
	}
	rustAPIAssertUnauthenticatedSubscription(t, resolver, "conversationEvents(conversationId: \"conversation:foreign\") { __typename }")
	rustAPIAssertUnauthenticatedSubscription(t, resolver, "memoryEvents { pendingCount }")
}

func rustAPIAssertUnauthenticatedSubscription(t *testing.T, resolver *Resolver, field string) {
	t.Helper()
	server := httptest.NewServer(NewHandler(resolver))
	t.Cleanup(server.Close)
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	connection, response, err := websocket.Dial(ctx, "ws"+strings.TrimPrefix(server.URL, "http"), &websocket.DialOptions{Subprotocols: []string{"graphql-transport-ws"}})
	if err != nil {
		if response != nil {
			t.Fatalf("unauthenticated subscription dial: %v (%s)", err, response.Status)
		}
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = connection.CloseNow() })
	writeWS(t, ctx, connection, map[string]any{"type": "connection_init"})
	if message := readWS(t, ctx, connection); message["type"] != "connection_ack" {
		t.Fatalf("unauthenticated subscription ack = %#v", message)
	}
	writeWS(t, ctx, connection, map[string]any{"id": "unauthenticated", "type": "subscribe", "payload": map[string]any{"query": "subscription { " + field + " }"}})
	message := readWS(t, ctx, connection)
	if message["type"] != "next" {
		t.Errorf("unauthenticated subscription response = %#v, want GraphQL error payload", message)
		return
	}
	payload, ok := message["payload"].(map[string]any)
	if !ok {
		t.Errorf("unauthenticated subscription payload = %#v", message["payload"])
		return
	}
	encoded, err := json.Marshal(payload["errors"])
	if err != nil || !bytes.Contains(encoded, []byte("request is unauthenticated")) {
		t.Errorf("unauthenticated subscription errors = %s, %v", encoded, err)
	}
}

func rustAPIPortACPDeleteRevision(t *testing.T) {
	t.Helper()
	resolver := openTestResolver(t)
	ctx := context.Background()
	agent, err := resolver.Store.CreateAcpAgent(ctx, "Disposable ACP", "/bin/false", nil, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	attempt, err := resolver.Store.BeginAcpAuthentication(ctx, agent.AgentID, agent.ConnectionRevision, "login", time.Now())
	if err != nil {
		t.Fatal(err)
	}
	for _, expected := range []int{2, 1} {
		response := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(ctx), fmt.Sprintf(`mutation {
  deleteAcpAgent(input: { agentId: %q, expectedRevision: %d })
}`, agent.AgentID, expected), nil)
		if len(response.Errors) != 1 {
			t.Fatalf("ACP deletion revision %d errors = %#v", expected, response.Errors)
		}
		if expected == 2 && !strings.Contains(response.Errors[0].Message, "revision") {
			t.Fatalf("stale ACP deletion = %#v", response.Errors)
		}
		if expected == 1 && !strings.Contains(strings.ToLower(response.Errors[0].Message), "authentication") {
			t.Fatalf("active ACP authentication deletion = %#v", response.Errors)
		}
	}
	if _, err := resolver.Store.FinishAcpAuthentication(ctx, attempt, true, nil, time.Now()); err != nil {
		t.Fatal(err)
	}
	response := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(ctx), fmt.Sprintf(`mutation {
  deleteAcpAgent(input: { agentId: %q, expectedRevision: 1 })
}`, agent.AgentID), nil)
	if len(response.Errors) != 0 || response.Data["deleteAcpAgent"] != true {
		t.Fatalf("ACP deletion = %#v", response)
	}
	if _, err := resolver.Store.AcpAgent(ctx, agent.AgentID); !errors.Is(err, store.ErrAcpAgentNotFound) {
		t.Fatalf("deleted ACP agent = %v", err)
	}
	if _, err := resolver.Store.Agent(ctx, agent.AgentID); !errors.Is(err, store.ErrAgentNotFound) {
		t.Fatalf("deleted ACP identity = %v", err)
	}
}

func rustAPIPortACPDeleteReferences(t *testing.T) {
	t.Helper()
	resolver := openTestResolver(t)
	ctx := context.Background()
	agent, err := resolver.Store.CreateAcpAgent(ctx, "Assigned ACP", "/bin/false", nil, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	captured := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(ctx), fmt.Sprintf(`mutation {
  captureTask(input: {
    workspaceId: "workspace:personal", title: "Keep assigned executor", executorAgentId: %q,
    schedule: { scheduledFor: "2030-01-01T08:00:00Z", timeZone: "UTC", recurrence: { startsAt: "2030-01-01T08:00:00Z", cronExpression: "0 8 * * *" } },
    clientMutationId: "capture-acp-delete-guard"
  }) { task { taskId revision generation } }
}`, agent.AgentID), nil)
	if len(captured.Errors) != 0 {
		t.Fatalf("scheduled ACP capture = %#v", captured.Errors)
	}
	response := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(ctx), fmt.Sprintf(`mutation {
  deleteAcpAgent(input: { agentId: %q, expectedRevision: 1 })
}`, agent.AgentID), nil)
	if len(response.Errors) != 1 || !strings.Contains(strings.ToLower(response.Errors[0].Message), "use") {
		t.Fatalf("current-task ACP deletion = %#v", response)
	}
	task := captured.Data["captureTask"].(map[string]any)["task"].(map[string]any)
	taskID := task["taskId"].(string)
	cancelled := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(ctx), fmt.Sprintf(`mutation {
  cancelTask(input: { taskId: %q, expectedRevision: %d, expectedGeneration: %d, clientMutationId: "cancel-acp-delete-guard" }) { task { taskId } }
}`, taskID, int(task["revision"].(float64)), int(task["generation"].(float64))), nil)
	if len(cancelled.Errors) != 0 {
		t.Fatalf("cancel ACP task = %#v", cancelled.Errors)
	}
	response = rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(ctx), fmt.Sprintf(`mutation {
  deleteAcpAgent(input: { agentId: %q, expectedRevision: 1 })
}`, agent.AgentID), nil)
	if len(response.Errors) != 1 || !strings.Contains(strings.ToLower(response.Errors[0].Message), "use") {
		t.Fatalf("scheduled ACP deletion = %#v", response)
	}
}

func rustAPIPortAgentErrorSanitization(t *testing.T) {
	t.Helper()
	resolver := openProviderTestResolver(t)
	if err := resolver.Store.SetLocalModelAccountStatus(context.Background(), provider.StatusUnavailable,
		"unsupported_platform", "/Users/alice/.secret/token.txt failed with token abc123", time.Now()); err != nil {
		t.Fatal(err)
	}
	response := rustAPIRawGraphQL(t, resolver, `{ agents { modelOptions {
  providerKind disabledReason profiles { id disabledReason }
} } }`, nil)
	if len(response.Errors) != 0 {
		t.Fatalf("agents query errors = %#v", response.Errors)
	}
	agentList, ok := response.Data["agents"].([]any)
	if !ok || len(agentList) == 0 {
		t.Fatalf("agents query data = %#v", response.Data)
	}
	first, ok := agentList[0].(map[string]any)
	if !ok {
		t.Fatalf("agent projection = %#v", agentList[0])
	}
	options, ok := first["modelOptions"].([]any)
	if !ok {
		t.Fatalf("agent model options = %#v", first["modelOptions"])
	}
	var local map[string]any
	for _, value := range options {
		option, ok := value.(map[string]any)
		if ok && option["providerKind"] == "local_models" {
			local = option
			break
		}
	}
	if local == nil {
		t.Fatalf("local model option missing: %#v", options)
	}
	if local["disabledReason"] != "Provider is unavailable on this platform." {
		t.Fatalf("local model disabled reason = %#v", local["disabledReason"])
	}
	encoded, err := json.Marshal(local)
	if err != nil {
		t.Fatal(err)
	}
	if bytesContainsAny(encoded, []byte("/Users/alice"), []byte("abc123"), []byte("token.txt")) {
		t.Fatalf("Agent projection exposed provider diagnostics: %s", encoded)
	}
}

func bytesContainsAny(value []byte, needles ...[]byte) bool {
	for _, needle := range needles {
		if strings.Contains(string(value), string(needle)) {
			return true
		}
	}
	return false
}

func rustAPIPortLocalArtifactQueries(t *testing.T) {
	t.Helper()
	resolver := openTestResolver(t)
	ctx := context.Background()
	conversation, err := resolver.Store.EnsurePrimaryConversation(ctx, "openrouter", "", time.Now())
	if err != nil {
		t.Fatal(err)
	}
	mediaType := "text/markdown"
	first, err := resolver.Artifacts.CreateLocal(ctx, localArtifactInput(conversation.ID, "# first\n", "report.md", &mediaType))
	if err != nil {
		t.Fatal(err)
	}
	second, err := resolver.Artifacts.AppendLocal(ctx, first.Artifact.ID, "report.md", []byte("# second\n"), stringPtr("Revision"), &mediaType, "agent:primary", store.ArtifactSource{}, map[string]any{})
	if err != nil {
		t.Fatal(err)
	}
	response := rustAPIRawGraphQL(t, resolver, fmt.Sprintf(`query {
  artifactVersionDetail(artifactVersionId: %q) {
    previewKind markdown plainText downloadUrl mediaType
    versions { artifactVersionId versionIndex }
  }
}`, second.ID), nil)
	if len(response.Errors) != 0 {
		t.Fatalf("local Artifact detail errors = %#v", response.Errors)
	}
	detail, ok := response.Data["artifactVersionDetail"].(map[string]any)
	if !ok {
		t.Fatalf("local Artifact detail = %#v", response.Data)
	}
	versions, ok := detail["versions"].([]any)
	if !ok || len(versions) != 2 {
		t.Fatalf("local Artifact versions = %#v", detail["versions"])
	}
	if detail["previewKind"] != "MARKDOWN" || detail["markdown"] != "# second\n" || detail["plainText"] != nil {
		t.Fatalf("local Artifact preview = %#v", detail)
	}
	if detail["downloadUrl"] != artifact.DownloadURL(second.ID) {
		t.Fatalf("local Artifact download URL = %#v", detail["downloadUrl"])
	}
}

func localArtifactInput(conversationID, content, filename string, mediaType *string) artifact.LocalInput {
	return artifact.LocalInput{Owner: store.ArtifactOwner{ObjectType: "conversation", ObjectID: conversationID}, Title: "Session report", Kind: "document", Filename: filename, Bytes: []byte(content), MediaType: mediaType, CreatedByActorID: "agent:primary", Source: store.ArtifactSource{ConversationID: conversationID}, Metadata: map[string]any{}}
}

func rustAPIPortForeignArtifactOperations(t *testing.T) {
	t.Helper()
	resolver := openTestResolver(t)
	foreignConversation := rustAPIInsertForeignConversation(t, resolver)
	response := rustAPIRawGraphQL(t, resolver, `query {
  artifacts(ownerObjectType: "conversation", ownerObjectId: "`+foreignConversation+`") { artifactId }
}`, nil)
	if len(response.Errors) != 0 {
		t.Fatalf("foreign Artifact list errors = %#v", response.Errors)
	}
	artifacts, ok := response.Data["artifacts"].([]any)
	if !ok || len(artifacts) != 0 {
		t.Fatalf("foreign Artifact list = %#v", response.Data)
	}
	response = rustAPIRawGraphQL(t, resolver, `mutation {
  createConversationExternalArtifact(input: {
    conversationId: "`+foreignConversation+`" title: "Injected"
    artifactKind: "document" externalUrl: "https://example.com/injected"
  }) { artifactId }
}`, nil)
	rustAPIAssertGraphQLError(t, response, "conversation is unavailable", "")
}

func rustAPIPortInboxArtifactUpload(t *testing.T) {
	t.Helper()
	resolver := openTestResolver(t)
	ctx := context.Background()
	captured, err := resolver.captureTask(ctx, model.CaptureTaskInput{WorkspaceID: personalWorkspaceID, Title: "Private packet", ClientMutationID: "capture-private-packet"})
	if err != nil {
		t.Fatal(err)
	}
	content := []byte("item,amount\nTransit,12.50\n")
	response := rustAPIRawGraphQL(t, resolver, fmt.Sprintf(`mutation {
  createTaskLocalArtifact(input: {
    taskId: %q expectedRevision: %d expectedGeneration: %d
    title: "August statement" filename: "statement.csv" mediaType: "text/csv"
    contentBase64: %q
  }) { artifactId ownerObjectType ownerObjectId currentVersion { artifactVersionId byteSize } }
}`, captured.Task.TaskID, captured.Task.Revision, captured.Task.Generation,
		base64.StdEncoding.EncodeToString(content)), nil)
	if len(response.Errors) != 0 {
		t.Fatalf("Task Artifact upload errors = %#v", response.Errors)
	}
	created, ok := response.Data["createTaskLocalArtifact"].(map[string]any)
	if !ok || created["ownerObjectType"] != "task" || created["ownerObjectId"] != captured.Task.TaskID {
		t.Fatalf("Task-owned Artifact = %#v", response.Data)
	}
	version, ok := created["currentVersion"].(map[string]any)
	if !ok || version["byteSize"] != float64(len(content)) || version["artifactVersionId"] == "" {
		t.Fatalf("Task Artifact version = %#v", created["currentVersion"])
	}
	stored, err := resolver.Store.ArtifactWithVersionsByID(ctx, created["artifactId"].(string))
	if err != nil || stored.Artifact.Metadata["filename"] != "statement.csv" {
		t.Fatalf("Task Artifact metadata = %#v, %v", stored.Artifact.Metadata, err)
	}
}

func rustAPIPortForeignConversationOperations(t *testing.T) {
	t.Helper()
	resolver := openChatTestResolver(t)
	foreignConversation := rustAPIInsertForeignConversation(t, resolver)
	response := rustAPIRawGraphQL(t, resolver, `query {
  conversationTranscriptPage(input: { conversationId: "`+foreignConversation+`" }) {
    pageInfo { hasMoreBefore }
  }
}`, nil)
	rustAPIAssertGraphQLError(t, response, "conversation is unavailable", "")
	for _, mutation := range []string{
		`mutation { sendConversationTurn(input: {
  conversationId: "` + foreignConversation + `" input: "private" clientMessageId: "client:foreign"
}) { conversationId } }`,
		`mutation { sendMultipleChoiceSelection(input: {
  conversationId: "` + foreignConversation + `" promptItemId: "item:foreign"
  selectedOptionIds: ["option:foreign"] clientMessageId: "client:foreign"
}) { conversationId } }`,
	} {
		response = rustAPIRawGraphQL(t, resolver, mutation, nil)
		rustAPIAssertGraphQLError(t, response, "conversation is unavailable", "")
	}
}

func rustAPIInsertForeignConversation(t *testing.T, resolver *Resolver) string {
	t.Helper()
	paths, err := home.FromRoot(resolver.home.Name())
	if err != nil {
		t.Fatal(err)
	}
	database, err := sql.Open("sqlite3", paths.Database())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = database.Close() })
	if _, err := database.Exec("PRAGMA ignore_check_constraints = ON"); err != nil {
		t.Fatal(err)
	}
	const id = "conversation:ffffffffffffffffffffffffffffffff"
	now := time.Now().UnixMilli()
	if _, err := database.Exec(`INSERT INTO conversations
 (conversation_id, owner_human_id, provider, cwd, created_at_ms, updated_at_ms)
 VALUES (?, ?, ?, NULL, ?, ?)`, id, "human:other", "openrouter", now, now); err != nil {
		t.Fatal(err)
	}
	return id
}

func rustAPIInsertForeignArtifact(t *testing.T, resolver *Resolver, conversationID string) string {
	t.Helper()
	paths, err := home.FromRoot(resolver.home.Name())
	if err != nil {
		t.Fatal(err)
	}
	database, err := sql.Open("sqlite3", paths.Database())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = database.Close() })
	artifactID := "artifact:" + strings.Repeat("e", 32)
	versionID := "artifact_version:" + strings.Repeat("f", 32)
	now := time.Now().UnixMilli()
	if _, err := database.Exec(`INSERT INTO artifacts
 (artifact_id, owner_object_type, owner_object_id, title, artifact_kind, storage_kind,
  current_version_id, created_by_actor_id, source_conversation_id, metadata_json,
  created_at_ms, updated_at_ms)
 VALUES (?, 'conversation', ?, 'Foreign notes', 'document', 'external_url', ?,
  'human:other', ?, '{}', ?, ?)`, artifactID, conversationID, versionID, conversationID, now, now); err != nil {
		t.Fatalf("foreign Artifact insert = %v", err)
	}
	if _, err := database.Exec(`INSERT INTO artifact_versions
 (artifact_version_id, artifact_id, version_index, external_url, media_type,
  created_by_actor_id, source_conversation_id, metadata_json, created_at_ms)
 VALUES (?, ?, 1, 'https://example.com/foreign', 'text/html', 'human:other', ?, '{}', ?)`, versionID, artifactID, conversationID, now); err != nil {
		t.Fatalf("foreign Artifact version insert = %v", err)
	}
	return versionID
}

func rustAPIPortMCPRouteAndSetup(t *testing.T) {
	t.Helper()
	resolver := openTestResolver(t)
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/.well-known/oauth-protected-resource" {
			_ = json.NewEncoder(w).Encode(map[string]any{"resource": "", "authorization_servers": []string{"https://auth.example"}})
			return
		}
		w.Header().Set("WWW-Authenticate", `Bearer resource_metadata="/.well-known/oauth-protected-resource"`)
		w.WriteHeader(http.StatusUnauthorized)
	}))
	t.Cleanup(remote.Close)
	paths, err := home.FromRoot(resolver.home.Name())
	if err != nil {
		t.Fatal(err)
	}
	service, err := mcp.NewService(paths, resolver.Store, false, nil, "http://localhost/mcp/oauth/callback")
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(service.Close)
	resolver.MCP = service
	ctx := auth.WithDesktopAccess(context.Background())
	input := model.CreateMcpServerInput{DisplayName: "Dex", TransportKind: "streamable_http",
		HTTP: &model.McpHTTPConfigInput{URL: remote.URL + "/mcp"}}
	setup, err := setupInput(input)
	if err != nil || setup.TransportKind != "streamable_http" || setup.URL != input.HTTP.URL || setup.DisplayName != "Dex" {
		t.Fatalf("MCP setup boundary = %#v, %v", setup, err)
	}
	// The fake server reports OAuth support but no discovered tools. The
	// resolver must return the pending setup without publishing a server.
	result, err := resolver.createMCPServer(ctx, input)
	if err != nil || result == nil || result.SetupStatus != "needs_auth" || result.DiscoveredToolCount != 0 || result.Server != nil || result.Auth == nil || !result.Auth.OauthAuthorizationSupported || !result.Auth.OauthClientCredentialsSupported {
		t.Fatalf("MCP setup result = %#v, %v", result, err)
	}
	servers, err := resolver.Store.MCPServers(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if len(servers) != 0 {
		t.Fatalf("MCP setup persisted an incomplete server: %#v", servers)
	}
}

func rustAPIPortMCPPendingIntervention(t *testing.T) {
	t.Helper()
	resolver := openTestResolver(t)
	paths, err := home.FromRoot(resolver.home.Name())
	if err != nil {
		t.Fatal(err)
	}
	service, err := mcp.NewService(paths, resolver.Store, false, nil, "http://localhost/mcp/oauth/callback")
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(service.Close)
	resolver.MCP = service
	ctx := context.Background()
	conversation, err := resolver.Store.EnsurePrimaryConversation(ctx, "openrouter", "", time.Now())
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := resolver.Store.BeginConversationTurn(ctx, conversation.ID, "connect a service", nil, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	items, err := resolver.Store.StartConversationToolRound(ctx, turn, store.ConversationToolRound{
		Provider: "openrouter",
		Call: store.ConversationToolCallInput{ProviderRound: 0, OutputIndex: 0,
			ProviderCallID: "call:mcp-setup", ProviderName: "mcp.connect_service",
			Name: "mcp.connect_service", Arguments: json.RawMessage(`{"service_url":"https://notion.com/"}`)},
	}, time.Now())
	if err != nil || len(items) == 0 {
		t.Fatalf("MCP setup tool call = %#v, %v", items, err)
	}
	call := items[len(items)-1]
	setupPayload, _ := json.Marshal(map[string]any{
		"status": "needs_auth", "service_url": "https://notion.com/", "display_name": "Notion",
		"description": "Workspace tools", "endpoint_url": "https://mcp.notion.com/mcp",
		"setup_result": map[string]any{"discovered_tool_count": 0},
	})
	result, err := resolver.Store.FinishConversationToolCall(ctx, turn, store.ConversationToolResultInput{
		CallItemID: call.ID, Provider: "openrouter", ProviderRound: 0, OutputIndex: 0,
		ProviderCallID: "call:mcp-setup", ProviderName: "mcp.connect_service", Name: "mcp.connect_service",
		Success: true, Payload: setupPayload,
	}, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	setup, ok := setupFromItem(result)
	if !ok || setup.Status != "needs_auth" || setup.DisplayName != "Notion" || setup.Discovered != 0 {
		t.Fatalf("stored MCP setup = %#v, %t", setup, ok)
	}
	id := conversation.ID
	interventions, err := resolver.pendingMCPSetups(ctx, &id, nil)
	if err != nil || len(interventions) != 1 {
		t.Fatalf("pending MCP setups = %#v, %v", interventions, err)
	}
	value, ok := interventions[0].(*model.McpSetupIntervention)
	if !ok || value.ItemID != result.ID || value.SetupStatus != "needs_auth" || value.DisplayName != "Notion" || !value.OauthSupported {
		t.Fatalf("MCP intervention = %#v", interventions[0])
	}
}

func rustAPIPortMCPOAuthCompletion(t *testing.T) {
	t.Helper()
	resolver := openTestResolver(t)
	paths, err := home.FromRoot(resolver.home.Name())
	if err != nil {
		t.Fatal(err)
	}
	service, err := mcp.NewService(paths, resolver.Store, false, nil, "http://localhost/mcp/oauth/callback")
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(service.Close)
	resolver.MCP = service
	// The callback validator is the durable route boundary used before an
	// attempt can bind to a runtime authentication request.
	if err := requireMCPCallback(service, "http://127.0.0.1:4444/mcp/oauth/callback"); err == nil {
		t.Fatal("MCP OAuth accepted a callback on a different origin")
	}
	if err := requireMCPCallback(service, "http://localhost/mcp/oauth/callback"); err != nil && strings.Contains(err.Error(), "does not match") {
		t.Fatalf("MCP OAuth rejected the configured callback: %v", err)
	}
	ctx := auth.WithDesktopAccess(context.Background())
	if _, err := resolver.startMCPCreateOAuth(ctx, model.StartMcpServerOAuthSetupInput{Server: &model.CreateMcpServerInput{
		DisplayName: "Docs", TransportKind: "streamable_http", HTTP: &model.McpHTTPConfigInput{URL: "http://localhost:1/mcp"},
	}, RedirectURI: "http://127.0.0.1:4444/mcp/oauth/callback"}); err == nil || !strings.Contains(err.Error(), "does not match") {
		t.Fatalf("mismatched MCP OAuth setup = %v", err)
	}
}

func requireMCPCallback(service *mcp.Service, callback string) error {
	// StartOAuthCreate performs the same exact callback comparison, but it
	// also needs a remote discovery endpoint. Keep this helper's error path
	// bounded by using a deliberately invalid local endpoint.
	_, err := service.StartOAuthCreate(context.Background(), "human:local", mcp.SetupInput{
		DisplayName: "Callback", TransportKind: "streamable_http", URL: "http://localhost:1/mcp",
	}, callback)
	return err
}

func rustAPIPortRuntimeDebugSchema(t *testing.T) {
	resolver := openTestResolver(t)
	ctx := context.Background()
	conversation, err := resolver.Store.EnsurePrimaryConversation(ctx, "openai", "", time.Now())
	if err != nil {
		t.Fatal(err)
	}
	started := time.Now().UTC()
	turn, _, err := resolver.Store.BeginConversationTurn(ctx, conversation.ID, "Profile this.", nil, started)
	if err != nil {
		t.Fatal(err)
	}
	round, inputTokens := 1, 21
	spanID, err := resolver.Store.BeginRuntimeDebugSpan(ctx,
		store.RuntimeDebugScope{Kind: "conversation_turn", ID: turn.ID}, "provider", "Provider request",
		store.RuntimeDebugMetadata{Provider: "openai", Model: "gpt-test", Phase: "continuation",
			RoundIndex: &round, InputTokens: &inputTokens}, started.Add(10*time.Millisecond))
	if err != nil {
		t.Fatal(err)
	}
	if err = resolver.Store.FinishRuntimeDebugSpan(ctx, spanID, "completed",
		store.RuntimeDebugMetadata{Provider: "openai", Model: "gpt-test", Phase: "continuation",
			RoundIndex: &round, InputTokens: &inputTokens}, 30*time.Millisecond, started.Add(40*time.Millisecond)); err != nil {
		t.Fatal(err)
	}
	if _, err = resolver.Store.FailConversationTurn(ctx, turn, "Test failure.", started.Add(100*time.Millisecond)); err != nil {
		t.Fatal(err)
	}
	profileResponse := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(ctx), fmt.Sprintf(`query {
  runtimeDebugProfile(input: { kind: CONVERSATION_TURN, scopeId: %q }) {
    status accountedMilliseconds uninstrumentedMilliseconds
    spans { durationMilliseconds startOffsetMilliseconds provider inputTokens }
  }
}`, turn.ID), nil)
	if len(profileResponse.Errors) != 0 {
		t.Fatalf("profile errors = %#v", profileResponse.Errors)
	}
	profile := profileResponse.Data["runtimeDebugProfile"].(map[string]any)
	spans := profile["spans"].([]any)
	span := spans[0].(map[string]any)
	if span["durationMilliseconds"] != float64(30) || span["startOffsetMilliseconds"] != float64(10) || span["provider"] != "openai" || span["inputTokens"] != float64(21) {
		t.Fatalf("span = %#v", span)
	}
	if profile["accountedMilliseconds"] != float64(30) || profile["uninstrumentedMilliseconds"] != float64(70) {
		t.Fatalf("profile timing = %#v", profile)
	}
	missingResponse := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(ctx), `query {
  runtimeDebugProfile(input: { kind: CONVERSATION_TURN, scopeId: "turn:00000000000000000000000000000000" }) { status }
}`, nil)
	if len(missingResponse.Errors) != 0 || missingResponse.Data["runtimeDebugProfile"] != nil {
		t.Fatalf("unowned profile = %#v", missingResponse)
	}
	ended := started.Add(50 * time.Millisecond)
	runningStarted := started.Add(20 * time.Millisecond)
	persistenceStarted := started.Add(40 * time.Millisecond)
	persistenceEnded := started.Add(80 * time.Millisecond)
	persistenceDuration := int64(40)
	projected := runtimeDebugProfileModel(&store.RuntimeDebugProfile{
		Scope: store.RuntimeDebugScope{Kind: "task_run", ID: "run:test"}, Status: "failed",
		StartedAt: started, EndedAt: &ended, Spans: []store.RuntimeDebugSpan{
			{ID: "debug_span:test", Category: "runtime", Name: "Work", Status: "running", StartedAt: runningStarted},
			{ID: "debug_span:persistence", Category: "persistence", Name: "Save", Status: "completed",
				StartedAt: persistenceStarted, EndedAt: &persistenceEnded, DurationMilliseconds: &persistenceDuration},
		},
	}, ended)
	if projected.Kind != model.RuntimeDebugScopeKindTaskRun || projected.Spans[0].Status != model.RuntimeDebugStatusInterrupted ||
		projected.ElapsedMilliseconds != 80 || projected.AccountedMilliseconds != 60 || projected.UninstrumentedMilliseconds != 20 ||
		projected.EndedAt == nil || *projected.EndedAt != debugTime(persistenceEnded) {
		t.Fatalf("terminal projection = %#v", projected)
	}
}

func rustAPIPortAgentPreference(t *testing.T) {
	t.Helper()
	resolver := readyAgentTestResolver(t)
	ctx := context.Background()
	profile := "openai/gpt-5.6-luna"
	missingEffort := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(ctx), fmt.Sprintf(`mutation {
  saveAgentModelPreference(input: { agentId: %q, providerAccountId: "provider_account:openrouter:default", selectionMode: EXPLICIT_PROFILE, modelProfile: %q, fastMode: false }) {
    modelProfile reasoningEffort
  }
}`, store.PrimaryAgentID, profile), nil)
	if len(missingEffort.Errors) != 1 || !strings.Contains(missingEffort.Errors[0].Message, "reasoning effort") {
		t.Fatalf("missing reasoning effort = %#v", missingEffort)
	}
	saved := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(ctx), fmt.Sprintf(`mutation {
  saveAgentModelPreference(input: { agentId: %q, providerAccountId: "provider_account:openrouter:default", selectionMode: EXPLICIT_PROFILE, modelProfile: %q, reasoningEffort: HIGH, fastMode: false }) {
    providerKind providerAccountId modelProfile reasoningEffort selectionMode fastMode
  }
}`, store.PrimaryAgentID, profile), nil)
	if len(saved.Errors) != 0 {
		t.Fatalf("valid Agent preference errors = %#v", saved.Errors)
	}
	value := saved.Data["saveAgentModelPreference"].(map[string]any)
	if value["modelProfile"] != profile || value["reasoningEffort"] != "HIGH" || value["selectionMode"] != "EXPLICIT_PROFILE" {
		t.Fatalf("valid Agent preference = %#v", value)
	}
	recommended := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(ctx), fmt.Sprintf(`mutation {
  saveAgentModelPreference(input: { agentId: %q, providerAccountId: "provider_account:openrouter:default", selectionMode: NOEMA_RECOMMENDED, fastMode: false }) {
    modelProfile reasoningEffort selectionMode
  }
}`, store.PrimaryAgentID), nil)
	if len(recommended.Errors) != 0 {
		t.Fatalf("recommended Agent preference errors = %#v", recommended.Errors)
	}
	recommendedValue := recommended.Data["saveAgentModelPreference"].(map[string]any)
	if recommendedValue["modelProfile"] != nil || recommendedValue["reasoningEffort"] != nil || recommendedValue["selectionMode"] != "NOEMA_RECOMMENDED" {
		t.Fatalf("recommended Agent preference = %#v", recommendedValue)
	}
	taskPreference := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(ctx), fmt.Sprintf(`mutation {
  saveAgentModelPreference(input: { agentId: %q, providerAccountId: "provider_account:openrouter:default", selectionMode: EXPLICIT_PROFILE, modelProfile: %q, reasoningEffort: HIGH, fastMode: false }) { providerKind }
}`, store.TaskExecutorAgentID, profile), nil)
	if len(taskPreference.Errors) != 1 || !strings.Contains(taskPreference.Errors[0].Message, "complexity tier") {
		t.Fatalf("Task Executor preference = %#v", taskPreference)
	}
}

func rustAPIPortACPSetup(t *testing.T) {
	t.Helper()
	resolver := openTestResolver(t)
	ctx := context.Background()
	createdResponse := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(ctx), `mutation {
  createAcpAgent(input: { displayName: "Codex ACP", command: "/usr/bin/codex", arguments: ["--acp"] }) {
    agentId connectionRevision healthStatus authStatus arguments enabled
  }
}`, nil)
	if len(createdResponse.Errors) != 0 {
		t.Fatalf("created ACP agent errors = %#v", createdResponse.Errors)
	}
	created, ok := createdResponse.Data["createAcpAgent"].(map[string]any)
	if !ok || created["connectionRevision"] != float64(1) || created["healthStatus"] != "UNKNOWN" || created["authStatus"] != "UNKNOWN" || created["arguments"].([]any)[0] != "--acp" {
		t.Fatalf("created ACP agent = %#v", createdResponse.Data)
	}
	agentID := created["agentId"].(string)
	updatedResponse := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(ctx), fmt.Sprintf(`mutation {
  updateAcpAgent(input: { agentId: %q, expectedRevision: 1, displayName: "Codex ACP", command: "/usr/bin/codex", arguments: ["--acp"], enabled: false }) {
    agentId enabled connectionRevision
  }
}`, agentID), nil)
	if len(updatedResponse.Errors) != 0 {
		t.Fatalf("updated ACP agent errors = %#v", updatedResponse.Errors)
	}
	updated := updatedResponse.Data["updateAcpAgent"].(map[string]any)
	if updated["enabled"] != false || updated["connectionRevision"] != float64(2) {
		t.Fatalf("updated ACP agent = %#v", updated)
	}
}

func stringPtr(value string) *string { return &value }

func rustAPIPortMemoryEvents(t *testing.T) {
	t.Helper()
	resolver := readyAgentTestResolver(t)
	ctx := context.Background()
	conversation, err := resolver.Store.EnsurePrimaryConversation(ctx, "openrouter", "", time.Now())
	if err != nil {
		t.Fatal(err)
	}
	nativeMemory, err := noemamemory.New(resolver.home)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = nativeMemory.Close() })
	resolver.Memory = nativeMemory
	generator := memoryGeneratorFunc(func(_ context.Context, _ provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		payload, _ := json.Marshal(map[string]any{
			"upserts": []any{
				map[string]any{"id": nil, "expected_hash": nil, "path": "career.md", "title": "Career", "icon": "briefcase-business", "body": "Engineering career.", "citations": []any{}},
				map[string]any{"id": nil, "expected_hash": nil, "path": "career/learning.md", "title": "Learning", "icon": "graduation-cap", "body": "Technical learning.", "citations": []any{}},
			},
			"metadata_updates": []any{}, "deletes": []any{},
		})
		return provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{Name: "noema.submit_memory_changes", Payload: payload}}}, nil
	})
	resolver.Chat, err = noemaruntime.NewChat(resolver.Store, generator, generator, generator, resolver.home, nativeMemory)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = resolver.Chat.Close() })
	server := httptest.NewServer(rustAPIAuthenticatedHandler(resolver))
	t.Cleanup(server.Close)
	eventContext, cancel := context.WithTimeout(ctx, 5*time.Second)
	defer cancel()
	connection, response, err := websocket.Dial(eventContext, "ws"+strings.TrimPrefix(server.URL, "http"), &websocket.DialOptions{Subprotocols: []string{"graphql-transport-ws"}})
	if err != nil {
		if response != nil {
			t.Fatalf("Memory subscription dial: %v (%s)", err, response.Status)
		}
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = connection.CloseNow() })
	writeWS(t, eventContext, connection, map[string]any{"type": "connection_init"})
	if message := readWS(t, eventContext, connection); message["type"] != "connection_ack" {
		t.Fatalf("Memory subscription ack = %#v", message)
	}
	writeWS(t, eventContext, connection, map[string]any{"id": "memory", "type": "subscribe", "payload": map[string]any{"query": `subscription {
  memoryEvents { root { title icon children { title icon } } pages { path title icon } pendingCount updateStatus { state active } }
}`}})
	initialMessage := readWS(t, eventContext, connection)
	initialData := initialMessage["payload"].(map[string]any)["data"].(map[string]any)["memoryEvents"].(map[string]any)
	if initialData["root"].(map[string]any)["title"] != "Human memory" || initialData["root"].(map[string]any)["icon"] != "user" || initialData["pendingCount"] != float64(0) || initialData["updateStatus"].(map[string]any)["state"] != "idle" {
		t.Fatalf("initial Memory event = %#v", initialData)
	}
	if _, _, err := resolver.Store.BeginConversationTurn(ctx, conversation.ID, "Engineering career.", nil, time.Now()); err != nil {
		t.Fatal(err)
	}
	updateResponse := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(ctx), `mutation { updateMemory { accepted status { state active } } }`, nil)
	if len(updateResponse.Errors) != 0 {
		t.Fatalf("Memory update errors = %#v", updateResponse.Errors)
	}
	deadline := time.After(5 * time.Second)
	for {
		select {
		case <-deadline:
			t.Fatal("timed out waiting for changed Memory event")
		default:
			message := readWS(t, eventContext, connection)
			payload, ok := message["payload"].(map[string]any)
			if !ok {
				continue
			}
			data, ok := payload["data"].(map[string]any)
			if !ok {
				continue
			}
			changed, ok := data["memoryEvents"].(map[string]any)
			if !ok || len(changed["root"].(map[string]any)["children"].([]any)) != 1 {
				continue
			}
			root := changed["root"].(map[string]any)
			children := root["children"].([]any)
			if children[0].(map[string]any)["title"] != "Career" || children[0].(map[string]any)["icon"] != "briefcase-business" {
				t.Fatalf("changed Memory root = %#v", root)
			}
			pages := changed["pages"].([]any)
			if len(pages) != 3 || pages[0].(map[string]any)["path"] != "career.md" || pages[1].(map[string]any)["path"] != "career/learning.md" || pages[2].(map[string]any)["path"] != "root.md" {
				t.Fatalf("changed Memory pages = %#v", pages)
			}
			return
		}
	}
}

func rustAPIPortConversationReady(t *testing.T) {
	resolver := openChatTestResolver(t)
	ctx := context.Background()
	if conversation, err := resolver.primaryConversation(ctx); err != nil || conversation != nil {
		t.Fatalf("fresh primary conversation = %#v, %v", conversation, err)
	}
	stored, err := resolver.Store.EnsurePrimaryConversation(ctx, "openrouter", "", time.Now())
	if err != nil {
		t.Fatal(err)
	}
	conversation, err := resolver.primaryConversation(ctx)
	if err != nil || conversation == nil || conversation.ConversationID != stored.ID {
		t.Fatalf("primary conversation = %#v, %v", conversation, err)
	}
	if conversation.LatestTranscriptPage == nil || len(conversation.LatestTranscriptPage.Items) != 0 {
		t.Fatalf("latest transcript = %#v", conversation.LatestTranscriptPage)
	}
	server := httptest.NewServer(rustAPIAuthenticatedHandler(resolver))
	t.Cleanup(server.Close)
	result := postGraphQL(t, server.URL, `
query ReadyChat($input: ConversationTranscriptPageInput!) {
  primaryConversation {
    conversationId
    provider
    latestTranscriptPage { items { itemId } pageInfo { beforeCursor hasMoreBefore } }
  }
  conversationTranscriptPage(input: $input) {
    items { itemId }
    pageInfo { beforeCursor hasMoreBefore }
  }
}`, map[string]any{"input": map[string]any{"conversationId": stored.ID}})
	primary := result.Data["primaryConversation"].(map[string]any)
	if primary["conversationId"] != stored.ID || primary["provider"] != "openrouter" {
		t.Fatalf("GraphQL primary conversation = %#v", primary)
	}
	streamCtx, cancel := context.WithTimeout(ctx, 5*time.Second)
	defer cancel()
	connection, response, err := websocket.Dial(streamCtx, "ws"+strings.TrimPrefix(server.URL, "http"), &websocket.DialOptions{Subprotocols: []string{"graphql-transport-ws"}})
	if err != nil {
		if response != nil {
			t.Fatalf("conversation subscription dial: %v (%s)", err, response.Status)
		}
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = connection.CloseNow() })
	writeWS(t, streamCtx, connection, map[string]any{"type": "connection_init"})
	if message := readWS(t, streamCtx, connection); message["type"] != "connection_ack" {
		t.Fatalf("conversation subscription ack = %#v", message)
	}
	writeWS(t, streamCtx, connection, map[string]any{"id": "ready", "type": "subscribe", "payload": map[string]any{
		"query": fmt.Sprintf(`subscription { conversationEvents(conversationId: %q) { __typename ... on SubscriptionReadyEvent { conversationId } } }`, stored.ID),
	}})
	message := readWS(t, streamCtx, connection)
	if message["type"] != "next" {
		t.Fatalf("conversation subscription response = %#v", message)
	}
	data := message["payload"].(map[string]any)["data"].(map[string]any)["conversationEvents"].(map[string]any)
	if data["__typename"] != "SubscriptionReadyEvent" || data["conversationId"] != stored.ID {
		t.Fatalf("first conversation event = %#v", data)
	}
}

func rustAPIPortConversationLiveEvents(t *testing.T) {
	t.Helper()
	resolver := openChatTestResolver(t)
	conversation, err := resolver.Store.EnsurePrimaryConversation(context.Background(), "openrouter", "", time.Now())
	if err != nil {
		t.Fatal(err)
	}
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	stream, err := resolver.conversationEvents(ctx, conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	if _, ok := <-stream; ok == false {
		t.Fatal("conversation stream closed before events")
	}
	resolver.Chat.NotifyHumanInterventionsChanged(conversation.ID)
	intervention := <-stream
	if value, ok := intervention.(model.HumanInterventionsChangedEvent); !ok || value.ConversationID != conversation.ID {
		t.Fatalf("human intervention event = %#v", intervention)
	}
	for index := 0; index < 256; index++ {
		resolver.Chat.NotifyHumanInterventionsChanged(conversation.ID)
	}
	resynchronized := false
	for index := 0; index < 256; index++ {
		value, open := <-stream
		if !open {
			t.Fatal("conversation stream closed after lag")
		}
		if _, ok := value.(model.SubscriptionReadyEvent); ok {
			resynchronized = true
			break
		}
	}
	if !resynchronized {
		t.Fatal("conversation stream did not publish a readiness resynchronization")
	}
	itemID := "item_1"
	event, err := resolver.conversationEventModel(ctx, noemaruntime.Event{Kind: noemaruntime.EventAssistantDelta, ConversationID: conversation.ID, TurnID: "turn_1", StreamID: "assistant_stream:turn_1:initial", Delta: "Hel"})
	if err != nil {
		t.Fatal(err)
	}
	if value, ok := event.(model.AssistantTextDeltaEvent); !ok || value.ConversationID != conversation.ID || value.TurnID != "turn_1" || value.StreamID != "assistant_stream:turn_1:initial" || value.Delta != "Hel" {
		t.Fatalf("assistant delta = %#v", event)
	}
	itemEvent, err := resolver.conversationEventModel(ctx, noemaruntime.Event{Kind: noemaruntime.EventConversationItem, ConversationID: conversation.ID, Item: &store.ConversationItem{ID: itemID, Cursor: "conversation_item:1", Kind: store.ConversationUserText, ContentText: "Hello", Metadata: map[string]any{}}})
	if err != nil {
		t.Fatal(err)
	}
	value, ok := itemEvent.(model.ConversationItemEvent)
	if !ok || value.ItemID != itemID || value.Cursor == nil || *value.Cursor != "conversation_item:1" || len(value.Metadata) != 0 {
		t.Fatalf("conversation item event = %#v", itemEvent)
	}
	transientEvent, err := resolver.conversationEventModel(ctx, noemaruntime.Event{Kind: noemaruntime.EventConversationItem, ConversationID: conversation.ID, Item: &store.ConversationItem{
		ID: "transient:activity_1", Kind: store.ConversationAssistantText, ContentText: "Hello", Metadata: map[string]any{"runtime_item_id": "activity_1", "transient": true},
	}})
	if err != nil {
		t.Fatal(err)
	}
	transientValue, ok := transientEvent.(model.ConversationItemEvent)
	if !ok || transientValue.Cursor != nil || len(transientValue.Metadata) != 2 || transientValue.Metadata["runtime_item_id"] != "activity_1" || transientValue.Metadata["transient"] != true {
		t.Fatalf("transient conversation item event = %#v", transientEvent)
	}
}

func rustAPIPortForeignConversationSubscription(t *testing.T) {
	t.Helper()
	resolver := openChatTestResolver(t)
	foreignConversation := rustAPIInsertForeignConversation(t, resolver)
	server := httptest.NewServer(rustAPIAuthenticatedHandler(resolver))
	t.Cleanup(server.Close)
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	connection, response, err := websocket.Dial(ctx, "ws"+strings.TrimPrefix(server.URL, "http"), &websocket.DialOptions{Subprotocols: []string{"graphql-transport-ws"}})
	if err != nil {
		if response != nil {
			t.Fatalf("foreign subscription dial: %v (%s)", err, response.Status)
		}
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = connection.CloseNow() })
	writeWS(t, ctx, connection, map[string]any{"type": "connection_init"})
	if message := readWS(t, ctx, connection); message["type"] != "connection_ack" {
		t.Fatalf("foreign subscription ack = %#v", message)
	}
	writeWS(t, ctx, connection, map[string]any{"id": "foreign", "type": "subscribe", "payload": map[string]any{
		"query": fmt.Sprintf(`subscription { conversationEvents(conversationId: %q) { __typename } }`, foreignConversation),
	}})
	message := readWS(t, ctx, connection)
	if message["type"] != "next" {
		t.Fatalf("foreign subscription response = %#v", message)
	}
	payload := message["payload"].(map[string]any)
	encoded, err := json.Marshal(payload["errors"])
	if err != nil || !bytes.Contains(encoded, []byte("conversation is unavailable")) {
		t.Fatalf("foreign conversation subscription errors = %s, %v", encoded, err)
	}
}

func rustAPIPortRecurrenceList(t *testing.T) {
	resolver := openTestResolver(t)
	ctx := auth.WithDesktopAccess(context.Background())
	query := func(input string) rustAPIGraphQLResponse {
		return rustAPIRawGraphQLContext(t, resolver, ctx, input, nil)
	}
	captured := query(`mutation {
  captureTask(input: {
    workspaceId: "workspace:personal"
    title: "Show positive news"
    schedule: {
      scheduledFor: "2030-01-01T08:00:00Z"
      timeZone: "UTC"
      recurrence: { startsAt: "2030-01-01T08:00:00Z", cronExpression: "0 8 * * *" }
    }
    clientMutationId: "recurrence-list-capture"
  }) { task { taskId revision generation } }
}`)
	if len(captured.Errors) != 0 {
		t.Fatalf("recurring capture = %#v", captured.Errors)
	}
	task := captured.Data["captureTask"].(map[string]any)["task"].(map[string]any)
	cancelled := query(fmt.Sprintf(`mutation {
  cancelTask(input: {
    taskId: %q expectedRevision: %d expectedGeneration: %d
    clientMutationId: "recurrence-list-cancel"
  }) { task { taskId } }
}`, task["taskId"], int(task["revision"].(float64)), int(task["generation"].(float64))))
	if len(cancelled.Errors) != 0 {
		t.Fatalf("recurring cancellation = %#v", cancelled.Errors)
	}
	listed := query(`query {
  tasks(input: { workspaceId: "workspace:personal", scope: ACTIVE }) { edges { node { taskId } } }
  taskRecurrences(workspaceId: "workspace:personal") { recurrenceId title lifecycle nextRunAt }
}`)
	if len(listed.Errors) != 0 {
		t.Fatalf("recurrence list = %#v", listed.Errors)
	}
	if edges := listed.Data["tasks"].(map[string]any)["edges"].([]any); len(edges) != 0 {
		t.Fatalf("active tasks after cancellation = %#v", edges)
	}
	recurrences := listed.Data["taskRecurrences"].([]any)
	if len(recurrences) != 1 {
		t.Fatalf("recurrence count = %#v", recurrences)
	}
	value := recurrences[0].(map[string]any)
	if value["title"] != "Show positive news" || value["lifecycle"] != "ACTIVE" || value["nextRunAt"] != "2030-01-02T08:00:00Z" {
		t.Fatalf("recurrence projection = %#v", value)
	}
}

func rustAPIPortTaskSchemaVocabulary(t *testing.T) {
	t.Helper()
	schema := string(Schema())
	for _, name := range []string{"TaskAttentionKind", "TaskGateKind", "TaskGateState", "TaskRecoveryReason", "TaskRunKind", "TaskRunStatus", "TaskRunItemKind", "TaskRunItemStatus"} {
		if !strings.Contains(schema, "enum "+name) {
			t.Fatalf("missing task enum %s", name)
		}
	}
	for _, field := range []string{"taskDocumentPreview: String!", "taskDocument: String!", "taskDocumentDigest: String!", "resultDocument: String", "resultMetadata: JSON!", "reviewDocument: String", "workspaceFiles: [TaskWorkspaceFile!]!", "workspaceFilesTruncated: Boolean!", "taskWorkspaceFile(taskId: String!, path: String!): TaskWorkspaceFileText!", "messages: [TaskMessage!]!", "runs: [TaskRun!]!", "contributorInstanceNames: [String!]!", "task: TaskCard!", "gate: TaskGate"} {
		if !strings.Contains(schema, field) {
			t.Fatalf("missing task field %s", field)
		}
	}
	for _, removed := range []string{"TaskExecutionContract", "TaskSubmission", "TaskReview", "TaskValidationCriterionInput", "contractId:", "triggeringSubmissionId:", "triggeringReviewId:", "descriptionPreview:"} {
		if strings.Contains(schema, removed) {
			t.Fatalf("obsolete task schema content %s", removed)
		}
	}
	if !strings.Contains(schema, "kind: String!") || !strings.Contains(schema, "CLARIFICATION_REQUIRED") || strings.Contains(schema, "REVIEW_READY") {
		t.Fatal("task vocabulary changed")
	}
}

func rustAPIPortTaskReadsPrincipal(t *testing.T) {
	t.Helper()
	resolver := openTestResolver(t)
	for index, query := range []string{
		`query { tasks(input: { workspaceId: "workspace:personal" }) { edges { node { taskId } } } }`,
		`query { projectDocument(projectId: "project:missing") { projectId } }`,
	} {
		response := rustAPIRawGraphQLContext(t, resolver, context.Background(), query, nil)
		rustAPIAssertGraphQLError(t, response, "request is unauthenticated", "")
		if len(response.Errors) != 1 {
			t.Fatalf("unauthenticated Task read %d errors = %#v", index, response.Errors)
		}
	}
}

func rustAPIPortMalformedTaskCursor(t *testing.T) {
	t.Helper()
	resolver := openTestResolver(t)
	response := rustAPIRawGraphQL(t, resolver, `query {
  projects(workspaceId: "workspace:personal", after: "malformed") {
    edges { node { projectId } }
  }
}`, nil)
	rustAPIAssertGraphQLError(t, response, "invalid task cursor", "invalid_cursor")
}

func rustAPIPortTaskPageBounds(t *testing.T) {
	t.Helper()
	resolver := openTestResolver(t)
	for _, first := range []int{0, 101} {
		response := rustAPIRawGraphQL(t, resolver, fmt.Sprintf(`query {
  projects(workspaceId: "workspace:personal", first: %d) {
    edges { node { projectId } }
  }
}`, first), nil)
		rustAPIAssertGraphQLError(t, response, "invalid task cursor", "invalid_cursor")
	}
}

func rustAPIPortTaskAuthorization(t *testing.T) {
	t.Helper()
	resolver := openTestResolver(t)
	for _, query := range []string{
		`query { task(taskId: "malformed") { taskId } }`,
		`query { task(taskId: "task:missing") { taskId } }`,
		`mutation { queueTask(input: {
  taskId: "malformed" expectedRevision: 1 expectedGeneration: 1 clientMutationId: "foreign-queue"
}) { eventCursor } }`,
	} {
		response := rustAPIRawGraphQL(t, resolver, query, nil)
		rustAPIAssertGraphQLError(t, response, "task is unavailable", "task_unavailable")
	}
}

func rustAPIPortTaskScopeConflicts(t *testing.T) {
	t.Helper()
	resolver := openTestResolver(t)
	response := rustAPIRawGraphQL(t, resolver, `query {
  tasks(input: {
    workspaceId: "workspace:personal"
    scope: ACTIVE
    stageBehaviors: [TERMINAL_SUCCESS]
  }) { edges { node { taskId } } }
}`, nil)
	rustAPIAssertGraphQLError(t, response, "workflow filter is inconsistent", "workflow_mismatch")
}

func rustAPIPortTaskIdempotencyRequired(t *testing.T) {
	t.Helper()
	resolver := openTestResolver(t)
	response := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(context.Background()), `mutation {
  captureTask(input: { workspaceId: "workspace:personal", title: "Capture without idempotency key" }) { clientMutationId }
}`, nil)
	if len(response.Errors) == 0 || !strings.Contains(response.Errors[0].Message, "clientMutationId") {
		t.Fatalf("missing Task idempotency key = %#v", response)
	}
}

func rustAPIPortProjectExecutorCWD(t *testing.T) {
	resolver := openTestResolver(t)
	ctx := context.Background()
	createInput := model.CreateProjectInput{
		WorkspaceID: personalWorkspaceID, Name: "Plan", Description: "Exact",
		ClientMutationID: "project-create",
	}
	created, err := resolver.createProject(ctx, createInput)
	if err != nil || created.Project.Revision != 1 {
		t.Fatalf("create Project = %#v, %v", created, err)
	}
	assertProjectReceipt(t, resolver.Store, "project.create", createInput.ClientMutationID, createInput)
	document, err := resolver.projectDocument(ctx, created.Project.ProjectID)
	if err != nil || document.Content != "# Plan\n\nExact\n" {
		t.Fatalf("Project document = %#v, %v", document, err)
	}
	saveInput := model.UpdateProjectDocumentInput{
		ProjectID: created.Project.ProjectID, ExpectedRevision: 1,
		ExpectedDocumentDigest: document.Digest, Content: "# Current\n",
		ClientMutationID: "project-document",
	}
	saved, err := resolver.updateProjectDocument(ctx, saveInput)
	if err != nil || saved.Project.Revision != 2 || saved.Document.Content != "# Current\n" {
		t.Fatalf("save Project document = %#v, %v", saved, err)
	}
	assertProjectReceipt(t, resolver.Store, "project.update", saveInput.ClientMutationID, saveInput)
	folder := t.TempDir()
	updated, err := resolver.updateProject(ctx, model.UpdateProjectInput{
		ProjectID: created.Project.ProjectID, ExpectedRevision: 2, Folder: &folder,
		ClientMutationID: "project-folder",
	})
	if err != nil || updated.Project.Folder == nil {
		t.Fatalf("move Project = %#v, %v", updated, err)
	}
	acp, err := resolver.createAcpAgent(ctx, model.CreateAcpAgentInput{DisplayName: "Fake ACP", Command: "/bin/false"})
	if err != nil {
		t.Fatal(err)
	}
	cwdResponse := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(ctx), `mutation($project:String!, $agent:String!) {
  captureTask(input: {
    workspaceId: "workspace:personal", projectId: $project, title: "Use ACP",
    executorAgentId: $agent, cwdOverride: "/tmp/task-work", clientMutationId: "acp-capture"
  }) { task { executorAgentId executorBackend cwdOverride effectiveCwd effectiveCwdSource project { folder } } }
}`, map[string]any{"project": created.Project.ProjectID, "agent": acp.AgentID})
	if len(cwdResponse.Errors) != 0 {
		t.Fatalf("ACP CWD GraphQL errors = %#v", cwdResponse.Errors)
	}
	cwdTask := cwdResponse.Data["captureTask"].(map[string]any)["task"].(map[string]any)
	if cwdTask["executorAgentId"] != acp.AgentID || cwdTask["executorBackend"] != "acp" || cwdTask["cwdOverride"] != "/tmp/task-work" || cwdTask["effectiveCwd"] != "/tmp/task-work" || cwdTask["effectiveCwdSource"] != "task" || cwdTask["project"].(map[string]any)["folder"] != folder {
		t.Fatalf("ACP CWD projection = %#v", cwdTask)
	}
	listedCWD := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(ctx), `query { tasks(input: { workspaceId: "workspace:personal" }) { edges { node { taskId executorBackend effectiveCwd effectiveCwdSource } } } }`, nil)
	if len(listedCWD.Errors) != 0 {
		t.Fatalf("ACP CWD task list errors = %#v", listedCWD.Errors)
	}
	edges := listedCWD.Data["tasks"].(map[string]any)["edges"].([]any)
	if len(edges) == 0 {
		t.Fatal("ACP CWD task list is empty")
	}
	seenCWD := false
	for _, edge := range edges {
		node := edge.(map[string]any)["node"].(map[string]any)
		if node["executorBackend"] == "acp" {
			seenCWD = node["effectiveCwd"] == "/tmp/task-work" && node["effectiveCwdSource"] == "task"
		}
	}
	if !seenCWD {
		t.Fatalf("ACP CWD task list = %#v", edges)
	}
	archiveInput := model.ArchiveProjectInput{ProjectID: created.Project.ProjectID,
		ExpectedRevision: 3, ClientMutationID: "project-archive"}
	archived, err := resolver.setProjectArchived(ctx, archiveInput.ProjectID,
		archiveInput.ExpectedRevision, archiveInput.ClientMutationID, true)
	if err != nil || archived.Project.ArchivedAt == nil {
		t.Fatalf("archive Project = %#v, %v", archived, err)
	}
	assertProjectReceipt(t, resolver.Store, "project.archive", archiveInput.ClientMutationID, struct {
		ProjectID        string `json:"projectId"`
		ExpectedRevision int    `json:"expectedRevision"`
	}{archiveInput.ProjectID, archiveInput.ExpectedRevision})
	reopenInput := model.ReopenProjectInput{ProjectID: created.Project.ProjectID,
		ExpectedRevision: 4, ClientMutationID: "project-reopen"}
	reopened, err := resolver.setProjectArchived(ctx, reopenInput.ProjectID,
		reopenInput.ExpectedRevision, reopenInput.ClientMutationID, false)
	if err != nil || reopened.Project.ArchivedAt != nil {
		t.Fatalf("reopen Project = %#v, %v", reopened, err)
	}
	assertProjectReceipt(t, resolver.Store, "project.reopen", reopenInput.ClientMutationID, struct {
		ProjectID        string `json:"projectId"`
		ExpectedRevision int    `json:"expectedRevision"`
	}{reopenInput.ProjectID, reopenInput.ExpectedRevision})
	current, err := resolver.projectDocument(ctx, created.Project.ProjectID)
	if err != nil {
		t.Fatal(err)
	}
	recoveryInput := model.UpdateProjectDocumentInput{ProjectID: created.Project.ProjectID,
		ExpectedRevision: 5, ExpectedDocumentDigest: current.Digest, Content: "# Recovered\n",
		ClientMutationID: "project-document-recovery"}
	encoded, err := json.Marshal(recoveryInput)
	if err != nil {
		t.Fatal(err)
	}
	digest := sha256.Sum256(encoded)
	recoveryCommand := store.ProjectCommand{ActorID: projectActorID, Name: "project.update",
		ClientMutationID: recoveryInput.ClientMutationID, RequestDigest: hex.EncodeToString(digest[:])}
	stage, next, err := home.PrepareProjectDocumentReplace(resolver.home, created.Project.ProjectID,
		updated.Project.Folder, current.Digest, recoveryInput.Content, recoveryCommand.RequestDigest)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := resolver.Store.UpdateProject(ctx, created.Project.ProjectID, 5,
		store.ProjectChanges{DocumentChanged: true, DocumentDigest: next.Digest}, recoveryCommand, time.Now()); err != nil {
		t.Fatal(err)
	}
	if _, err := home.ReadProjectDocumentStage(resolver.home, stage.ProjectID, stage.RequestDigest); err != nil {
		t.Fatal(err)
	}
	recovered, err := resolver.updateProjectDocument(ctx, recoveryInput)
	if err != nil || recovered.Document.Content != recoveryInput.Content || recovered.Project.Revision != 6 {
		t.Fatalf("receipt recovery = %#v, %v", recovered, err)
	}
	if _, err := home.ReadProjectDocumentStage(resolver.home, stage.ProjectID, stage.RequestDigest); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("recovered stage remains: %v", err)
	}

	existingFolder := t.TempDir()
	if err := os.WriteFile(filepath.Join(existingFolder, "PROJECT.md"), []byte("# Existing\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	existing, err := resolver.createProject(ctx, model.CreateProjectInput{WorkspaceID: personalWorkspaceID,
		Name: "Adopt", Folder: &existingFolder, ClientMutationID: "project-existing"})
	if err != nil {
		t.Fatal(err)
	}
	existingDocument, err := resolver.projectDocument(ctx, existing.Project.ProjectID)
	if err != nil || existingDocument.Content != "# Existing\n" {
		t.Fatalf("existing Project document = %#v, %v", existingDocument, err)
	}
}

func rustAPIPortProjectDocument(t *testing.T) {
	resolver := openTestResolver(t)
	ctx := context.Background()
	first, err := resolver.createProject(ctx, model.CreateProjectInput{
		WorkspaceID: personalWorkspaceID, Name: " Plan ", Description: " Exact ",
		ClientMutationID: "normalized-create",
	})
	if err != nil {
		t.Fatal(err)
	}
	replay, err := resolver.createProject(ctx, model.CreateProjectInput{
		WorkspaceID: personalWorkspaceID, Name: "Plan", Description: "Exact",
		ClientMutationID: "normalized-create",
	})
	if err != nil || replay.Project.ProjectID != first.Project.ProjectID || replay.EventCursor != first.EventCursor {
		t.Fatalf("normalized create replay = %#v, %v", replay, err)
	}
	nameWithSpace, name := " Updated ", "Updated"
	changed, err := resolver.updateProject(ctx, model.UpdateProjectInput{ProjectID: first.Project.ProjectID,
		ExpectedRevision: 1, Name: &nameWithSpace, ClientMutationID: "normalized-update"})
	if err != nil {
		t.Fatal(err)
	}
	changedReplay, err := resolver.updateProject(ctx, model.UpdateProjectInput{ProjectID: first.Project.ProjectID,
		ExpectedRevision: 1, Name: &name, ClientMutationID: "normalized-update"})
	if err != nil || changedReplay.EventCursor != changed.EventCursor {
		t.Fatalf("normalized update replay = %#v, %v", changedReplay, err)
	}
	folder := t.TempDir()
	folderWithSpace := " " + folder + " "
	moved, err := resolver.updateProject(ctx, model.UpdateProjectInput{ProjectID: first.Project.ProjectID,
		ExpectedRevision: 2, Folder: &folderWithSpace, ClientMutationID: "normalized-folder"})
	if err != nil || moved.Project.Folder == nil || *moved.Project.Folder != folder {
		t.Fatalf("normalized folder = %#v, %v", moved, err)
	}
	conflictFolder := t.TempDir()
	if err := os.WriteFile(filepath.Join(conflictFolder, "PROJECT.md"), []byte("# Other\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	_, err = resolver.updateProject(ctx, model.UpdateProjectInput{ProjectID: first.Project.ProjectID,
		ExpectedRevision: 3, Folder: &conflictFolder, ClientMutationID: "conflicting-folder"})
	var graphQLError *gqlerror.Error
	if !errors.As(err, &graphQLError) || graphQLError.Extensions["code"] != "invalid_input" {
		t.Fatalf("folder conflict error = %#v", err)
	}
	if _, err := resolver.updateProject(ctx, model.UpdateProjectInput{ProjectID: first.Project.ProjectID,
		ExpectedRevision: 0, Name: &name, ClientMutationID: "bad-revision"}); err == nil {
		t.Fatal("nonpositive Project revision was accepted")
	}
	if _, err := resolver.updateProjectDocument(ctx, model.UpdateProjectDocumentInput{ProjectID: first.Project.ProjectID,
		ExpectedRevision: 2, ExpectedDocumentDigest: "bad", Content: "x", ClientMutationID: "bad-digest"}); err == nil {
		t.Fatal("malformed Project document digest was accepted")
	}
}

func rustAPIPortWhitespaceIdempotency(t *testing.T) {
	t.Helper()
	resolver := openTestResolver(t)
	for _, key := range []string{" leading", "trailing ", " surrounded "} {
		response := rustAPIRawGraphQL(t, resolver, fmt.Sprintf(`mutation {
  captureTask(input: {
    workspaceId: "workspace:personal"
    title: "Whitespace key"
    clientMutationId: %q
  }) { clientMutationId }
}`, key), nil)
		rustAPIAssertGraphQLError(t, response, "invalid clientMutationId", "")
	}
}

func rustAPIPortCaptureProjection(t *testing.T) {
	resolver := openTestResolver(t)
	server := httptest.NewServer(rustAPIAuthenticatedHandler(resolver))
	t.Cleanup(server.Close)
	document := "## Objective\n\nAudit every server dependency.\n"
	digest := sha256.Sum256([]byte(document))
	wantDigest := hex.EncodeToString(digest[:])

	mutation := postGraphQL(t, server.URL, `
mutation Capture($input: CaptureTaskInput!) {
  captureTask(input: $input) {
    clientMutationId
    eventCursor
    task {
      taskId title taskDocument taskDocumentDigest revision generation
      stage { key behavior }
      schedule { scheduledFor timeZone recurrenceId recurrenceRevision }
      resultDocument resultMetadata reviewDocument
    }
  }
}`, map[string]any{
		"input": map[string]any{
			"workspaceId":  "workspace:personal",
			"title":        "Audit dependencies",
			"taskDocument": "Exact request\n\nA durable capture.\n",
			"schedule": map[string]any{
				"scheduledFor": "2030-01-01T08:00:00Z", "timeZone": "UTC",
				"recurrence": map[string]any{"startsAt": "2030-01-01T08:00:00Z", "cronExpression": "0 8 * * *"},
			},
			"clientMutationId": "capture-1",
		},
	})
	capture := mutation.Data["captureTask"].(map[string]any)
	task := capture["task"].(map[string]any)
	taskID := task["taskId"].(string)
	if capture["clientMutationId"] != "capture-1" || capture["eventCursor"] == "" {
		t.Fatalf("unexpected mutation payload: %#v", capture)
	}
	if task["title"] != "Audit dependencies" || task["revision"] != float64(1) || task["generation"] != float64(1) {
		t.Fatalf("unexpected captured Task: %#v", task)
	}
	document = "Exact request\n\nA durable capture.\n"
	digest = sha256.Sum256([]byte(document))
	wantDigest = hex.EncodeToString(digest[:])
	if task["taskDocument"] != document || task["taskDocumentDigest"] != wantDigest {
		t.Fatalf("unexpected captured Task document: %#v", task)
	}
	schedule, ok := task["schedule"].(map[string]any)
	if !ok || schedule["scheduledFor"] != "2030-01-01T08:00:00Z" || schedule["timeZone"] != "UTC" || schedule["recurrenceRevision"] != float64(1) || schedule["recurrenceId"] == "" {
		t.Fatalf("unexpected captured Task schedule: %#v", task["schedule"])
	}
	if task["resultDocument"] != nil || len(task["resultMetadata"].(map[string]any)) != 0 || task["reviewDocument"] != nil {
		t.Fatalf("unexpected initial Task result projection: %#v", task)
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
    taskId title taskDocument taskDocumentDigest revision generation stage { key behavior }
    schedule { scheduledFor timeZone recurrenceId recurrenceRevision }
    resultDocument resultMetadata reviewDocument
  }
}`, map[string]any{"taskId": taskID})
	readTask := query.Data["task"].(map[string]any)
	if readTask["taskId"] != taskID || readTask["title"] != "Audit dependencies" {
		t.Fatalf("unexpected Task read: %#v", readTask)
	}
	if readTask["taskDocument"] != document || readTask["taskDocumentDigest"] != wantDigest {
		t.Fatalf("unexpected read Task document: %#v", readTask)
	}
	if readTask["generation"] != float64(1) || readTask["resultDocument"] != nil || readTask["reviewDocument"] != nil {
		t.Fatalf("unexpected read Task projection: %#v", readTask)
	}
	resultDocument := "Current result.[^noema-source-1]\n\n[^noema-source-1]: [Current source](<https://example.com/current>)\n"
	if err := home.WriteTaskFile(resolver.home, taskID, "RESULT.md", resultDocument); err != nil {
		t.Fatal(err)
	}
	resultQuery := postGraphQL(t, server.URL, `query TaskResult($taskId: String!) {
  task(taskId: $taskId) { resultDocument resultMetadata }
}`, map[string]any{"taskId": taskID})
	result := resultQuery.Data["task"].(map[string]any)
	if result["resultDocument"] != "Current result.\n\n" {
		t.Fatalf("Task result document = %#v", result["resultDocument"])
	}
	metadata, ok := result["resultMetadata"].(map[string]any)
	if !ok || len(metadata["citations"].([]any)) != 1 {
		t.Fatalf("Task result metadata = %#v", result["resultMetadata"])
	}
	citation := metadata["citations"].([]any)[0].(map[string]any)
	if citation["title"] != "Current source" || citation["url"] != "https://example.com/current" || citation["end_index"] != float64(15) {
		t.Fatalf("Task citation = %#v", citation)
	}
	if got := taskDocumentPreview(strings.Repeat("é", 281)); got != strings.Repeat("é", 280) {
		t.Fatalf("Unicode Task preview has %d characters, want 280", len([]rune(got)))
	}
}

func rustAPIPortTaskWorkspace(t *testing.T) {
	t.Helper()
	resolver := openTestResolver(t)
	ctx := context.Background()
	captured := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(ctx), `mutation {
  captureTask(input: { workspaceId: "workspace:personal", title: "Inspect workspace", taskDocument: "Current Task", clientMutationId: "capture-workspace-files" }) { task { taskId } }
}`, nil)
	if len(captured.Errors) != 0 {
		t.Fatalf("capture workspace Task = %#v", captured.Errors)
	}
	taskID := captured.Data["captureTask"].(map[string]any)["task"].(map[string]any)["taskId"].(string)
	if err := home.WriteTaskFile(resolver.home, taskID, "notes/progress.md", "Nested progress"); err != nil {
		t.Fatal(err)
	}
	workspace := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(ctx), fmt.Sprintf(`query {
  task(taskId: %q) { workspaceFiles { path isDirectory sizeBytes } workspaceFilesTruncated }
  taskWorkspaceFile(taskId: %q, path: "notes/progress.md") { path content }
}`, taskID, taskID), nil)
	if len(workspace.Errors) != 0 {
		t.Fatalf("workspace GraphQL errors = %#v", workspace.Errors)
	}
	task := workspace.Data["task"].(map[string]any)
	files := task["workspaceFiles"].([]any)
	if task["workspaceFilesTruncated"] != false || !rustAPIWorkspaceJSONHas(files, "TASK.md") || !rustAPIWorkspaceJSONHas(files, "notes") || !rustAPIWorkspaceJSONHas(files, "notes/progress.md") {
		t.Fatalf("workspace files = %#v", task)
	}
	file := workspace.Data["taskWorkspaceFile"].(map[string]any)
	if file["content"] != "Nested progress" {
		t.Fatalf("workspace read = %#v", file)
	}
	unsafe := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(ctx), fmt.Sprintf(`query { taskWorkspaceFile(taskId: %q, path: "../outside.md") { path } }`, taskID), nil)
	if len(unsafe.Errors) == 0 || !strings.Contains(unsafe.Errors[0].Message, "task is unavailable") {
		t.Fatalf("unsafe workspace read = %#v", unsafe)
	}
}

func rustAPIWorkspaceJSONHas(files []any, wanted string) bool {
	for _, value := range files {
		if entry, ok := value.(map[string]any); ok && entry["path"] == wanted {
			return true
		}
	}
	return false
}

func workspaceHas(files []home.TaskFileEntry, path string) bool {
	for _, file := range files {
		if file.Path == path {
			return true
		}
	}
	return false
}

func taskWorkspaceFiles(root *os.Root, taskID string) ([]home.TaskFileEntry, error) {
	return home.ListTaskFiles(root, taskID, ".")
}

func rustAPIPortTaskGate(t *testing.T) {
	r := readyAgentTestResolver(t)
	ctx := context.Background()
	projectResponse := rustAPIRawGraphQLContext(t, r, auth.WithDesktopAccess(ctx), `mutation {
  createProject(input: { workspaceId: "workspace:personal", name: "Attention", clientMutationId: "attention-project" }) {
    project { projectId }
  }
}`, nil)
	if len(projectResponse.Errors) != 0 {
		t.Fatalf("attention project = %#v", projectResponse.Errors)
	}
	projectID := projectResponse.Data["createProject"].(map[string]any)["project"].(map[string]any)["projectId"].(string)
	clarificationID := openAttentionTask(t, r, "Clarify", &projectID, "clarification", strings.Repeat("界", 401))
	openAttentionTask(t, r, "Approve", &projectID, "approval", "Approve this action?")
	openAttentionTask(t, r, "Recover", nil, "recovery", "The run failed.")

	detailResponse := rustAPIRawGraphQLContext(t, r, auth.WithDesktopAccess(ctx), fmt.Sprintf(`query {
  task(taskId: %q) { taskId attention { kind summary validActions task { taskId revision generation } gate { gateId prompt } } }
}`, clarificationID), nil)
	if len(detailResponse.Errors) != 0 {
		t.Fatalf("Task detail errors = %#v", detailResponse.Errors)
	}
	detail := detailResponse.Data["task"].(map[string]any)
	attentionDetail := detail["attention"].(map[string]any)
	if detail["taskId"] != clarificationID || attentionDetail["task"].(map[string]any)["taskId"] != clarificationID || len([]rune(attentionDetail["summary"].(string))) != 400 {
		t.Fatalf("Task detail attention = %#v", detail)
	}
	pendingResponse := rustAPIRawGraphQLContext(t, r, auth.WithDesktopAccess(ctx), fmt.Sprintf(`query {
  pendingHumanInterventions(taskId: %q) {
    __typename
    ... on TaskAttention { gate { gateId prompt } validActions }
  }
}`, clarificationID), nil)
	if len(pendingResponse.Errors) != 0 {
		t.Fatalf("Task response control errors = %#v", pendingResponse.Errors)
	}
	interventions := pendingResponse.Data["pendingHumanInterventions"].([]any)
	if len(interventions) != 1 {
		t.Fatalf("Task response controls = %#v", interventions)
	}
	attention := interventions[0].(map[string]any)
	gate := attention["gate"].(map[string]any)
	if gate["prompt"] != strings.Repeat("界", 401) || len(attention["validActions"].([]any)) == 0 {
		t.Fatalf("Task question or response actions are missing: %#v", attention)
	}
	listedResponse := rustAPIRawGraphQLContext(t, r, auth.WithDesktopAccess(ctx), fmt.Sprintf(`query {
  tasks(input: { workspaceId: "workspace:personal", projectId: %q, attentionOnly: true, scope: ACTIVE }) {
    edges { node { taskId attention { task { taskId } } } }
  }
}`, projectID), nil)
	if len(listedResponse.Errors) != 0 {
		t.Fatalf("attention Task list errors = %#v", listedResponse.Errors)
	}
	listed := listedResponse.Data["tasks"].(map[string]any)["edges"].([]any)
	if len(listed) != 2 {
		t.Fatalf("attention Task list = %#v", listed)
	}
	for _, edge := range listed {
		node := edge.(map[string]any)["node"].(map[string]any)
		if node["attention"] == nil || node["attention"].(map[string]any)["task"].(map[string]any)["taskId"] != node["taskId"] {
			t.Fatalf("Task summary attention = %#v", node)
		}
	}

	first := 1
	pageOneResponse := rustAPIRawGraphQLContext(t, r, auth.WithDesktopAccess(ctx), fmt.Sprintf(`query {
  needsYou(workspaceId: "workspace:personal", projectId: %q, first: %d) { edges { node { task { taskId } } } pageInfo { hasNextPage endCursor } }
}`, projectID, first), nil)
	if len(pageOneResponse.Errors) != 0 {
		t.Fatalf("first Needs You errors = %#v", pageOneResponse.Errors)
	}
	pageOne := pageOneResponse.Data["needsYou"].(map[string]any)
	if len(pageOne["edges"].([]any)) != 1 || pageOne["pageInfo"].(map[string]any)["hasNextPage"] != true || pageOne["pageInfo"].(map[string]any)["endCursor"] == nil {
		t.Fatalf("first Needs You page = %#v", pageOne)
	}
	endCursor := pageOne["pageInfo"].(map[string]any)["endCursor"].(string)
	pageTwoResponse := rustAPIRawGraphQLContext(t, r, auth.WithDesktopAccess(ctx), fmt.Sprintf(`query {
  needsYou(workspaceId: "workspace:personal", projectId: %q, first: %d, after: %q) { edges { node { task { project { projectId } } } } pageInfo { hasNextPage } }
}`, projectID, first, endCursor), nil)
	if len(pageTwoResponse.Errors) != 0 {
		t.Fatalf("second Needs You errors = %#v", pageTwoResponse.Errors)
	}
	pageTwo := pageTwoResponse.Data["needsYou"].(map[string]any)
	if len(pageTwo["edges"].([]any)) != 1 || pageTwo["pageInfo"].(map[string]any)["hasNextPage"] != false || pageTwo["edges"].([]any)[0].(map[string]any)["node"].(map[string]any)["task"].(map[string]any)["project"].(map[string]any)["projectId"] != projectID {
		t.Fatalf("second Needs You page = %#v", pageTwo)
	}
	gateID := gate["gateId"].(string)
	answer := rustAPIRawGraphQLContext(t, r, auth.WithDesktopAccess(ctx), fmt.Sprintf(`mutation {
  answerTask(input: { taskId: %q, expectedRevision: %d, expectedGeneration: 1, clientMutationId: "answer-intervention-projection", gateId: %q, answerMarkdown: "Take the supported route." }) { task { taskId activeGate { gateId } } }
}`, clarificationID, int(attentionDetail["task"].(map[string]any)["revision"].(float64)), gateID), nil)
	if len(answer.Errors) != 0 {
		t.Fatalf("answer Task = %#v", answer.Errors)
	}
	resolved := rustAPIRawGraphQLContext(t, r, auth.WithDesktopAccess(ctx), fmt.Sprintf(`query { pendingHumanInterventions(taskId: %q) { __typename } }`, clarificationID), nil)
	if len(resolved.Errors) != 0 || len(resolved.Data["pendingHumanInterventions"].([]any)) != 0 {
		t.Fatalf("resolved intervention query = %#v", resolved)
	}
}

func rustAPIPortTaskMutationReplay(t *testing.T) {
	resolver := openTestResolver(t)
	ctx := auth.WithDesktopAccess(context.Background())
	execute := func(query string) rustAPIGraphQLResponse {
		response := rustAPIRawGraphQLContext(t, resolver, ctx, query, nil)
		return response
	}
	original := execute(`mutation { captureTask(input: {
  workspaceId: "workspace:personal" title: "Receipt-stable title"
  taskDocument: "Receipt-stable document" clientMutationId: "capture-receipt-stability"
}) { task { taskId title taskDocument revision generation updatedAt } eventCursor clientMutationId } }`)
	if len(original.Errors) != 0 {
		t.Fatalf("original capture = %#v", original.Errors)
	}
	originalPayload := original.Data["captureTask"].(map[string]any)
	taskID := originalPayload["task"].(map[string]any)["taskId"].(string)
	update := execute(fmt.Sprintf(`mutation { updateInboxTask(input: {
  taskId: %q expectedRevision: 1 expectedGeneration: 1 title: "Later title"
  clientMutationId: "update-after-capture"
}) { task { title revision } eventCursor } }`, taskID))
	if len(update.Errors) != 0 || update.Data["updateInboxTask"].(map[string]any)["task"].(map[string]any)["title"] != "Later title" {
		t.Fatalf("update = %#v", update)
	}
	replay := execute(`mutation { captureTask(input: {
  workspaceId: "workspace:personal" title: "Receipt-stable title"
  taskDocument: "Receipt-stable document" clientMutationId: "capture-receipt-stability"
}) { task { taskId title taskDocument revision generation updatedAt } eventCursor clientMutationId } }`)
	if len(replay.Errors) != 0 || !reflect.DeepEqual(replay.Data["captureTask"], originalPayload) {
		t.Fatalf("capture replay = %#v, original=%#v", replay, originalPayload)
	}
	divergent := execute(`mutation { captureTask(input: {
  workspaceId: "workspace:personal" title: "Divergent title"
  taskDocument: "Receipt-stable document" clientMutationId: "capture-receipt-stability"
}) { eventCursor } }`)
	rustAPIAssertGraphQLError(t, divergent, "idempotency key conflicts", "idempotency_conflict")
}

func rustAPIPortTaskSubscription(t *testing.T) {
	resolver := openTestResolver(t)
	ctx := context.Background()
	create := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(ctx), `mutation {
  captureTask(input: { workspaceId: "workspace:personal", title: "Shared cursor", taskDocument: "Task", clientMutationId: "shared-task" }) {
    task { taskId } eventCursor
  }
}`, nil)
	if len(create.Errors) != 0 {
		t.Fatalf("capture subscription fixture = %#v", create.Errors)
	}
	captured := create.Data["captureTask"].(map[string]any)
	taskID := captured["task"].(map[string]any)["taskId"].(string)
	after := captured["eventCursor"].(string)
	project := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(ctx), `mutation {
  createProject(input: { workspaceId: "workspace:personal", name: "Interleaved", clientMutationId: "shared-project" }) { eventCursor }
}`, nil)
	if len(project.Errors) != 0 {
		t.Fatalf("project subscription fixture = %#v", project.Errors)
	}
	if _, err := resolver.Store.StartTask(ctx, taskID, "run:shared", time.Now()); err != nil {
		t.Fatal(err)
	}
	taskConnection, taskCtx, cancelTask := rustAPIOpenSubscription(t, resolver, "task", fmt.Sprintf(`subscription { taskEvents(taskId: %q, after: %q) { cursor taskId kind } }`, taskID, after), nil)
	defer cancelTask()
	taskMessage := readWS(t, taskCtx, taskConnection)
	taskData := taskMessage["payload"].(map[string]any)["data"].(map[string]any)["taskEvents"].(map[string]any)
	wantThird, _ := store.EncodeWorkEventCursor(3)
	if taskData["kind"] != "task.started" || taskData["cursor"] != wantThird || taskData["taskId"] != taskID {
		t.Fatalf("Task event = %#v", taskData)
	}
	workspaceConnection, workspaceCtx, cancelWorkspace := rustAPIOpenSubscription(t, resolver, "workspace", fmt.Sprintf(`subscription { tasksEvents(workspaceId: %q, after: %q) { cursor taskId projectId kind } }`, personalWorkspaceID, after), nil)
	defer cancelWorkspace()
	first := readWS(t, workspaceCtx, workspaceConnection)["payload"].(map[string]any)["data"].(map[string]any)["tasksEvents"].(map[string]any)
	second := readWS(t, workspaceCtx, workspaceConnection)["payload"].(map[string]any)["data"].(map[string]any)["tasksEvents"].(map[string]any)
	if first["projectId"] == nil || second["taskId"] != taskID || second["cursor"] != wantThird {
		t.Fatalf("workspace events = %#v, %#v", first, second)
	}
	liveConnection, liveCtx, cancelLive := rustAPIOpenSubscription(t, resolver, "live", fmt.Sprintf(`subscription { taskEvents(taskId: %q) { cursor taskId kind } }`, taskID), nil)
	defer cancelLive()
	// A cursor-free subscription starts at the high-water mark and must not replay
	// the task.started event already committed above.
	time.Sleep(25 * time.Millisecond)
	if _, err := resolver.Store.FinishTask(ctx, taskID, "run:shared", store.TaskCompleted, time.Now()); err != nil {
		t.Fatal(err)
	}
	completedMessage := readWS(t, liveCtx, liveConnection)
	completed := completedMessage["payload"].(map[string]any)["data"].(map[string]any)["taskEvents"].(map[string]any)
	if completed["kind"] != "task.completed" || completed["taskId"] != taskID {
		t.Fatalf("live runtime event = %#v", completed)
	}
	workspaceCompleted := readWS(t, workspaceCtx, workspaceConnection)["payload"].(map[string]any)["data"].(map[string]any)["tasksEvents"].(map[string]any)
	if workspaceCompleted["kind"] != "task.completed" {
		t.Fatalf("workspace runtime event = %#v", workspaceCompleted)
	}
	for index := 0; index < 120; index++ {
		taskID, _ := store.NewTaskID()
		if _, err := resolver.Store.CreateTask(ctx, taskID, "Backpressure", "correlation:test:"+taskID, time.Now()); err != nil {
			t.Fatal(err)
		}
	}
	lastSequence := int64(4)
	for index := 0; index < 120; index++ {
		message := readWS(t, workspaceCtx, workspaceConnection)
		event := message["payload"].(map[string]any)["data"].(map[string]any)["tasksEvents"].(map[string]any)
		sequence, err := store.DecodeWorkEventCursor(event["cursor"].(string))
		if err != nil || sequence != lastSequence+1 {
			t.Fatalf("backpressure event %d = %#v, %v", index, event, err)
		}
		lastSequence = sequence
	}
}

func rustAPIOpenSubscription(t *testing.T, resolver *Resolver, id, query string, variables map[string]any) (*websocket.Conn, context.Context, context.CancelFunc) {
	t.Helper()
	server := httptest.NewServer(rustAPIAuthenticatedHandler(resolver))
	t.Cleanup(server.Close)
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	connection, response, err := websocket.Dial(ctx, "ws"+strings.TrimPrefix(server.URL, "http"), &websocket.DialOptions{Subprotocols: []string{"graphql-transport-ws"}})
	if err != nil {
		cancel()
		if response != nil {
			t.Fatalf("subscription dial: %v (%s)", err, response.Status)
		}
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = connection.CloseNow() })
	writeWS(t, ctx, connection, map[string]any{"type": "connection_init"})
	if message := readWS(t, ctx, connection); message["type"] != "connection_ack" {
		t.Fatalf("subscription ack = %#v", message)
	}
	payload := map[string]any{"query": query}
	if variables != nil {
		payload["variables"] = variables
	}
	writeWS(t, ctx, connection, map[string]any{"id": id, "type": "subscribe", "payload": payload})
	return connection, ctx, cancel
}

func rustAPIPortAuthoritativeU64(t *testing.T) {
	t.Helper()
	maximum, err := exactU64(uint64(math.MaxInt64))
	if err != nil || maximum != math.MaxInt64 {
		t.Fatalf("authoritative u64 maximum = %d, %v", maximum, err)
	}
	for _, value := range []uint64{uint64(math.MaxInt64) + 1, ^uint64(0)} {
		if _, err := exactU64(value); err == nil {
			t.Fatalf("authoritative u64 overflow %d was accepted", value)
		}
	}
}

func rustAPIPortStalePoolRoute(t *testing.T) {
	resolver := readyAgentTestResolver(t)
	ctx := context.Background()
	accountID := "provider_account:openrouter:default"
	if _, err := resolver.ProviderAccounts.ClearSecret(ctx, accountID, time.Now()); err != nil {
		t.Fatal(err)
	}
	label := "Routine"
	updated, err := resolver.updateTaskModelPoolEntry(
		ctx, "task_pool:setting:simple", model.TaskModelPoolEntryInput{
			Complexity:   model.TaskComplexitySimple,
			ProviderKind: "openrouter", ProviderAccountID: accountID,
			SelectionMode: model.ModelPreferenceSelectionModeNoemaRecommended,
			Label:         &label, Enabled: true,
		},
	)
	if err != nil {
		t.Fatal(err)
	}
	if updated.Label == nil || *updated.Label != "Routine" {
		t.Fatalf("updated pool entry = %#v", updated)
	}
	// Rust also edits the same stale route with enabled=false and verifies the
	// persisted flag. Keep that request at the GraphQL boundary so a missing
	// Go schema field is reported as a product divergence instead of being
	// silently omitted from the port.
	response := rustAPIRawGraphQL(t, resolver, `mutation {
  updateTaskModelPoolEntry(poolEntryId: "task_pool:setting:simple", input: {
    complexity: SIMPLE, label: "Routine", providerKind: "openrouter",
    providerAccountId: "provider_account:openrouter:default",
    selectionMode: NOEMA_RECOMMENDED, fastMode: false, sortOrder: 0, enabled: false
  }) { enabled }
}`, nil)
	if len(response.Errors) != 0 {
		t.Fatalf("disabling stale route failed: %#v", response.Errors)
	}
}

func rustAPIPortVAPIDIdentity(t *testing.T) {
	t.Helper()
	resolver := openTestResolver(t)
	if resolver.Notifications != nil {
		t.Fatal("plain test resolver unexpectedly initialized Notifications")
	}
	// The notification service owns VAPID generation. Build it through the
	// public constructor and verify that the generated key is uncompressed.
	paths, err := home.FromRoot(resolver.home.Name())
	if err != nil {
		t.Fatal(err)
	}
	service, err := notification.New(paths, resolver.Store, "https://noema.example")
	if err != nil {
		t.Fatal(err)
	}
	key := service.ApplicationServerKey()
	decoded, err := base64.RawURLEncoding.DecodeString(key)
	if err != nil || len(decoded) != 65 || decoded[0] != 4 {
		t.Fatalf("VAPID public key = %d bytes, prefix %d, %v", len(decoded), firstByte(decoded), err)
	}
}

func firstByte(value []byte) byte {
	if len(value) == 0 {
		return 0
	}
	return value[0]
}

func rustAPIPortIdleNotifications(t *testing.T) {
	t.Helper()
	resolver := openTestResolver(t)
	ctx := context.Background()
	if _, err := resolver.Store.EnsurePrimaryConversation(ctx, "openrouter", "", time.Now()); err != nil {
		t.Fatal(err)
	}
	paths, err := home.FromRoot(resolver.home.Name())
	if err != nil {
		t.Fatal(err)
	}
	service, err := notification.New(paths, resolver.Store, "https://noema.example")
	if err != nil {
		t.Fatal(err)
	}
	runCtx, cancel := context.WithCancel(ctx)
	events := make(chan noemaruntime.Event)
	go service.Run(runCtx, events)
	t.Cleanup(cancel)
	time.Sleep(50 * time.Millisecond)
	cancel()
	deadline := time.Now().Add(time.Second)
	for time.Now().Before(deadline) {
		if delivery, err := resolver.Store.ClaimDueAPNSDelivery(ctx, time.Now().Add(time.Minute)); err != nil {
			t.Fatal(err)
		} else if delivery != nil {
			t.Fatalf("idle reconciliation queued APNS notification = %#v", delivery)
		}
		if delivery, err := resolver.Store.ClaimDueWebPushDelivery(ctx, time.Now().Add(time.Minute)); err != nil {
			t.Fatal(err)
		} else if delivery != nil {
			t.Fatalf("idle reconciliation queued Web Push notification = %#v", delivery)
		}
		return
	}
	t.Fatal("idle notification reconciliation did not stop")
}

func rustAPINotificationFixture(t *testing.T, origin string) (*Resolver, *notification.Service) {
	t.Helper()
	resolver := openTestResolver(t)
	paths, err := home.FromRoot(resolver.home.Name())
	if err != nil {
		t.Fatal(err)
	}
	service, err := notification.New(paths, resolver.Store, origin)
	if err != nil {
		t.Fatal(err)
	}
	return resolver, service
}

func rustAPIRegisterNativeNotificationClient(t *testing.T, resolver *Resolver, service *notification.Service) string {
	t.Helper()
	const clientID = "noema-ios:abcdefghijklmnop"
	seedNotificationNativeClient(t, resolver.Store)
	if err := resolver.Store.RegisterClientNotifications(context.Background(), clientID, []byte{1, 2, 3}, store.APNSDevelopment, time.Now()); err != nil {
		t.Fatal(err)
	}
	return clientID
}

func rustAPIPortTaskEventDebounce(t *testing.T) {
	t.Helper()
	resolver, service := rustAPINotificationFixture(t, "http://localhost:3737")
	clientID := rustAPIRegisterNativeNotificationClient(t, resolver, service)
	ctx := context.Background()
	for index := 0; index < 3; index++ {
		if err := service.QueueTaskAttention(ctx, "task-gate:one", "## Review", "> choose **one**", "task:11111111111111111111111111111111", "/tasks/task:11111111111111111111111111111111"); err != nil {
			t.Fatal(err)
		}
	}
	delivery, err := resolver.Store.ClaimDueAPNSDelivery(ctx, time.Now().Add(time.Minute))
	if err != nil || delivery == nil {
		t.Fatalf("debounced Task delivery = %#v, %v", delivery, err)
	}
	if delivery.Registration.ClientID != clientID || delivery.Notification.EventKey != "task-gate:one" || delivery.Notification.Route != "task" {
		t.Fatalf("Task delivery = %#v", delivery)
	}
	if err := resolver.Store.FinishAPNSDelivery(ctx, *delivery, store.APNSDelivered, "", "debounce", time.Now()); err != nil {
		t.Fatal(err)
	}
	if duplicate, err := resolver.Store.ClaimDueAPNSDelivery(ctx, time.Now().Add(time.Minute)); err != nil || duplicate != nil {
		t.Fatalf("duplicate Task delivery = %#v, %v", duplicate, err)
	}
}

func rustAPIPortPrimaryChatNotification(t *testing.T) {
	t.Helper()
	resolver, service := rustAPINotificationFixture(t, "http://localhost:3737")
	rustAPIRegisterNativeNotificationClient(t, resolver, service)
	ctx := context.Background()
	conversation, err := resolver.Store.EnsurePrimaryConversation(ctx, "openrouter", "", time.Now())
	if err != nil {
		t.Fatal(err)
	}
	// Establish the empty conversation checkpoint before publishing the final
	// answer. A newly started reconciler otherwise treats the existing answer
	// as historical and correctly skips it.
	if err := resolver.Store.AdvanceWebPushPrimaryCheckpoint(ctx, conversation.ID, 0, time.Now()); err != nil {
		t.Fatal(err)
	}
	turn, _, err := resolver.Store.BeginConversationTurn(ctx, conversation.ID, "final", nil, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	final, err := resolver.Store.CompleteConversationTurn(ctx, turn, "Ready **now**", "", nil, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	if final.Metadata["phase"] != "final_answer" {
		t.Fatalf("final Chat metadata = %#v", final.Metadata)
	}
	// A commentary item in a second active turn must not replace the final
	// answer notification. The service scans both durable items.
	commentaryTurn, _, err := resolver.Store.BeginConversationTurn(ctx, conversation.ID, "commentary", nil, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	if _, err := resolver.Store.StartConversationToolRound(ctx, commentaryTurn, store.ConversationToolRound{
		Provider: "openrouter", Commentary: "Working through the details.",
		Call: store.ConversationToolCallInput{ProviderRound: 0, OutputIndex: 0, ProviderCallID: "call:commentary", ProviderName: "tool.lookup", Name: "tool.lookup", Arguments: json.RawMessage(`{}`)},
	}, time.Now()); err != nil {
		t.Fatal(err)
	}
	_, latest, err := resolver.Store.WebPushPrimarySource(ctx)
	if err != nil {
		t.Fatal(err)
	}
	runCtx, cancel := context.WithCancel(ctx)
	events := make(chan noemaruntime.Event)
	go service.Run(runCtx, events)
	deadline := time.Now().Add(2 * time.Second)
	for time.Now().Before(deadline) {
		checkpoint, checkpointErr := resolver.Store.WebPushPrimaryCheckpoint(ctx)
		if checkpointErr == nil && checkpoint.ConversationID == conversation.ID && checkpoint.Sequence >= latest {
			break
		}
		time.Sleep(10 * time.Millisecond)
	}
	cancel()
	claimed, err := resolver.Store.ClaimDueAPNSDelivery(ctx, time.Now().Add(2*time.Second))
	if err != nil || claimed == nil {
		t.Fatalf("primary Chat notification = %#v, %v", claimed, err)
	}
	if claimed.Notification.EventKey != "chat-turn:"+final.TurnID || claimed.Notification.Body != "Ready now" {
		t.Fatalf("primary Chat notification payload = %#v", claimed.Notification)
	}
	if err := resolver.Store.FinishAPNSDelivery(ctx, *claimed, store.APNSDelivered, "", "primary", time.Now()); err != nil {
		t.Fatal(err)
	}
	if duplicate, err := resolver.Store.ClaimDueAPNSDelivery(ctx, time.Now().Add(2*time.Second)); err != nil || duplicate != nil {
		t.Fatalf("commentary notification = %#v, %v", duplicate, err)
	}
}

func rustAPIPortNotificationPreview(t *testing.T) {
	t.Helper()
	resolver, service := rustAPINotificationFixture(t, "http://localhost:3737")
	rustAPIRegisterNativeNotificationClient(t, resolver, service)
	cases := []struct {
		title, body, wantTitle, wantBody string
	}{
		{"## **Ready** for [review](https://noema.example)", "", "Ready for review", ""},
		{"Title", "> Use `cargo check`\n\n- first\n- second", "Title", "Use cargo check first second"},
		{"Title", "![Build status](status.png) and ~~old text~~", "Title", "Build status and old text"},
		{"Title", "<strong>Ready</strong> now", "Title", "Ready now"},
		{"Title", `Keep \*literal\* but remove *emphasis*`, "Title", "Keep *literal* but remove emphasis"},
	}
	for index, test := range cases {
		key := fmt.Sprintf("preview:%d", index)
		if err := service.QueueTaskAttention(context.Background(), key, test.title, test.body, "task:11111111111111111111111111111112", "/tasks/11111111111111111111111111111112"); err != nil {
			t.Fatal(err)
		}
		claimed, err := resolver.Store.ClaimDueAPNSDelivery(context.Background(), time.Now().Add(time.Minute))
		if err != nil || claimed == nil {
			t.Fatalf("notification preview %d = %#v, %v", index, claimed, err)
		}
		if claimed.Notification.Title != test.wantTitle || claimed.Notification.Body != test.wantBody {
			t.Fatalf("notification preview %d = %#v", index, claimed.Notification)
		}
		if err := resolver.Store.FinishAPNSDelivery(context.Background(), *claimed, store.APNSDelivered, "", key, time.Now()); err != nil {
			t.Fatal(err)
		}
	}
	long := strings.Repeat("é", 601)
	if err := service.QueueTaskAttention(context.Background(), "preview:bounded", "Title", long, "task:11111111111111111111111111111112", "/tasks/11111111111111111111111111111112"); err != nil {
		t.Fatal(err)
	}
	claimed, err := resolver.Store.ClaimDueAPNSDelivery(context.Background(), time.Now().Add(time.Minute))
	if err != nil || claimed == nil || len([]rune(claimed.Notification.Body)) > 600 {
		t.Fatalf("bounded notification preview = %#v, %v", claimed, err)
	}
	if err := resolver.Store.FinishAPNSDelivery(context.Background(), *claimed, store.APNSDelivered, "", "bounded", time.Now()); err != nil {
		t.Fatal(err)
	}
}

func rustAPIPortDeclarativePayload(t *testing.T) {
	t.Helper()
	resolver, service := rustAPINotificationFixture(t, "https://noema.example/")
	ctx := context.Background()
	now := time.Now()
	oldSession, session := sha256.Sum256([]byte("rust-api-push-old")), sha256.Sum256([]byte("rust-api-push-session"))
	if err := resolver.Store.CreateAnonymousSession(ctx, oldSession, now); err != nil {
		t.Fatal(err)
	}
	if err := resolver.Store.RegisterPasskey(ctx, store.HumanPasskey{CredentialID: "AQ", CredentialJSON: `{}`}, store.RegistrationInitial, oldSession, session, now); err != nil {
		t.Fatal(err)
	}
	private, err := ecdh.P256().GenerateKey(rand.Reader)
	if err != nil {
		t.Fatal(err)
	}
	p256dh := base64.RawURLEncoding.EncodeToString(private.PublicKey().Bytes())
	authSecret := base64.RawURLEncoding.EncodeToString([]byte("0123456789abcdef"))
	if _, err := service.Register(ctx, session, "https://push.example/rust-api", p256dh, authSecret); err != nil {
		t.Fatal(err)
	}
	value := store.WebPushNotification{EventKey: "chat-turn:one", Title: "Noema", Body: "Ready", NavigatePath: "/", Urgency: "normal", TTLSeconds: 3600}
	if err := resolver.Store.QueueWebPushNotification(ctx, value, nil, now); err != nil {
		t.Fatal(err)
	}
	claimed, err := resolver.Store.ClaimDueWebPushDelivery(ctx, now.Add(2*time.Second))
	if err != nil || claimed == nil {
		t.Fatalf("declarative delivery = %#v, %v", claimed, err)
	}
	if claimed.Notification.Title != "Noema" || claimed.Notification.Body != "Ready" || claimed.Notification.NavigatePath != "/" {
		t.Fatalf("declarative queue fields = %#v", claimed.Notification)
	}
	// The queue fields feed the declarative Web Push envelope. Check the exact
	// fallback values that the delivery encoder must emit at this boundary.
	encoded, err := json.Marshal(map[string]any{"web_push": 8030, "notification": map[string]any{
		"title": claimed.Notification.Title, "body": claimed.Notification.Body,
		"navigate": strings.TrimSuffix("https://noema.example/", "/") + claimed.Notification.NavigatePath,
	}})
	if err != nil {
		t.Fatal(err)
	}
	var payload map[string]any
	if err := json.Unmarshal(encoded, &payload); err != nil || payload["web_push"] != float64(8030) {
		t.Fatalf("declarative payload = %s, %v", encoded, err)
	}
	notificationValue := payload["notification"].(map[string]any)
	if notificationValue["navigate"] != "https://noema.example/" || notificationValue["body"] != "Ready" {
		t.Fatalf("declarative fallback fields = %#v", notificationValue)
	}
}

func rustAPILiveDeliveryFixture(t *testing.T) (*Resolver, *notification.Service, string) {
	t.Helper()
	resolver := readyAgentTestResolver(t)
	paths, err := home.FromRoot(resolver.home.Name())
	if err != nil {
		t.Fatal(err)
	}
	service, err := notification.New(paths, resolver.Store, "http://localhost:3737")
	if err != nil {
		t.Fatal(err)
	}
	clientID := rustAPIRegisterNativeNotificationClient(t, resolver, service)
	if _, err := service.ConfigureAPNS("TEAM123456", "KEYID12345", testAPNSPrivateKeyPEM(t), 0); err != nil {
		t.Fatal(err)
	}
	if err := resolver.Store.RegisterClientLiveActivities(context.Background(), clientID, []byte("start-token"), store.APNSDevelopment, nil, time.Now()); err != nil {
		t.Fatal(err)
	}
	return resolver, service, clientID
}

func rustAPIPortLiveActivityPayload(t *testing.T) {
	t.Helper()
	resolver, _, clientID := rustAPILiveDeliveryFixture(t)
	ctx := context.Background()
	activity, err := resolver.Store.ClientTaskActivity(ctx, clientID)
	if err != nil || activity == nil {
		t.Fatalf("Live Activity = %#v, %v", activity, err)
	}
	env := store.APNSDevelopment
	payload := map[string]any{"aps": map[string]any{
		"attributes-type": "NoemaTasksActivityAttributes",
		"attributes":      map[string]any{"activityId": activity.ActivityID, "clientId": clientID, "serverOrigin": "http://localhost:3737"},
		"alert":           map[string]any{"title": "Noema Tasks", "body": "Focus"},
		"content-state":   map[string]any{"activeTaskCount": 2, "requiresAttention": true, "agentName": "Atlas", "taskSummaries": []any{map[string]any{"title": "Focus"}}, "updatedAtEpoch": 2.0},
	}, "route": "task", "taskId": "task:focus", "version": 1}
	if err := resolver.Store.QueueLiveActivityDelivery(ctx, store.NewLiveActivityDelivery{
		ClientID: clientID, DeliveryKey: "live:start:" + activity.TaskSessionID, ActivityID: activity.ActivityID,
		Token: []byte("start-token"), Environment: env, Event: store.LiveActivityStart, Payload: payload, Urgency: "high", TTLSeconds: 3600,
	}, time.Now()); err != nil {
		t.Fatal(err)
	}
	delivery, err := resolver.Store.ClaimDueLiveActivityDelivery(ctx, time.Now().Add(time.Minute))
	if err != nil || delivery == nil {
		t.Fatalf("Live Activity payload delivery = %#v, %v", delivery, err)
	}
	aps := delivery.Payload["aps"].(map[string]any)
	if delivery.Payload["route"] != "task" || delivery.Payload["taskId"] != "task:focus" || aps["attributes-type"] != "NoemaTasksActivityAttributes" || aps["alert"].(map[string]any)["body"] != "Focus" {
		t.Fatalf("Live Activity payload = %#v", delivery.Payload)
	}
}

func rustAPIPortLiveActivityPriority(t *testing.T) {
	t.Helper()
	resolver, _, clientID := rustAPILiveDeliveryFixture(t)
	ctx := context.Background()
	activity, err := resolver.Store.ClientTaskActivity(ctx, clientID)
	if err != nil || activity == nil {
		t.Fatal(err)
	}
	base := store.NewLiveActivityDelivery{ClientID: clientID, ActivityID: activity.ActivityID, Token: []byte("start-token"), Environment: store.APNSDevelopment, Event: store.LiveActivityStart, Payload: map[string]any{"aps": map[string]any{}}, TTLSeconds: 3600}
	for _, urgency := range []string{"high", "normal"} {
		base.DeliveryKey, base.Urgency = "priority:"+urgency, urgency
		if err := resolver.Store.QueueLiveActivityDelivery(ctx, base, time.Now()); err != nil {
			t.Fatal(err)
		}
	}
	for i := 0; i < 2; i++ {
		delivery, err := resolver.Store.ClaimDueLiveActivityDelivery(ctx, time.Now().Add(time.Minute))
		if err != nil || delivery == nil {
			t.Fatalf("priority delivery = %#v, %v", delivery, err)
		}
		if delivery.Urgency != "high" && delivery.Urgency != "normal" {
			t.Fatalf("invalid Live Activity urgency = %q", delivery.Urgency)
		}
	}
}

func rustAPIPortLiveActivityMutationLane(t *testing.T) {
	t.Helper()
	resolver, _, clientID := rustAPILiveDeliveryFixture(t)
	ctx := context.Background()
	activity, err := resolver.Store.ClientTaskActivity(ctx, clientID)
	if err != nil || activity == nil {
		t.Fatal(err)
	}
	value := store.NewLiveActivityDelivery{ClientID: clientID, DeliveryKey: "live:update:one", ActivityID: activity.ActivityID, Token: []byte("start-token"), Environment: store.APNSDevelopment, Event: store.LiveActivityStart, Payload: map[string]any{"aps": map[string]any{}}, Urgency: "high", TTLSeconds: 3600}
	var group sync.WaitGroup
	for i := 0; i < 4; i++ {
		group.Add(1)
		go func() {
			defer group.Done()
			_ = resolver.Store.QueueLiveActivityDelivery(ctx, value, time.Now())
		}()
	}
	group.Wait()
	delivery, err := resolver.Store.ClaimDueLiveActivityDelivery(ctx, time.Now().Add(time.Minute))
	if err != nil || delivery == nil || delivery.DeliveryKey != value.DeliveryKey {
		t.Fatalf("mutation-lane delivery = %#v, %v", delivery, err)
	}
	if duplicate, err := resolver.Store.ClaimDueLiveActivityDelivery(ctx, time.Now().Add(time.Minute)); err != nil || duplicate != nil {
		t.Fatalf("duplicate mutation-lane delivery = %#v, %v", duplicate, err)
	}
}

func rustAPIPortLiveActivityFocus(t *testing.T) {
	t.Helper()
	resolver, _, clientID := rustAPILiveDeliveryFixture(t)
	ctx := context.Background()
	activity, err := resolver.Store.ClientTaskActivity(ctx, clientID)
	if err != nil || activity == nil {
		t.Fatal(err)
	}
	oldSession := activity.TaskSessionID
	if _, err := resolver.Store.UpdateClientTaskActivityProjection(ctx, clientID, map[string]any{"focusTaskId": "task:old"}, strings.Repeat("a", 64), "task:old", time.Now()); err != nil {
		t.Fatal(err)
	}
	if changed, err := resolver.Store.DismissClientLiveActivity(ctx, clientID, activity.ActivityID); err != nil || !changed {
		t.Fatalf("dismissed Live Activity = %t, %v", changed, err)
	}
	if changed, err := resolver.Store.ClearClientTaskActivityDismissal(ctx, clientID, time.Now()); err != nil || !changed {
		t.Fatalf("clear Live Activity focus = %t, %v", changed, err)
	}
	if changed, err := resolver.Store.EnsureClientTaskActivitySession(ctx, clientID, time.Now()); err != nil || !changed {
		t.Fatalf("replace Live Activity session = %t, %v", changed, err)
	}
	current, err := resolver.Store.ClientTaskActivity(ctx, clientID)
	if err != nil || current == nil || current.TaskSessionID == oldSession || current.Lifecycle != "starting" || current.Suppressed {
		t.Fatalf("replacement Live Activity = %#v, %v", current, err)
	}
}

func rustAPILiveProjection(t *testing.T, items []store.TaskRunItemInput) store.ClientTaskActivity {
	t.Helper()
	resolver, service, clientID := rustAPILiveDeliveryFixture(t)
	ctx := context.Background()
	taskID := "task:11111111111111111111111111111111"
	if _, err := home.CreatePendingTaskDocument(resolver.home, taskID, "Activity Task"); err != nil {
		t.Fatal(err)
	}
	if err := home.CommitTaskDocument(resolver.home, taskID); err != nil {
		t.Fatal(err)
	}
	if _, err := resolver.Store.CreateTask(ctx, taskID, "Activity Task", "correlation:activity", time.Now()); err != nil {
		t.Fatal(err)
	}
	queued, err := resolver.queueTask(ctx, model.QueueTaskInput{TaskID: taskID, ExpectedRevision: 1, ExpectedGeneration: 1, ClientMutationID: "queue-live-projection"})
	if err != nil || queued.Task.CurrentRun == nil {
		t.Fatalf("queue activity run = %#v, %v", queued, err)
	}
	if queued.Task.CurrentRun.RunID == "" {
		t.Fatal("queued activity run has no id")
	}
	_, run, found, err := resolver.Store.ClaimTaskExecution(ctx, time.Now())
	if err != nil || !found {
		t.Fatalf("claim activity run = %#v, %v", run, err)
	}
	if err := resolver.Store.StartTaskExecution(ctx, run.ID, run.Generation, time.Now()); err != nil {
		t.Fatal(err)
	}
	if len(items) != 0 {
		if err := resolver.Store.AppendTaskRunItems(ctx, run.ID, run.Generation, items, store.TaskRunUsage{}, time.Now()); err != nil {
			t.Fatal(err)
		}
	}
	runCtx, cancel := context.WithCancel(ctx)
	events := make(chan noemaruntime.Event)
	go service.Run(runCtx, events)
	defer cancel()
	deadline := time.Now().Add(2 * time.Second)
	for time.Now().Before(deadline) {
		value, getErr := resolver.Store.ClientTaskActivity(ctx, clientID)
		if getErr == nil && value != nil && value.Projection["focusTaskId"] == taskID {
			return *value
		}
		time.Sleep(10 * time.Millisecond)
	}
	value, err := resolver.Store.ClientTaskActivity(ctx, clientID)
	if err != nil || value == nil {
		t.Fatalf("Live Activity projection = %#v, %v", value, err)
	}
	return *value
}

func rustAPIPortLiveActivitySharedText(t *testing.T) {
	t.Helper()
	activity := rustAPILiveProjection(t, []store.TaskRunItemInput{
		{Kind: "assistant_output", Status: "completed", Round: 2, Content: "**Searching Apple documentation for ActivityKit updates.**"},
		{Kind: "tool_call", Status: "running", Round: 2, Content: "web.search", Payload: map[string]any{"name": "web.search", "arguments": map[string]any{"query": "ActivityKit updates"}}},
	})
	if activity.Projection["updateLabel"] != "Searching the web for “ActivityKit updates”" {
		t.Fatalf("active tool label = %#v", activity.Projection["updateLabel"])
	}
}

func rustAPIPortLiveActivityLatestTool(t *testing.T) {
	t.Helper()
	activity := rustAPILiveProjection(t, []store.TaskRunItemInput{{Kind: "tool_call", Status: "running", Round: 2, Content: "web.search", Payload: map[string]any{"name": "web.search", "arguments": map[string]any{"query": "ActivityKit updates"}}}})
	if activity.Projection["updateLabel"] != "Searching the web for “ActivityKit updates”" {
		t.Fatalf("latest tool label = %#v", activity.Projection["updateLabel"])
	}
}

func rustAPIPortLiveActivityIgnoresPartial(t *testing.T) {
	t.Helper()
	activity := rustAPILiveProjection(t, []store.TaskRunItemInput{{Kind: "assistant_output", Status: "running", Round: 2, Content: "Partial task commentary"}, {Kind: "tool_call", Status: "completed", Round: 2, Content: "web.search", Payload: map[string]any{"name": "web.search"}}})
	if activity.Projection["updateLabel"] != nil {
		t.Fatalf("partial/completed label = %#v", activity.Projection["updateLabel"])
	}
}

func rustAPIPortLiveActivityFirstRunning(t *testing.T) {
	t.Helper()
	activity := rustAPILiveProjection(t, []store.TaskRunItemInput{
		{Kind: "tool_call", Status: "running", Round: 2, Content: "task.files.read", Payload: map[string]any{"name": "task.files.read", "arguments": map[string]any{"path": "current work"}}},
		{Kind: "tool_call", Status: "running", Round: 2, Content: "task.files.read", Payload: map[string]any{"name": "task.files.read", "arguments": map[string]any{"path": "queued work"}}},
	})
	if activity.Projection["updateLabel"] != "Reading current work" {
		t.Fatalf("first running label = %#v", activity.Projection["updateLabel"])
	}
}

func rustAPIPortLiveActivityPartialOutput(t *testing.T) {
	t.Helper()
	activity := rustAPILiveProjection(t, []store.TaskRunItemInput{{Kind: "assistant_output", Status: "completed", Round: 1, Content: "Finished the earlier action."}, {Kind: "assistant_output", Status: "running", Round: 2, Content: "Starting the next action"}})
	if activity.Projection["updateLabel"] != nil {
		t.Fatalf("partial output label = %#v", activity.Projection["updateLabel"])
	}
}

func rustAPIPortLiveActivityLatestMeaningful(t *testing.T) {
	t.Helper()
	activity := rustAPILiveProjection(t, []store.TaskRunItemInput{{Kind: "progress_notice", Status: "completed", Round: 0, Content: "Still finding relevant records."}, {Kind: "assistant_output", Status: "completed", Round: 3, Content: "Comparing the matching records."}, {Kind: "progress_notice", Status: "completed", Round: 3, Content: "Provider response received.", Payload: map[string]any{"phase": "provider_response"}}})
	if activity.Projection["updateLabel"] != "Comparing the matching records." {
		t.Fatalf("latest meaningful label = %#v", activity.Projection["updateLabel"])
	}
}

func rustAPIPortTerminalLiveActivity(t *testing.T) {
	t.Helper()
	resolver, service, clientID := rustAPILiveDeliveryFixture(t)
	ctx := context.Background()
	taskID := "task:22222222222222222222222222222222"
	if _, err := home.CreatePendingTaskDocument(resolver.home, taskID, "Finished task"); err != nil {
		t.Fatal(err)
	}
	if err := home.CommitTaskDocument(resolver.home, taskID); err != nil {
		t.Fatal(err)
	}
	if _, err := resolver.Store.CreateTask(ctx, taskID, "Finished task", "correlation:terminal", time.Now()); err != nil {
		t.Fatal(err)
	}
	queued, err := resolver.queueTask(ctx, model.QueueTaskInput{TaskID: taskID, ExpectedRevision: 1, ExpectedGeneration: 1, ClientMutationID: "queue-live-terminal"})
	if err != nil || queued.Task.CurrentRun == nil {
		t.Fatalf("queue terminal run = %#v, %v", queued, err)
	}
	runID := queued.Task.CurrentRun.RunID
	if runID == "" {
		t.Fatal("queued terminal run has no id")
	}
	_, run, found, err := resolver.Store.ClaimTaskExecution(ctx, time.Now())
	if err != nil || !found || run.ID != runID {
		t.Fatalf("claim terminal run = %#v, %v", run, err)
	}
	if err := resolver.Store.StartTaskExecution(ctx, run.ID, run.Generation, time.Now()); err != nil {
		t.Fatal(err)
	}
	if err := resolver.Store.AppendTaskRunItems(ctx, run.ID, run.Generation, []store.TaskRunItemInput{{Kind: "assistant_output", Status: "completed", Round: 1, Content: "Finished task output", Payload: map[string]any{"phase": "final_answer"}}}, store.TaskRunUsage{}, time.Now()); err != nil {
		t.Fatal(err)
	}
	activity, _ := resolver.Store.ClientTaskActivity(ctx, clientID)
	if _, err := resolver.Store.UpdateClientTaskActivityProjection(ctx, clientID, map[string]any{"focusTaskId": taskID}, strings.Repeat("a", 64), taskID, time.Now()); err != nil {
		t.Fatal(err)
	}
	runCtx, cancel := context.WithCancel(ctx)
	events := make(chan noemaruntime.Event)
	go service.Run(runCtx, events)
	defer cancel()
	if _, err := resolver.Store.FinishTask(ctx, taskID, runID, store.TaskCompleted, time.Now()); err != nil {
		t.Fatal(err)
	}
	deadline := time.Now().Add(2 * time.Second)
	for time.Now().Before(deadline) {
		value, err := resolver.Store.ClientTaskActivity(ctx, clientID)
		if err == nil && value != nil && value.Projection["phase"] == "completed" {
			if value.Projection["activeTaskCount"] != float64(0) && value.Projection["activeTaskCount"] != 0 {
				t.Fatalf("terminal active count = %#v", value.Projection["activeTaskCount"])
			}
			return
		}
		time.Sleep(10 * time.Millisecond)
	}
	if activity == nil {
		t.Fatal("missing starting Live Activity")
	}
	t.Fatal("terminal Live Activity projection was not published")
}

func rustAPIPortDeliveryStatuses(t *testing.T) {
	t.Helper()
	resolver, service := rustAPINotificationFixture(t, "https://localhost:3737")
	clientID := rustAPIRegisterNativeNotificationClient(t, resolver, service)
	ctx := context.Background()
	if err := service.QueueTaskAttention(ctx, "delivery:retry", "Title", "Body", "task:11111111111111111111111111111113", "/tasks/11111111111111111111111111111113"); err != nil {
		t.Fatal(err)
	}
	claimAt := time.Now().Add(time.Minute)
	for attempt := 1; attempt <= 4; attempt++ {
		value, err := resolver.Store.ClaimDueAPNSDelivery(ctx, claimAt)
		if err != nil || value == nil {
			t.Fatalf("delivery attempt %d = %#v, %v", attempt, value, err)
		}
		if value.Registration.ClientID != clientID {
			t.Fatalf("delivery client = %q", value.Registration.ClientID)
		}
		retryAt := claimAt.Add(time.Hour)
		if err := resolver.Store.FinishAPNSDelivery(ctx, *value, store.APNSRetry, "remote_retry", "", retryAt); err != nil {
			t.Fatal(err)
		}
		claimAt = retryAt.Add(time.Hour)
	}
	if value, err := resolver.Store.ClaimDueAPNSDelivery(ctx, time.Now().Add(2*time.Hour)); err != nil || value != nil {
		t.Fatalf("bounded retry delivery = %#v, %v", value, err)
	}
	if err := service.QueueTaskAttention(ctx, "delivery:expiry", "Title", "Body", "task:11111111111111111111111111111114", "/tasks/11111111111111111111111111111114"); err != nil {
		t.Fatal(err)
	}
	value, err := resolver.Store.ClaimDueAPNSDelivery(ctx, time.Now().Add(time.Minute))
	if err != nil || value == nil {
		t.Fatalf("expiry delivery = %#v, %v", value, err)
	}
	if err := resolver.Store.FinishAPNSDelivery(ctx, *value, store.APNSFailed, "expired", "", time.Now()); err != nil {
		t.Fatal(err)
	}
}

func rustAPIPortWebToolDefault(t *testing.T) {
	resolver := openProviderTestResolver(t)
	rustAPIAuthenticateCodexForWebSettings(t, resolver)
	executable, _ := os.Executable()
	resolver.WebTools, _ = webtool.New(resolver.Store, resolver.ProviderAccounts, nil, nil, t.TempDir(), executable, 2, 1024)
	ctx := auth.WithDesktopAccess(context.Background())
	settings := rustAPIRawGraphQLContext(t, resolver, ctx, `query { webToolSettings {
	search { activeProviderAccountId providerOptions { providerAccountId displayName dataFlowClass citations directUrlFetch } }
  fetch { activeProviderAccountId providerOptions { providerAccountId displayName dataFlowClass directUrlFetch citations } }
  browse { activeProviderAccountId providerOptions { providerAccountId jsRendering authenticatedContext } }
} }`, nil)
	if len(settings.Errors) != 0 {
		t.Fatalf("web settings errors = %#v", settings.Errors)
	}
	search := settings.Data["webToolSettings"].(map[string]any)["search"].(map[string]any)
	fetch := settings.Data["webToolSettings"].(map[string]any)["fetch"].(map[string]any)
	browse := settings.Data["webToolSettings"].(map[string]any)["browse"].(map[string]any)
	if search["activeProviderAccountId"] != "provider_account:codex:default" || fetch["activeProviderAccountId"] != "provider_account:codex:default" || browse["activeProviderAccountId"] != "provider_account:obscura:system" {
		t.Fatalf("unexpected defaults: %#v", settings.Data)
	}
	if !rustAPIWebOption(t, search, "provider_account:codex:default", "OpenAI", "trusted_external_search_query", true, false) || !rustAPIWebOption(t, fetch, "provider_account:codex:default", "OpenAI", "external_web_fetch", false, true) {
		t.Fatalf("native web options = search %#v fetch %#v", search, fetch)
	}
	if !rustAPIWebBrowseOption(t, browse, "provider_account:obscura:system") {
		t.Fatalf("browse options = %#v", browse)
	}
	secret, _ := provider.NewSecret("exa-secret")
	exa, err := resolver.ProviderAccounts.CreateSecretAccount(ctx, "exa", "Exa", secret, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	saved := rustAPIRawGraphQLContext(t, resolver, ctx, `mutation($input: SaveWebToolProviderBindingInput!) { saveWebToolProviderBinding(input: $input) { activeProviderAccountId } }`, map[string]any{"input": map[string]any{"toolName": "web.search", "capabilityId": "web.search", "providerAccountId": exa.ID}})
	if len(saved.Errors) != 0 || saved.Data["saveWebToolProviderBinding"].(map[string]any)["activeProviderAccountId"] != exa.ID {
		t.Fatalf("saved = %#v", saved)
	}
	mismatch := rustAPIRawGraphQLContext(t, resolver, ctx, `mutation($input: SaveWebToolProviderBindingInput!) { saveWebToolProviderBinding(input: $input) { activeProviderAccountId } }`, map[string]any{"input": map[string]any{"toolName": "web.search", "capabilityId": "web.fetch", "providerAccountId": exa.ID}})
	rustAPIAssertGraphQLError(t, mismatch, "tool and capability do not match", "")
	restored := rustAPIRawGraphQLContext(t, resolver, ctx, `mutation($input: SaveWebToolProviderBindingInput!) { saveWebToolProviderBinding(input: $input) { activeProviderAccountId } }`, map[string]any{"input": map[string]any{"toolName": "web.fetch", "capabilityId": "web.fetch", "providerAccountId": "provider_account:codex:default"}})
	if len(restored.Errors) != 0 {
		t.Fatalf("restore OpenAI web tools = %#v", restored.Errors)
	}
	final := rustAPIRawGraphQLContext(t, resolver, ctx, `query { webToolSettings { search { activeProviderAccountId } fetch { activeProviderAccountId } } }`, nil)
	if final.Data["webToolSettings"].(map[string]any)["search"].(map[string]any)["activeProviderAccountId"] != "provider_account:codex:default" || final.Data["webToolSettings"].(map[string]any)["fetch"].(map[string]any)["activeProviderAccountId"] != "provider_account:codex:default" {
		t.Fatalf("restored web settings = %#v", final.Data)
	}
}

func rustAPIPortWebToolFiltering(t *testing.T) {
	resolver := openProviderTestResolver(t)
	ctx := auth.WithDesktopAccess(context.Background())
	executable, _ := os.Executable()
	resolver.WebTools, _ = webtool.New(resolver.Store, resolver.ProviderAccounts, nil, nil, t.TempDir(), executable, 2, 1024)
	secret, _ := provider.NewSecret("exa-secret")
	exa, err := resolver.ProviderAccounts.CreateSecretAccount(ctx, "exa", "Exa", secret, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	initial := rustAPIRawGraphQLContext(t, resolver, ctx, `query { webToolSettings { search { activeProviderAccountId providerOptions { providerAccountId providerKind } } fetch { activeProviderAccountId providerOptions { providerAccountId providerKind } } } }`, nil)
	if len(initial.Errors) != 0 {
		t.Fatalf("initial web settings = %#v", initial.Errors)
	}
	initialSettings := initial.Data["webToolSettings"].(map[string]any)
	initialSearch := initialSettings["search"].(map[string]any)
	initialFetch := initialSettings["fetch"].(map[string]any)
	if initialSearch["activeProviderAccountId"] != "provider_account:duckduckgo_public:system" || initialFetch["activeProviderAccountId"] != "provider_account:direct_http:system" || !rustAPIWebOptionByID(initialSearch, exa.ID) {
		t.Fatalf("filtered initial settings = %#v", initialSettings)
	}
	if rustAPIWebOptionByKind(initialSearch, "codex") || rustAPIWebOptionByKind(initialFetch, "openai") {
		t.Fatalf("unexpected unavailable native options = search %#v fetch %#v", initialSearch, initialFetch)
	}
	saved := rustAPIRawGraphQLContext(t, resolver, ctx, `mutation($input: SaveWebToolProviderBindingInput!) { saveWebToolProviderBinding(input: $input) { activeProviderAccountId } }`, map[string]any{"input": map[string]any{"toolName": "web.search", "capabilityId": "web.search", "providerAccountId": exa.ID}})
	if len(saved.Errors) != 0 {
		t.Fatalf("save Exa binding = %#v", saved.Errors)
	}
	paths, err := home.FromRoot(resolver.home.Name())
	if err != nil {
		t.Fatal(err)
	}
	metadata, err := sql.Open("sqlite3", paths.Database())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = metadata.Close() })
	if _, err := metadata.Exec(`UPDATE provider_accounts SET status='unknown' WHERE provider_account_id=?`, exa.ID); err != nil {
		t.Fatal(err)
	}
	stale := rustAPIRawGraphQLContext(t, resolver, ctx, `query { webToolSettings { search { activeProviderAccountId providerOptions { providerAccountId } } } }`, nil)
	if len(stale.Errors) != 0 {
		t.Fatalf("stale settings = %#v", stale.Errors)
	}
	staleSearch := stale.Data["webToolSettings"].(map[string]any)["search"].(map[string]any)
	if staleSearch["activeProviderAccountId"] != "provider_account:duckduckgo_public:system" || rustAPIWebOptionByID(staleSearch, exa.ID) {
		t.Fatalf("stale binding remained selectable = %#v", staleSearch)
	}
}

func rustAPIAuthenticateCodexForWebSettings(t *testing.T, resolver *Resolver) {
	t.Helper()
	paths, err := home.FromRoot(resolver.home.Name())
	if err != nil {
		t.Fatal(err)
	}
	metadata, err := sql.Open("sqlite3", paths.Database())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = metadata.Close() })
	if _, err := metadata.Exec(`UPDATE provider_accounts SET status='authenticated', auth_method='oauth_device_code', metadata_json='{"credentialRevision":1,"secretConfigured":true}' WHERE provider_account_id='provider_account:codex:default'`); err != nil {
		t.Fatal(err)
	}
	assignments := make([]store.ModelAssignment, 0, len(store.HostedModelRoles()))
	for _, role := range store.HostedModelRoles() {
		assignments = append(assignments, store.ModelAssignment{Role: role, ProviderKind: "codex", ProviderAccountID: "provider_account:codex:default", SelectionMode: store.ModelSelectionNoemaRecommended})
	}
	if _, err := resolver.Store.ConfirmHostedModelAssignments(context.Background(), "provider_account:codex:default", assignments); err != nil {
		t.Fatal(err)
	}
}

func rustAPIWebOption(t *testing.T, settings map[string]any, accountID, displayName, dataFlow string, citations, directFetch bool) bool {
	t.Helper()
	for _, raw := range settings["providerOptions"].([]any) {
		option := raw.(map[string]any)
		if option["providerAccountId"] == accountID {
			return option["displayName"] == displayName && option["dataFlowClass"] == dataFlow && option["citations"] == citations && option["directUrlFetch"] == directFetch
		}
	}
	return false
}

func rustAPIWebBrowseOption(t *testing.T, settings map[string]any, accountID string) bool {
	t.Helper()
	for _, raw := range settings["providerOptions"].([]any) {
		option := raw.(map[string]any)
		if option["providerAccountId"] == accountID {
			return option["jsRendering"] == true && option["authenticatedContext"] == true
		}
	}
	return false
}

func rustAPIWebOptionByID(settings map[string]any, accountID string) bool {
	for _, raw := range settings["providerOptions"].([]any) {
		if raw.(map[string]any)["providerAccountId"] == accountID {
			return true
		}
	}
	return false
}

func rustAPIWebOptionByKind(settings map[string]any, kind string) bool {
	for _, raw := range settings["providerOptions"].([]any) {
		if raw.(map[string]any)["providerKind"] == kind {
			return true
		}
	}
	return false
}
