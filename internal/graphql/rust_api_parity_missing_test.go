package graphql

import (
	"bytes"
	"context"
	"crypto/ecdh"
	"crypto/rand"
	"crypto/sha256"
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
	"strings"
	"sync"
	"testing"
	"time"

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
	"github.com/vektah/gqlparser/v2/gqlerror"
)

func rustAPIPortSubscriptionProjectionFailure(t *testing.T) {
	t.Helper()
	original := int64(1)
	cursor := original
	if _, err := store.DecodeWorkEventCursor("malformed"); err == nil {
		t.Fatal("malformed subscription cursor was accepted")
	}
	if cursor != original {
		t.Fatalf("cursor changed after failed projection: %d", cursor)
	}
	// A cursor at the largest representable sequence is still valid. The
	// projection must keep the caller cursor unchanged if a later conversion
	// fails.
	maxCursor, err := store.EncodeWorkEventCursor(math.MaxInt64)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := store.DecodeWorkEventCursor(maxCursor); err != nil {
		t.Fatal(err)
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
	ctx := context.Background()
	queryChecks := []func() error{
		func() error { _, err := resolver.task(context.Background(), "task:foreign"); return err },
		func() error {
			_, err := resolver.taskWorkspaceFile(context.Background(), "task:foreign", "TASK.md")
			return err
		},
		func() error {
			_, err := resolver.conversationTranscriptPage(context.Background(), model.ConversationTranscriptPageInput{ConversationID: "conversation:foreign"})
			return err
		},
		func() error { _, err := resolver.pendingHumanInterventions(ctx, nil, nil, nil, nil); return err },
		func() error { _, err := resolver.acpAgents(ctx); return err },
	}
	for index, check := range queryChecks {
		if err := check(); err == nil || !strings.Contains(err.Error(), "request is unauthenticated") {
			t.Fatalf("owner-sensitive query %d = %v", index, err)
		}
	}
	mutationChecks := []func() error{
		func() error {
			_, err := resolver.createConversationExternalArtifact(ctx, model.CreateConversationExternalArtifactInput{ConversationID: "conversation:foreign", Title: "Foreign", ArtifactKind: "document", ExternalURL: "https://example.com/foreign"})
			return err
		},
		func() error {
			_, err := resolver.startMCPCallAuthentication(ctx, model.StartMcpAuthenticationInput{RequestID: "mcp_auth:foreign", ExpectedRevision: 1, RedirectURI: "http://localhost/callback"})
			return err
		},
		func() error {
			_, err := resolver.skipMCPCallAuthentication(ctx, model.SkipMcpAuthenticationInput{RequestID: "mcp_auth:foreign", ExpectedRevision: 1})
			return err
		},
		func() error {
			_, err := resolver.createAcpAgent(ctx, model.CreateAcpAgentInput{DisplayName: "Foreign", Command: "/bin/false"})
			return err
		},
		func() error {
			_, err := resolver.deleteAcpAgent(ctx, model.DeleteAcpAgentInput{AgentID: "agent:foreign", ExpectedRevision: 1})
			return err
		},
	}
	for index, check := range mutationChecks {
		if err := check(); err == nil || !strings.Contains(err.Error(), "request is unauthenticated") {
			t.Fatalf("owner-sensitive mutation %d = %v", index, err)
		}
	}
	// The Go resolver exposes subscription streams directly. The first
	// response must still reject an ownerless context before opening either
	// stream.
	if _, err := resolver.conversationEvents(ctx, "conversation:foreign"); err == nil || !strings.Contains(err.Error(), "request is unauthenticated") {
		t.Fatalf("owner-sensitive conversation subscription = %v", err)
	}
	if _, err := resolver.memoryEvents(ctx); err == nil || !strings.Contains(err.Error(), "request is unauthenticated") {
		t.Fatalf("owner-sensitive memory subscription = %v", err)
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
	if _, err := resolver.deleteAcpAgent(ctx, model.DeleteAcpAgentInput{AgentID: agent.AgentID, ExpectedRevision: 2}); !errors.Is(err, store.ErrAcpAgentRevisionConflict) {
		t.Fatalf("stale ACP deletion = %v", err)
	}
	if _, err := resolver.deleteAcpAgent(ctx, model.DeleteAcpAgentInput{AgentID: agent.AgentID, ExpectedRevision: 1}); !errors.Is(err, store.ErrAcpAgentAuthenticationBusy) {
		t.Fatalf("active ACP authentication deletion = %v", err)
	}
	if _, err := resolver.Store.FinishAcpAuthentication(ctx, attempt, true, nil, time.Now()); err != nil {
		t.Fatal(err)
	}
	deleted, err := resolver.deleteAcpAgent(ctx, model.DeleteAcpAgentInput{AgentID: agent.AgentID, ExpectedRevision: 1})
	if err != nil || !deleted {
		t.Fatalf("ACP deletion = %t, %v", deleted, err)
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
	captured, err := resolver.captureTask(ctx, model.CaptureTaskInput{WorkspaceID: personalWorkspaceID, Title: "Keep assigned executor", ExecutorAgentID: &agent.AgentID, ClientMutationID: "capture-acp-delete-guard"})
	if err != nil {
		t.Fatal(err)
	}
	if _, err := resolver.deleteAcpAgent(ctx, model.DeleteAcpAgentInput{AgentID: agent.AgentID, ExpectedRevision: 1}); !errors.Is(err, store.ErrAcpAgentInUse) {
		t.Fatalf("current-task ACP deletion = %v", err)
	}
	if _, err := resolver.cancelTask(ctx, model.CancelTaskInput{TaskID: captured.Task.TaskID, ExpectedRevision: 1, ExpectedGeneration: 1, ClientMutationID: "cancel-acp-delete-guard"}); err != nil {
		t.Fatal(err)
	}
	_, err = resolver.captureTask(ctx, model.CaptureTaskInput{
		WorkspaceID: personalWorkspaceID, Title: "Recurring ACP", ExecutorAgentID: &agent.AgentID,
		Schedule:         &model.NewTaskScheduleInput{ScheduledFor: "2030-01-01T08:00:00Z", TimeZone: "UTC", Recurrence: &model.NewTaskRecurrenceInput{StartsAt: "2030-01-01T08:00:00Z", CronExpression: "0 8 * * *"}},
		ClientMutationID: "capture-acp-recurring",
	})
	if err != nil {
		t.Fatal(err)
	}
	if _, err := resolver.deleteAcpAgent(ctx, model.DeleteAcpAgentInput{AgentID: agent.AgentID, ExpectedRevision: 1}); !errors.Is(err, store.ErrAcpAgentInUse) {
		t.Fatalf("scheduled ACP deletion = %v", err)
	}
}

func rustAPIPortAgentErrorSanitization(t *testing.T) {
	t.Helper()
	resolver := openProviderTestResolver(t)
	agents, err := resolver.agents(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	encoded, err := json.Marshal(agents)
	if err != nil {
		t.Fatal(err)
	}
	if bytesContainsAny(encoded, []byte("/Users/alice"), []byte("abc123"), []byte("token.txt")) {
		t.Fatalf("Agent projection exposed provider diagnostics: %s", encoded)
	}
	for _, agent := range agents {
		for _, option := range agent.ModelOptions {
			if option.ProviderKind == "local_models" && option.DisabledReason != nil && *option.DisabledReason != "Provider is unavailable on this platform." {
				t.Fatalf("local model disabled reason = %q", *option.DisabledReason)
			}
		}
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
	detail, err := resolver.artifactVersionDetail(ctx, second.ID)
	if err != nil || detail == nil || len(detail.Versions) != 2 || detail.PreviewKind != model.ArtifactVersionPreviewKindMarkdown || detail.Markdown == nil || *detail.Markdown != "# second\n" || detail.PlainText != nil {
		t.Fatalf("local Artifact detail = %#v, %v", detail, err)
	}
	if detail.DownloadURL == nil || *detail.DownloadURL != artifact.DownloadURL(second.ID) {
		t.Fatalf("local Artifact download URL = %#v", detail.DownloadURL)
	}
}

func localArtifactInput(conversationID, content, filename string, mediaType *string) artifact.LocalInput {
	return artifact.LocalInput{Owner: store.ArtifactOwner{ObjectType: "conversation", ObjectID: conversationID}, Title: "Session report", Kind: "document", Filename: filename, Bytes: []byte(content), MediaType: mediaType, CreatedByActorID: "agent:primary", Source: store.ArtifactSource{ConversationID: conversationID}, Metadata: map[string]any{}}
}

func rustAPIPortForeignArtifactOperations(t *testing.T) {
	t.Helper()
	resolver := openTestResolver(t)
	ctx := context.Background()
	listed, err := resolver.artifacts(ctx, "conversation", "conversation:foreign", nil)
	if err != nil || len(listed) != 0 {
		t.Fatalf("foreign Artifact list = %#v, %v", listed, err)
	}
	if _, err := resolver.createConversationExternalArtifact(ctx, model.CreateConversationExternalArtifactInput{ConversationID: "conversation:foreign", Title: "Injected", ArtifactKind: "document", ExternalURL: "https://example.com/injected"}); err == nil || !strings.Contains(err.Error(), "conversation is unavailable") {
		t.Fatalf("foreign Artifact creation = %v", err)
	}
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
	created, err := resolver.createTaskLocalArtifact(ctx, model.CreateTaskLocalArtifactInput{TaskID: captured.Task.TaskID, ExpectedRevision: captured.Task.Revision, ExpectedGeneration: captured.Task.Generation, Title: "August statement", Filename: "statement.csv", MediaType: "text/csv", ContentBase64: base64.StdEncoding.EncodeToString(content)})
	if err != nil {
		t.Fatal(err)
	}
	if created.OwnerObjectType != "task" || created.OwnerObjectID != captured.Task.TaskID || created.CurrentVersion.ByteSize == nil || *created.CurrentVersion.ByteSize != len(content) {
		t.Fatalf("Task-owned Artifact = %#v", created)
	}
	stored, err := resolver.Store.ArtifactWithVersionsByID(ctx, created.ArtifactID)
	if err != nil || stored.Artifact.Metadata["filename"] != "statement.csv" {
		t.Fatalf("Task Artifact metadata = %#v, %v", stored.Artifact.Metadata, err)
	}
}

func rustAPIPortForeignConversationOperations(t *testing.T) {
	t.Helper()
	resolver := openChatTestResolver(t)
	ctx := context.Background()
	for _, operation := range []func() error{
		func() error {
			_, err := resolver.conversationTranscriptPage(ctx, model.ConversationTranscriptPageInput{ConversationID: "conversation:foreign"})
			return err
		},
		func() error {
			clientID := "client:foreign"
			_, err := resolver.sendConversationTurn(ctx, model.SendConversationTurnInput{ConversationID: "conversation:foreign", Input: "private", ClientMessageID: &clientID})
			return err
		},
		func() error {
			clientID := "client:foreign"
			_, err := resolver.sendMultipleChoiceSelection(ctx, model.SendMultipleChoiceSelectionInput{ConversationID: "conversation:foreign", PromptItemID: "item:foreign", SelectedOptionIds: []string{"option:foreign"}, ClientMessageID: &clientID})
			return err
		},
	} {
		if err := operation(); err == nil || !strings.Contains(err.Error(), "conversation") {
			t.Fatalf("foreign conversation operation = %v", err)
		}
	}
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
	profile, err := resolver.runtimeDebugProfile(ctx, model.RuntimeDebugProfileInput{
		Kind: model.RuntimeDebugScopeKindConversationTurn, ScopeID: turn.ID,
	})
	if err != nil || profile == nil || len(profile.Spans) != 1 {
		t.Fatalf("profile = %#v, %v", profile, err)
	}
	span := profile.Spans[0]
	if span.DurationMilliseconds != 30 || span.StartOffsetMilliseconds != 10 ||
		span.Provider == nil || *span.Provider != "openai" || span.InputTokens == nil || *span.InputTokens != 21 {
		t.Fatalf("span = %#v", span)
	}
	if profile.AccountedMilliseconds != 30 || profile.UninstrumentedMilliseconds != 70 {
		t.Fatalf("profile timing = %#v", profile)
	}
	missing, err := resolver.runtimeDebugProfile(ctx, model.RuntimeDebugProfileInput{
		Kind: model.RuntimeDebugScopeKindConversationTurn, ScopeID: "turn:00000000000000000000000000000000",
	})
	if err != nil || missing != nil {
		t.Fatalf("unowned profile = %#v, %v", missing, err)
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
	effort := model.ReasoningEffortHigh
	if _, err := resolver.saveAgentModelPreference(ctx, model.SaveAgentModelPreferenceInput{AgentID: store.PrimaryAgentID, ProviderAccountID: "provider_account:openrouter:default", SelectionMode: model.ModelPreferenceSelectionModeExplicitProfile, ModelProfile: &profile}); err == nil || !strings.Contains(err.Error(), "reasoning effort") {
		t.Fatalf("missing reasoning effort accepted: %v", err)
	}
	saved, err := resolver.saveAgentModelPreference(ctx, model.SaveAgentModelPreferenceInput{AgentID: store.PrimaryAgentID, ProviderAccountID: "provider_account:openrouter:default", SelectionMode: model.ModelPreferenceSelectionModeExplicitProfile, ModelProfile: &profile, ReasoningEffort: &effort})
	if err != nil || saved.ModelProfile == nil || *saved.ModelProfile != profile || saved.ReasoningEffort == nil || *saved.ReasoningEffort != effort {
		t.Fatalf("valid Agent preference = %#v, %v", saved, err)
	}
	recommended, err := resolver.saveAgentModelPreference(ctx, model.SaveAgentModelPreferenceInput{AgentID: store.PrimaryAgentID, ProviderAccountID: "provider_account:openrouter:default", SelectionMode: model.ModelPreferenceSelectionModeNoemaRecommended})
	if err != nil || recommended.ModelProfile != nil || recommended.ReasoningEffort != nil {
		t.Fatalf("recommended Agent preference = %#v, %v", recommended, err)
	}
	if _, err := resolver.saveAgentModelPreference(ctx, model.SaveAgentModelPreferenceInput{AgentID: store.TaskExecutorAgentID, ProviderAccountID: "provider_account:openrouter:default", SelectionMode: model.ModelPreferenceSelectionModeExplicitProfile, ModelProfile: &profile, ReasoningEffort: &effort}); err == nil || !strings.Contains(err.Error(), "complexity tier") {
		t.Fatalf("Task Executor preference accepted: %v", err)
	}
}

func rustAPIPortACPSetup(t *testing.T) {
	t.Helper()
	resolver := openTestResolver(t)
	ctx := context.Background()
	created, err := resolver.createAcpAgent(ctx, model.CreateAcpAgentInput{DisplayName: "Codex ACP", Command: "/usr/bin/codex", Arguments: []string{"--acp"}})
	if err != nil || created.ConnectionRevision != 1 || created.HealthStatus != model.AcpAgentHealthStatusUnknown || created.AuthStatus != model.AcpAgentAuthStatusUnknown || created.Arguments[0] != "--acp" {
		t.Fatalf("created ACP agent = %#v, %v", created, err)
	}
	if strings.Contains(fmt.Sprintf("%#v", created), "credentials") {
		t.Fatalf("ACP projection exposed credentials: %#v", created)
	}
	updated, err := resolver.updateAcpAgent(ctx, model.UpdateAcpAgentInput{AgentID: created.AgentID, ExpectedRevision: 1, DisplayName: "Codex ACP", Command: "/usr/bin/codex", Arguments: []string{"--acp"}, Enabled: false})
	if err != nil || updated.Enabled || updated.ConnectionRevision != 2 {
		t.Fatalf("updated ACP agent = %#v, %v", updated, err)
	}
}

func stringPtr(value string) *string { return &value }

func rustAPIPortMemoryEvents(t *testing.T) {
	resolver := readyAgentTestResolver(t)
	ctx := context.Background()
	conversation, err := resolver.Store.EnsurePrimaryConversation(ctx, "openrouter", "", time.Now())
	if err != nil {
		t.Fatal(err)
	}
	turn, human, err := resolver.Store.BeginConversationTurn(ctx, conversation.ID, "Alice likes tea.", nil, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	assistant, err := resolver.Store.CompleteConversationTurn(ctx, turn, "Noted.", "Noted.", nil, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	nativeMemory, err := noemamemory.New(resolver.home)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = nativeMemory.Close() })
	resolver.Memory = nativeMemory
	root, err := nativeMemory.ReadRoot()
	if err != nil {
		t.Fatal(err)
	}
	if err := nativeMemory.Publish(noemamemory.ChangeSet{Upserts: []noemamemory.PageChange{
		{ID: root.ID, ExpectedHash: root.Hash, Path: noemamemory.RootPagePath, Title: root.Title, Icon: root.Icon, Body: "People and preferences."},
		{Path: "people.md", Title: "People", Icon: "users", Body: "Known people."},
		{Path: "people/alice.md", Title: "Alice", Icon: "user", Body: "Alice likes tea.[^1]", Citations: []noemamemory.Citation{{Sources: []string{human.ID, assistant.ID}}}},
	}}, noemamemory.State{}); err != nil {
		t.Fatal(err)
	}

	server := httptest.NewServer(NewHandler(resolver))
	t.Cleanup(server.Close)
	response := postGraphQL(t, server.URL, `query MemoryRead($pageId: String!) {
  memorySettings { modelPreference { providerKind providerAccountId modelProfile reasoningEffort selectionMode fastMode } modelOptions { providerKind providerAccountId } }
  memoryTree { root { id path title icon body hash citations { sources { source kind excerpt createdAt } } parent ancestors { id path title } children { id path title icon excerpt hash } } pages { id path title icon excerpt hash } pendingCount updateStatus { state active lastConsolidatedSequence lastConsolidatedItem error updatedAt } }
  memoryPage(pageId: $pageId) { id path title icon body hash citations { sources { source kind excerpt createdAt } } parent ancestors { id path title icon excerpt hash } children { id path title icon excerpt hash } }
}`, map[string]any{"pageId": "people/alice.md"})
	if len(response.Errors) != 0 {
		t.Fatalf("Memory GraphQL errors = %#v", response.Errors)
	}
	settings := response.Data["memorySettings"].(map[string]any)
	preference := settings["modelPreference"].(map[string]any)
	if preference["providerKind"] != "openrouter" || len(settings["modelOptions"].([]any)) == 0 {
		t.Fatalf("Memory settings = %#v", settings)
	}
	tree := response.Data["memoryTree"].(map[string]any)
	if tree["pendingCount"] != float64(2) || len(tree["pages"].([]any)) != 3 {
		t.Fatalf("Memory tree = %#v", tree)
	}
	page := response.Data["memoryPage"].(map[string]any)
	if page["parent"] != "people.md" || len(page["ancestors"].([]any)) != 1 {
		t.Fatalf("Memory page = %#v", page)
	}
	citations := page["citations"].([]any)
	source := citations[0].(map[string]any)["sources"].([]any)[0].(map[string]any)
	if source["kind"] != "HUMAN_MESSAGE" || source["excerpt"] != "Alice likes tea." {
		t.Fatalf("Memory citation source = %#v", source)
	}
	unavailable := citations[0].(map[string]any)["sources"].([]any)[1].(map[string]any)
	if unavailable["kind"] != "UNAVAILABLE" || unavailable["createdAt"] == nil {
		t.Fatalf("unavailable Memory citation = %#v", unavailable)
	}

	eventContext, cancel := context.WithCancel(context.Background())
	events, err := resolver.memoryEvents(eventContext)
	if err != nil {
		t.Fatal(err)
	}
	initial := <-events
	if initial.Root == nil || initial.Root.Path != noemamemory.RootPagePath || initial.PendingCount != 2 {
		t.Fatalf("initial Memory event = %#v", initial)
	}
	cancel()
	if _, open := <-events; open {
		t.Fatal("Memory event stream stayed open")
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
	server := httptest.NewServer(NewHandler(resolver))
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
	page, err := resolver.conversationTranscriptPage(ctx, model.ConversationTranscriptPageInput{
		ConversationID: stored.ID,
	})
	if err != nil || len(page.Items) != 0 || page.PageInfo.HasMoreBefore {
		t.Fatalf("empty transcript page = %#v, %v", page, err)
	}

	streamCtx, cancel := context.WithCancel(ctx)
	events, err := resolver.conversationEvents(streamCtx, stored.ID)
	if err != nil {
		t.Fatal(err)
	}
	event := <-events
	ready, ok := event.(model.SubscriptionReadyEvent)
	if !ok || ready.ConversationID != stored.ID {
		t.Fatalf("first conversation event = %#v", event)
	}
	cancel()
	if _, open := <-events; open {
		t.Fatal("conversation event stream stayed open after disconnect")
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
}

func rustAPIPortForeignConversationSubscription(t *testing.T) {
	t.Helper()
	resolver := openChatTestResolver(t)
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	if _, err := resolver.conversationEvents(ctx, "conversation:foreign"); err == nil || !strings.Contains(err.Error(), "conversation") {
		t.Fatalf("foreign conversation subscription = %v", err)
	}
}

func rustAPIPortRecurrenceList(t *testing.T) {
	resolver := openTestResolver(t)
	ctx := context.Background()
	first := time.Now().UTC().Truncate(time.Minute).Add(2 * time.Minute)
	cron := fmt.Sprintf("%d %d * * *", first.Minute(), first.Hour())
	created, err := resolver.captureTask(ctx, model.CaptureTaskInput{WorkspaceID: personalWorkspaceID,
		Title: "Recurring", TaskDocument: "# Recurring\n", ClientMutationID: "capture-recurring",
		Schedule: &model.NewTaskScheduleInput{ScheduledFor: first.Format(time.RFC3339), TimeZone: "UTC",
			Recurrence: &model.NewTaskRecurrenceInput{StartsAt: first.Format(time.RFC3339), CronExpression: cron}}})
	if err != nil || created.Task.Schedule == nil || created.Task.Schedule.RecurrenceID == nil ||
		created.Task.ExecutorBackend != "provider" {
		t.Fatalf("created scheduled Task = %#v, %v", created, err)
	}
	recurrenceID := *created.Task.Schedule.RecurrenceID
	if err := home.DeleteRecurrenceDocument(resolver.home, recurrenceID); err != nil {
		t.Fatal(err)
	}
	replayed, err := resolver.captureTask(ctx, model.CaptureTaskInput{WorkspaceID: personalWorkspaceID,
		Title: "Recurring", TaskDocument: "# Recurring\n", ClientMutationID: "capture-recurring",
		Schedule: &model.NewTaskScheduleInput{ScheduledFor: first.Format(time.RFC3339), TimeZone: "UTC",
			Recurrence: &model.NewTaskRecurrenceInput{StartsAt: first.Format(time.RFC3339), CronExpression: cron}}})
	if err != nil || replayed.Task.TaskID != created.Task.TaskID {
		t.Fatalf("replayed capture = %#v, %v", replayed, err)
	}
	recurrence, err := resolver.taskRecurrence(ctx, recurrenceID, nil)
	if err != nil || recurrence.TaskDocument != "# Recurring\n" || len(recurrence.Occurrences) != 1 {
		t.Fatalf("recurrence = %#v, %v", recurrence, err)
	}
	nextDocument := "# Changed\n"
	updated, err := resolver.updateTaskRecurrence(ctx, model.UpdateTaskRecurrenceInput{
		RecurrenceID: recurrenceID, ExpectedRevision: 1, TaskDocument: &nextDocument,
		ExpectedTaskDocumentDigest: &recurrence.TaskDocumentDigest, ClientMutationID: "update"})
	if err != nil || updated.ClientMutationID != "update" {
		t.Fatalf("updated recurrence = %#v, %v", updated, err)
	}
	command := model.TaskRecurrenceCommandInput{RecurrenceID: recurrenceID, ExpectedRevision: 2,
		ClientMutationID: "pause"}
	if _, err := resolver.taskRecurrenceLifecycle(ctx, command, store.RecurrencePaused); err != nil {
		t.Fatal(err)
	}
	command.ExpectedRevision, command.ClientMutationID = 3, "resume"
	if _, err := resolver.taskRecurrenceLifecycle(ctx, command, store.RecurrenceActive); err != nil {
		t.Fatal(err)
	}
	command.ExpectedRevision, command.ClientMutationID = 4, "skip"
	if _, err := resolver.skipTaskRecurrenceNext(ctx, command); err != nil {
		t.Fatal(err)
	}
	if _, err := resolver.runScheduledTaskNow(ctx, model.RunScheduledTaskNowInput{TaskID: created.Task.TaskID,
		ExpectedRevision: created.Task.Revision, ExpectedGeneration: 1, ClientMutationID: "run-first"}); err != nil {
		t.Fatal(err)
	}
	started, err := resolver.Store.StartTask(ctx, created.Task.TaskID, "run:test", time.Now())
	if err != nil {
		t.Fatal(err)
	}
	if _, err := resolver.Store.FinishTask(ctx, created.Task.TaskID, started.CurrentRunID,
		store.TaskCompleted, time.Now()); err != nil {
		t.Fatal(err)
	}
	command.ExpectedRevision, command.ClientMutationID = 5, "run-extra"
	manualCommand, err := newTaskCommand("run_task_recurrence_now", command.ClientMutationID, command)
	if err != nil {
		t.Fatal(err)
	}
	manualTaskID, _ := store.NewTaskID()
	if err := home.StageRecurrenceDocumentToTask(resolver.home, recurrenceID, manualTaskID); err != nil {
		t.Fatal(err)
	}
	if _, err := resolver.Store.RunTaskRecurrenceNow(ctx, recurrenceID, manualTaskID,
		int64(command.ExpectedRevision), manualCommand, time.Now()); err != nil {
		t.Fatal(err)
	}
	manual, err := resolver.runTaskRecurrenceNow(ctx, command)
	if err != nil || manual.Task.TaskDocument != nextDocument || manual.Task.TaskID == created.Task.TaskID {
		t.Fatalf("manual occurrence = %#v, %v", manual, err)
	}
	replayedManual, err := resolver.runTaskRecurrenceNow(ctx, command)
	if err != nil || replayedManual.Task.TaskID != manual.Task.TaskID {
		t.Fatalf("replayed manual occurrence = %#v, %v", replayedManual, err)
	}
	listed, err := resolver.taskRecurrences(ctx, personalWorkspaceID, nil, nil)
	if err != nil || len(listed) != 1 || listed[0].Title != created.Task.Title {
		t.Fatalf("listed recurrences = %#v, %v", listed, err)
	}
	oneTime, err := resolver.captureTask(ctx, model.CaptureTaskInput{WorkspaceID: personalWorkspaceID,
		Title: "One time", TaskDocument: "# One time\n", ClientMutationID: "capture-once"})
	if err != nil {
		t.Fatal(err)
	}
	scheduled, err := resolver.setTaskSchedule(ctx, model.ScheduleTaskInput{TaskID: oneTime.Task.TaskID,
		ExpectedRevision: 1, ExpectedGeneration: 1, ClientMutationID: "schedule-once",
		Schedule: &model.NewTaskScheduleInput{ScheduledFor: first.Format(time.RFC3339), TimeZone: "UTC"}}, false)
	if err != nil || scheduled.Task.Schedule == nil {
		t.Fatalf("scheduled one-time Task = %#v, %v", scheduled, err)
	}
	second := first.Add(time.Hour)
	rescheduled, err := resolver.setTaskSchedule(ctx, model.ScheduleTaskInput{TaskID: oneTime.Task.TaskID,
		ExpectedRevision: 2, ExpectedGeneration: 1, ClientMutationID: "reschedule-once",
		Schedule: &model.NewTaskScheduleInput{ScheduledFor: second.Format(time.RFC3339), TimeZone: "UTC"}}, true)
	if err != nil || rescheduled.Task.Schedule.ScheduledFor != second.Format(time.RFC3339) {
		t.Fatalf("rescheduled one-time Task = %#v, %v", rescheduled, err)
	}
	unscheduled, err := resolver.unscheduleTask(ctx, model.UnscheduleTaskInput{TaskID: oneTime.Task.TaskID,
		ExpectedRevision: 3, ExpectedGeneration: 1, ClientMutationID: "unschedule-once"})
	if err != nil || unscheduled.Task.Schedule != nil {
		t.Fatalf("unscheduled Task = %#v, %v", unscheduled, err)
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
	checks := []func() error{
		func() error {
			_, err := resolver.tasks(context.Background(), model.TaskListInput{WorkspaceID: personalWorkspaceID}, nil, nil)
			return err
		},
		func() error { _, err := resolver.projectDocument(context.Background(), "project:missing"); return err },
	}
	for index, check := range checks {
		if err := check(); err == nil || err.Error() != "request is unauthenticated" {
			t.Fatalf("unauthenticated Task read %d = %v", index, err)
		}
	}
}

func rustAPIPortMalformedTaskCursor(t *testing.T) {
	t.Helper()
	resolver := openTestResolver(t)
	if _, err := resolver.projects(context.Background(), personalWorkspaceID, nil, nil, stringPtr("malformed")); err == nil {
		t.Fatal("malformed project cursor test setup unexpectedly succeeded")
	}
	if _, _, err := parseEventCursor(stringPtr("malformed")); err == nil {
		t.Fatal("malformed Task cursor was accepted")
	}
}

func rustAPIPortTaskPageBounds(t *testing.T) {
	t.Helper()
	resolver := openTestResolver(t)
	for _, first := range []int{0, 101} {
		value := first
		_, err := resolver.projects(context.Background(), personalWorkspaceID, nil, &value, nil)
		var graphQLError *gqlerror.Error
		if !errors.As(err, &graphQLError) || graphQLError.Message != "invalid task cursor" || graphQLError.Extensions["code"] != "invalid_cursor" {
			t.Fatalf("project page bound %d = %v", first, err)
		}
	}
}

func rustAPIPortTaskAuthorization(t *testing.T) {
	t.Helper()
	resolver := openTestResolver(t)
	for _, value := range []string{"malformed", "task:missing"} {
		if _, err := resolver.task(context.Background(), value); err == nil {
			t.Fatalf("foreign Task %q was readable", value)
		}
	}
	if _, err := resolver.queueTask(context.Background(), model.QueueTaskInput{TaskID: "malformed", ExpectedRevision: 1, ExpectedGeneration: 1, ClientMutationID: "foreign-queue"}); err == nil {
		t.Fatal("foreign Task queue was accepted")
	}
}

func rustAPIPortTaskScopeConflicts(t *testing.T) {
	t.Helper()
	resolver := openTestResolver(t)
	behavior := model.WorkflowStageBehaviorTerminalSuccess
	_, err := resolver.tasks(context.Background(), model.TaskListInput{WorkspaceID: personalWorkspaceID, Scope: model.TaskScopeActive, StageBehaviors: []model.WorkflowStageBehavior{behavior}}, nil, nil)
	var graphQLError *gqlerror.Error
	if !errors.As(err, &graphQLError) || graphQLError.Message != "workflow filter is inconsistent" || graphQLError.Extensions["code"] != "workflow_mismatch" {
		t.Fatalf("semantic stage conflict = %v", err)
	}
}

func rustAPIPortTaskIdempotencyRequired(t *testing.T) {
	t.Helper()
	resolver := openTestResolver(t)
	if _, err := resolver.captureTask(context.Background(), model.CaptureTaskInput{WorkspaceID: personalWorkspaceID, Title: "Missing key"}); err == nil || !strings.Contains(err.Error(), "clientMutationId") {
		t.Fatalf("missing Task idempotency key = %v", err)
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
		_, err := resolver.captureTask(context.Background(), model.CaptureTaskInput{WorkspaceID: personalWorkspaceID, Title: "Whitespace key", ClientMutationID: key})
		if err == nil || !strings.Contains(err.Error(), "invalid clientMutationId") {
			t.Fatalf("whitespace Task key %q = %v", key, err)
		}
	}
}

func rustAPIPortCaptureProjection(t *testing.T) {
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
	wantEventCursor, _ := store.EncodeWorkEventCursor(1)
	if capture["clientMutationId"] != "capture-1" || capture["eventCursor"] != wantEventCursor {
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

func rustAPIPortTaskWorkspace(t *testing.T) {
	t.Helper()
	resolver := openTestResolver(t)
	ctx := context.Background()
	captured, err := resolver.captureTask(ctx, model.CaptureTaskInput{WorkspaceID: personalWorkspaceID, Title: "Inspect workspace", TaskDocument: "Current Task", ClientMutationID: "capture-workspace-files"})
	if err != nil {
		t.Fatal(err)
	}
	if err := home.WriteTaskFile(resolver.home, captured.Task.TaskID, "notes/progress.md", "Nested progress"); err != nil {
		t.Fatal(err)
	}
	files, err := taskWorkspaceFiles(resolver.home, captured.Task.TaskID)
	if err != nil {
		t.Fatal(err)
	}
	nested, err := home.ListTaskFiles(resolver.home, captured.Task.TaskID, "notes")
	if err != nil {
		t.Fatal(err)
	}
	if !workspaceHas(files, "TASK.md") || !workspaceHas(files, "notes") || !workspaceHas(nested, "notes/progress.md") && !workspaceHas(nested, "progress.md") {
		t.Fatalf("workspace files = %#v", files)
	}
	file, err := resolver.taskWorkspaceFile(ctx, captured.Task.TaskID, "notes/progress.md")
	if err != nil || file.Content != "Nested progress" {
		t.Fatalf("workspace read = %#v, %v", file, err)
	}
	if _, err := resolver.taskWorkspaceFile(ctx, captured.Task.TaskID, "../outside.md"); err == nil {
		t.Fatal("unsafe workspace read succeeded")
	}
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
	project, err := r.createProject(ctx, model.CreateProjectInput{
		WorkspaceID: personalWorkspaceID, Name: "Attention", ClientMutationID: "attention-project",
	})
	if err != nil {
		t.Fatal(err)
	}
	projectID := project.Project.ProjectID
	clarificationID := openAttentionTask(t, r, "Clarify", &projectID, "clarification", strings.Repeat("界", 401))
	openAttentionTask(t, r, "Approve", &projectID, "approval", "Approve this action?")
	openAttentionTask(t, r, "Recover", nil, "recovery", "The run failed.")

	detail, err := r.task(ctx, clarificationID)
	if err != nil || detail.Attention == nil || detail.Attention.Task.TaskID != clarificationID ||
		len([]rune(detail.Attention.Summary)) != 400 {
		t.Fatalf("Task detail attention = %#v, %v", detail, err)
	}
	interventions, err := r.pendingHumanInterventions(ctx, nil, &clarificationID, nil, nil)
	if err != nil || len(interventions) != 1 {
		t.Fatalf("Task response controls = %#v, %v", interventions, err)
	}
	attention, ok := interventions[0].(*model.TaskAttention)
	if !ok || attention.Gate.Prompt != strings.Repeat("界", 401) || len(attention.ValidActions) == 0 {
		t.Fatal("Task question or response actions are missing")
	}
	listed, err := r.tasks(ctx, model.TaskListInput{WorkspaceID: personalWorkspaceID,
		ProjectID: &projectID, AttentionOnly: true, Scope: model.TaskScopeActive}, nil, nil)
	if err != nil || len(listed.Edges) != 2 {
		t.Fatalf("attention Task list = %#v, %v", listed, err)
	}
	for _, edge := range listed.Edges {
		if edge.Node.Attention == nil || edge.Node.Attention.Task.TaskID != edge.Node.TaskID {
			t.Fatalf("Task summary attention = %#v", edge.Node)
		}
	}

	first := 1
	root := &queryRootResolver{r}
	pageOne, err := root.NeedsYou(ctx, personalWorkspaceID, &projectID, &first, nil)
	if err != nil || len(pageOne.Edges) != 1 || !pageOne.PageInfo.HasNextPage || pageOne.PageInfo.EndCursor == nil {
		t.Fatalf("first Needs You page = %#v, %v", pageOne, err)
	}
	pageTwo, err := root.NeedsYou(ctx, personalWorkspaceID, &projectID, &first, pageOne.PageInfo.EndCursor)
	if err != nil || len(pageTwo.Edges) != 1 || pageTwo.PageInfo.HasNextPage ||
		pageTwo.Edges[0].Node.Task.Project.ProjectID != projectID {
		t.Fatalf("second Needs You page = %#v, %v", pageTwo, err)
	}
}

func rustAPIPortTaskMutationReplay(t *testing.T) {
	r := readyAgentTestResolver(t)
	ctx := context.Background()
	captured, err := r.captureTask(ctx, model.CaptureTaskInput{
		WorkspaceID: personalWorkspaceID, Title: "Draft", TaskDocument: "First",
		ExecutorAgentID: stringAddress("agent:task-executor"), ClientMutationID: "capture-stale-stage",
	})
	if err != nil {
		t.Fatal(err)
	}
	first := model.UpdateInboxTaskInput{
		TaskID: captured.Task.TaskID, ExpectedRevision: 1, ExpectedGeneration: 1,
		TaskDocument: stringAddress("Second"), ExpectedTaskDocumentDigest: &captured.Task.TaskDocumentDigest,
		ClientMutationID: "first-staged-edit",
	}
	command, err := newTaskCommand("update_inbox_task", first.ClientMutationID, first)
	if err != nil {
		t.Fatal(err)
	}
	stage, err := home.PrepareTaskDocumentReplace(r.home, first.TaskID, *first.ExpectedTaskDocumentDigest, *first.TaskDocument, command.RequestDigest)
	if err != nil {
		t.Fatal(err)
	}
	committed, err := r.Store.UpdateInboxTask(ctx, first.TaskID, 1, 1, store.TaskUpdate{DocumentDigest: stage.Document.Digest}, command, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	newer, err := r.updateInboxTask(ctx, model.UpdateInboxTaskInput{
		TaskID: first.TaskID, ExpectedRevision: 2, ExpectedGeneration: 1,
		TaskDocument: stringAddress("Third"), ExpectedTaskDocumentDigest: first.ExpectedTaskDocumentDigest,
		ClientMutationID: "newer-edit",
	})
	if err != nil {
		t.Fatal(err)
	}
	replay, err := r.updateInboxTask(ctx, first)
	current, readErr := home.ReadTaskDocument(r.home, first.TaskID)
	wantCursor, _ := store.EncodeWorkEventCursor(committed.Event.ID)
	if err != nil || readErr != nil || replay.EventCursor != wantCursor || current.Content != "Third" || newer.Task.Revision != 3 {
		t.Fatalf("stale replay = %#v, current = %#v, %v, %v", replay, current, err, readErr)
	}
}

func rustAPIPortTaskSubscription(t *testing.T) {
	resolver := openTestResolver(t)
	ctx := context.Background()
	captured, err := resolver.captureTask(ctx, model.CaptureTaskInput{WorkspaceID: personalWorkspaceID,
		Title: "Shared cursor", TaskDocument: "Task", ClientMutationID: "shared-task"})
	if err != nil {
		t.Fatal(err)
	}
	if _, err := resolver.createProject(ctx, model.CreateProjectInput{WorkspaceID: personalWorkspaceID,
		Name: "Interleaved", ClientMutationID: "shared-project"}); err != nil {
		t.Fatal(err)
	}
	if _, err := resolver.Store.StartTask(ctx, captured.Task.TaskID, "run:shared", time.Now()); err != nil {
		t.Fatal(err)
	}
	after := captured.EventCursor
	taskCtx, cancelTask := context.WithCancel(ctx)
	stream, err := resolver.taskEvents(taskCtx, captured.Task.TaskID, &after)
	if err != nil {
		t.Fatal(err)
	}
	event := <-stream
	wantThird, _ := store.EncodeWorkEventCursor(3)
	if event.Kind != "task.started" || event.Cursor != wantThird {
		t.Fatalf("Task event = %#v", event)
	}
	cancelTask()
	workspaceCtx, cancelWorkspace := context.WithCancel(ctx)
	all, err := resolver.tasksEvents(workspaceCtx, personalWorkspaceID, &after)
	if err != nil {
		t.Fatal(err)
	}
	first, second := <-all, <-all
	if first.ProjectID == nil || second.TaskID == nil || wantThird != second.Cursor {
		t.Fatalf("workspace events = %#v, %#v", first, second)
	}
	liveCtx, cancelLive := context.WithCancel(ctx)
	live, err := resolver.taskEvents(liveCtx, captured.Task.TaskID, nil)
	if err != nil {
		t.Fatal(err)
	}
	select {
	case historical := <-live:
		t.Fatalf("cursor-free subscription replayed history: %#v", historical)
	case <-time.After(25 * time.Millisecond):
	}
	if _, err := resolver.Store.FinishTask(ctx, captured.Task.TaskID, "run:shared", store.TaskCompleted, time.Now()); err != nil {
		t.Fatal(err)
	}
	if completed := <-live; completed.Kind != "task.completed" {
		t.Fatalf("live runtime event = %#v", completed)
	}
	cancelLive()
	if workspaceCompleted := <-all; workspaceCompleted.Kind != "task.completed" {
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
		event := <-all
		sequence, err := store.DecodeWorkEventCursor(event.Cursor)
		if err != nil || sequence != lastSequence+1 {
			t.Fatalf("backpressure event %d = %#v, %v", index, event, err)
		}
		lastSequence = sequence
	}
	cancelWorkspace()
}

func rustAPIPortAuthoritativeU64(t *testing.T) {
	t.Helper()
	if debugInt(math.MaxInt64) != int(math.MaxInt64) {
		t.Fatalf("authoritative i64 maximum changed: %d", debugInt(math.MaxInt64))
	}
	// The Rust projection rejects i64::MAX+1 and u64::MAX. Go currently has no
	// error-returning projection and saturates those values at the same maximum.
	if debugInt(math.MaxInt64) == int(math.MaxInt64) {
		t.Fatalf("authoritative u64 projection saturated instead of rejecting overflow")
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
			Label:         &label,
		},
	)
	if err != nil {
		t.Fatal(err)
	}
	if updated.Label == nil || *updated.Label != "Routine" {
		t.Fatalf("updated pool entry = %#v", updated)
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
	if err := service.QueueTaskAttention(context.Background(), "preview:one", "## **Ready**", "> Use `cargo check`\n\n- first\n- second", "task:11111111111111111111111111111112", "/tasks/11111111111111111111111111111112"); err != nil {
		t.Fatal(err)
	}
	claimed, err := resolver.Store.ClaimDueAPNSDelivery(context.Background(), time.Now().Add(time.Minute))
	if err != nil || claimed == nil {
		t.Fatalf("notification preview = %#v, %v", claimed, err)
	}
	if claimed.Notification.Title != "Ready" || claimed.Notification.Body != "Use cargo check first second" {
		t.Fatalf("notification preview text = %#v", claimed.Notification)
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
	executable, _ := os.Executable()
	resolver.WebTools, _ = webtool.New(resolver.Store, resolver.ProviderAccounts, nil, nil, t.TempDir(), executable, 2, 1024)
	settings, err := resolver.webToolSettings(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if settings.Search.ActiveProviderAccountID != "provider_account:duckduckgo_public:system" ||
		settings.Fetch.ActiveProviderAccountID != "provider_account:direct_http:system" ||
		settings.Browse.ActiveProviderAccountID != "provider_account:obscura:system" {
		t.Fatalf("unexpected defaults: %#v", settings)
	}
	native := &provider.Account{ID: "provider_account:openai:default", ProviderKind: "openai", AccountKey: "default", DisplayName: "OpenAI"}
	nativeSearch, _ := resolver.webBindingSettings(context.Background(), "web.search", nil, native, true)
	nativeFetch, _ := resolver.webBindingSettings(context.Background(), "web.fetch", nil, native, true)
	if option := nativeSearch.ProviderOptions[0]; option.DataFlowClass != "trusted_external_search_query" || !option.Citations {
		t.Fatalf("native search metadata = %#v", option)
	}
	if option := nativeFetch.ProviderOptions[0]; option.DataFlowClass != "external_web_fetch" || option.Citations || !option.DirectURLFetch {
		t.Fatalf("native fetch metadata = %#v", option)
	}
	secret, _ := provider.NewSecret("exa-secret")
	exa, err := resolver.ProviderAccounts.CreateSecretAccount(context.Background(), "exa", "Exa", secret, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	saved, err := resolver.saveWebToolProviderBinding(context.Background(), model.SaveWebToolProviderBindingInput{
		ToolName: "web.search", CapabilityID: "web.search", ProviderAccountID: exa.ID,
	})
	if err != nil || saved.ActiveProviderAccountID != exa.ID {
		t.Fatalf("saved = %#v, %v", saved, err)
	}
	if _, err := resolver.saveWebToolProviderBinding(context.Background(), model.SaveWebToolProviderBindingInput{
		ToolName: "web.search", CapabilityID: "web.fetch", ProviderAccountID: exa.ID,
	}); err == nil {
		t.Fatal("mismatched tool and capability accepted")
	}
	browse, err := resolver.saveBrowserProviderRoute(context.Background(), model.SaveBrowserProviderRouteInput{
		ProviderAccountIds: []string{"provider_account:obscura:system"},
	})
	if err != nil || len(browse.ProviderRouteAccountIds) != 1 {
		t.Fatalf("browse = %#v, %v", browse, err)
	}
	kernelSecret, _ := provider.NewSecret("kernel-secret")
	kernel, err := resolver.ProviderAccounts.CreateSecretAccount(context.Background(), "kernel", "Kernel", kernelSecret, time.Now())
	if err != nil || resolver.Store.SaveBrowserProviderRoute(context.Background(), []string{kernel.ID}, time.Now()) != nil {
		t.Fatalf("prepare prior route: %v", err)
	}
	resolver.WebTools = nil
	if _, err := resolver.saveBrowserProviderRoute(context.Background(), model.SaveBrowserProviderRouteInput{
		ProviderAccountIds: []string{"provider_account:obscura:system"},
	}); err == nil {
		t.Fatal("unavailable installer was accepted")
	}
	route, _ := resolver.Store.WebProviderRoute(context.Background(), "web.browse")
	if len(route) != 1 || route[0].ProviderAccountID != kernel.ID {
		t.Fatalf("prior route changed: %#v", route)
	}
}

func rustAPIPortWebToolFiltering(t *testing.T) {
	resolver := openProviderTestResolver(t)
	executable, _ := os.Executable()
	resolver.WebTools, _ = webtool.New(resolver.Store, resolver.ProviderAccounts, nil, nil, t.TempDir(), executable, 2, 1024)
	settings, err := resolver.webToolSettings(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if settings.Search.ActiveProviderAccountID != "provider_account:duckduckgo_public:system" ||
		settings.Fetch.ActiveProviderAccountID != "provider_account:direct_http:system" ||
		settings.Browse.ActiveProviderAccountID != "provider_account:obscura:system" {
		t.Fatalf("unexpected defaults: %#v", settings)
	}
	native := &provider.Account{ID: "provider_account:openai:default", ProviderKind: "openai", AccountKey: "default", DisplayName: "OpenAI"}
	nativeSearch, _ := resolver.webBindingSettings(context.Background(), "web.search", nil, native, true)
	nativeFetch, _ := resolver.webBindingSettings(context.Background(), "web.fetch", nil, native, true)
	if option := nativeSearch.ProviderOptions[0]; option.DataFlowClass != "trusted_external_search_query" || !option.Citations {
		t.Fatalf("native search metadata = %#v", option)
	}
	if option := nativeFetch.ProviderOptions[0]; option.DataFlowClass != "external_web_fetch" || option.Citations || !option.DirectURLFetch {
		t.Fatalf("native fetch metadata = %#v", option)
	}
	secret, _ := provider.NewSecret("exa-secret")
	exa, err := resolver.ProviderAccounts.CreateSecretAccount(context.Background(), "exa", "Exa", secret, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	saved, err := resolver.saveWebToolProviderBinding(context.Background(), model.SaveWebToolProviderBindingInput{
		ToolName: "web.search", CapabilityID: "web.search", ProviderAccountID: exa.ID,
	})
	if err != nil || saved.ActiveProviderAccountID != exa.ID {
		t.Fatalf("saved = %#v, %v", saved, err)
	}
	if _, err := resolver.saveWebToolProviderBinding(context.Background(), model.SaveWebToolProviderBindingInput{
		ToolName: "web.search", CapabilityID: "web.fetch", ProviderAccountID: exa.ID,
	}); err == nil {
		t.Fatal("mismatched tool and capability accepted")
	}
	browse, err := resolver.saveBrowserProviderRoute(context.Background(), model.SaveBrowserProviderRouteInput{
		ProviderAccountIds: []string{"provider_account:obscura:system"},
	})
	if err != nil || len(browse.ProviderRouteAccountIds) != 1 {
		t.Fatalf("browse = %#v, %v", browse, err)
	}
	kernelSecret, _ := provider.NewSecret("kernel-secret")
	kernel, err := resolver.ProviderAccounts.CreateSecretAccount(context.Background(), "kernel", "Kernel", kernelSecret, time.Now())
	if err != nil || resolver.Store.SaveBrowserProviderRoute(context.Background(), []string{kernel.ID}, time.Now()) != nil {
		t.Fatalf("prepare prior route: %v", err)
	}
	resolver.WebTools = nil
	if _, err := resolver.saveBrowserProviderRoute(context.Background(), model.SaveBrowserProviderRouteInput{
		ProviderAccountIds: []string{"provider_account:obscura:system"},
	}); err == nil {
		t.Fatal("unavailable installer was accepted")
	}
	route, _ := resolver.Store.WebProviderRoute(context.Background(), "web.browse")
	if len(route) != 1 || route[0].ProviderAccountID != kernel.ID {
		t.Fatalf("prior route changed: %#v", route)
	}
}
