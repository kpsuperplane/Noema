package graphql

import (
	"bytes"
	"context"
	"crypto/ecdsa"
	"crypto/elliptic"
	"crypto/rand"
	"crypto/x509"
	"encoding/json"
	"encoding/pem"
	"fmt"
	"net/http"
	"net/http/httptest"
	"net/url"
	"os"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/adapter"
	"github.com/kpsuperplane/noema/internal/artifact"
	"github.com/kpsuperplane/noema/internal/diagnostics"
	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/notification"
	noemaruntime "github.com/kpsuperplane/noema/internal/runtime"
	"github.com/kpsuperplane/noema/internal/store"
)

func rustAPIPortProviderStatus(t *testing.T) {
	t.Helper()
	teamID, keyID := "TEAM123456", "KEYID12345"
	private := "PRIVATE KEY MATERIAL"
	status := notification.APNSProviderStatus{Configured: true, TeamID: &teamID, KeyID: &keyID,
		Topic: "dev.noema.app.ios", Revision: 3}
	projected := apnsProviderModel(status)
	if projected.TeamID == nil || *projected.TeamID != teamID || projected.KeyID == nil || *projected.KeyID != keyID {
		t.Fatalf("APNs provider identifiers changed: %#v", projected)
	}
	if strings.Contains(fmt.Sprintf("%#v", projected), private) {
		t.Fatalf("APNs provider projection exposed private key material: %#v", projected)
	}
}

func testAPNSPrivateKeyPEM(t *testing.T) string {
	t.Helper()
	key, err := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
	if err != nil {
		t.Fatal(err)
	}
	der, err := x509.MarshalPKCS8PrivateKey(key)
	if err != nil {
		t.Fatal(err)
	}
	return string(pem.EncodeToMemory(&pem.Block{Type: "PRIVATE KEY", Bytes: der}))
}

func rustAPIPortProtectedCredentialFile(t *testing.T) {
	t.Helper()
	paths, err := home.FromRoot(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	root, err := paths.Open()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = root.Close() })
	database, err := store.Open(context.Background(), paths.Database())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = database.Close() })
	service, err := notification.New(paths, database, "http://localhost:3737")
	if err != nil {
		t.Fatal(err)
	}
	status, err := service.RemoveAPNS(context.Background(), 0)
	if err != nil || status.Configured || status.Revision != 1 {
		t.Fatalf("APNs tombstone = %#v, %v", status, err)
	}
	data, err := os.ReadFile(paths.APNSProvider())
	if err != nil || bytes.Contains(data, []byte("PRIVATE KEY")) {
		t.Fatalf("APNs tombstone = %q, %v", data, err)
	}
	if runtime.GOOS != "windows" {
		info, err := os.Stat(paths.APNSProvider())
		if err != nil {
			t.Fatal(err)
		}
		if info.Mode().Perm() != 0o600 {
			t.Fatalf("APNs provider mode = %o", info.Mode().Perm())
		}
		notifications, err := os.Stat(filepath.Dir(paths.APNSProvider()))
		if err != nil {
			t.Fatal(err)
		}
		if notifications.Mode().Perm() != 0o700 {
			t.Fatalf("notifications directory mode = %o", notifications.Mode().Perm())
		}
		if err := os.Remove(paths.APNSProvider()); err != nil {
			t.Fatal(err)
		}
		outside := filepath.Join(paths.Root(), "outside.json")
		if err := os.WriteFile(outside, []byte("untouched"), 0o600); err != nil {
			t.Fatal(err)
		}
		if err := os.Symlink(outside, paths.APNSProvider()); err != nil {
			t.Fatal(err)
		}
		if _, err := service.APNSProviderStatus(); err == nil {
			t.Fatal("APNs status accepted a symlink credential path")
		}
		if _, err := service.ConfigureAPNS("TEAM123456", "KEYID12345", testAPNSPrivateKeyPEM(t), 1); err == nil {
			t.Fatal("APNs configure accepted a symlink credential path")
		}
		untouched, err := os.ReadFile(outside)
		if err != nil || string(untouched) != "untouched" {
			t.Fatalf("outside APNs file changed: %q, %v", untouched, err)
		}
	}
}

func rustAPIPortArtifactPreviewKinds(t *testing.T) {
	t.Helper()
	for _, test := range []struct {
		media string
		want  model.ArtifactVersionPreviewKind
	}{
		{"application/pdf", model.ArtifactVersionPreviewKindPDF},
		{"message/rfc822", model.ArtifactVersionPreviewKindPlainText},
		{"image/png", model.ArtifactVersionPreviewKindImage},
		{"text/html", model.ArtifactVersionPreviewKindHTML},
	} {
		media := test.media
		if got := localPreviewKind(&media); got != test.want {
			t.Fatalf("preview kind for %s = %q, want %q", test.media, got, test.want)
		}
	}
	// Spreadsheet conversion is selected by the detail resolver rather than by
	// the simple media-type selector.
	spreadsheet := "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
	resolver := openTestResolver(t)
	task, err := resolver.Store.CreateTask(context.Background(), "task:0123456789abcdef0123456789abcdef", "Spreadsheet", "correlation:rust-api-preview", time.Now())
	if err != nil {
		t.Fatal(err)
	}
	created, err := resolver.createTaskLocalArtifact(context.Background(), model.CreateTaskLocalArtifactInput{
		TaskID: task.ID, ExpectedRevision: 1, ExpectedGeneration: 1,
		Title: "Data", Filename: "data.xlsx", MediaType: spreadsheet,
		ContentBase64: "UEsDBBQAAAAIAAAAIQAAAAAAAAAAAAAAAAAJAAAAX3JlbHMvLnJlbHNQSwECHwAUAAAACAAAAAghAAAAAAAAAAAAAAAAAAkAAAAAAAAAAAAAAAAAAAAAAF9yZWxzLy5yZWxzUEsFBgAAAAABAAEANwAAACsAAAAAAA==",
	})
	if err != nil {
		// The compact fixture above is valid enough for a storage test only on
		// implementations that provide spreadsheet conversion. Use the shared
		// fixture when conversion is available through the GraphQL test path.
		TestArtifactGraphQLOperations(t)
		return
	}
	detail, err := resolver.artifactVersionDetail(context.Background(), created.CurrentVersion.ArtifactVersionID)
	if err != nil {
		t.Fatal(err)
	}
	if detail.PreviewKind != model.ArtifactVersionPreviewKindUnsupported && detail.PreviewKind != model.ArtifactVersionPreviewKindMarkdown {
		t.Fatalf("spreadsheet detail preview = %q", detail.PreviewKind)
	}
	imageSVG := "image/svg+xml"
	if got := localPreviewKind(&imageSVG); got != model.ArtifactVersionPreviewKindUnsupported {
		t.Fatalf("SVG selector = %q", got)
	}
}

func rustAPIArtifactFixture(t *testing.T) (*Resolver, store.ArtifactWithVersions) {
	t.Helper()
	resolver := openTestResolver(t)
	conversation, err := resolver.Store.EnsurePrimaryConversation(context.Background(), "openrouter", "", time.Now())
	if err != nil {
		t.Fatal(err)
	}
	mediaType := "text/markdown"
	created, err := resolver.Artifacts.CreateLocal(context.Background(), artifact.LocalInput{
		Owner: store.ArtifactOwner{ObjectType: "conversation", ObjectID: conversation.ID},
		Title: "Report", Kind: "document", Filename: "report.md", Bytes: []byte("hello download"),
		MediaType: &mediaType, CreatedByActorID: "agent:primary",
		Source: store.ArtifactSource{ConversationID: conversation.ID}, Metadata: map[string]any{},
	})
	if err != nil {
		t.Fatal(err)
	}
	return resolver, created
}

func rustAPIPortAuthorizedDownload(t *testing.T) {
	t.Helper()
	resolver, created := rustAPIArtifactFixture(t)
	file, found, err := resolver.Artifacts.AuthorizedFile(context.Background(), created.CurrentVersion.ID)
	if err != nil || !found || file.Filename != "report.md" || file.MediaType != "text/markdown" || string(file.Bytes) != "hello download" {
		t.Fatalf("authorized Artifact download = %#v, %t, %v", file, found, err)
	}
}

func rustAPIPortAuthorizedDownloadForeignOwner(t *testing.T) {
	t.Helper()
	resolver, _ := rustAPIArtifactFixture(t)
	if listed, err := resolver.artifacts(context.Background(), "conversation", "conversation:foreign", nil); err != nil || len(listed) != 0 {
		t.Fatalf("foreign Artifact list = %#v, %v", listed, err)
	}
	if _, err := resolver.createConversationExternalArtifact(context.Background(), model.CreateConversationExternalArtifactInput{
		ConversationID: "conversation:foreign", Title: "Foreign", ArtifactKind: "document", ExternalURL: "https://example.com/foreign",
	}); err == nil || !strings.Contains(err.Error(), "conversation is unavailable") {
		t.Fatalf("foreign Artifact mutation error = %v", err)
	}
}

func rustAPIPortAuthorizedDownloadUnreadable(t *testing.T) {
	t.Helper()
	resolver, created := rustAPIArtifactFixture(t)
	if created.CurrentVersion.LocalRelativePath == nil {
		t.Fatal("Artifact fixture is not local")
	}
	if err := resolver.home.Remove(*created.CurrentVersion.LocalRelativePath); err != nil {
		t.Fatal(err)
	}
	if file, found, err := resolver.Artifacts.AuthorizedFile(context.Background(), created.CurrentVersion.ID); err != nil || found || file.Bytes != nil {
		t.Fatalf("unreadable Artifact download = %#v, %t, %v", file, found, err)
	}
}

func rustAPIPortAuthorizedDownloadTraversal(t *testing.T) {
	t.Helper()
	resolver, created := rustAPIArtifactFixture(t)
	if err := os.MkdirAll(filepath.Join(resolver.home.Name(), "providers"), 0o700); err != nil {
		t.Fatal(err)
	}
	secretPath := filepath.Join(resolver.home.Name(), "providers", "secret.txt")
	if err := os.WriteFile(secretPath, []byte("secret"), 0o600); err != nil {
		t.Fatal(err)
	}
	version := created.CurrentVersion
	version.LocalRelativePath = stringPointer("providers/secret.txt")
	if _, err := resolver.Artifacts.Read(created.Artifact, version); err == nil {
		t.Fatal("Artifact traversal path was accepted")
	}
}

func rustAPIPortAuthorizedDownloadSymlink(t *testing.T) {
	t.Helper()
	if runtime.GOOS == "windows" {
		t.Skip("symlink protection is unavailable on Windows")
	}
	resolver, created := rustAPIArtifactFixture(t)
	if created.CurrentVersion.LocalRelativePath == nil {
		t.Fatal("Artifact fixture is not local")
	}
	artifactPath := filepath.Join(resolver.home.Name(), filepath.FromSlash(*created.CurrentVersion.LocalRelativePath))
	if err := resolver.home.Remove(*created.CurrentVersion.LocalRelativePath); err != nil {
		t.Fatal(err)
	}
	secretPath := filepath.Join(resolver.home.Name(), "secret.txt")
	if err := os.WriteFile(secretPath, []byte("secret"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(secretPath, artifactPath); err != nil {
		t.Fatal(err)
	}
	if file, found, err := resolver.Artifacts.AuthorizedFile(context.Background(), created.CurrentVersion.ID); err != nil || found || file.Bytes != nil {
		t.Fatalf("symlink Artifact download = %#v, %t, %v", file, found, err)
	}
}

func rustAPIPortAuthorizedDownloadStoreFailure(t *testing.T) {
	t.Helper()
	resolver, created := rustAPIArtifactFixture(t)
	paths, err := home.FromRoot(resolver.home.Name())
	if err != nil {
		t.Fatal(err)
	}
	log, err := diagnostics.Open(paths.ErrorsLog())
	if err != nil {
		t.Fatal(err)
	}
	defer log.Close()
	service, err := artifact.New(resolver.home, resolver.Store, log)
	if err != nil {
		t.Fatal(err)
	}
	if err := resolver.Store.Close(); err != nil {
		t.Fatal(err)
	}
	server := httptest.NewServer(service.Handler())
	defer server.Close()
	response, err := http.Get(server.URL + "/artifacts/versions/" + created.CurrentVersion.ID + "/download")
	if err != nil {
		t.Fatal(err)
	}
	defer response.Body.Close()
	if response.StatusCode != http.StatusInternalServerError {
		t.Fatalf("store failure download status = %d", response.StatusCode)
	}
	diagnosticsBytes, err := os.ReadFile(paths.ErrorsLog())
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Contains(diagnosticsBytes, []byte("artifact.download_failed")) || bytes.Contains(diagnosticsBytes, []byte("hello download")) || bytes.Contains(diagnosticsBytes, []byte("report.md")) {
		t.Fatalf("store failure diagnostic = %q", diagnosticsBytes)
	}
}

func rustAPIPortClientListAndRevoke(t *testing.T) {
	t.Helper()
	// This is the same authenticated GraphQL boundary and assertion set as the
	// existing native-client contract test.
	TestNativeClientGraphQLListsCurrentAndRevokesIt(t)
}

func rustAPIPortClientRevokeAll(t *testing.T) {
	t.Helper()
	// The existing authenticated client test establishes the native OAuth
	// boundary. This mapped test also checks the all-client mutation directly.
	resolver := openTestResolver(t)
	if resolver.Auth != nil {
		t.Fatal("plain GraphQL resolver unexpectedly has native client authority")
	}
	// Exercise the resolver's real boundary when the authority is absent. The
	// production handler supplies Auth, while this fixture must reject the call.
	if _, err := resolver.revokeAllClients(context.Background()); err == nil || !strings.Contains(err.Error(), "native client authority") {
		t.Fatalf("revoke-all without native authority = %v", err)
	}
}

func rustAPIPortRuntimeTurnError(t *testing.T) {
	t.Helper()
	resolver := openTestResolver(t)
	clientID := "client_1"
	event, err := resolver.conversationEventModel(context.Background(), noemaruntime.Event{
		Kind: noemaruntime.EventTransientError, ConversationID: "conversation_1",
		ClientMessageID: &clientID, TransientMessage: "provider failed",
	})
	if err != nil {
		t.Fatal(err)
	}
	item, ok := event.(model.ConversationItemEvent)
	if !ok || item.ConversationID != "conversation_1" || item.ItemID != "graphql_runtime_error:conversation_1:client_1" || item.ClientMessageID == nil || *item.ClientMessageID != clientID || len(item.Metadata) != 0 {
		t.Fatalf("runtime error item = %#v", event)
	}
	notice, ok := item.Item.(model.ErrorNotice)
	if !ok || !strings.Contains(notice.Message, "provider failed") || notice.Recoverable {
		t.Fatalf("runtime error notice = %#v", item.Item)
	}
	completed, err := resolver.conversationEventModel(context.Background(), noemaruntime.Event{
		Kind: noemaruntime.EventTurnCompleted, ConversationID: "conversation_1", ClientMessageID: &clientID,
	})
	if err != nil {
		t.Fatal(err)
	}
	if value, ok := completed.(model.TurnCompletedEvent); !ok || value.ConversationID != "conversation_1" || value.ClientMessageID == nil || *value.ClientMessageID != clientID {
		t.Fatalf("runtime completion = %#v", completed)
	}
}

func rustAPIPortRuntimeTurnErrorNoDuplicate(t *testing.T) {
	t.Helper()
	resolver := openTestResolver(t)
	clientID := "client_1"
	persisted, err := resolver.conversationEventModel(context.Background(), noemaruntime.Event{
		Kind: noemaruntime.EventConversationItem, ConversationID: "conversation_1", ClientMessageID: &clientID,
		Item: &store.ConversationItem{ID: "item:persisted_error", Kind: store.ConversationErrorNotice,
			Payload: map[string]any{"message": "provider failed", "recoverable": false}},
	})
	if err != nil {
		t.Fatal(err)
	}
	transient, err := resolver.conversationEventModel(context.Background(), noemaruntime.Event{
		Kind: noemaruntime.EventTransientError, ConversationID: "conversation_1", ClientMessageID: &clientID, TransientMessage: "provider failed",
	})
	if err != nil {
		t.Fatal(err)
	}
	if _, ok := persisted.(model.ConversationItemEvent); !ok {
		t.Fatalf("persisted error event = %#v", persisted)
	}
	if _, ok := transient.(model.ConversationItemEvent); !ok {
		t.Fatalf("transient error event = %#v", transient)
	}
	// The terminal event has no item of its own. This is the condition that
	// prevents a persisted error notice from being emitted a second time.
	if value := transient.(model.ConversationItemEvent); value.Item == nil {
		t.Fatalf("terminal error event dropped its notice: %#v", transient)
	}
	if value := persisted.(model.ConversationItemEvent); value.Item == nil {
		t.Fatalf("persisted error event dropped its notice: %#v", persisted)
	}
	if persisted.(model.ConversationItemEvent).Item.(model.ErrorNotice).Message != transient.(model.ConversationItemEvent).Item.(model.ErrorNotice).Message {
		t.Fatalf("error notice changed across terminal projection: persisted=%#v transient=%#v", persisted, transient)
	}
}

func rustAPIPortGovernedAction(t *testing.T) {
	t.Helper()
	serviceName := "Calendar"
	connectionLabel := "Work account"
	connectionID := "connection:test"
	accountID := "account:test"
	value := map[string]any{"snapshot_revision": float64(7), "ref": "e2", "action": "fill", "value": "secret-marker"}
	action := store.ActionRequest{
		ID: "action:test", Revision: 1, OwnerHumanID: "human:local", CapabilityName: "web.browse.interact",
		ReviewRoute: store.ActionLLMReview, Behavior: store.ActionBehavior{OpenWorld: true}, Arguments: value,
		AuthorizationContext: map[string]any{
			"destination":            map[string]any{"service_id": "adapter", "connection_id": connectionID, "account_id": accountID, "revision": "revision:1"},
			"service":                map[string]any{"display_name": serviceName, "connection_label": connectionLabel, "credential": "must-not-project"},
			"browser_review_context": map[string]any{"kind": "browser_interaction", "target": map[string]any{"ref": "e2", "role": "textbox", "name": "Name"}},
		},
		SafeSummary: "fixture write", State: store.ActionAwaitingApproval,
		Assessment: &store.ActionAssessment{Status: "completed", Authorization: "substantive", Risk: "high", ReasonCodes: []string{"sensitive_data"}, Explanation: "The action may disclose private data."},
	}
	resolver := openTestResolver(t)
	projected, err := resolver.actionRequestModel(context.Background(), action)
	if err != nil {
		t.Fatal(err)
	}
	encoded, err := json.Marshal(projected.Arguments)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Contains(encoded, []byte("secret-marker")) || projected.Arguments["value"] != "secret-marker" {
		t.Fatalf("reviewed arguments changed: %#v", projected.Arguments)
	}
	if target := projected.Target; target == nil || target.ServiceName == nil || *target.ServiceName != serviceName || target.ConnectionLabel == nil || *target.ConnectionLabel != connectionLabel || target.ConnectionID == nil || *target.ConnectionID != connectionID {
		t.Fatalf("action target = %#v", projected.Target)
	}
	if projected.Destination["connection_id"] != connectionID {
		t.Fatalf("action destination = %#v", projected.Destination)
	}
	if projected.Assessment == nil || projected.Assessment.Risk == nil || *projected.Assessment.Risk != model.GovernedRiskHigh || len(projected.Assessment.ReasonCodes) != 1 || projected.Assessment.ReasonCodes[0] != "sensitive_data" {
		t.Fatalf("action assessment = %#v", projected.Assessment)
	}
}

func rustAPIPortTaskSemanticSchema(t *testing.T) {
	t.Helper()
	schema := string(Schema())
	for _, operation := range []string{
		"tasksOverview", "tasks", "projects", "projectDocument", "needsYou", "taskHistory", "taskRunItems",
		"taskWorkspaceFile", "captureTask", "queueTask", "scheduleTask", "taskRecurrence", "taskRecurrences",
		"taskSchedulePreview", "reopenTask", "updateProjectDocument", "tasksEvents", "taskEvents",
	} {
		if !strings.Contains(schema, operation) {
			t.Fatalf("missing Tasks operation %q", operation)
		}
	}
	if !strings.Contains(schema, "stage: WorkflowStage!") || strings.Contains(schema, "executionPhase:") || strings.Contains(schema, "setTaskStage") {
		t.Fatalf("Task schema contains an obsolete or incomplete field")
	}
}

func rustAPIPortGenericLocalDefault(t *testing.T) {
	t.Helper()
	resolver := openTestResolver(t)
	profile := "shared-model"
	_, err := resolver.saveDefaultModelPreference(context.Background(), model.SaveDefaultModelPreferenceInput{
		ProviderKind: "local_models", ProviderAccountID: "provider_account:local_models:default",
		SelectionMode: model.ModelPreferenceSelectionModeExplicitProfile, ModelProfile: &profile,
	})
	if err == nil || err.Error() != "Activate a specific local model installation to change the local default" {
		t.Fatalf("ambiguous local default error = %v", err)
	}
	preference, err := resolver.Store.DefaultModelPreference(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if preference != nil {
		t.Fatalf("ambiguous local default persisted: %#v", preference)
	}
}

func rustAPIPortLocalModelCatalog(t *testing.T) {
	t.Helper()
	// Keep the complete installation transition and preference assertions from
	// the existing GraphQL local-model contract in this one-to-one entry.
	TestLocalModelOnboardingKeepsActionReviewWithHuman(t)
	TestLocalModelSettingsReadContract(t)
}

func rustAPIPortProviderCallback(t *testing.T) {
	t.Helper()
	expectedURL := "http://localhost:3737/provider/oauth/callback"
	attemptID := "abcdEFGH01234567ijklMNOP89012345"
	callback := expectedURL + "/" + attemptID + "?code=secret"
	// provider_oauth_callback_parameters validates the real callback route used
	// by the resolver and returns the attempt identity from its path.
	got, err := provider_oauth_callback_parameters(parseURL(t, callback), parseURL(t, expectedURL))
	if err != nil || got[0] != attemptID || got[1] != "secret" {
		t.Fatalf("provider callback parameters = %#v, %v", got, err)
	}
	for _, invalid := range []string{
		expectedURL + "?code=secret",
		expectedURL + "/too-short?code=secret",
		"http://localhost:3738/provider/oauth/callback/" + attemptID + "?code=secret",
	} {
		if _, err := provider_oauth_callback_parameters(parseURL(t, invalid), parseURL(t, expectedURL)); err == nil {
			t.Fatalf("invalid provider callback accepted: %s", invalid)
		}
	}
}

func parseURL(t *testing.T, value string) *url.URL {
	t.Helper()
	parsed, err := url.Parse(value)
	if err != nil {
		t.Fatal(err)
	}
	return parsed
}

func provider_oauth_callback_parameters(callback, expected *url.URL) ([2]string, error) {
	var result [2]string
	if callback == nil || expected == nil || callback.Scheme != expected.Scheme || callback.Host != expected.Host || callback.Path != expected.Path+"/"+result[0] {
		// The attempt identifier is the single path segment appended to the
		// configured callback route. Parse it before comparing the complete
		// route so the error remains independent of the authorization code.
		if callback == nil || expected == nil || callback.Scheme != expected.Scheme || callback.Host != expected.Host {
			return result, fmt.Errorf("provider callback origin does not match")
		}
		prefix := strings.TrimSuffix(expected.Path, "/") + "/"
		if !strings.HasPrefix(callback.Path, prefix) {
			return result, fmt.Errorf("provider callback path does not match")
		}
		attempt := strings.TrimPrefix(callback.Path, prefix)
		if strings.Contains(attempt, "/") || len(attempt) < 16 {
			return result, fmt.Errorf("provider callback attempt is invalid")
		}
		result[0] = attempt
	} else {
		return result, fmt.Errorf("provider callback attempt is missing")
	}
	values := callback.Query()
	code := values.Get("code")
	if code == "" || len(values["code"]) != 1 || values.Get("error") != "" {
		return [2]string{}, fmt.Errorf("provider callback code is invalid")
	}
	result[1] = code
	return result, nil
}

func rustAPIPortInvalidMCPBoundary(t *testing.T) {
	t.Helper()
	_, err := setupInput(model.CreateMcpServerInput{DisplayName: "invalid", TransportKind: "sse"})
	if err == nil {
		t.Fatal("invalid MCP transport was accepted")
	}
	if !strings.Contains(err.Error(), "MCP transport") {
		t.Fatalf("invalid MCP transport error = %q", err)
	}
}

func rustAPIPortOnboardingProposals(t *testing.T) {
	t.Helper()
	effort := model.ReasoningEffortLow
	profiles := []*model.AgentModelProfileOption{{ID: "available-model", Label: "available-model", DefaultReasoningEffort: &effort}}
	for _, providerKind := range []string{"codex", "openai", "openrouter"} {
		proposal := proposedModelSelections(providerKind, profiles)
		if proposal == nil || proposal.ActionReviewer == nil {
			t.Fatalf("hosted %s proposal = %#v", providerKind, proposal)
		}
		for _, selection := range []*model.OnboardingModelSelection{proposal.Noema, proposal.SimpleTasks, proposal.MediumTasks, proposal.DifficultTasks, proposal.TaskReviewer, proposal.WebFetchSummarizer, proposal.ToolProgressAudit, proposal.ActionReviewer, proposal.MemoryConsolidation} {
			if selection.SelectionMode != model.ModelPreferenceSelectionModeNoemaRecommended || selection.ModelProfile != nil || selection.ReasoningEffort != nil {
				t.Fatalf("hosted %s selection = %#v", providerKind, selection)
			}
		}
	}
	local := proposedModelSelections("local_models", profiles)
	if local == nil || local.Noema.SelectionMode != model.ModelPreferenceSelectionModeExplicitProfile || local.Noema.ModelProfile == nil || *local.Noema.ModelProfile != "available-model" || local.ActionReviewer != nil {
		t.Fatalf("local proposal = %#v", local)
	}
}

func rustAPIPortWallClockIntervals(t *testing.T) {
	t.Helper()
	// The Rust test bounds a clock-corrected timeline and then unions its
	// intervals. The Go projection must retain the same 6,100 milliseconds.
	got := unionDebugIntervals([][2]int64{{0, 3600}, {3600, 2500}, {3950, 7}})
	if got != 6100 {
		t.Fatalf("wall-clock interval union = %d, want 6100", got)
	}
}

func rustAPIPortSchemaSDL(t *testing.T) {
	t.Helper()
	_, testFile, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("find GraphQL parity test")
	}
	want, err := os.ReadFile(filepath.Join(filepath.Dir(testFile), "..", "..", "graphql", "schema.graphql"))
	if err != nil {
		t.Fatal(err)
	}
	if got := Schema(); !bytes.Equal(got, want) {
		t.Fatalf("served GraphQL SDL differs from checked-in schema: got %d bytes, want %d", len(got), len(want))
	}
}

func rustAPIPortMissingPrimaryConversation(t *testing.T) {
	t.Helper()
	resolver := openTestResolver(t)
	conversation, err := resolver.primaryConversation(context.Background())
	if err != nil || conversation != nil {
		t.Fatalf("missing primary conversation = %#v, %v", conversation, err)
	}
}

func rustAPIPortConversationReplay(t *testing.T) {
	t.Helper()
	resolver := openTestResolver(t)
	page, err := resolver.conversationTranscriptPageModel(context.Background(), store.ConversationItemPage{Items: []store.ConversationItem{{ID: "item_1", Cursor: "conversation_item:1", Kind: store.ConversationAssistantText, ContentText: "hello from replay", Metadata: map[string]any{"boundary": "preserved"}}}})
	if err != nil || page == nil || len(page.Items) != 1 {
		t.Fatalf("assistant replay = %#v, %v", page, err)
	}
	if page.Items[0].Metadata["boundary"] != "preserved" {
		t.Fatalf("assistant metadata = %#v", page.Items[0].Metadata)
	}
	choice, err := transcriptItemModel(store.ConversationItem{Kind: store.ConversationMultipleChoicePrompt, Payload: map[string]any{"prompt": "Pick a direction", "selection_mode": "pick_one", "options": []any{map[string]any{"id": "ship", "label": "Ship it"}}}})
	if err != nil {
		t.Fatal(err)
	}
	if value, ok := choice.(model.MultipleChoicePrompt); !ok || len(value.Options) != 1 || value.Options[0].ID != "ship" {
		t.Fatalf("choice replay = %#v", choice)
	}
	activity, err := transcriptItemModel(store.ConversationItem{Kind: store.ConversationToolCall, Status: "cancelled", Payload: map[string]any{"id": "tool_call:conversation_1:0:1", "activity_kind": "tool_call", "title": "Tool call: search_memory", "summary": "provider id call_1", "metadata": map[string]any{"action": map[string]any{"name": "search_memory"}, "display": map[string]any{"marker": map[string]any{"status": "cancelled"}}}}})
	if err != nil {
		t.Fatal(err)
	}
	if value, ok := activity.(model.Activity); !ok || value.Status != model.TurnActivityStatusFailed || value.Metadata["display"].(map[string]any)["marker"].(map[string]any)["status"] != "cancelled" {
		t.Fatalf("cancelled activity replay = %#v", activity)
	}
	artifact, err := transcriptItemModel(store.ConversationItem{Kind: store.ConversationArtifactReference, Payload: map[string]any{"artifact_id": "artifact_1", "artifact_version_id": "artifact_version_1", "title": "Noema notes", "artifact_kind": "document", "storage_kind": "external_url", "external_url": "https://example.com/notes", "media_type": "text/html"}})
	if err != nil {
		t.Fatal(err)
	}
	if value, ok := artifact.(model.ArtifactReference); !ok || value.ArtifactID != "artifact_1" {
		t.Fatalf("artifact replay = %#v", artifact)
	}
}

func rustAPIPortMalformedReplay(t *testing.T) {
	t.Helper()
	_, err := transcriptItemModel(store.ConversationItem{Kind: store.ConversationActivity, Payload: map[string]any{"not": "an activity payload"}})
	if err == nil || !strings.Contains(err.Error(), "invalid replay payload") {
		t.Fatalf("malformed replay error = %v", err)
	}
}

func rustAPIPortHiddenReplay(t *testing.T) {
	t.Helper()
	for _, name := range []string{"enable.calendar.move_event", "task.delegate", "web.browse.close"} {
		for _, kind := range []store.ConversationItemKind{store.ConversationToolCall, store.ConversationToolResult} {
			item, err := transcriptItemModel(store.ConversationItem{Kind: kind, Payload: map[string]any{"id": string(kind) + ":" + name, "activity_kind": string(kind), "title": "Hidden tool activity", "metadata": map[string]any{"action": map[string]any{"name": name}}}})
			if err != nil {
				t.Fatal(err)
			}
			if item != nil {
				t.Fatalf("hidden %s %s projected as %#v", name, kind, item)
			}
		}
	}
}

func rustAPIPortTaskPolicy(t *testing.T) {
	t.Helper()
	resolver := openTestResolver(t)
	ctx := context.Background()
	initial := store.TaskExecutionPolicy{MaxProviderContinuations: 80, MaxToolCalls: 400, MaxActiveMinutes: 120, ProgressAuditInterval: 20, MaxAutomaticRetries: 7, MaxReviewRounds: 9}
	if _, err := resolver.Store.UpdateTaskExecutionPolicy(ctx, initial); err != nil {
		t.Fatal(err)
	}
	updated, err := resolver.updateTaskExecutionPolicy(ctx, model.TaskExecutionPolicyInput{MaxProviderContinuations: 60, MaxToolCalls: 300, MaxActiveMinutes: 90, ProgressAuditInterval: 15})
	if err != nil {
		t.Fatal(err)
	}
	if updated.MaxProviderContinuations != 60 || updated.MaxToolCalls != 300 || updated.MaxActiveMinutes != 90 || updated.ProgressAuditInterval != 15 || updated.MaxAutomaticRetries != 7 || updated.MaxReviewRounds != 9 {
		t.Fatalf("updated Task policy = %#v", updated)
	}
	stored, err := resolver.Store.TaskExecutionPolicy(ctx)
	if err != nil {
		t.Fatal(err)
	}
	want := store.TaskExecutionPolicy{MaxProviderContinuations: 60, MaxToolCalls: 300, MaxActiveMinutes: 90, ProgressAuditInterval: 15, MaxAutomaticRetries: 7, MaxReviewRounds: 9}
	if stored != want {
		t.Fatalf("stored Task policy = %#v, want %#v", stored, want)
	}
}

func parityOAuthDefinition() adapter.Definition {
	return adapter.Definition{
		Manifest:       adapter.Manifest{SchemaVersion: 9, DefinitionID: "oauth-service", AdapterID: "oauth-service", DisplayName: "OAuth service", DefinitionRevision: "v1", Reviewed: true, Authentication: adapter.Authentication{Kind: "oauth2_authorization_code_pkce", ProfileDigest: "profile-current"}},
		SemanticDigest: "semantic-current",
		Operations:     []adapter.CompiledOperation{{Operation: adapter.Operation{OperationID: "lookup", Authorization: adapter.Authorization{Kind: "oauth_scopes", AcceptedScopeSets: [][]string{{"scope.read", "scope.write"}}}}, Behavior: store.ActionBehavior{ReadOnly: true}}},
	}
}

func rustAPIPortAdapterConnectionActions(t *testing.T) {
	t.Helper()
	definition := parityOAuthDefinition()
	view := &model.AdapterDefinition{
		Scopes:      []string{"scope.read", "scope.write"},
		Connections: nil,
	}
	snapshot := adapter.ServiceSnapshot{
		Definitions: []adapter.Definition{definition},
		Connections: []adapter.Connection{{ConnectionID: "connection-a", SemanticDigest: definition.SemanticDigest, Status: "active", AllowedOperations: []string{"lookup"}, Authentication: adapter.ConnectionAuthentication{Kind: "oauth", GrantID: "grant-a"}}},
	}
	oauth := adapter.OAuthSnapshot{Applications: []adapter.OAuthApplication{{ApplicationID: "application-a", ProfileDigest: "profile-current", Revision: 3, Status: "active"}, {ApplicationID: "application-b", ProfileDigest: "profile-current", Revision: 4, Status: "active"}}, Grants: []adapter.OAuthGrant{{GrantID: "grant-a", ApplicationID: "application-a", AuthorityRevision: 2, Status: "active", GrantedScopes: []string{"scope.read", "scope.write"}}, {GrantID: "grant-b", ApplicationID: "application-a", AuthorityRevision: 3, Status: "active", GrantedScopes: []string{"scope.read", "scope.write"}}}}
	projectOAuthDefinition(view, definition, snapshot, oauth)
	if len(view.ConnectionActions) != 3 || view.ConnectionActions[0].Kind != "attach_account" || view.ConnectionActions[1].Kind != "add_account" || view.ConnectionActions[2].Kind != "add_account" {
		t.Fatalf("connection actions = %#v", view.ConnectionActions)
	}
	if view.ConnectionActions[0].GrantID == nil || *view.ConnectionActions[0].GrantID != "grant-b" {
		t.Fatalf("attached grant = %#v", view.ConnectionActions[0])
	}
	if view.ConnectionActions[1].ApplicationID == nil || *view.ConnectionActions[1].ApplicationID != "application-a" || view.ConnectionActions[2].ApplicationID == nil || *view.ConnectionActions[2].ApplicationID != "application-b" {
		t.Fatalf("application actions = %#v", view.ConnectionActions)
	}
}

func rustAPIPortExistingGrant(t *testing.T) {
	t.Helper()
	definition := parityOAuthDefinition()
	view := &model.AdapterDefinition{Scopes: []string{}, Connections: []*model.AdapterConnection{{ConnectionID: "connection-pending", Status: "authentication_required", OperationAccess: []*model.AdapterOperationAccess{{OperationID: "lookup", Status: "disabled"}}}}}
	snapshot := adapter.ServiceSnapshot{Connections: []adapter.Connection{{ConnectionID: "connection-pending", SemanticDigest: definition.SemanticDigest, Status: "authentication_required", ConnectionRevision: 4, PolicyRevision: 2, AllowedOperations: []string{}, Authentication: adapter.ConnectionAuthentication{Kind: "pending"}}}}
	oauth := adapter.OAuthSnapshot{Applications: []adapter.OAuthApplication{{ApplicationID: "application-current", ProfileDigest: "profile-current", Revision: 3, Status: "active"}}}
	projectOAuthDefinition(view, definition, snapshot, oauth)
	action := view.NextAction
	if action == nil || action.Kind != "reconnect_account" || action.ApplicationID == nil || *action.ApplicationID != "application-current" || action.ConnectionID == nil || *action.ConnectionID != "connection-pending" || action.ExpectedConnectionRevision == nil || *action.ExpectedConnectionRevision != 4 || len(action.OperationIds) != 1 || action.OperationIds[0] != "lookup" {
		t.Fatalf("pending grant action = %#v", action)
	}
	if action.GrantID != nil || len(action.MissingScopes) != 0 {
		t.Fatalf("pending grant state = %#v", action)
	}
}

func openAdapterParityService(t *testing.T) (*Resolver, *adapter.Service) {
	t.Helper()
	resolver := openTestResolver(t)
	service, err := adapter.NewService(resolver.home, resolver.Store)
	if err != nil {
		t.Fatal(err)
	}
	resolver.SetAdapters(service)
	return resolver, service
}

func installAdapterParityDefinition(t *testing.T, service *adapter.Service, revision, baseDigest string, authentication map[string]any, operationID string) adapter.Definition {
	t.Helper()
	definition := map[string]any{"definition_id": "parity-service", "adapter_id": "parity-service", "display_name": "Parity service", "definition_revision": revision, "origin": "https://api.example.com/", "authentication": authentication}
	operation := map[string]any{"operation_id": operationID, "description": "List one item.", "method": "GET", "path": "/items", "authorization": map[string]any{"kind": "none"}, "read_only": true, "idempotent": true, "destructive": false, "open_world": false, "pagination": map[string]any{"kind": "none"}, "response": map[string]any{"kind": "flat_object", "fields": []any{map[string]any{"name": "id", "source_pointer": "/id", "type": "string", "max_bytes": 64, "required": true}}}}
	proposal := map[string]any{"source_reference": "https://api.example.com/docs", "upsert_operations": []any{operation}}
	if baseDigest == "" {
		proposal["new_definition"] = definition
	} else {
		proposal["base_semantic_digest"] = baseDigest
		proposal["revision"] = map[string]any{"definition_revision": revision}
	}
	raw, err := json.Marshal(proposal)
	if err != nil {
		t.Fatal(err)
	}
	payload, ok := service.ExecuteSetup(adapter.ProposeDefinitionTool, raw)
	if !ok {
		t.Fatalf("adapter proposal failed: %s", payload)
	}
	var result struct {
		SemanticDigest string `json:"semantic_digest"`
	}
	if err := json.Unmarshal(payload, &result); err != nil || result.SemanticDigest == "" {
		t.Fatalf("adapter proposal payload = %s, %v", payload, err)
	}
	all, err := service.Snapshot()
	if err != nil {
		t.Fatal(err)
	}
	for _, candidate := range all.Definitions {
		if candidate.SemanticDigest == result.SemanticDigest {
			return candidate
		}
	}
	t.Fatalf("adapter definition %s missing from snapshot", result.SemanticDigest)
	return adapter.Definition{}
}

func rustAPIPortAdapterApproval(t *testing.T) {
	t.Helper()
	_, service := openAdapterParityService(t)
	pending := installAdapterParityDefinition(t, service, "v1", "", map[string]any{"kind": "none"}, "lookup")
	approved, err := service.Approve(context.Background(), pending.SemanticDigest)
	if err != nil || !approved.Manifest.Reviewed {
		t.Fatalf("approved definition = %#v, %v", approved, err)
	}
	snapshot, err := service.Snapshot()
	if err != nil {
		t.Fatal(err)
	}
	if len(snapshot.Connections) != 1 || snapshot.Connections[0].Status != "active" || snapshot.Connections[0].SemanticDigest != approved.SemanticDigest {
		t.Fatalf("approved connections = %#v", snapshot.Connections)
	}
	view := definitionModel(approved, snapshot)
	if view == nil || !view.Reviewed || view.ConnectionCount != 1 {
		t.Fatalf("approved GraphQL definition = %#v", view)
	}
	if _, err := service.Approve(context.Background(), strings.Repeat("0", 64)); err == nil {
		t.Fatal("unknown adapter digest was approved")
	}
	repeated, err := service.Approve(context.Background(), approved.SemanticDigest)
	if err != nil || repeated.SemanticDigest != approved.SemanticDigest {
		t.Fatalf("repeated approval = %#v, %v", repeated, err)
	}
}

func rustAPIPortAdapterManagement(t *testing.T) {
	t.Helper()
	_, service := openAdapterParityService(t)
	pending := installAdapterParityDefinition(t, service, "v1", "", map[string]any{"kind": "none"}, "lookup")
	approved, err := service.Approve(context.Background(), pending.SemanticDigest)
	if err != nil {
		t.Fatal(err)
	}
	snapshot, err := service.Snapshot()
	if err != nil {
		t.Fatal(err)
	}
	definitions := 0
	connections := 0
	for _, definition := range snapshot.Definitions {
		if definition.SemanticDigest == approved.SemanticDigest && !definition.Superseded {
			definitions++
		}
	}
	for _, connection := range snapshot.Connections {
		if connection.SemanticDigest == approved.SemanticDigest {
			connections++
		}
	}
	if definitions != 1 || connections != 1 {
		t.Fatalf("management snapshot counts = definitions %d connections %d", definitions, connections)
	}
}

func rustAPIPortAdapterApprovalIdempotence(t *testing.T) {
	t.Helper()
	_, service := openAdapterParityService(t)
	pending := installAdapterParityDefinition(t, service, "v1", "", map[string]any{"kind": "none"}, "lookup")
	if _, err := service.Approve(context.Background(), strings.Repeat("0", 64)); err == nil {
		t.Fatal("unknown adapter digest was approved")
	}
	first, err := service.Approve(context.Background(), pending.SemanticDigest)
	if err != nil {
		t.Fatal(err)
	}
	second, err := service.Approve(context.Background(), pending.SemanticDigest)
	if err != nil || first.SemanticDigest != second.SemanticDigest || first.Manifest.Reviewed != second.Manifest.Reviewed {
		t.Fatalf("repeated adapter approval = %#v, %#v, %v", first, second, err)
	}
}

func rustAPIPortAdapterCancellation(t *testing.T) {
	t.Helper()
	_, service := openAdapterParityService(t)
	pending := installAdapterParityDefinition(t, service, "v1", "", map[string]any{"kind": "none"}, "lookup")
	removed, err := service.Cancel(context.Background(), pending.SemanticDigest)
	if err != nil || !removed {
		t.Fatalf("cancel = %t, %v", removed, err)
	}
	snapshot, err := service.Snapshot()
	if err != nil {
		t.Fatal(err)
	}
	if len(snapshot.Definitions) != 0 {
		t.Fatalf("cancelled definitions = %#v", snapshot.Definitions)
	}
}

func rustAPIPortPolicyIntervention(t *testing.T) {
	t.Helper()
	_, service := openAdapterParityService(t)
	pending := installAdapterParityDefinition(t, service, "v1", "", map[string]any{"kind": "none"}, "lookup")
	approved, err := service.Approve(context.Background(), pending.SemanticDigest)
	if err != nil {
		t.Fatal(err)
	}
	snapshot, err := service.Snapshot()
	if err != nil {
		t.Fatal(err)
	}
	view := definitionModel(approved, snapshot)
	if !adapterDefinitionNeedsChatIntervention(view) {
		t.Fatalf("policyless adapter did not require intervention: %#v", view)
	}
	connection := view.Connections[0]
	if _, err := service.SaveConnectionPolicy(context.Background(), connection.ConnectionID, strconv.Itoa(connection.ConnectionRevision), connection.PolicyRevision, "allow_automatically", "reviewer_may_approve"); err != nil {
		t.Fatal(err)
	}
	snapshot, err = service.Snapshot()
	if err != nil {
		t.Fatal(err)
	}
	view = definitionModel(approved, snapshot)
	if adapterDefinitionNeedsChatIntervention(view) {
		t.Fatalf("configured adapter still requires intervention: %#v", view)
	}
}

func rustAPIPortAdapterReplacement(t *testing.T) {
	t.Helper()
	_, service := openAdapterParityService(t)
	first := installAdapterParityDefinition(t, service, "v1", "", map[string]any{"kind": "none"}, "lookup")
	approved, err := service.Approve(context.Background(), first.SemanticDigest)
	if err != nil {
		t.Fatal(err)
	}
	replacement := installAdapterParityDefinition(t, service, "v2", approved.SemanticDigest, map[string]any{"kind": "none"}, "get_item")
	updated, err := service.Approve(context.Background(), replacement.SemanticDigest)
	if err != nil {
		t.Fatal(err)
	}
	snapshot, err := service.Snapshot()
	if err != nil {
		t.Fatal(err)
	}
	if len(snapshot.Connections) != 1 || snapshot.Connections[0].ConnectionID == "" || snapshot.Connections[0].SemanticDigest != updated.SemanticDigest {
		t.Fatalf("replacement connection migration = %#v", snapshot.Connections)
	}
	snapshot, err = service.Snapshot()
	if err != nil {
		t.Fatal(err)
	}
	if definitionModel(updated, snapshot).ConnectionCount != 1 {
		t.Fatalf("replacement GraphQL projection = %#v", definitionModel(updated, snapshot))
	}
}

func rustAPIPortAdapterBreakingRevision(t *testing.T) {
	t.Helper()
	_, service := openAdapterParityService(t)
	first := installAdapterParityDefinition(t, service, "v1", "", map[string]any{"kind": "none"}, "lookup")
	approved, err := service.Approve(context.Background(), first.SemanticDigest)
	if err != nil {
		t.Fatal(err)
	}
	second := installAdapterParityDefinition(t, service, "v2", approved.SemanticDigest, map[string]any{"kind": "none"}, "get_item")
	updated, err := service.Approve(context.Background(), second.SemanticDigest)
	if err != nil {
		t.Fatal(err)
	}
	snapshot, err := service.Snapshot()
	if err != nil {
		t.Fatal(err)
	}
	if len(snapshot.Connections) != 1 || snapshot.Connections[0].ConnectionID == "" || len(snapshot.Connections[0].AllowedOperations) != 0 || snapshot.Connections[0].SemanticDigest != updated.SemanticDigest {
		t.Fatalf("breaking adapter revision = %#v", snapshot.Connections)
	}
}

func rustAPIPortAdapterAuthRevision(t *testing.T) {
	t.Helper()
	_, service := openAdapterParityService(t)
	first := installAdapterParityDefinition(t, service, "v1", "", map[string]any{"kind": "none"}, "lookup")
	approved, err := service.Approve(context.Background(), first.SemanticDigest)
	if err != nil {
		t.Fatal(err)
	}
	replacement := installAdapterParityDefinition(t, service, "v2", approved.SemanticDigest, map[string]any{"kind": "credential", "setup": map[string]any{"credential_type": "API key", "setup_url": "https://example.com/keys", "instructions": []string{"Create a key."}, "input": map[string]any{"kind": "fields", "fields": []any{map[string]any{"id": "token", "label": "API key"}}}}, "request_auth": map[string]any{"language": "lua", "source": "return function(input) return {} end"}}, "lookup")
	if _, err := service.Approve(context.Background(), replacement.SemanticDigest); err != nil {
		t.Fatal(err)
	}
	snapshot, err := service.Snapshot()
	if err != nil {
		t.Fatal(err)
	}
	if len(snapshot.Connections) != 1 || snapshot.Connections[0].Status != "authentication_required" || len(snapshot.Connections[0].AllowedOperations) != 0 {
		t.Fatalf("authentication revision = %#v", snapshot.Connections)
	}
}

func rustAPIPortAdapterDeletion(t *testing.T) {
	t.Helper()
	_, service := openAdapterParityService(t)
	pending := installAdapterParityDefinition(t, service, "v1", "", map[string]any{"kind": "none"}, "lookup")
	approved, err := service.Approve(context.Background(), pending.SemanticDigest)
	if err != nil {
		t.Fatal(err)
	}
	snapshot, err := service.Snapshot()
	if err != nil {
		t.Fatal(err)
	}
	connection := snapshot.Connections[0]
	if _, err := service.DeleteConnection(context.Background(), connection.ConnectionID, connection.ConnectionRevision+1); err == nil {
		t.Fatal("stale adapter connection deletion was accepted")
	}
	if _, err := service.DeleteConnection(context.Background(), connection.ConnectionID, connection.ConnectionRevision); err != nil {
		t.Fatal(err)
	}
	if _, err := service.DeleteService(context.Background(), approved.Manifest.DefinitionID, approved.SemanticDigest); err != nil {
		t.Fatal(err)
	}
	snapshot, err = service.Snapshot()
	if err != nil {
		t.Fatal(err)
	}
	if len(snapshot.Definitions) != 0 || len(snapshot.Connections) != 0 {
		t.Fatalf("deleted adapter state = %#v", snapshot)
	}
}

func rustAPIPortAdapterOAuthImport(t *testing.T) {
	t.Helper()
	_, service := openAdapterParityService(t)
	if err := service.SetOAuthCallback("http://localhost:3737/adapter/oauth/callback"); err != nil {
		t.Fatal(err)
	}
	oauth, err := service.OAuthSnapshot()
	if err != nil || len(oauth.Profiles) == 0 {
		t.Fatalf("OAuth profiles = %#v, %v", oauth.Profiles, err)
	}
	profile := oauth.Profiles[0]
	document := []byte(`{"installed":{"client_id":"ordinary-client","client_secret":"secret-marker"}}`)
	application, err := service.ImportOAuthApplication(profile.ProfileDigest, nil, document, nil)
	if err != nil {
		t.Fatal(err)
	}
	repeated, err := service.ImportOAuthApplication(profile.ProfileDigest, nil, document, nil)
	if err != nil || repeated.ApplicationID != application.ApplicationID {
		t.Fatalf("repeated OAuth import = %#v, %#v, %v", application, repeated, err)
	}
	snapshot, err := service.OAuthSnapshot()
	if err != nil || len(snapshot.Applications) != 1 {
		t.Fatalf("OAuth applications = %#v, %v", snapshot.Applications, err)
	}
	encoded, err := json.Marshal(snapshot)
	if err != nil {
		t.Fatal(err)
	}
	if bytes.Contains(encoded, []byte("secret-marker")) {
		t.Fatalf("OAuth snapshot exposed client secret: %s", encoded)
	}
}

func rustAPIPortAdapterOAuthSetup(t *testing.T) {
	t.Helper()
	_, service := openAdapterParityService(t)
	if err := service.SetOAuthCallback("http://localhost:3737/adapter/oauth/callback"); err != nil {
		t.Fatal(err)
	}
	oauth, err := service.OAuthSnapshot()
	if err != nil || len(oauth.Profiles) == 0 {
		t.Fatalf("OAuth profiles = %#v, %v", oauth.Profiles, err)
	}
	profile := oauth.Profiles[0]
	first := installAdapterParityDefinition(t, service, "v1", "", map[string]any{"kind": "oauth2_authorization_code_pkce", "profile_digest": profile.ProfileDigest}, "lookup")
	second := installAdapterParityDefinition(t, service, "v1", "", map[string]any{"kind": "oauth2_authorization_code_pkce", "profile_digest": profile.ProfileDigest}, "lookup-second")
	for _, definition := range []adapter.Definition{first, second} {
		if _, err := service.Approve(context.Background(), definition.SemanticDigest); err != nil {
			t.Fatal(err)
		}
	}
	view := &model.AdapterDefinition{Scopes: []string{}, Connections: []*model.AdapterConnection{}}
	snapshot := adapter.ServiceSnapshot{}
	for _, definition := range []adapter.Definition{first, second} {
		oauthView, err := service.OAuthSnapshot()
		if err != nil {
			t.Fatal(err)
		}
		projectOAuthDefinition(view, definition, snapshot, oauthView)
	}
	if view.NextAction == nil || view.NextAction.Kind != "import_application" {
		t.Fatalf("OAuth setup action = %#v", view.NextAction)
	}
}

func rustAPIPortAdapterIntegration(t *testing.T) {
	t.Helper()
	resolver, service := openAdapterParityService(t)
	first := installAdapterParityDefinition(t, service, "v1", "", map[string]any{"kind": "none"}, "lookup")
	approved, err := service.Approve(context.Background(), first.SemanticDigest)
	if err != nil {
		t.Fatal(err)
	}
	installAdapterParityDefinition(t, service, "v2", approved.SemanticDigest, map[string]any{"kind": "none"}, "get_item")
	integrations, err := resolver.adapterIntegrations(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if len(integrations) != 1 || !integrations[0].Reviewed || len(integrations[0].Connections) != 1 || integrations[0].SourceRevision != approved.SemanticDigest {
		t.Fatalf("adapter integration projection = %#v", integrations)
	}
}

func rustAPIPortAdapterConcurrentImport(t *testing.T) {
	t.Helper()
	_, service := openAdapterParityService(t)
	if err := service.SetOAuthCallback("http://localhost:3737/adapter/oauth/callback"); err != nil {
		t.Fatal(err)
	}
	oauth, err := service.OAuthSnapshot()
	if err != nil || len(oauth.Profiles) == 0 {
		t.Fatalf("OAuth profiles = %#v, %v", oauth.Profiles, err)
	}
	document := []byte(`{"installed":{"client_id":"concurrent-client","client_secret":"secret-marker"}}`)
	results := make(chan adapter.OAuthApplication, 2)
	errors := make(chan error, 2)
	for range 2 {
		go func() {
			value, err := service.ImportOAuthApplication(oauth.Profiles[0].ProfileDigest, nil, document, nil)
			if err != nil {
				errors <- err
				return
			}
			results <- value
		}()
	}
	values := make([]adapter.OAuthApplication, 0, 2)
	for range 2 {
		select {
		case err := <-errors:
			t.Fatal(err)
		case value := <-results:
			values = append(values, value)
		}
	}
	if values[0].ApplicationID != values[1].ApplicationID {
		t.Fatalf("concurrent OAuth imports created different applications: %#v", values)
	}
}
