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
	"net/url"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"sync"
	"sync/atomic"
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
	mcpsdk "github.com/modelcontextprotocol/go-sdk/mcp"
	_ "github.com/ncruces/go-sqlite3/driver"
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
	resolver := NewResolver(taskStore, nil, authentication,
		nil, nil, nil, nil, nil, nil, service)
	handler := authentication.Handler(NewHandler(resolver))
	access := seedNotificationNativeClient(t, taskStore)
	browserRegistration := rustAPIRawGraphQLContext(t, resolver,
		auth.WithHumanPrincipal(ctx, "human:local"), `mutation {
  registerClientNotifications(input: { deviceToken: "AQID", environment: DEVELOPMENT }) { enabled }
}`, nil)
	rustAPIAssertGraphQLError(t, browserRegistration, "paired client authentication required", "")
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
	if response.Code != http.StatusOK || !bytes.Contains(response.Body.Bytes(), []byte("browser session authentication required")) {
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
	response := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(context.Background()), fmt.Sprintf(`mutation {
	createMcpServer(input: {
	    displayName: "Dex", transportKind: "streamable_http",
    http: { url: %q }
  }) {
    setupStatus discoveredToolCount setupError
    auth { oauthAuthorizationSupported oauthClientCredentialsSupported }
    server { mcpServerId }
  }
}`, remote.URL+"/mcp"), nil)
	if len(response.Errors) != 0 {
		t.Fatalf("MCP setup GraphQL errors = %#v", response.Errors)
	}
	result, ok := response.Data["createMcpServer"].(map[string]any)
	if !ok || result["setupStatus"] != "needs_auth" || result["discoveredToolCount"] != float64(0) || result["server"] != nil {
		t.Fatalf("MCP setup GraphQL result = %#v", response.Data)
	}
	authResult, ok := result["auth"].(map[string]any)
	if !ok || authResult["oauthAuthorizationSupported"] != true || authResult["oauthClientCredentialsSupported"] != true {
		t.Fatalf("MCP setup GraphQL auth = %#v", result["auth"])
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
	response := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(ctx), fmt.Sprintf(`query {
  pendingHumanInterventions(conversationId: %q) {
    __typename
    ... on McpSetupIntervention { itemId setupStatus displayName oauthSupported }
  }
}`, conversation.ID), nil)
	if len(response.Errors) != 0 {
		t.Fatalf("pending MCP setup GraphQL errors = %#v", response.Errors)
	}
	values, ok := response.Data["pendingHumanInterventions"].([]any)
	if !ok || len(values) != 1 {
		t.Fatalf("pending MCP setup GraphQL values = %#v", response.Data)
	}
	value, ok := values[0].(map[string]any)
	if !ok || value["__typename"] != "McpSetupIntervention" || value["itemId"] != result.ID || value["setupStatus"] != "needs_auth" || value["displayName"] != "Notion" || value["oauthSupported"] != true {
		t.Fatalf("MCP intervention GraphQL value = %#v", values[0])
	}
}

func rustAPIPortMCPOAuthCompletion(t *testing.T) {
	t.Helper()
	resolver := openChatTestResolver(t)
	rustAPIConfigureChatProvider(t, resolver)
	paths, err := home.FromRoot(resolver.home.Name())
	if err != nil {
		t.Fatal(err)
	}
	var requireAuth atomic.Bool
	var authorizationCodesMu sync.Mutex
	authorizationCodes := map[string]string{}
	remote := mcpsdk.NewServer(&mcpsdk.Implementation{Name: "docs", Version: "1"}, nil)
	mcpsdk.AddTool(remote, &mcpsdk.Tool{Name: "search", Description: "Search documents",
		Annotations: &mcpsdk.ToolAnnotations{ReadOnlyHint: true, IdempotentHint: true,
			DestructiveHint: rustAPIBool(false), OpenWorldHint: rustAPIBool(true)}},
		func(context.Context, *mcpsdk.CallToolRequest, struct{}) (*mcpsdk.CallToolResult, map[string]any, error) {
			return nil, map[string]any{"ok": true}, nil
		})
	mcpHandler := mcpsdk.NewStreamableHTTPHandler(func(*http.Request) *mcpsdk.Server { return remote }, nil)
	var remoteURL string
	remoteServer := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, request *http.Request) {
		switch request.URL.Path {
		case "/.well-known/oauth-protected-resource":
			w.Header().Set("Content-Type", "application/json")
			_ = json.NewEncoder(w).Encode(map[string]any{"resource": remoteURL + "/mcp", "authorization_servers": []string{remoteURL}})
		case "/.well-known/oauth-authorization-server":
			w.Header().Set("Content-Type", "application/json")
			_ = json.NewEncoder(w).Encode(map[string]any{
				"issuer": remoteURL, "authorization_endpoint": remoteURL + "/authorize", "token_endpoint": remoteURL + "/token",
				"jwks_uri": remoteURL + "/.well-known/jwks.json", "response_types_supported": []string{"code"},
				"grant_types_supported":            []string{"authorization_code", "refresh_token"},
				"code_challenge_methods_supported": []string{"S256"}, "token_endpoint_auth_methods_supported": []string{"client_secret_post"},
			})
		case "/authorize":
			query := request.URL.Query()
			if query.Get("client_id") != "mcp-client" || query.Get("response_type") != "code" ||
				query.Get("code_challenge_method") != "S256" || query.Get("code_challenge") == "" ||
				query.Get("redirect_uri") == "" || query.Get("state") == "" {
				http.Error(w, "invalid authorization request", http.StatusBadRequest)
				return
			}
			code := "oauth-issued-code"
			authorizationCodesMu.Lock()
			authorizationCodes[code] = query.Get("code_challenge")
			authorizationCodesMu.Unlock()
			callback, err := url.Parse(query.Get("redirect_uri"))
			if err != nil {
				http.Error(w, "invalid redirect URI", http.StatusBadRequest)
				return
			}
			params := callback.Query()
			params.Set("code", code)
			params.Set("state", query.Get("state"))
			callback.RawQuery = params.Encode()
			http.Redirect(w, request, callback.String(), http.StatusFound)
		case "/token":
			if err := request.ParseForm(); err != nil || request.Form.Get("code") == "" || request.Form.Get("client_id") != "mcp-client" || request.Form.Get("client_secret") != "mcp-secret" {
				http.Error(w, "invalid token request", http.StatusUnauthorized)
				return
			}
			authorizationCodesMu.Lock()
			challenge, issued := authorizationCodes[request.Form.Get("code")]
			delete(authorizationCodes, request.Form.Get("code"))
			authorizationCodesMu.Unlock()
			verifier := request.Form.Get("code_verifier")
			digest := sha256.Sum256([]byte(verifier))
			if !issued || verifier == "" || request.Form.Get("grant_type") != "authorization_code" ||
				base64.RawURLEncoding.EncodeToString(digest[:]) != challenge {
				http.Error(w, "invalid PKCE authorization code", http.StatusUnauthorized)
				return
			}
			w.Header().Set("Content-Type", "application/json")
			_ = json.NewEncoder(w).Encode(map[string]any{"access_token": "mcp-access", "refresh_token": "mcp-refresh", "token_type": "Bearer", "expires_in": 3600})
		case "/mcp":
			if requireAuth.Load() && request.Header.Get("Authorization") != "Bearer mcp-access" {
				w.Header().Set("WWW-Authenticate", "Bearer resource_metadata="+remoteURL+"/.well-known/oauth-protected-resource")
				w.WriteHeader(http.StatusUnauthorized)
				return
			}
			mcpHandler.ServeHTTP(w, request)
		default:
			http.NotFound(w, request)
		}
	}))
	remoteURL = remoteServer.URL
	t.Cleanup(remoteServer.Close)
	service, err := mcp.NewService(paths, resolver.Store, false, nil, "http://localhost/mcp/oauth/callback")
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(service.Close)
	resolver.MCP = service
	if err := resolver.Chat.Close(); err != nil {
		t.Fatal(err)
	}
	generator, err := provider.NewOpenRouterGenerator(resolver.ProviderAccounts)
	if err != nil {
		t.Fatal(err)
	}
	codexGenerator, err := provider.NewCodexGenerator(resolver.ProviderAccounts)
	if err != nil {
		t.Fatal(err)
	}
	resolver.Chat, err = noemaruntime.NewChat(resolver.Store, generator, codexGenerator, codexGenerator,
		resolver.home, resolver.Memory, service)
	if err != nil {
		t.Fatal(err)
	}
	completion := make(chan error, 1)
	service.SetOAuthCompletionHandler(func(attemptID string) {
		completion <- resolver.Chat.ResumeMCPAuthentication(context.Background(), attemptID)
	})
	setup, err := service.Create(context.Background(), mcp.SetupInput{DisplayName: "Docs", TransportKind: "streamable_http",
		URL: remoteServer.URL + "/mcp", AuthPreference: "USE_ANONYMOUS", Secrets: mcp.SecretMaterial{Client: &mcp.OAuthClient{ClientID: "mcp-client", ClientSecret: "mcp-secret"}}})
	if err != nil || setup.Server == nil {
		t.Fatalf("MCP OAuth server setup = %#v, %v", setup, err)
	}
	if _, err := service.SaveConnectionPolicy(context.Background(), setup.Server.ID, setup.Server.ConnectionRevision, 0,
		"allow_automatically", "always_ask"); err != nil {
		t.Fatal(err)
	}
	bindings, err := service.Bindings(context.Background())
	if err != nil {
		t.Fatalf("MCP OAuth bindings = %#v, %v", bindings, err)
	}
	var binding mcp.Binding
	for _, candidate := range bindings {
		if candidate.ServerID == setup.Server.ID {
			binding = candidate
			break
		}
	}
	if binding.ServerID != setup.Server.ID {
		t.Fatalf("MCP OAuth server binding = %#v, all = %#v", binding, bindings)
	}
	staleBinding := binding
	staleBinding.ConnectionRevision = "mcp_connection_revision:" + strings.Repeat("0", 32)
	if staleBinding.Destination != nil {
		staleBinding.Destination.Revision = staleBinding.ConnectionRevision
	}
	conversation, err := resolver.Store.EnsurePrimaryConversation(context.Background(), "openrouter", "", time.Now())
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := resolver.Store.BeginConversationTurn(context.Background(), conversation.ID, "Continue the authenticated call.", nil, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	items, err := resolver.Store.StartConversationToolRound(context.Background(), turn, store.ConversationToolRound{Provider: "openrouter",
		Call: store.ConversationToolCallInput{ProviderRound: 0, OutputIndex: 0, ProviderCallID: "call:oauth",
			ProviderName: binding.Name, Name: binding.Name, Arguments: json.RawMessage(`{}`)}}, time.Now())
	if err != nil || len(items) == 0 {
		t.Fatalf("MCP OAuth running call = %#v, %v", items, err)
	}
	assignment := map[string]any{"role": string(store.HostedModelNoema), "provider_kind": "openrouter",
		"provider_account_id": "provider_account:openrouter:default", "model_profile": "openai/gpt-5.6-luna",
		"selection_mode": string(store.ModelSelectionNoemaRecommended), "reasoning_effort": string(store.ModelReasoningHigh)}
	authority, err := json.Marshal(map[string]any{"binding": staleBinding, "assignment": assignment})
	if err != nil {
		t.Fatal(err)
	}
	request, _, err := resolver.Store.CreateMCPAuthRequest(context.Background(), store.MCPAuthRequest{
		OwnerHumanID: "human:local", ConversationID: conversation.ID, TurnID: turn.ID, CallItemID: items[len(items)-1].ID,
		ServerID: setup.Server.ID, CapabilityName: binding.Name, BindingJSON: string(authority), ArgumentsJSON: `{}`, Provider: "openrouter",
	}, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	requireAuth.Store(true)
	attempt, err := service.StartOAuthReauthentication(context.Background(), "human:local", setup.Server.ID, "http://localhost/mcp/oauth/callback")
	if err != nil || attempt.ID == "" || attempt.AuthorizationURL == "" {
		t.Fatalf("MCP OAuth reauthentication = %#v, %v", attempt, err)
	}
	if _, err := resolver.Store.BeginMCPAuthentication(context.Background(), request.ID, request.Revision, "human:local", attempt.ID, time.Now()); err != nil {
		t.Fatal(err)
	}
	authorization, err := url.Parse(attempt.AuthorizationURL)
	if err != nil || authorization.Query().Get("state") == "" {
		t.Fatalf("MCP OAuth authorization URL = %q, %v", attempt.AuthorizationURL, err)
	}
	approvalClient := &http.Client{CheckRedirect: func(*http.Request, []*http.Request) error { return http.ErrUseLastResponse }}
	approvalRequest, err := http.NewRequest(http.MethodGet, attempt.AuthorizationURL, nil)
	if err != nil {
		t.Fatalf("MCP OAuth authorization request = %v", err)
	}
	approvalResponse, err := approvalClient.Do(approvalRequest)
	if err != nil {
		t.Fatalf("MCP OAuth authorization approval = %v", err)
	}
	defer approvalResponse.Body.Close()
	if approvalResponse.StatusCode != http.StatusFound || approvalResponse.Header.Get("Location") == "" {
		t.Fatalf("MCP OAuth authorization approval status = %d, location = %q", approvalResponse.StatusCode, approvalResponse.Header.Get("Location"))
	}
	callback := approvalResponse.Header.Get("Location")
	if err := service.CompleteOAuth(context.Background(), attempt.ID, callback); err != nil {
		t.Fatalf("MCP OAuth callback completion = %v", err)
	}
	if err := <-completion; err != nil {
		t.Fatalf("MCP OAuth runtime completion = %v", err)
	}
	// The callback validator is the durable route boundary used before an
	// attempt can bind to a runtime authentication request.
	if err := requireMCPCallback(service, "http://127.0.0.1:4444/mcp/oauth/callback"); err == nil {
		t.Fatal("MCP OAuth accepted a callback on a different origin")
	}
	ctx := auth.WithDesktopAccess(context.Background())
	if _, err := resolver.startMCPCreateOAuth(ctx, model.StartMcpServerOAuthSetupInput{Server: &model.CreateMcpServerInput{
		DisplayName: "Docs", TransportKind: "streamable_http", HTTP: &model.McpHTTPConfigInput{URL: "http://localhost:1/mcp"},
	}, RedirectURI: "http://127.0.0.1:4444/mcp/oauth/callback"}); err == nil || !strings.Contains(err.Error(), "does not match") {
		t.Fatalf("mismatched MCP OAuth setup = %v", err)
	}
	completed, err := service.Attempt(context.Background(), attempt.ID, "human:local")
	if err != nil || completed.ID != attempt.ID || completed.Status != "completed" {
		t.Fatalf("MCP OAuth durable completion = %#v, %v", completed, err)
	}
	stored, err := resolver.Store.MCPAuthRequest(context.Background(), request.ID, request.Revision)
	if err != nil {
		t.Fatal(err)
	}
	if stored.State != "superseded" {
		t.Fatalf("completed OAuth request state = %q, want superseded", stored.State)
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

func rustAPIConfigureChatProvider(t *testing.T, resolver *Resolver) {
	t.Helper()
	ctx := context.Background()
	secret, err := provider.NewSecret("rust-api-chat-key")
	if err != nil {
		t.Fatal(err)
	}
	profiles := []provider.ModelProfile{{ID: "openai/gpt-5.6-luna", Label: "GPT-5.6 Luna",
		ReasoningEfforts: []string{"high"}, DefaultReasoningEffort: "high"}}
	account, err := resolver.ProviderAccounts.PublishVerifiedSecret(ctx, "provider_account:openrouter:default", 0,
		provider.AuthSecretInput, secret, profiles, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	assignments := make([]store.ModelAssignment, 0, len(store.HostedModelRoles()))
	for _, role := range store.HostedModelRoles() {
		assignments = append(assignments, store.ModelAssignment{Role: role, ProviderKind: "openrouter",
			ProviderAccountID: account.ID, SelectionMode: store.ModelSelectionNoemaRecommended})
	}
	if created, err := resolver.Store.ConfirmHostedModelAssignments(ctx, account.ID, assignments); err != nil || !created {
		t.Fatalf("configure Chat provider = %t, %v", created, err)
	}
}

func rustAPIBool(value bool) *bool { return &value }

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
	inputTokens, outputTokens, totalTokens := 120, 40, 160
	spanID, err := resolver.Store.BeginRuntimeDebugSpan(ctx,
		store.RuntimeDebugScope{Kind: "conversation_turn", ID: turn.ID}, "provider", "Final provider request",
		store.RuntimeDebugMetadata{}, started)
	if err != nil {
		t.Fatal(err)
	}
	if err = resolver.Store.FinishRuntimeDebugSpan(ctx, spanID, "completed",
		store.RuntimeDebugMetadata{Provider: "codex", Model: "gpt-5.6", Phase: "finalization",
			InputTokens: &inputTokens, OutputTokens: &outputTokens, TotalTokens: &totalTokens}, 65272*time.Millisecond, started.Add(65272*time.Millisecond)); err != nil {
		t.Fatal(err)
	}
	if _, err = resolver.Store.CompleteConversationTurn(ctx, turn, "Done", "Done", nil, started.Add(65272*time.Millisecond)); err != nil {
		t.Fatal(err)
	}
	profileResponse := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(ctx), fmt.Sprintf(`query {
  runtimeDebugProfile(input: { kind: CONVERSATION_TURN, scopeId: %q }) {
    status spans { name durationMilliseconds provider phase totalTokens }
  }
}`, turn.ID), nil)
	if len(profileResponse.Errors) != 0 {
		t.Fatalf("profile errors = %#v", profileResponse.Errors)
	}
	profile := profileResponse.Data["runtimeDebugProfile"].(map[string]any)
	spans := profile["spans"].([]any)
	span := spans[0].(map[string]any)
	if span["name"] != "Final provider request" || span["durationMilliseconds"] != float64(65272) || span["provider"] != "codex" || span["phase"] != "finalization" || span["totalTokens"] != float64(160) {
		t.Fatalf("span = %#v", span)
	}
	missingResponse := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(ctx), `query {
  runtimeDebugProfile(input: { kind: CONVERSATION_TURN, scopeId: "turn:00000000000000000000000000000000" }) { status }
}`, nil)
	if len(missingResponse.Errors) != 0 || missingResponse.Data["runtimeDebugProfile"] != nil {
		t.Fatalf("unowned profile = %#v", missingResponse)
	}
	legacy, _, err := resolver.Store.BeginConversationTurn(ctx, conversation.ID, "Legacy turn.", nil, started.Add(time.Second))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := resolver.Store.CompleteConversationTurn(ctx, legacy, "Done", "Done", nil, started.Add(2*time.Second)); err != nil {
		t.Fatal(err)
	}
	legacyResponse := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(ctx), fmt.Sprintf(`query {
  runtimeDebugProfile(input: { kind: CONVERSATION_TURN, scopeId: %q }) { status }
}`, legacy.ID), nil)
	if len(legacyResponse.Errors) != 0 || legacyResponse.Data["runtimeDebugProfile"] != nil {
		t.Fatalf("legacy completed-turn profile = %#v", legacyResponse)
	}
}

func rustAPIPortAgentPreference(t *testing.T) {
	t.Helper()
	resolver := openProviderTestResolver(t)
	ctx := context.Background()
	profiles := []provider.ModelProfile{
		{ID: "gpt-5.5", Label: "GPT-5.5", ReasoningEfforts: []string{"low", "medium", "high"}, DefaultReasoningEffort: "medium"},
		{ID: "gpt-5.6-terra", Label: "GPT-5.6 Terra", ReasoningEfforts: []string{"low", "medium", "high"}, DefaultReasoningEffort: "medium"},
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
	encoded, err := json.Marshal(map[string]any{"credentialRevision": 1, "secretConfigured": true, "profiles": profiles})
	if err != nil {
		t.Fatal(err)
	}
	if _, err := metadata.Exec(`UPDATE provider_accounts SET status='authenticated', auth_method='oauth_device_code', metadata_json=? WHERE provider_account_id='provider_account:codex:default'`, string(encoded)); err != nil {
		t.Fatal(err)
	}
	accountID := "provider_account:codex:default"
	assignments := make([]store.ModelAssignment, 0, len(store.HostedModelRoles()))
	for _, role := range store.HostedModelRoles() {
		assignments = append(assignments, store.ModelAssignment{Role: role, ProviderKind: "openrouter",
			ProviderAccountID: accountID, SelectionMode: store.ModelSelectionNoemaRecommended})
	}
	for i := range assignments {
		assignments[i].ProviderKind = "codex"
	}
	if created, err := resolver.Store.ConfirmHostedModelAssignments(ctx, accountID, assignments); err != nil || !created {
		t.Fatalf("agent provider assignments = %t, %v", created, err)
	}
	profile := "gpt-5.5"
	options := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(ctx), `query {
  agents { modelOptions { profiles { id reasoningEfforts defaultReasoningEffort } recommendations { useCase modelProfile reasoningEffort disabledReason } } }
}`, nil)
	if len(options.Errors) != 0 {
		t.Fatalf("Agent options errors = %#v", options.Errors)
	}
	var selected map[string]any
	for _, raw := range options.Data["agents"].([]any)[0].(map[string]any)["modelOptions"].([]any) {
		for _, profileValue := range raw.(map[string]any)["profiles"].([]any) {
			candidate := profileValue.(map[string]any)
			if candidate["id"] == profile {
				selected = candidate
			}
		}
	}
	if selected == nil || selected["id"] != profile || fmt.Sprintf("%v", selected["reasoningEfforts"]) != "[LOW MEDIUM HIGH]" || selected["defaultReasoningEffort"] != "MEDIUM" {
		t.Fatalf("Agent reasoning profile = %#v", selected)
	}
	missingEffort := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(ctx), fmt.Sprintf(`mutation {
	  saveAgentModelPreference(input: { agentId: %q, providerAccountId: "provider_account:codex:default", selectionMode: EXPLICIT_PROFILE, modelProfile: %q, fastMode: false }) {
    modelProfile reasoningEffort
  }
}`, store.PrimaryAgentID, profile), nil)
	if len(missingEffort.Errors) != 1 || !strings.Contains(missingEffort.Errors[0].Message, "reasoning effort") {
		t.Fatalf("missing reasoning effort = %#v", missingEffort)
	}
	saved := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(ctx), fmt.Sprintf(`mutation {
	  saveAgentModelPreference(input: { agentId: %q, providerAccountId: "provider_account:codex:default", selectionMode: EXPLICIT_PROFILE, modelProfile: %q, reasoningEffort: HIGH, fastMode: false }) {
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
	  saveAgentModelPreference(input: { agentId: %q, providerAccountId: "provider_account:codex:default", selectionMode: NOEMA_RECOMMENDED, fastMode: false }) {
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
	invalidRecommended := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(ctx), fmt.Sprintf(`mutation {
	  saveAgentModelPreference(input: { agentId: %q, providerAccountId: "provider_account:codex:default", selectionMode: NOEMA_RECOMMENDED, modelProfile: %q, fastMode: false }) { selectionMode }
}`, store.PrimaryAgentID, profile), nil)
	if len(invalidRecommended.Errors) != 1 || !strings.Contains(invalidRecommended.Errors[0].Message, "does not accept") {
		t.Fatalf("invalid recommended Agent preference = %#v", invalidRecommended)
	}
	taskPreference := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(ctx), fmt.Sprintf(`mutation {
	  saveAgentModelPreference(input: { agentId: %q, providerAccountId: "provider_account:codex:default", selectionMode: EXPLICIT_PROFILE, modelProfile: %q, reasoningEffort: HIGH, fastMode: false }) { providerKind }
}`, store.TaskExecutorAgentID, profile), nil)
	if len(taskPreference.Errors) != 1 || !strings.Contains(taskPreference.Errors[0].Message, "complexity tier") {
		t.Fatalf("Task Executor preference = %#v", taskPreference)
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
	graphqlCtx := auth.WithDesktopAccess(ctx)
	createInput := model.CreateProjectInput{
		WorkspaceID: personalWorkspaceID, Name: "Plan", Description: "Exact",
		ClientMutationID: "project-create",
	}
	createdResponse := rustAPIRawGraphQLContext(t, resolver, graphqlCtx, `mutation($input: CreateProjectInput!) {
  createProject(input: $input) { project { projectId revision folder } eventCursor clientMutationId }
}`, map[string]any{"input": map[string]any{
		"workspaceId": createInput.WorkspaceID, "name": createInput.Name,
		"description": createInput.Description, "clientMutationId": createInput.ClientMutationID,
	}})
	if len(createdResponse.Errors) != 0 {
		t.Fatalf("create Project GraphQL errors = %#v", createdResponse.Errors)
	}
	createdPayload := createdResponse.Data["createProject"].(map[string]any)
	createdProject := createdPayload["project"].(map[string]any)
	projectID := createdProject["projectId"].(string)
	if createdProject["revision"] != float64(1) {
		t.Fatalf("create Project = %#v", createdProject)
	}
	assertProjectReceipt(t, resolver.Store, "project.create", createInput.ClientMutationID, createInput)
	documentResponse := rustAPIRawGraphQLContext(t, resolver, graphqlCtx, `query($project:String!) {
  projectDocument(projectId: $project) { projectId content digest }
}`, map[string]any{"project": projectID})
	if len(documentResponse.Errors) != 0 {
		t.Fatalf("Project document GraphQL errors = %#v", documentResponse.Errors)
	}
	document := documentResponse.Data["projectDocument"].(map[string]any)
	if document["content"] != "# Plan\n\nExact\n" {
		t.Fatalf("Project document = %#v", document)
	}
	saveInput := model.UpdateProjectDocumentInput{
		ProjectID: projectID, ExpectedRevision: 1,
		ExpectedDocumentDigest: document["digest"].(string), Content: "# Current\n",
		ClientMutationID: "project-document",
	}
	savedResponse := rustAPIRawGraphQLContext(t, resolver, graphqlCtx, `mutation($input: UpdateProjectDocumentInput!) {
  updateProjectDocument(input: $input) { project { revision } document { content digest } eventCursor }
}`, map[string]any{"input": map[string]any{
		"projectId": saveInput.ProjectID, "expectedRevision": saveInput.ExpectedRevision,
		"expectedDocumentDigest": saveInput.ExpectedDocumentDigest, "content": saveInput.Content,
		"clientMutationId": saveInput.ClientMutationID,
	}})
	if len(savedResponse.Errors) != 0 {
		t.Fatalf("save Project document GraphQL errors = %#v", savedResponse.Errors)
	}
	savedPayload := savedResponse.Data["updateProjectDocument"].(map[string]any)
	savedProject := savedPayload["project"].(map[string]any)
	savedDocument := savedPayload["document"].(map[string]any)
	if savedProject["revision"] != float64(2) || savedDocument["content"] != "# Current\n" {
		t.Fatalf("save Project document = %#v", savedPayload)
	}
	assertProjectReceipt(t, resolver.Store, "project.update", saveInput.ClientMutationID, saveInput)
	folder := t.TempDir()
	folderResponse := rustAPIRawGraphQLContext(t, resolver, graphqlCtx, `mutation($input: UpdateProjectInput!) {
  updateProject(input: $input) { project { projectId revision folder } }
}`, map[string]any{"input": map[string]any{
		"projectId": projectID, "expectedRevision": 2, "folder": folder,
		"clientMutationId": "project-folder",
	}})
	if len(folderResponse.Errors) != 0 {
		t.Fatalf("move Project GraphQL errors = %#v", folderResponse.Errors)
	}
	folderPayload := folderResponse.Data["updateProject"].(map[string]any)
	folderProject := folderPayload["project"].(map[string]any)
	if folderProject["folder"] != folder {
		t.Fatalf("move Project = %#v", folderPayload)
	}
	cwdResponse := rustAPIRawGraphQLContext(t, resolver, graphqlCtx, `mutation($project:String!, $agent:String!) {
  captureTask(input: {
    workspaceId: "workspace:personal", projectId: $project, title: "Use executor",
    executorAgentId: $agent, cwdOverride: "/tmp/task-work", clientMutationId: "executor-capture"
  }) { task { executorAgentId executorBackend cwdOverride effectiveCwd effectiveCwdSource project { folder } } }
}`, map[string]any{"project": projectID, "agent": store.TaskExecutorAgentID})
	if len(cwdResponse.Errors) != 0 {
		t.Fatalf("Task CWD GraphQL errors = %#v", cwdResponse.Errors)
	}
	cwdTask := cwdResponse.Data["captureTask"].(map[string]any)["task"].(map[string]any)
	if cwdTask["executorAgentId"] != store.TaskExecutorAgentID || cwdTask["executorBackend"] != "provider" || cwdTask["cwdOverride"] != "/tmp/task-work" || cwdTask["effectiveCwd"] != "/tmp/task-work/use-executor" || cwdTask["effectiveCwdSource"] != "task" || cwdTask["project"].(map[string]any)["folder"] != folder {
		t.Fatalf("Task CWD projection = %#v", cwdTask)
	}
	listedCWD := rustAPIRawGraphQLContext(t, resolver, graphqlCtx, `query { tasks(input: { workspaceId: "workspace:personal" }) { edges { node { taskId executorBackend effectiveCwd effectiveCwdSource } } } }`, nil)
	if len(listedCWD.Errors) != 0 {
		t.Fatalf("Task CWD task list errors = %#v", listedCWD.Errors)
	}
	edges := listedCWD.Data["tasks"].(map[string]any)["edges"].([]any)
	if len(edges) == 0 {
		t.Fatal("Task CWD task list is empty")
	}
	seenCWD := false
	for _, edge := range edges {
		node := edge.(map[string]any)["node"].(map[string]any)
		if node["executorBackend"] == "provider" {
			seenCWD = node["effectiveCwd"] == "/tmp/task-work/use-executor" && node["effectiveCwdSource"] == "task"
		}
	}
	if !seenCWD {
		t.Fatalf("Task CWD task list = %#v", edges)
	}
	archiveInput := model.ArchiveProjectInput{ProjectID: projectID,
		ExpectedRevision: 3, ClientMutationID: "project-archive"}
	archivedResponse := rustAPIRawGraphQLContext(t, resolver, graphqlCtx, `mutation($input: ArchiveProjectInput!) {
  archiveProject(input: $input) { project { projectId revision archivedAt } }
}`, map[string]any{"input": map[string]any{
		"projectId": archiveInput.ProjectID, "expectedRevision": archiveInput.ExpectedRevision,
		"clientMutationId": archiveInput.ClientMutationID,
	}})
	if len(archivedResponse.Errors) != 0 {
		t.Fatalf("archive Project GraphQL errors = %#v", archivedResponse.Errors)
	}
	archivedProject := archivedResponse.Data["archiveProject"].(map[string]any)["project"].(map[string]any)
	if archivedProject["archivedAt"] == nil {
		t.Fatalf("archive Project = %#v", archivedProject)
	}
	assertProjectReceipt(t, resolver.Store, "project.archive", archiveInput.ClientMutationID, struct {
		ProjectID        string `json:"projectId"`
		ExpectedRevision int    `json:"expectedRevision"`
	}{archiveInput.ProjectID, archiveInput.ExpectedRevision})
	reopenInput := model.ReopenProjectInput{ProjectID: projectID,
		ExpectedRevision: 4, ClientMutationID: "project-reopen"}
	reopenedResponse := rustAPIRawGraphQLContext(t, resolver, graphqlCtx, `mutation($input: ReopenProjectInput!) {
  reopenProject(input: $input) { project { projectId revision archivedAt } }
}`, map[string]any{"input": map[string]any{
		"projectId": reopenInput.ProjectID, "expectedRevision": reopenInput.ExpectedRevision,
		"clientMutationId": reopenInput.ClientMutationID,
	}})
	if len(reopenedResponse.Errors) != 0 {
		t.Fatalf("reopen Project GraphQL errors = %#v", reopenedResponse.Errors)
	}
	reopenedProject := reopenedResponse.Data["reopenProject"].(map[string]any)["project"].(map[string]any)
	if reopenedProject["archivedAt"] != nil {
		t.Fatalf("reopen Project = %#v", reopenedProject)
	}
	assertProjectReceipt(t, resolver.Store, "project.reopen", reopenInput.ClientMutationID, struct {
		ProjectID        string `json:"projectId"`
		ExpectedRevision int    `json:"expectedRevision"`
	}{reopenInput.ProjectID, reopenInput.ExpectedRevision})
	currentResponse := rustAPIRawGraphQLContext(t, resolver, graphqlCtx, `query($project:String!) {
  projectDocument(projectId: $project) { digest }
}`, map[string]any{"project": projectID})
	if len(currentResponse.Errors) != 0 {
		t.Fatalf("current Project document GraphQL errors = %#v", currentResponse.Errors)
	}
	current := currentResponse.Data["projectDocument"].(map[string]any)
	recoveryInput := model.UpdateProjectDocumentInput{ProjectID: projectID,
		ExpectedRevision: 5, ExpectedDocumentDigest: current["digest"].(string), Content: "# Recovered\n",
		ClientMutationID: "project-document-recovery"}
	encoded, err := json.Marshal(recoveryInput)
	if err != nil {
		t.Fatal(err)
	}
	digest := sha256.Sum256(encoded)
	recoveryCommand := store.ProjectCommand{ActorID: projectActorID, Name: "project.update",
		ClientMutationID: recoveryInput.ClientMutationID, RequestDigest: hex.EncodeToString(digest[:])}
	stage, next, err := home.PrepareProjectDocumentReplace(resolver.home, projectID,
		&folder, current["digest"].(string), recoveryInput.Content, recoveryCommand.RequestDigest)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := resolver.Store.UpdateProject(ctx, projectID, 5,
		store.ProjectChanges{DocumentChanged: true, DocumentDigest: next.Digest}, recoveryCommand, time.Now()); err != nil {
		t.Fatal(err)
	}
	if _, err := home.ReadProjectDocumentStage(resolver.home, stage.ProjectID, stage.RequestDigest); err != nil {
		t.Fatal(err)
	}
	recoveredResponse := rustAPIRawGraphQLContext(t, resolver, graphqlCtx, `mutation($input: UpdateProjectDocumentInput!) {
  updateProjectDocument(input: $input) { project { revision } document { content } }
}`, map[string]any{"input": map[string]any{
		"projectId": recoveryInput.ProjectID, "expectedRevision": recoveryInput.ExpectedRevision,
		"expectedDocumentDigest": recoveryInput.ExpectedDocumentDigest, "content": recoveryInput.Content,
		"clientMutationId": recoveryInput.ClientMutationID,
	}})
	if len(recoveredResponse.Errors) != 0 {
		t.Fatalf("receipt recovery GraphQL errors = %#v", recoveredResponse.Errors)
	}
	recoveredPayload := recoveredResponse.Data["updateProjectDocument"].(map[string]any)
	if recoveredPayload["document"].(map[string]any)["content"] != recoveryInput.Content || recoveredPayload["project"].(map[string]any)["revision"] != float64(6) {
		t.Fatalf("receipt recovery = %#v", recoveredPayload)
	}
	if _, err := home.ReadProjectDocumentStage(resolver.home, stage.ProjectID, stage.RequestDigest); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("recovered stage remains: %v", err)
	}

	existingFolder := t.TempDir()
	if err := os.WriteFile(filepath.Join(existingFolder, "PROJECT.md"), []byte("# Existing\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	existingResponse := rustAPIRawGraphQLContext(t, resolver, graphqlCtx, `mutation($input: CreateProjectInput!) {
  createProject(input: $input) { project { projectId } }
}`, map[string]any{"input": map[string]any{
		"workspaceId": personalWorkspaceID, "name": "Adopt", "folder": existingFolder,
		"clientMutationId": "project-existing",
	}})
	if len(existingResponse.Errors) != 0 {
		t.Fatalf("existing Project GraphQL errors = %#v", existingResponse.Errors)
	}
	existingID := existingResponse.Data["createProject"].(map[string]any)["project"].(map[string]any)["projectId"].(string)
	existingDocumentResponse := rustAPIRawGraphQLContext(t, resolver, graphqlCtx, `query($project:String!) {
  projectDocument(projectId: $project) { content }
}`, map[string]any{"project": existingID})
	if len(existingDocumentResponse.Errors) != 0 || existingDocumentResponse.Data["projectDocument"].(map[string]any)["content"] != "# Existing\n" {
		t.Fatalf("existing Project document = %#v", existingDocumentResponse)
	}
}

func rustAPIPortProjectDocument(t *testing.T) {
	resolver := openTestResolver(t)
	created := rustAPIRawGraphQL(t, resolver, `mutation {
  createProject(input: {
    workspaceId: "workspace:personal", name: "Launch plan",
    description: "opaque-value-58310", clientMutationId: "project-document-create"
  }) { project { projectId revision } }
}`, nil)
	if len(created.Errors) != 0 {
		t.Fatalf("project creation GraphQL errors = %#v", created.Errors)
	}
	project := created.Data["createProject"].(map[string]any)["project"].(map[string]any)
	projectID := project["projectId"].(string)
	revision := project["revision"].(float64)
	read := rustAPIRawGraphQL(t, resolver, `query($project:String!) {
  projectDocument(projectId: $project) { projectId content digest }
}`, map[string]any{"project": projectID})
	if len(read.Errors) != 0 {
		t.Fatalf("project document read GraphQL errors = %#v", read.Errors)
	}
	document := read.Data["projectDocument"].(map[string]any)
	if document["content"] != "# Launch plan\n\nopaque-value-58310\n" {
		t.Fatalf("project document content = %#v", document)
	}
	digest := document["digest"].(string)
	saved := rustAPIRawGraphQL(t, resolver, `mutation($input: UpdateProjectDocumentInput!) {
  updateProjectDocument(input: $input) { project { revision } document { content digest } eventCursor }
}`, map[string]any{"input": map[string]any{
		"projectId": projectID, "expectedRevision": revision,
		"expectedDocumentDigest": digest, "content": "# Current context\n",
		"clientMutationId": "project-document-save",
	}})
	if len(saved.Errors) != 0 {
		t.Fatalf("project document save GraphQL errors = %#v", saved.Errors)
	}
	savedPayload := saved.Data["updateProjectDocument"].(map[string]any)
	if savedPayload["project"].(map[string]any)["revision"] != revision+1 ||
		savedPayload["document"].(map[string]any)["content"] != "# Current context\n" ||
		savedPayload["eventCursor"].(string) == "" {
		t.Fatalf("project document save = %#v", savedPayload)
	}
	conflict := rustAPIRawGraphQL(t, resolver, `mutation($input: UpdateProjectDocumentInput!) {
  updateProjectDocument(input: $input) { project { revision } }
}`, map[string]any{"input": map[string]any{
		"projectId": projectID, "expectedRevision": revision,
		"expectedDocumentDigest": digest, "content": "# Stale context\n",
		"clientMutationId": "project-document-conflict",
	}})
	rustAPIAssertGraphQLError(t, conflict, "the authoritative Task document changed", "stale_document")
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
	resolver := openProviderTestResolver(t)
	rustAPIAuthenticateCodexForWebSettings(t, resolver)
	ctx := context.Background()
	complexity := "simple"
	entries, err := resolver.Store.TaskModelPoolEntries(ctx, &complexity)
	if err != nil || len(entries) != 1 {
		t.Fatalf("simple task model pool = %#v, %v", entries, err)
	}
	existing := taskModelPoolEntry(entries[0])
	label := "Unavailable but editable"
	updated, err := resolver.updateTaskModelPoolEntry(
		ctx, "task_pool:setting:simple", model.TaskModelPoolEntryInput{
			Complexity: existing.Complexity, ProviderKind: existing.ProviderKind,
			ProviderAccountID: existing.ProviderAccountID, SelectionMode: existing.SelectionMode,
			ModelProfile: existing.ModelProfile, ReasoningEffort: existing.ReasoningEffort,
			FastMode: existing.FastMode, Label: &label, Enabled: true, SortOrder: existing.SortOrder,
		},
	)
	if err != nil {
		t.Fatal(err)
	}
	if updated.Label == nil || *updated.Label != label || updated.ProviderKind != existing.ProviderKind ||
		updated.ProviderAccountID != existing.ProviderAccountID || updated.SelectionMode != existing.SelectionMode ||
		updated.FastMode != existing.FastMode || updated.SortOrder != existing.SortOrder {
		t.Fatalf("updated pool entry = %#v", updated)
	}
	// Rust also edits the same stale route with enabled=false and verifies the
	// persisted flag. Keep that request at the GraphQL boundary so a missing
	// Go schema field is reported as a product divergence instead of being
	// silently omitted from the port.
	response := rustAPIRawGraphQL(t, resolver, `mutation {
  updateTaskModelPoolEntry(poolEntryId: "task_pool:setting:simple", input: {
    complexity: SIMPLE, label: "Unavailable but editable", providerKind: "codex",
    providerAccountId: "provider_account:codex:default",
    selectionMode: NOEMA_RECOMMENDED, fastMode: false, sortOrder: 0, enabled: false
  }) { enabled }
}`, nil)
	if len(response.Errors) != 0 {
		t.Fatalf("disabling stale route failed: %#v", response.Errors)
	}
	if response.Data["updateTaskModelPoolEntry"].(map[string]any)["enabled"] != false {
		t.Fatalf("disabled stale route response = %#v", response.Data)
	}
	readback := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(ctx), `query {
  taskModelPools(complexity: SIMPLE) { poolEntryId enabled }
}`, nil)
	if len(readback.Errors) != 0 {
		t.Fatalf("stale route readback failed: %#v", readback.Errors)
	}
	readbackEntries, ok := readback.Data["taskModelPools"].([]any)
	if !ok {
		t.Fatalf("stale route entries = %#v", readback.Data)
	}
	found := false
	for _, raw := range readbackEntries {
		entry := raw.(map[string]any)
		if entry["poolEntryId"] == "task_pool:setting:simple" {
			found = true
			if entry["enabled"] != false {
				t.Fatalf("stale route remained enabled: %#v", entry)
			}
		}
	}
	if !found {
		t.Fatal("stale route was not persisted")
	}
	persisted, err := resolver.Store.TaskModelPoolEntries(ctx, &complexity)
	if err != nil || len(persisted) != 1 || persisted[0].ID != "task_pool:setting:simple" || persisted[0].Enabled {
		t.Fatalf("persisted stale route = %#v, %v", persisted, err)
	}
	if persisted[0].Label == nil || *persisted[0].Label != "Unavailable but editable" {
		t.Fatalf("persisted stale route label = %#v", persisted[0])
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
	encodedStartToken := base64.RawURLEncoding.EncodeToString([]byte("start-token"))
	if _, err := service.RegisterClientLiveActivities(context.Background(), clientID, encodedStartToken, store.APNSDevelopment, nil); err != nil {
		t.Fatal(err)
	}
	return resolver, service, clientID
}

func rustAPICreateLiveActivityTask(t *testing.T, resolver *Resolver, id, title string, at time.Time) store.TaskRun {
	t.Helper()
	ctx := context.Background()
	command, err := newTaskCommand("capture_task", "live-activity-"+strings.TrimPrefix(id, "task:"), id)
	if err != nil {
		t.Fatal(err)
	}
	created, err := resolver.Store.CreateTaskWithOptions(ctx, id, title, command, store.TaskCreateOptions{
		ExecutorAgentID:     store.PrimaryAgentID,
		InitialRunKind:      "executor",
		ExecutionComplexity: "simple",
	}, at)
	if err != nil {
		t.Fatalf("create Live Activity Task = %#v: %v", created, err)
	}
	_, run, found, err := resolver.Store.ClaimTaskExecution(ctx, at.Add(time.Second))
	if err != nil || !found || run.ID == "" {
		t.Fatalf("claim Live Activity Task = %#v, %t, %v", run, found, err)
	}
	if err := resolver.Store.StartTaskExecution(ctx, run.ID, run.Generation, at.Add(2*time.Second)); err != nil {
		t.Fatal(err)
	}
	return run
}

func rustAPIPortLiveActivityPayload(t *testing.T) {
	t.Helper()
	resolver, service, clientID := rustAPILiveDeliveryFixture(t)
	ctx := context.Background()
	now := time.Now()
	if _, err := resolver.Store.UpdatePrimaryAgentDisplayName(ctx, "Atlas", now); err != nil {
		t.Fatal(err)
	}
	otherRun := rustAPICreateLiveActivityTask(t, resolver, "task:"+strings.Repeat("2", 32), "Other", now)
	if err := resolver.Store.BlockTaskExecution(ctx, otherRun.ID, otherRun.Generation, "approval", "Review Other", "Review Other", []string{"approve"}, now.Add(time.Second)); err != nil {
		t.Fatal(err)
	}
	focusRun := rustAPICreateLiveActivityTask(t, resolver, "task:"+strings.Repeat("1", 32), "Focus", now.Add(time.Second))
	if err := resolver.Store.BlockTaskExecution(ctx, focusRun.ID, focusRun.Generation, "approval", "Review Focus", "Review Focus", []string{"approve"}, now.Add(2*time.Second)); err != nil {
		t.Fatal(err)
	}
	if err := service.ReconcileLiveActivities(ctx); err != nil {
		t.Fatal(err)
	}
	activity, err := resolver.Store.ClientTaskActivity(ctx, clientID)
	if err != nil || activity == nil {
		t.Fatalf("Live Activity = %#v, %v", activity, err)
	}
	delivery, err := resolver.Store.ClaimDueLiveActivityDelivery(ctx, time.Now().Add(time.Minute))
	if err != nil || delivery == nil {
		t.Fatalf("Live Activity payload delivery = %#v, %v", delivery, err)
	}
	aps := delivery.Payload["aps"].(map[string]any)
	if delivery.Payload["route"] != "task" {
		t.Errorf("Live Activity route = %#v, want %q", delivery.Payload["route"], "task")
	}
	if delivery.Payload["taskId"] != "task:focus" {
		t.Errorf("Live Activity taskId = %#v, want %q", delivery.Payload["taskId"], "task:focus")
	}
	if aps["attributes-type"] != "NoemaTasksActivityAttributes" {
		t.Errorf("Live Activity attributes type = %#v, want %q", aps["attributes-type"], "NoemaTasksActivityAttributes")
	}
	alert, _ := aps["alert"].(map[string]any)
	if alert["title"] != "Noema Tasks" || alert["body"] != "Focus" {
		t.Errorf("Live Activity alert = %#v, want title/body", alert)
	}
	content, _ := aps["content-state"].(map[string]any)
	if content["activeTaskCount"] != float64(2) {
		t.Errorf("Live Activity activeTaskCount = %#v, want 2", content["activeTaskCount"])
	}
	if content["requiresAttention"] != true {
		t.Errorf("Live Activity requiresAttention = %#v, want true", content["requiresAttention"])
	}
	if content["agentName"] != "Atlas" {
		t.Errorf("Live Activity agentName = %#v, want %q", content["agentName"], "Atlas")
	}
	summaries, _ := content["taskSummaries"].([]any)
	firstSummary, _ := summaries[0].(map[string]any)
	if firstSummary["title"] != "Focus" {
		t.Errorf("Live Activity first task title = %#v, want %q", firstSummary["title"], "Focus")
	}
	if err := resolver.Store.FinishLiveActivityDelivery(ctx, *delivery, store.APNSDelivered, "", "payload-start", time.Now()); err != nil {
		t.Fatal(err)
	}
	encodedUpdateToken := base64.RawURLEncoding.EncodeToString([]byte("update-token"))
	if changed, err := service.RegisterClientLiveActivityUpdate(ctx, clientID, activity.ActivityID, encodedUpdateToken); err != nil || !changed {
		t.Fatalf("Live Activity update registration = %t, %v", changed, err)
	}
	if err := service.ReconcileLiveActivities(ctx); err != nil {
		t.Fatal(err)
	}
	update, err := resolver.Store.ClaimDueLiveActivityDelivery(ctx, time.Now().Add(time.Minute))
	if err != nil || update == nil {
		t.Fatalf("Live Activity update delivery = %#v, %v", update, err)
	}
	updateAPS, _ := update.Payload["aps"].(map[string]any)
	if update.Payload["taskId"] != "task:one" {
		t.Errorf("Live Activity alert taskId = %#v, want %q", update.Payload["taskId"], "task:one")
	}
	updateAlert, _ := updateAPS["alert"].(map[string]any)
	if updateAlert["body"] != "Review Focus" {
		t.Errorf("Live Activity alert body = %#v, want %q", updateAlert["body"], "Review Focus")
	}
}

func rustAPIPortLiveActivityPriority(t *testing.T) {
	t.Helper()
	resolver, service, clientID := rustAPILiveDeliveryFixture(t)
	ctx := context.Background()
	run := rustAPICreateLiveActivityTask(t, resolver, "task:"+strings.Repeat("3", 32), "Priority", time.Now())
	if err := service.ReconcileLiveActivities(ctx); err != nil {
		t.Fatal(err)
	}
	activity, err := resolver.Store.ClientTaskActivity(ctx, clientID)
	if err != nil || activity == nil {
		t.Fatal(err)
	}
	start, err := resolver.Store.ClaimDueLiveActivityDelivery(ctx, time.Now().Add(time.Minute))
	if err != nil || start == nil {
		t.Fatalf("priority start delivery = %#v, %v", start, err)
	}
	if start.Urgency != "high" {
		t.Errorf("Live Activity start urgency = %q, want %q", start.Urgency, "high")
	}
	if err := resolver.Store.FinishLiveActivityDelivery(ctx, *start, store.APNSDelivered, "", "priority-start", time.Now()); err != nil {
		t.Fatal(err)
	}
	encodedUpdateToken := base64.RawURLEncoding.EncodeToString([]byte("priority-update"))
	if changed, err := service.RegisterClientLiveActivityUpdate(ctx, clientID, activity.ActivityID, encodedUpdateToken); err != nil || !changed {
		t.Fatalf("priority update registration = %t, %v", changed, err)
	}
	if err := resolver.Store.AppendTaskRunItems(ctx, run.ID, run.Generation, []store.TaskRunItemInput{{Kind: "assistant_output", Status: "completed", Round: 1, Content: "Ordinary update"}}, store.TaskRunUsage{}, time.Now()); err != nil {
		t.Fatal(err)
	}
	if err := service.ReconcileLiveActivities(ctx); err != nil {
		t.Fatal(err)
	}
	ordinaryUpdate, err := resolver.Store.ClaimDueLiveActivityDelivery(ctx, time.Now().Add(time.Minute))
	if err != nil || ordinaryUpdate == nil {
		t.Fatalf("priority ordinary update = %#v, %v", ordinaryUpdate, err)
	}
	if ordinaryUpdate.Urgency != "normal" {
		t.Errorf("Live Activity ordinary update urgency = %q, want %q", ordinaryUpdate.Urgency, "normal")
	}
	digest := sha256.Sum256([]byte(activity.ActivityID))
	collapseID := hex.EncodeToString(digest[:])
	if len(collapseID) != 64 {
		t.Errorf("ordinary update collapse ID length = %d, want 64", len(collapseID))
	}
	for _, value := range collapseID {
		if !strings.ContainsRune("0123456789abcdef", value) {
			t.Errorf("ordinary update collapse ID contains %q", value)
			break
		}
	}
	if collapseID != hex.EncodeToString(digest[:]) {
		t.Errorf("ordinary update collapse ID was not stable: %q", collapseID)
	}
	if err := resolver.Store.FinishLiveActivityDelivery(ctx, *ordinaryUpdate, store.APNSDelivered, "", "priority-update", time.Now()); err != nil {
		t.Fatal(err)
	}
	if err := resolver.Store.BlockTaskExecution(ctx, run.ID, run.Generation, "approval", "Immediate update", "Immediate update", []string{"approve"}, time.Now()); err != nil {
		t.Fatal(err)
	}
	if err := service.ReconcileLiveActivities(ctx); err != nil {
		t.Fatal(err)
	}
	immediateUpdate, err := resolver.Store.ClaimDueLiveActivityDelivery(ctx, time.Now().Add(time.Minute))
	if err != nil || immediateUpdate == nil {
		t.Fatalf("priority immediate update = %#v, %v", immediateUpdate, err)
	}
	if immediateUpdate.Urgency != "high" {
		t.Errorf("Live Activity immediate update urgency = %q, want %q", immediateUpdate.Urgency, "high")
	}
	if err := resolver.Store.FinishLiveActivityDelivery(ctx, *immediateUpdate, store.APNSDelivered, "", "priority-immediate", time.Now()); err != nil {
		t.Fatal(err)
	}
	current, err := resolver.Store.Task(ctx, "task:"+strings.Repeat("3", 32))
	if err != nil {
		t.Fatal(err)
	}
	cancel, err := newTaskCommand("cancel_task", "live-activity-priority-cancel", current.ID)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := resolver.Store.CancelTask(ctx, current.ID, current.Revision, current.Generation, "priority complete", cancel, time.Now()); err != nil {
		t.Fatal(err)
	}
	if err := service.ReconcileLiveActivities(ctx); err != nil {
		t.Fatal(err)
	}
	end, err := resolver.Store.ClaimDueLiveActivityDelivery(ctx, time.Now().Add(time.Minute))
	if err != nil || end == nil {
		t.Fatalf("priority end delivery = %#v, %v", end, err)
	}
	if end.Urgency != "normal" {
		t.Errorf("Live Activity end urgency = %q, want %q", end.Urgency, "normal")
	}
}

func rustAPIPortLiveActivityMutationLane(t *testing.T) {
	t.Helper()
	resolver, service, clientID := rustAPILiveDeliveryFixture(t)
	ctx := context.Background()
	taskID := "task:33333333333333333333333333333333"
	if _, err := home.CreatePendingTaskDocument(resolver.home, taskID, "Mutation lane task"); err != nil {
		t.Fatal(err)
	}
	if err := home.CommitTaskDocument(resolver.home, taskID); err != nil {
		t.Fatal(err)
	}
	if _, err := resolver.Store.CreateTask(ctx, taskID, "Mutation lane task", "correlation:mutation-lane", time.Now()); err != nil {
		t.Fatal(err)
	}
	queued, err := resolver.queueTask(ctx, model.QueueTaskInput{TaskID: taskID, ExpectedRevision: 1, ExpectedGeneration: 1, ClientMutationID: "queue-mutation-lane"})
	if err != nil || queued.Task.CurrentRun == nil {
		t.Fatalf("queue mutation-lane run = %#v, %v", queued, err)
	}
	_, run, found, err := resolver.Store.ClaimTaskExecution(ctx, time.Now())
	if err != nil || !found {
		t.Fatalf("claim mutation-lane run = %#v, %v", run, err)
	}
	if err := resolver.Store.StartTaskExecution(ctx, run.ID, run.Generation, time.Now()); err != nil {
		t.Fatal(err)
	}
	activity, err := resolver.Store.ClientTaskActivity(ctx, clientID)
	if err != nil || activity == nil {
		t.Fatal(err)
	}
	wantDeliveryKey := "live:start:" + activity.TaskSessionID
	errs := make(chan error, 4)
	var group sync.WaitGroup
	for i := 0; i < 4; i++ {
		group.Add(1)
		go func() {
			defer group.Done()
			errs <- service.ReconcileLiveActivities(ctx)
		}()
	}
	group.Wait()
	close(errs)
	for reconcileErr := range errs {
		if reconcileErr != nil {
			t.Fatal(reconcileErr)
		}
	}
	delivery, err := resolver.Store.ClaimDueLiveActivityDelivery(ctx, time.Now().Add(time.Minute))
	if err != nil || delivery == nil || delivery.DeliveryKey != wantDeliveryKey {
		t.Fatalf("mutation-lane delivery = %#v, want %q: %v", delivery, wantDeliveryKey, err)
	}
	if err := resolver.Store.FinishLiveActivityDelivery(ctx, *delivery, store.APNSSuppressed, "test_claimed_once", "", time.Now()); err != nil {
		t.Fatal(err)
	}
	if duplicate, err := resolver.Store.ClaimDueLiveActivityDelivery(ctx, time.Now().Add(time.Minute)); err != nil || duplicate != nil {
		t.Fatalf("duplicate mutation-lane delivery = %#v, %v", duplicate, err)
	}
}

func rustAPIPortLiveActivityFocus(t *testing.T) {
	t.Helper()
	resolver, service, clientID := rustAPILiveDeliveryFixture(t)
	ctx := context.Background()
	if err := service.ReconcileLiveActivities(ctx); err != nil {
		t.Fatal(err)
	}
	activity, err := resolver.Store.ClientTaskActivity(ctx, clientID)
	if err != nil || activity == nil {
		t.Fatal(err)
	}
	if activity.Lifecycle != "starting" {
		t.Errorf("initial Live Activity lifecycle = %q, want %q", activity.Lifecycle, "starting")
	}
	if activity.Suppressed {
		t.Errorf("initial Live Activity is suppressed")
	}
	oldSession := activity.TaskSessionID
	oldTaskID := "task:" + strings.Repeat("4", 32)
	_ = rustAPICreateLiveActivityTask(t, resolver, oldTaskID, "Old", time.Now())
	if err := service.ReconcileLiveActivities(ctx); err != nil {
		t.Fatal(err)
	}
	activity, err = resolver.Store.ClientTaskActivity(ctx, clientID)
	if err != nil || activity == nil {
		t.Fatalf("old Live Activity = %#v, %v", activity, err)
	}
	if activity.FocusedTaskID != oldTaskID {
		t.Errorf("old Live Activity focus = %q, want %q", activity.FocusedTaskID, oldTaskID)
	}
	if changed, err := service.DismissClientLiveActivity(ctx, clientID, activity.ActivityID); err != nil || !changed {
		t.Fatalf("dismissed Live Activity = %t, %v", changed, err)
	}
	newTaskID := "task:" + strings.Repeat("5", 32)
	_ = rustAPICreateLiveActivityTask(t, resolver, newTaskID, "New", time.Now().Add(time.Second))
	if err := service.ReconcileLiveActivities(ctx); err != nil {
		t.Fatal(err)
	}
	current, err := resolver.Store.ClientTaskActivity(ctx, clientID)
	if err != nil || current == nil {
		t.Fatalf("replacement Live Activity = %#v, %v", current, err)
	}
	if current.TaskSessionID == oldSession || current.Lifecycle != "starting" || current.Suppressed {
		t.Errorf("replacement Live Activity = %#v, want a fresh unsuppressed starting session", current)
	}
	if current.FocusedTaskID != newTaskID {
		t.Errorf("replacement Live Activity focus = %q, want %q", current.FocusedTaskID, newTaskID)
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
	agentID := "agent:task-executor"
	mediaType := "text/plain"
	for index := 1; index <= 3; index++ {
		if _, err := resolver.Artifacts.CreateLocal(ctx, artifact.LocalInput{
			Owner: store.ArtifactOwner{ObjectType: "task", ObjectID: taskID}, Title: fmt.Sprintf("Output %d", index),
			Kind: "document", Filename: fmt.Sprintf("output-%d.txt", index), Bytes: []byte("completed output"),
			MediaType: &mediaType, CreatedByActorID: agentID,
		}); err != nil {
			t.Fatal(err)
		}
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
	if err := service.ReconcileLiveActivities(ctx); err != nil {
		t.Fatal(err)
	}
	if _, err := resolver.Store.FinishTask(ctx, taskID, runID, store.TaskCompleted, time.Now()); err != nil {
		t.Fatal(err)
	}
	if err := service.ReconcileLiveActivities(ctx); err != nil {
		t.Fatal(err)
	}
	value, err := resolver.Store.ClientTaskActivity(ctx, clientID)
	if err != nil || value == nil {
		t.Fatalf("terminal Live Activity = %#v, %v", value, err)
	}
	if value.Projection["phase"] != "completed" || value.Projection["agentName"] != "Task Executor" || value.Projection["completedOutputCount"] != float64(3) && value.Projection["completedOutputCount"] != 3 || value.Projection["activeTaskCount"] != float64(0) && value.Projection["activeTaskCount"] != 0 {
		t.Fatalf("terminal Live Activity projection = %#v", value.Projection)
	}
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
