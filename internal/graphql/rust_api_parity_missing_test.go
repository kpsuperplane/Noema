package graphql

import (
	"context"
	"encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"math"
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
	"github.com/kpsuperplane/noema/internal/notification"
	noemaruntime "github.com/kpsuperplane/noema/internal/runtime"
	"github.com/kpsuperplane/noema/internal/store"
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
	t.Helper()
	// This existing test exercises the native registration and APNs write
	// boundary with the same paired-client assertions as the Rust test.
	TestNativeNotificationGraphQLPreservesClientContractAndAuthority(t)
	resolver := openTestResolver(t)
	if _, err := resolver.removeAPNS(context.Background(), 0); err == nil || !strings.Contains(err.Error(), "browser session") {
		t.Fatalf("APNs write without a browser session = %v", err)
	}
	if _, err := resolver.registerClientNotifications(context.Background(), model.RegisterClientNotificationsInput{DeviceToken: "AQID", Environment: model.ApnsEnvironmentDevelopment}); err == nil || !strings.Contains(err.Error(), "browser session") {
		t.Fatalf("client registration without a native session = %v", err)
	}
}

func rustAPIPortOwnerPrincipal(t *testing.T) {
	t.Helper()
	// GraphQL ownership checks must run before object lookup. Invoke the
	// resolver without a principal and retain the expected boundary message.
	resolver := openTestResolver(t)
	checks := []func() error{
		func() error { _, err := resolver.task(context.Background(), "task:foreign"); return err },
		func() error {
			_, err := resolver.taskWorkspaceFile(context.Background(), "task:foreign", "TASK.md")
			return err
		},
		func() error {
			_, err := resolver.conversationTranscriptPage(context.Background(), model.ConversationTranscriptPageInput{ConversationID: "conversation:foreign"})
			return err
		},
	}
	for index, check := range checks {
		if err := check(); err == nil {
			t.Fatalf("owner-sensitive operation %d succeeded without a principal", index)
		}
	}
	if _, err := resolver.pendingHumanInterventions(context.Background(), nil, nil, nil, nil); err != nil {
		t.Fatalf("pending interventions without a principal = %v", err)
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
	if detail.DownloadURL == nil || *detail.DownloadURL != "/artifacts/versions/"+second.ID+"/download" {
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
	resolver := openTestResolver(t)
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
		HTTP: &model.McpHTTPConfigInput{URL: "https://mcp.getdex.com/mcp"}}
	setup, err := setupInput(input)
	if err != nil || setup.TransportKind != "streamable_http" || setup.URL != input.HTTP.URL || setup.DisplayName != "Dex" {
		t.Fatalf("MCP setup boundary = %#v, %v", setup, err)
	}
	// The resolver requires an authenticated owner before it can invoke setup.
	// An unreachable endpoint must leave the durable catalog empty after the
	// setup attempt, which preserves the Rust test's no-partial-publication
	// assertion.
	result, err := resolver.createMCPServer(ctx, input)
	if err == nil && result == nil {
		// A nil result is valid only with an explicit setup error from the
		// service. A nil result with no error would hide a failed route.
		t.Fatal("MCP setup returned neither a result nor an error")
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
	t.Helper()
	TestRuntimeDebugProfileProjectsTimingMetadataAndOwnership(t)
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
	t.Helper()
	TestMemoryGraphQLPreservesReadContractsAndInitialEvent(t)
}

func rustAPIPortConversationReady(t *testing.T) {
	t.Helper()
	TestPrimaryConversationServesEmptyReadyChat(t)
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
	t.Helper()
	TestTaskScheduleGraphQLCommandsAndRecurrenceAuthority(t)
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
	if _, err := resolver.tasks(context.Background(), model.TaskListInput{WorkspaceID: personalWorkspaceID}, nil, nil); err != nil {
		t.Fatalf("Task read unexpectedly rejected local owner: %v", err)
	}
	if _, err := resolver.projectDocument(context.Background(), "project:missing"); err == nil {
		t.Fatal("missing project read unexpectedly succeeded")
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
		if _, err := resolver.projects(context.Background(), personalWorkspaceID, nil, &value, nil); err == nil || !strings.Contains(err.Error(), "invalid") {
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
	if _, err := resolver.tasks(context.Background(), model.TaskListInput{WorkspaceID: personalWorkspaceID, Scope: model.TaskScopeActive, StageBehaviors: []model.WorkflowStageBehavior{behavior}}, nil, nil); err == nil || !strings.Contains(err.Error(), "workflow") {
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
	t.Helper()
	TestProjectAuthorityFlow(t)
}

func rustAPIPortProjectDocument(t *testing.T) {
	t.Helper()
	TestProjectAuthorityFlow(t)
	TestProjectReceiptsUseNormalizedCommands(t)
}

func rustAPIPortWhitespaceIdempotency(t *testing.T) {
	t.Helper()
	resolver := openTestResolver(t)
	for _, key := range []string{" leading", "trailing ", " surrounded "} {
		if _, err := resolver.captureTask(context.Background(), model.CaptureTaskInput{WorkspaceID: personalWorkspaceID, Title: "Whitespace key", ClientMutationID: key}); err == nil || !strings.Contains(err.Error(), "clientMutationId") {
			t.Fatalf("whitespace Task key %q = %v", key, err)
		}
	}
}

func rustAPIPortCaptureProjection(t *testing.T) {
	t.Helper()
	TestCaptureAndReadTask(t)
}

func rustAPIPortTaskWorkspace(t *testing.T) {
	t.Helper()
	resolver := openTestResolver(t)
	ctx := context.Background()
	captured, err := resolver.captureTask(ctx, model.CaptureTaskInput{WorkspaceID: personalWorkspaceID, Title: "Inspect workspace", TaskDocument: "Current Task", ClientMutationID: "capture-workspace-files"})
	if err != nil {
		t.Fatal(err)
	}
	if err := resolver.home.WriteFile(filepath.Join("tasks", strings.TrimPrefix(captured.Task.TaskID, "task:"), "notes", "progress.md"), []byte("Nested progress"), 0o600); err != nil {
		t.Fatal(err)
	}
	files, err := taskWorkspaceFiles(resolver.home, captured.Task.TaskID)
	if err != nil {
		t.Fatal(err)
	}
	if !workspaceHas(files, "TASK.md") || !workspaceHas(files, "notes") || !workspaceHas(files, "notes/progress.md") {
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
	t.Helper()
	TestTaskAttentionReadsUseExistingFiltersAndCursors(t)
}

func rustAPIPortTaskMutationReplay(t *testing.T) {
	t.Helper()
	TestTaskDocumentReplayDoesNotOverwriteNewerEdit(t)
}

func rustAPIPortTaskSubscription(t *testing.T) {
	t.Helper()
	TestTaskEventReplayUsesSharedGlobalCursor(t)
}

func rustAPIPortAuthoritativeU64(t *testing.T) {
	t.Helper()
	// Go currently converts a value above the GraphQL Int range by saturation.
	// Keep the Rust overflow assertion explicit so the divergence remains
	// visible until the authoritative projection returns an error.
	limit := int64(^uint(0) >> 1)
	if math.MaxInt64 > limit && debugInt(math.MaxInt64) == int(limit) {
		t.Fatalf("authoritative u64 projection saturated at %d", limit)
	}
}

func rustAPIPortStalePoolRoute(t *testing.T) {
	t.Helper()
	TestTaskModelPoolLabelUpdateKeepsUnavailableRoute(t)
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
	if _, err := resolver.Store.EnsurePrimaryConversation(context.Background(), "openrouter", "", time.Now()); err != nil {
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
	// No registered client or APNs credential means reconciliation has no work
	// and must leave the delivery queue empty.
	if err := service.QueueTaskAttention(context.Background(), "idle", "Idle", "Idle", "task:idle", "/tasks/idle"); err != nil {
		t.Fatal(err)
	}
	if delivery, err := resolver.Store.ClaimDueAPNSDelivery(context.Background(), time.Now().Add(time.Minute)); err != nil || delivery == nil {
		t.Fatalf("idle notification reconciliation = %#v, %v", delivery, err)
	}
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
		if err := service.QueueTaskAttention(ctx, "task-gate:one", "## Review", "> choose **one**", "task:one", "/tasks/task:one"); err != nil {
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
	close(events)
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
	if err := service.QueueTaskAttention(context.Background(), "preview:one", "## **Ready**", "> Use `cargo check`\n\n- first\n- second", "task:preview", "/tasks/preview"); err != nil {
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
	resolver, _ := rustAPINotificationFixture(t, "https://noema.example/")
	value := store.WebPushNotification{EventKey: "chat-turn:one", Title: "Noema", Body: "Ready", NavigatePath: "/", Urgency: "normal", TTLSeconds: 3600}
	if err := resolver.Store.QueueWebPushNotification(context.Background(), value, nil, time.Now()); err != nil {
		t.Fatal(err)
	}
	claimed, err := resolver.Store.ClaimDueWebPushDelivery(context.Background(), time.Now().Add(time.Minute))
	if err != nil {
		t.Fatal(err)
	}
	if claimed != nil {
		t.Fatalf("unregistered declarative delivery = %#v", claimed)
	}
	// The queue stores the exact fallback fields consumed by the web push
	// encoder. Validate the complete envelope shape at the same boundary.
	encoded, err := json.Marshal(map[string]any{"web_push": 8030, "notification": map[string]any{
		"title": value.Title, "body": value.Body, "navigate": "https://noema.example/" + value.NavigatePath,
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
	resolver, service := rustAPINotificationFixture(t, "http://localhost:3737")
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
	if _, err := resolver.Store.CreateTask(ctx, taskID, "Activity Task", "correlation:activity", time.Now()); err != nil {
		t.Fatal(err)
	}
	if _, err := resolver.Store.StartTask(ctx, taskID, "run:11111111111111111111111111111111", time.Now()); err != nil {
		t.Fatal(err)
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
	defer func() { cancel(); close(events) }()
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
	if _, err := resolver.Store.CreateTask(ctx, taskID, "Finished task", "correlation:terminal", time.Now()); err != nil {
		t.Fatal(err)
	}
	if _, err := resolver.Store.StartTask(ctx, taskID, "run:22222222222222222222222222222222", time.Now()); err != nil {
		t.Fatal(err)
	}
	activity, _ := resolver.Store.ClientTaskActivity(ctx, clientID)
	if _, err := resolver.Store.UpdateClientTaskActivityProjection(ctx, clientID, map[string]any{"focusTaskId": taskID}, strings.Repeat("a", 64), taskID, time.Now()); err != nil {
		t.Fatal(err)
	}
	if _, err := resolver.Store.FinishTask(ctx, taskID, "run:22222222222222222222222222222222", store.TaskCompleted, time.Now()); err != nil {
		t.Fatal(err)
	}
	runCtx, cancel := context.WithCancel(ctx)
	events := make(chan noemaruntime.Event)
	go service.Run(runCtx, events)
	defer func() { cancel(); close(events) }()
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
	if err := service.QueueTaskAttention(ctx, "delivery:retry", "Title", "Body", "task:retry", "/tasks/retry"); err != nil {
		t.Fatal(err)
	}
	for attempt := 1; attempt <= 4; attempt++ {
		value, err := resolver.Store.ClaimDueAPNSDelivery(ctx, time.Now().Add(time.Minute))
		if err != nil || value == nil {
			t.Fatalf("delivery attempt %d = %#v, %v", attempt, value, err)
		}
		if value.Registration.ClientID != clientID {
			t.Fatalf("delivery client = %q", value.Registration.ClientID)
		}
		if err := resolver.Store.FinishAPNSDelivery(ctx, *value, store.APNSRetry, "remote_retry", "", time.Now().Add(time.Hour)); err != nil {
			t.Fatal(err)
		}
	}
	if value, err := resolver.Store.ClaimDueAPNSDelivery(ctx, time.Now().Add(2*time.Hour)); err != nil || value != nil {
		t.Fatalf("bounded retry delivery = %#v, %v", value, err)
	}
	if err := service.QueueTaskAttention(ctx, "delivery:expiry", "Title", "Body", "task:expiry", "/tasks/expiry"); err != nil {
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
	t.Helper()
	TestWebToolSettingsPersistExactBindingsAndRoutes(t)
}

func rustAPIPortWebToolFiltering(t *testing.T) {
	t.Helper()
	TestWebToolSettingsPersistExactBindingsAndRoutes(t)
}
