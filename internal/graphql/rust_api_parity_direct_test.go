package graphql

import (
	"bytes"
	"context"
	"crypto/ecdsa"
	"crypto/elliptic"
	"crypto/rand"
	"crypto/sha256"
	"crypto/x509"
	"database/sql"
	"encoding/base64"
	"encoding/json"
	"encoding/pem"
	"fmt"
	"net/http"
	"net/http/httptest"
	"net/url"
	"os"
	"path/filepath"
	"runtime"
	"sort"
	"strconv"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/adapter"
	"github.com/kpsuperplane/noema/internal/artifact"
	"github.com/kpsuperplane/noema/internal/auth"
	"github.com/kpsuperplane/noema/internal/diagnostics"
	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/localmodel"
	"github.com/kpsuperplane/noema/internal/notification"
	"github.com/kpsuperplane/noema/internal/provider"
	noemaruntime "github.com/kpsuperplane/noema/internal/runtime"
	"github.com/kpsuperplane/noema/internal/store"
	_ "github.com/ncruces/go-sqlite3/driver"
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
	key := testAPNSPrivateKeyPEM(t)
	for expected := 0; expected < 3; expected++ {
		if _, err := service.ConfigureAPNS("TEAM123456", "KEYID12345", key, expected); err != nil {
			t.Fatal(err)
		}
	}
	status, err := service.RemoveAPNS(context.Background(), 3)
	if err != nil || status.Configured || status.Revision != 4 {
		t.Fatalf("APNs tombstone = %#v, %v", status, err)
	}
	readStatus, err := service.APNSProviderStatus()
	if err != nil || readStatus.Configured || readStatus.Revision != 4 {
		t.Fatalf("APNs tombstone read = %#v, %v", readStatus, err)
	}
	data, err := os.ReadFile(paths.APNSProvider())
	if err != nil || bytes.Contains(data, []byte("PRIVATE KEY")) || !bytes.Contains(data, []byte(`"revision": 4`)) {
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
		ContentBase64: base64.StdEncoding.EncodeToString(spreadsheetXLSX(t)),
	})
	if err != nil {
		t.Fatal(err)
	}
	detail, err := resolver.artifactVersionDetail(context.Background(), created.CurrentVersion.ArtifactVersionID)
	if err != nil {
		t.Fatal(err)
	}
	if detail.PreviewKind != model.ArtifactVersionPreviewKindMarkdown || detail.Markdown == nil || !strings.Contains(*detail.Markdown, "| Name | Count |") {
		t.Fatalf("spreadsheet detail preview = %#v", detail)
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
	foreignConversation := rustAPIInsertForeignConversation(t, resolver)
	foreignVersion := rustAPIInsertForeignArtifact(t, resolver, foreignConversation)
	if _, found, err := resolver.Artifacts.AuthorizedFile(context.Background(), foreignVersion); err != nil || found {
		t.Fatalf("foreign Artifact download = %t, %v", found, err)
	}
	if listed, err := resolver.artifacts(context.Background(), "conversation", foreignConversation, nil); err != nil || len(listed) != 0 {
		t.Fatalf("foreign Artifact list = %#v, %v", listed, err)
	}
	if _, err := resolver.createConversationExternalArtifact(context.Background(), model.CreateConversationExternalArtifactInput{
		ConversationID: foreignConversation, Title: "Foreign", ArtifactKind: "document", ExternalURL: "https://example.com/foreign",
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
	paths, err := home.FromRoot(resolver.home.Name())
	if err != nil {
		t.Fatal(err)
	}
	metadata, err := sql.Open("sqlite3", paths.Database())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = metadata.Close() })
	if _, err := metadata.Exec(`UPDATE artifact_versions SET local_relative_path=?, content_sha256=NULL WHERE artifact_version_id=?`, "providers/secret.txt", created.CurrentVersion.ID); err != nil {
		t.Fatal(err)
	}
	if file, found, err := resolver.Artifacts.AuthorizedFile(context.Background(), created.CurrentVersion.ID); err != nil || found || file.Bytes != nil {
		t.Fatalf("authorized traversal download = %#v, %t, %v", file, found, err)
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
	response, err := http.Get(server.URL + artifact.DownloadURL(created.CurrentVersion.ID))
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
	ctx := context.Background()
	paths, err := home.FromRoot(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	root, err := paths.Open()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = root.Close() })
	database, err := store.Open(ctx, paths.Database())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = database.Close() })
	now := time.Now()
	oldSession, newSession := sha256.Sum256([]byte("rust-api-client-old")), sha256.Sum256([]byte("rust-api-client-new"))
	if err := database.CreateAnonymousSession(ctx, oldSession, now); err != nil {
		t.Fatal(err)
	}
	if err := database.RegisterPasskey(ctx, store.HumanPasskey{CredentialID: "AQ", CredentialJSON: `{}`}, store.RegistrationInitial, oldSession, newSession, now); err != nil {
		t.Fatal(err)
	}
	config, recovery, err := auth.LoadConfig(paths, "127.0.0.1:3737")
	if err != nil {
		t.Fatal(err)
	}
	authentication, err := auth.New(paths, database, config, recovery)
	if err != nil {
		t.Fatal(err)
	}
	clientID := "noema-desktop:abcdefghijklmnop"
	redirect := "http://127.0.0.1:49152/oauth/callback"
	nonce := "rust-api-client-code"
	access, refresh := "rust-api-client-access", "rust-api-client-refresh"
	if err := database.InsertNativeOAuthCode(ctx, sha256.Sum256([]byte(nonce)), clientID, "Noema Desktop", redirect, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM", now.Unix(), now.Add(10*time.Minute).Unix()); err != nil {
		t.Fatal(err)
	}
	if err := database.ExchangeNativeOAuthCode(ctx, sha256.Sum256([]byte(nonce)), clientID, redirect, "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk", "10000000000000000000000000000000", sha256.Sum256([]byte(access)), sha256.Sum256([]byte(refresh)), now.Unix()); err != nil {
		t.Fatal(err)
	}
	artifacts, err := artifact.New(root, database, nil)
	if err != nil {
		t.Fatal(err)
	}
	handler := authentication.Handler(NewHandler(NewResolver(database, root, authentication, nil, nil, nil, nil, artifacts, nil, nil)))
	list := nativeGraphQLRequest(t, access, `{ clients { clientId displayName createdAt revokedAt isCurrent } }`)
	listed := httptest.NewRecorder()
	handler.ServeHTTP(listed, list)
	var payload struct {
		Data struct {
			Clients []modelClient `json:"clients"`
		} `json:"data"`
	}
	if err := json.Unmarshal(listed.Body.Bytes(), &payload); err != nil || listed.Code != http.StatusOK || len(payload.Data.Clients) != 1 || !payload.Data.Clients[0].IsCurrent {
		t.Fatalf("client list = %d %s, %v", listed.Code, listed.Body.String(), err)
	}
	revoke := nativeGraphQLRequest(t, access, `mutation { revokeClient(clientId: "`+clientID+`") { clientId revokedAt isCurrent } }`)
	revoked := httptest.NewRecorder()
	handler.ServeHTTP(revoked, revoke)
	if revoked.Code != http.StatusOK || !bytes.Contains(revoked.Body.Bytes(), []byte(`"isCurrent":true`)) || !bytes.Contains(revoked.Body.Bytes(), []byte(`"revokedAt":`)) {
		t.Fatalf("client revocation = %d %s", revoked.Code, revoked.Body.String())
	}
	rejected := httptest.NewRecorder()
	handler.ServeHTTP(rejected, nativeGraphQLRequest(t, access, `{ clients { clientId } }`))
	if rejected.Code != http.StatusUnauthorized {
		t.Fatalf("revoked bearer = %d", rejected.Code)
	}
}

func rustAPIPortClientRevokeAll(t *testing.T) {
	t.Helper()
	ctx := context.Background()
	paths, err := home.FromRoot(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	root, err := paths.Open()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = root.Close() })
	database, err := store.Open(ctx, paths.Database())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = database.Close() })
	now := time.Now()
	// Rust executes this operation with RequestPrincipal::local. The Go HTTP
	// admission layer requires the equivalent authenticated setup before it
	// will dispatch a browser-independent GraphQL request.
	oldSession, newSession := sha256.Sum256([]byte("revoke-all-old")), sha256.Sum256([]byte("revoke-all-new"))
	if err := database.CreateAnonymousSession(ctx, oldSession, now); err != nil {
		t.Fatal(err)
	}
	if err := database.RegisterPasskey(ctx, store.HumanPasskey{CredentialID: "AQ", CredentialJSON: `{}`}, store.RegistrationInitial, oldSession, newSession, now); err != nil {
		t.Fatal(err)
	}
	var firstAccess string
	for index, clientID := range []string{"noema-ios:abcdefghijklmnop", "noema-desktop:qrstuvwxyzabcdef"} {
		code, access, refresh := fmt.Sprintf("revoke-all-code-%d", index), fmt.Sprintf("revoke-all-access-%d", index), fmt.Sprintf("revoke-all-refresh-%d", index)
		if index == 0 {
			firstAccess = access
		}
		redirect := "http://127.0.0.1:49152/oauth/callback"
		if index == 0 {
			redirect = "noema://oauth/callback"
		}
		if err := database.InsertNativeOAuthCode(ctx, sha256.Sum256([]byte(code)), clientID, clientID, redirect, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM", now.Unix(), now.Add(10*time.Minute).Unix()); err != nil {
			t.Fatal(err)
		}
		familyID := fmt.Sprintf("%031x%d", index+1, index+1)
		if err := database.ExchangeNativeOAuthCode(ctx, sha256.Sum256([]byte(code)), clientID, redirect, "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk", familyID, sha256.Sum256([]byte(access)), sha256.Sum256([]byte(refresh)), now.Unix()); err != nil {
			t.Fatal(err)
		}
	}
	config, recovery, err := auth.LoadConfig(paths, "127.0.0.1:3737")
	if err != nil {
		t.Fatal(err)
	}
	authentication, err := auth.New(paths, database, config, recovery)
	if err != nil {
		t.Fatal(err)
	}
	resolver := NewResolver(database, root, authentication, nil, nil, nil, nil, nil, nil, nil)
	handler := authentication.Handler(NewHandler(resolver))
	revoke := httptest.NewRecorder()
	handler.ServeHTTP(revoke, nativeGraphQLRequest(t, firstAccess, `mutation { revokeAllClients }`))
	if revoke.Code != http.StatusOK || !bytes.Contains(revoke.Body.Bytes(), []byte(`"revokeAllClients":2`)) {
		t.Fatalf("revoke-all clients = %d %s", revoke.Code, revoke.Body.String())
	}
	clients, err := database.NativeOAuthClients(ctx)
	if err != nil || len(clients) != 2 {
		t.Fatalf("retained clients = %#v, %v", clients, err)
	}
	for _, client := range clients {
		if client.RevokedAt == nil {
			t.Fatalf("client remained active after revoke-all: %#v", client)
		}
	}
}

func rustAPIPortRuntimeTurnError(t *testing.T) {
	t.Helper()
	resolver := &Resolver{}
	clientID := "client_1"
	conversationID := "conversation_1"
	itemEvent, err := resolver.conversationEventModel(context.Background(), noemaruntime.Event{
		Kind: noemaruntime.EventTransientError, ConversationID: conversationID,
		ClientMessageID: &clientID, TransientMessage: "provider failed",
	})
	if err != nil {
		t.Fatal(err)
	}
	item, ok := itemEvent.(model.ConversationItemEvent)
	if !ok || item.ConversationID != conversationID || item.ItemID != "graphql_runtime_error:"+conversationID+":"+clientID || item.ClientMessageID == nil || *item.ClientMessageID != clientID || len(item.Metadata) != 0 {
		t.Fatalf("runtime error item = %#v", itemEvent)
	}
	notice, ok := item.Item.(model.ErrorNotice)
	if !ok || !strings.Contains(notice.Message, "provider failed") || notice.Recoverable {
		t.Fatalf("runtime error notice = %#v", item.Item)
	}
	completedEvent, err := resolver.conversationEventModel(context.Background(), noemaruntime.Event{
		Kind: noemaruntime.EventTurnCompleted, ConversationID: conversationID, ClientMessageID: &clientID,
	})
	if err != nil {
		t.Fatal(err)
	}
	completed, ok := completedEvent.(model.TurnCompletedEvent)
	if !ok || completed.ConversationID != conversationID || completed.ClientMessageID == nil || *completed.ClientMessageID != clientID {
		t.Fatalf("runtime completion = %#v", completedEvent)
	}
}

func rustAPIPortRuntimeTurnErrorNoDuplicate(t *testing.T) {
	t.Helper()
	resolver := openChatTestResolver(t)
	rustAPIConfigureChatProvider(t, resolver)
	conversation, err := resolver.Store.EnsurePrimaryConversation(context.Background(), "openrouter", "", time.Now())
	if err != nil {
		t.Fatal(err)
	}
	seedTurn, _, err := resolver.Store.BeginConversationTurn(context.Background(), conversation.ID,
		"Persist one provider error.", nil, time.Now().UTC())
	if err != nil {
		t.Fatal(err)
	}
	seedNotice, err := resolver.Store.FailConversationTurn(context.Background(), seedTurn, "provider failed", time.Now().UTC())
	if err != nil {
		t.Fatal(err)
	}
	persisted, err := resolver.Store.VisibleConversationItem(context.Background(), seedNotice.ID)
	if err != nil || persisted == nil || persisted.ID != seedNotice.ID ||
		persisted.Kind != store.ConversationErrorNotice || persisted.Payload["message"] != "provider failed" ||
		persisted.Payload["recoverable"] != false {
		t.Fatalf("persisted error notice = %#v, %v", persisted, err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	stream, err := resolver.conversationEvents(ctx, conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	if ready := <-stream; ready.(model.SubscriptionReadyEvent).ConversationID != conversation.ID {
		t.Fatalf("conversation stream was not ready: %#v", ready)
	}
	replaceChatTransport(t, chatRoundTripFunc(func(*http.Request) (*http.Response, error) {
		return nil, fmt.Errorf("provider failed")
	}))
	clientID := "client_1"
	if _, err := resolver.sendConversationTurn(ctx, model.SendConversationTurnInput{ConversationID: conversation.ID,
		Input: "Cause one provider error.", ClientMessageID: &clientID}); err != nil {
		t.Fatal(err)
	}
	errorNoticeCount, completionCount := 0, 0
	for completionCount == 0 {
		select {
		case event := <-stream:
			switch value := event.(type) {
			case model.ConversationItemEvent:
				if _, ok := value.Item.(model.ErrorNotice); ok {
					errorNoticeCount++
				}
			case model.TurnCompletedEvent:
				completionCount++
			}
		case <-ctx.Done():
			t.Fatalf("runtime terminal events timed out: notices=%d completions=%d", errorNoticeCount, completionCount)
		}
	}
	if errorNoticeCount != 1 || completionCount != 1 {
		t.Fatalf("runtime error event counts = %d, %d", errorNoticeCount, completionCount)
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
			"origin":                 "test",
			"browser_review_context": map[string]any{"kind": "browser_interaction", "page": map[string]any{"url": "https://example.com/form", "title": "Example form"}, "target": map[string]any{"ref": "e2", "role": "textbox", "name": "Name"}},
		},
		SafeSummary: "fixture write", State: store.ActionAwaitingApproval,
		Assessment: &store.ActionAssessment{Status: "completed", Authorization: "substantive", Risk: "high", ReasonCodes: []string{"sensitive_data"}, Explanation: "The action may disclose private data."},
	}
	available := true
	projected, err := actionRequestModelWithBrowserSession(action, &available)
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
	targetArguments, ok := projected.Arguments["target"].(map[string]any)
	if !ok || targetArguments["name"] != "Name" {
		t.Fatalf("reviewed browser target = %#v", projected.Arguments["target"])
	}
	if target := projected.Target; target == nil || target.ServiceName == nil || *target.ServiceName != serviceName || target.ConnectionLabel == nil || *target.ConnectionLabel != connectionLabel || target.ConnectionID == nil || *target.ConnectionID != connectionID || target.AccountID == nil || *target.AccountID != accountID {
		t.Fatalf("action target = %#v", projected.Target)
	}
	if projected.BrowserSessionAvailable == nil || !*projected.BrowserSessionAvailable {
		t.Fatalf("browser session availability = %#v", projected.BrowserSessionAvailable)
	}
	if projected.Destination["connection_id"] != connectionID {
		t.Fatalf("action destination = %#v", projected.Destination)
	}
	if projected.Consequence != "This changes data outside Noema in Work account." {
		t.Fatalf("action consequence = %q", projected.Consequence)
	}
	if projected.Disclosure == nil || projected.Disclosure.Recipient != "Work account" || projected.Disclosure.ContentSummary != "the reviewed request data" {
		t.Fatalf("action disclosure = %#v", projected.Disclosure)
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
	response := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(context.Background()), `mutation($profile:String!) {
  saveDefaultModelPreference(input: { providerKind: "local_models", providerAccountId: "provider_account:local_models:default", selectionMode: EXPLICIT_PROFILE, modelProfile: $profile, fastMode: false }) {
    providerKind providerAccountId modelProfile selectionMode
  }
}`, map[string]any{"profile": profile})
	if len(response.Errors) != 1 || response.Errors[0].Message != "Activate a specific local model installation to change the local default" {
		t.Fatalf("ambiguous local default error = %#v", response.Errors)
	}
	preference := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(context.Background()), `query { defaultModelPreference { providerKind providerAccountId modelProfile selectionMode } }`, nil)
	if len(preference.Errors) != 0 {
		t.Fatalf("ambiguous local default query = %#v", preference.Errors)
	}
	if preference.Data["defaultModelPreference"] != nil {
		t.Fatalf("ambiguous local default persisted: %#v", preference.Data)
	}
}

func rustAPIPortLocalModelCatalog(t *testing.T) {
	resolver := openProviderTestResolver(t)
	ctx := context.Background()
	now := time.Now()
	queued, err := resolver.Store.QueueLocalModel(ctx, store.LocalModelInstallation{
		ID:         "local_model_installation:test",
		ModelID:    "test-model",
		Name:       "Test model",
		File:       "test-model.gguf",
		SourceKind: "local_file",
		Backend:    "cpu",
		TotalBytes: 4,
		CreatedAt:  now,
	})
	if err != nil {
		t.Fatal(err)
	}
	verifying, err := resolver.Store.UpdateLocalModel(
		ctx, queued.ID, "verifying", 4, 4, 0, "", "", "", "", now,
	)
	if err != nil {
		t.Fatal(err)
	}
	installed, err := resolver.Store.UpdateLocalModel(
		ctx, verifying.ID, "installed", 4, 4, 4,
		"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
		"models/blobs/a.gguf", "", "", now,
	)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := resolver.Store.ActivateLocalModel(ctx, installed.ID, false, now); err != nil {
		t.Fatal(err)
	}
	service, err := localmodel.New(resolver.Store, resolver.home.Name(), "")
	if err != nil {
		t.Fatal(err)
	}
	resolver.SetLocalModels(service)
	t.Cleanup(service.Close)

	setupResponse := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(ctx), `query {
  onboardingModelSetup(providerAccountId: "provider_account:local_models:default") {
    profiles { id disabledReason }
    proposedSelections { actionReviewer { selectionMode } }
  }
}`, nil)
	if len(setupResponse.Errors) != 0 {
		t.Fatalf("local setup GraphQL errors = %#v", setupResponse.Errors)
	}
	setup := setupResponse.Data["onboardingModelSetup"].(map[string]any)
	if len(setup["profiles"].([]any)) != 1 || setup["proposedSelections"].(map[string]any)["actionReviewer"] != nil {
		t.Fatalf("local setup = %#v", setupResponse.Data)
	}
	selection := map[string]any{"selectionMode": "EXPLICIT_PROFILE", "modelProfile": "test-model", "fastMode": false}
	confirmResponse := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(ctx), `mutation($input: ConfirmOnboardingModelSelectionsInput!) {
  confirmOnboardingModelSelections(input: $input) { isUserOnboarded }
}`, map[string]any{"input": map[string]any{
		"providerAccountId": "provider_account:local_models:default", "noema": selection, "simpleTasks": selection,
		"mediumTasks": selection, "difficultTasks": selection, "taskReviewer": selection,
		"webFetchSummarizer": selection, "toolProgressAudit": selection, "memoryConsolidation": selection,
	}})
	if len(confirmResponse.Errors) != 0 {
		t.Fatalf("local model confirmation GraphQL errors = %#v", confirmResponse.Errors)
	}
	assignments, err := resolver.Store.HostedModelAssignments(ctx)
	if err != nil {
		t.Fatal(err)
	}
	if len(assignments) != 8 {
		t.Fatalf("local assignment count = %d", len(assignments))
	}
	for _, assignment := range assignments {
		if assignment.Role == store.HostedModelActionReviewer {
			t.Fatal("local onboarding assigned action review")
		}
	}
}

func rustAPIPortProviderCallback(t *testing.T) {
	t.Helper()
	resolver := openProviderTestResolver(t)
	attempt, err := resolver.OpenRouter.StartAuth(context.Background(), "openrouter", "", provider.AuthOAuthPKCE)
	if err != nil {
		t.Fatal(err)
	}
	handler := resolver.OpenRouter.CallbackHandler()
	invalid := []struct {
		path   string
		status int
	}{
		{"/provider/oauth/callback?code=secret", http.StatusBadRequest},
		{"/provider/oauth/callback/too-short?code=secret", http.StatusBadRequest},
		{"/provider/oauth/callback/" + attempt.ID + "/extra?code=secret", http.StatusBadRequest},
	}
	for _, test := range invalid {
		recorder := httptest.NewRecorder()
		handler.ServeHTTP(recorder, httptest.NewRequest(http.MethodGet, test.path, nil))
		if recorder.Code != test.status {
			t.Fatalf("provider callback %s = %d, want %d", test.path, recorder.Code, test.status)
		}
	}
	// Use the production callback handler with the valid path. The route parser
	// must extract this exact attempt identity before the provider exchange.
	request := httptest.NewRequest(http.MethodGet, "/provider/oauth/callback/"+attempt.ID+"?code=secret", nil)
	recorder := httptest.NewRecorder()
	handler.ServeHTTP(recorder, request)
	if recorder.Code != http.StatusOK {
		t.Fatalf("valid provider callback = %d", recorder.Code)
	}
	view, found := resolver.OpenRouter.Attempt(attempt.ID)
	if !found || view.Status == provider.AuthAttemptWaiting {
		t.Fatalf("provider callback attempt = %#v, found=%t", view, found)
	}
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
	started := time.Date(2026, time.July, 21, 21, 0, 0, 0, time.UTC)
	ended := started.Add(6100 * time.Millisecond)
	firstEnd := started.Add(3600 * time.Millisecond)
	secondStart, secondEnd := started.Add(3600*time.Millisecond), started.Add(6100*time.Millisecond)
	thirdStart, thirdEnd := started.Add(3950*time.Millisecond), started.Add(3957*time.Millisecond)
	profile := &store.RuntimeDebugProfile{Scope: store.RuntimeDebugScope{Kind: "conversation_turn", ID: "turn:intervals"}, Status: "completed", StartedAt: started, EndedAt: &ended,
		Spans: []store.RuntimeDebugSpan{
			{ID: "span:first", Category: "provider", Status: "completed", StartedAt: started, EndedAt: &firstEnd},
			{ID: "span:second", Category: "tool", Status: "completed", StartedAt: secondStart, EndedAt: &secondEnd},
			{ID: "span:third", Category: "runtime", Status: "completed", StartedAt: thirdStart, EndedAt: &thirdEnd},
		}}
	projected := runtimeDebugProfileModel(profile, ended)
	if projected.AccountedMilliseconds != 6100 || projected.UninstrumentedMilliseconds != 0 {
		t.Fatalf("wall-clock interval union = %d/%d, want 6100/0", projected.AccountedMilliseconds, projected.UninstrumentedMilliseconds)
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
	for _, conversationExists := range []bool{false, true} {
		resolver := openTestResolver(t)
		if conversationExists {
			paths, err := home.FromRoot(resolver.home.Name())
			if err != nil {
				t.Fatal(err)
			}
			metadata, err := sql.Open("sqlite3", paths.Database())
			if err != nil {
				t.Fatal(err)
			}
			now := time.Now().UnixMilli()
			if _, err := metadata.Exec(`INSERT INTO conversations
 (conversation_id, owner_human_id, provider, cwd, created_at_ms, updated_at_ms)
 VALUES (?, 'human:local', 'openrouter', NULL, ?, ?)`, "conversation:"+strings.Repeat("a", 32), now, now); err != nil {
				_ = metadata.Close()
				t.Fatal(err)
			}
			_ = metadata.Close()
		}
		response := rustAPIRawGraphQL(t, resolver, `query { primaryConversation { conversationId provider } }`, nil)
		if len(response.Errors) != 0 {
			t.Fatalf("primary conversation errors (exists=%t) = %#v", conversationExists, response.Errors)
		}
		if value := response.Data["primaryConversation"]; value != nil {
			t.Fatalf("primary conversation (exists=%t) = %#v, want null", conversationExists, value)
		}
	}
}

func rustAPIPortConversationReplay(t *testing.T) {
	t.Helper()
	resolver := openTestResolver(t)
	page, err := resolver.conversationTranscriptPageModel(context.Background(), store.ConversationItemPage{Items: []store.ConversationItem{{ID: "item_1", Cursor: "conversation_item:1", Kind: store.ConversationAssistantText, ContentText: "hello from replay", Metadata: map[string]any{"boundary": "preserved"}}}})
	if err != nil || page == nil || len(page.Items) != 1 {
		t.Fatalf("assistant replay = %#v, %v", page, err)
	}
	if value, ok := page.Items[0].Item.(model.AssistantText); !ok || value.Text != "hello from replay" {
		t.Fatalf("assistant replay text = %#v", page.Items[0].Item)
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
			item, err := transcriptItemModel(store.ConversationItem{Kind: kind, Status: "completed", Payload: map[string]any{"id": string(kind) + ":" + name, "activity_kind": string(kind), "title": "Hidden tool activity", "metadata": map[string]any{"action": map[string]any{"name": name}}}})
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
	for _, action := range view.ConnectionActions {
		if action.GrantID != nil && *action.GrantID == "grant-a" {
			t.Fatalf("superseded grant remained selectable: %#v", view.ConnectionActions)
		}
	}
}

func rustAPIPortExistingGrant(t *testing.T) {
	t.Helper()
	definition := parityOAuthDefinition()
	view := &model.AdapterDefinition{Scopes: []string{}, Connections: []*model.AdapterConnection{{ConnectionID: "connection-pending", Status: "authentication_required", OperationAccess: []*model.AdapterOperationAccess{{OperationID: "lookup", Status: "disabled"}}}}}
	snapshot := adapter.ServiceSnapshot{Connections: []adapter.Connection{{ConnectionID: "connection-pending", SemanticDigest: definition.SemanticDigest, Status: "authentication_required", ConnectionRevision: 4, PolicyRevision: 2, AllowedOperations: []string{}, Authentication: adapter.ConnectionAuthentication{Kind: "oauth", GrantID: "grant-a"}}}}
	oauth := adapter.OAuthSnapshot{Applications: []adapter.OAuthApplication{{ApplicationID: "application-current", ProfileDigest: "profile-current", Revision: 1, Status: "active"}}, Grants: []adapter.OAuthGrant{{GrantID: "grant-a", ApplicationID: "application-current", AuthorityRevision: 2, Status: "authentication_required", GrantedScopes: []string{"scope.read", "scope.write"}}}}
	projectOAuthDefinition(view, definition, snapshot, oauth)
	action := view.NextAction
	if action == nil || action.Kind != "reconnect_account" || action.ApplicationID == nil || *action.ApplicationID != "application-current" || action.ExpectedApplicationRevision == nil || *action.ExpectedApplicationRevision != 1 {
		t.Fatalf("pending grant action = %#v", action)
	}
	sharedRepair := &model.AdapterConnection{ConnectionID: "shared", Status: "authentication_required", GrantID: stringPointer("grant-a")}
	if adapterConnectionNeedsChatIntervention(sharedRepair) {
		t.Fatal("existing OAuth grant incorrectly created a chat intervention")
	}
	initialSetup := &model.AdapterConnection{ConnectionID: "initial", Status: "authentication_required"}
	if !adapterConnectionNeedsChatIntervention(initialSetup) {
		t.Fatal("missing OAuth grant did not create a chat intervention")
	}
	connections := []*model.AdapterConnection{
		{ConnectionID: "b", Status: "active", GrantID: stringPointer("grant-a")},
		{ConnectionID: "a", Status: "active", GrantID: stringPointer("grant-a")},
	}
	sort.Slice(connections, func(i, j int) bool { return connections[i].ConnectionID < connections[j].ConnectionID })
	if connections[0].ConnectionID != "a" {
		t.Fatalf("grant fallback was not deterministic: %#v", connections)
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
	return installAdapterParityDefinitionNamed(t, service, "parity-service", revision, baseDigest, authentication, operationID)
}

func installAdapterParityDefinitionNamed(t *testing.T, service *adapter.Service, definitionID, revision, baseDigest string, authentication map[string]any, operationID string) adapter.Definition {
	t.Helper()
	definition := map[string]any{"definition_id": definitionID, "adapter_id": definitionID, "display_name": "Parity service", "definition_revision": revision, "origin": "https://api.example.com/", "authentication": authentication}
	authorization := map[string]any{"kind": "none"}
	if authentication["kind"] == "oauth2_authorization_code_pkce" {
		authorization = map[string]any{"kind": "oauth_scopes", "accepted_scope_sets": [][]string{{"scope.read", "scope.write"}}}
	}
	operation := map[string]any{"operation_id": operationID, "description": "List one item.", "method": "GET", "path": "/items", "authorization": authorization, "read_only": true, "idempotent": true, "destructive": false, "open_world": false, "pagination": map[string]any{"kind": "none"}, "response": map[string]any{"kind": "flat_object", "fields": []any{map[string]any{"name": "id", "source_pointer": "/id", "type": "string", "max_bytes": 64, "required": true}}}}
	proposal := map[string]any{"source_reference": "https://api.example.com/docs", "upsert_operations": []any{operation}}
	if baseDigest == "" {
		proposal["new_definition"] = definition
	} else {
		proposal["base_semantic_digest"] = baseDigest
		revisionValue := map[string]any{"definition_revision": revision}
		if authentication["kind"] != "none" {
			revisionValue["authentication"] = authentication
		}
		proposal["revision"] = revisionValue
		if operationID != "lookup" {
			proposal["remove_operation_ids"] = []string{"lookup"}
		}
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

func rustAPIApproveAdapter(t *testing.T, resolver *Resolver, semanticDigest string) rustAPIGraphQLResponse {
	t.Helper()
	return rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(context.Background()), fmt.Sprintf(`mutation {
  approveAdapterDefinition(input: { semanticDigest: %q }) {
    semanticDigest reviewed superseded connectionCount
    connections { connectionId status connectionRevision policyRevision allowedOperations policyConfigured }
  }
}`, semanticDigest), nil)
}

func rustAPICancelAdapter(t *testing.T, resolver *Resolver, semanticDigest string) rustAPIGraphQLResponse {
	t.Helper()
	return rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(context.Background()), fmt.Sprintf(`mutation {
  cancelAdapterDefinition(input: { semanticDigest: %q })
}`, semanticDigest), nil)
}

func rustAPIPortAdapterApproval(t *testing.T) {
	t.Helper()
	resolver, service := openAdapterParityService(t)
	pending := installAdapterParityDefinition(t, service, "v1", "", map[string]any{"kind": "none"}, "lookup")
	approved := rustAPIApproveAdapter(t, resolver, pending.SemanticDigest)
	if len(approved.Errors) != 0 {
		t.Fatalf("approved definition GraphQL errors = %#v", approved.Errors)
	}
	value := approved.Data["approveAdapterDefinition"].(map[string]any)
	if value["semanticDigest"] == pending.SemanticDigest || value["reviewed"] != true || value["connectionCount"] != float64(1) {
		t.Fatalf("approved definition = %#v, pending=%q", value, pending.SemanticDigest)
	}
	reviewedDigest := value["semanticDigest"].(string)
	connections := value["connections"].([]any)
	if len(connections) != 1 || connections[0].(map[string]any)["status"] != "active" || connections[0].(map[string]any)["policyConfigured"] != false {
		t.Fatalf("approved connections = %#v", connections)
	}
	unknown := rustAPIApproveAdapter(t, resolver, strings.Repeat("0", 64))
	if len(unknown.Errors) != 1 {
		t.Fatal("unknown adapter digest was approved")
	}
	repeated := rustAPIApproveAdapter(t, resolver, reviewedDigest)
	if len(repeated.Errors) != 0 || repeated.Data["approveAdapterDefinition"].(map[string]any)["semanticDigest"] != reviewedDigest {
		t.Fatalf("repeated approval = %#v", repeated)
	}
}

func rustAPIPortAdapterManagement(t *testing.T) {
	t.Helper()
	resolver, service := openAdapterParityService(t)
	pending := installAdapterParityDefinition(t, service, "v1", "", map[string]any{"kind": "none"}, "lookup")
	approved, err := service.Approve(context.Background(), pending.SemanticDigest)
	if err != nil {
		t.Fatal(err)
	}
	response := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(context.Background()), `query {
  adapterManagement {
    definitions { semanticDigest connectionCount }
    oauthState { profiles { profileDigest } }
    integrations { connections { connectionId } }
  }
}`, nil)
	if len(response.Errors) != 0 {
		t.Fatalf("adapter management errors = %#v", response.Errors)
	}
	management, ok := response.Data["adapterManagement"].(map[string]any)
	if !ok {
		t.Fatalf("adapter management data = %#v", response.Data)
	}
	definitions, ok := management["definitions"].([]any)
	if !ok {
		t.Fatalf("adapter management definitions = %#v", management["definitions"])
	}
	definitionConnections := 0
	for _, value := range definitions {
		definition, ok := value.(map[string]any)
		if !ok {
			t.Fatalf("adapter management definition = %#v", value)
		}
		definitionConnections += int(definition["connectionCount"].(float64))
	}
	integrations, ok := management["integrations"].([]any)
	if !ok {
		t.Fatalf("adapter management integrations = %#v", management["integrations"])
	}
	integrationConnections := 0
	for _, value := range integrations {
		integration, ok := value.(map[string]any)
		if !ok {
			t.Fatalf("adapter management integration = %#v", value)
		}
		integrationConnections += len(integration["connections"].([]any))
	}
	if definitionConnections != 1 || integrationConnections != definitionConnections {
		t.Fatalf("management GraphQL counts = definitions %d integrations %d (approved %s)", definitionConnections, integrationConnections, approved.SemanticDigest)
	}
}

func rustAPIPortAdapterApprovalIdempotence(t *testing.T) {
	t.Helper()
	resolver, service := openAdapterParityService(t)
	pending := installAdapterParityDefinition(t, service, "v1", "", map[string]any{"kind": "none"}, "lookup")
	if response := rustAPIApproveAdapter(t, resolver, strings.Repeat("0", 64)); len(response.Errors) == 0 {
		t.Fatal("unknown adapter digest was approved")
	}
	first := rustAPIApproveAdapter(t, resolver, pending.SemanticDigest)
	if len(first.Errors) != 0 {
		t.Fatalf("first adapter approval = %#v", first.Errors)
	}
	reviewedDigest := first.Data["approveAdapterDefinition"].(map[string]any)["semanticDigest"].(string)
	second := rustAPIApproveAdapter(t, resolver, reviewedDigest)
	if len(second.Errors) != 0 || first.Data["approveAdapterDefinition"].(map[string]any)["semanticDigest"] != second.Data["approveAdapterDefinition"].(map[string]any)["semanticDigest"] || second.Data["approveAdapterDefinition"].(map[string]any)["reviewed"] != true {
		t.Fatalf("repeated adapter approval = %#v, %#v", first, second)
	}
}

func rustAPIPortAdapterCancellation(t *testing.T) {
	t.Helper()
	resolver, service := openAdapterParityService(t)
	pending := installAdapterParityDefinition(t, service, "v1", "", map[string]any{"kind": "none"}, "lookup")
	conversation, err := resolver.Store.EnsurePrimaryConversation(context.Background(), "openrouter", "", time.Now())
	if err != nil {
		t.Fatal(err)
	}
	if interventions := rustAPIAdapterInterventions(t, resolver, conversation.ID); len(interventions) != 1 {
		t.Fatalf("pending adapter review interventions = %#v", interventions)
	}
	removed := rustAPICancelAdapter(t, resolver, pending.SemanticDigest)
	if len(removed.Errors) != 0 || removed.Data["cancelAdapterDefinition"] != true {
		t.Fatalf("cancel = %#v", removed)
	}
	snapshot, err := service.Snapshot()
	if err != nil {
		t.Fatal(err)
	}
	if len(snapshot.Definitions) != 0 {
		t.Fatalf("cancelled definitions = %#v", snapshot.Definitions)
	}
	if interventions := rustAPIAdapterInterventions(t, resolver, conversation.ID); len(interventions) != 0 {
		t.Fatalf("cancelled adapter interventions = %#v", interventions)
	}
	response := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(context.Background()), `query { adapterDefinitions { semanticDigest } }`, nil)
	if len(response.Errors) != 0 || len(response.Data["adapterDefinitions"].([]any)) != 0 {
		t.Fatalf("cancelled adapter GraphQL definitions = %#v", response)
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
	resolver, service := openAdapterParityService(t)
	first := installAdapterParityDefinition(t, service, "v1", "", map[string]any{"kind": "none"}, "lookup")
	approved, err := service.Approve(context.Background(), first.SemanticDigest)
	if err != nil {
		t.Fatal(err)
	}
	conversation, err := resolver.Store.EnsurePrimaryConversation(context.Background(), "openrouter", "", time.Now())
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
	response := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(context.Background()), `query {
  adapterDefinitions { semanticDigest superseded connectionCount }
}`, nil)
	if len(response.Errors) != 0 {
		t.Fatalf("replacement GraphQL errors = %#v", response.Errors)
	}
	definitions := response.Data["adapterDefinitions"].([]any)
	if len(definitions) != 1 || definitions[0].(map[string]any)["semanticDigest"] != updated.SemanticDigest || definitions[0].(map[string]any)["superseded"] != false || definitions[0].(map[string]any)["connectionCount"] != float64(1) {
		t.Fatalf("replacement GraphQL definitions = %#v", definitions)
	}
	interventions := rustAPIAdapterInterventions(t, resolver, conversation.ID)
	for _, intervention := range interventions {
		if intervention["semanticDigest"] == first.SemanticDigest {
			t.Fatalf("superseded adapter intervention remained: %#v", interventions)
		}
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
	preApproval, err := service.Snapshot()
	if err != nil {
		t.Fatal(err)
	}
	transition := definitionModel(second, preApproval).Transition
	if transition == nil || len(transition.AddedOperations) != 1 || transition.AddedOperations[0] != "get_item" || len(transition.RemovedOperations) != 1 || transition.RemovedOperations[0] != "lookup" || transition.AffectedConnections != 1 {
		t.Fatalf("breaking adapter transition = %#v", transition)
	}
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
	snapshot, err := service.Snapshot()
	if err != nil || len(snapshot.Connections) != 1 {
		t.Fatalf("initial authentication connection = %#v, %v", snapshot.Connections, err)
	}
	connectionID := snapshot.Connections[0].ConnectionID
	replacement := installAdapterParityDefinition(t, service, "v2", approved.SemanticDigest, map[string]any{"kind": "credential", "setup": map[string]any{"credential_type": "API key", "setup_url": "https://example.com/keys", "instructions": []string{"Create a key."}, "input": map[string]any{"kind": "fields", "fields": []any{map[string]any{"id": "token", "label": "API key"}}}}, "request_auth": map[string]any{"language": "lua", "source": "return function(input) return {} end"}}, "lookup")
	if _, err := service.Approve(context.Background(), replacement.SemanticDigest); err != nil {
		t.Fatal(err)
	}
	snapshot, err = service.Snapshot()
	if err != nil {
		t.Fatal(err)
	}
	if len(snapshot.Connections) != 1 || snapshot.Connections[0].ConnectionID != connectionID || snapshot.Connections[0].Status != "authentication_required" || len(snapshot.Connections[0].AllowedOperations) != 0 {
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
	resolver, service := openAdapterParityService(t)
	if err := service.SetOAuthCallback("http://localhost:3737/adapter/oauth/callback"); err != nil {
		t.Fatal(err)
	}
	oauth, err := service.OAuthSnapshot()
	if err != nil || len(oauth.Profiles) == 0 {
		t.Fatalf("OAuth profiles = %#v, %v", oauth.Profiles, err)
	}
	profile := oauth.Profiles[0]
	document := []byte(`{"installed":{"client_id":"ordinary-client","client_secret":"secret-marker","discard":"raw-upload-marker"}}`)
	wrongClientType := []byte(`{"web":{"client_id":"ordinary-client","client_secret":"secret-marker"}}`)
	if _, err := service.ImportOAuthApplication(profile.ProfileDigest, nil, wrongClientType, nil); err == nil {
		t.Fatal("wrong OAuth client type was accepted")
	}
	projectLabel := "Personal APIs"
	application, err := service.ImportOAuthApplication(profile.ProfileDigest, &projectLabel, document, nil)
	if err != nil {
		t.Fatal(err)
	}
	repeated, err := service.ImportOAuthApplication(profile.ProfileDigest, &projectLabel, document, nil)
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
	graphqlDocument := base64.StdEncoding.EncodeToString(document)
	response := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(context.Background()), `mutation($profile:String!, $document:String!) {
  importAdapterOauthApplication(input: { profileDigest: $profile, projectLabel: "Personal APIs", clientDocumentBase64: $document }) {
    applicationId clientId projectLabel grantCount accountCount revision
  }
}`, map[string]any{"profile": profile.ProfileDigest, "document": graphqlDocument})
	if len(response.Errors) != 0 {
		t.Fatalf("OAuth import GraphQL errors = %#v", response.Errors)
	}
	imported, ok := response.Data["importAdapterOauthApplication"].(map[string]any)
	if !ok || imported["clientId"] != "ordinary-client" || imported["projectLabel"] != "Personal APIs" || imported["grantCount"] != float64(0) || imported["accountCount"] != float64(0) {
		t.Fatalf("OAuth import GraphQL projection = %#v", response.Data)
	}
	stateResponse := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(context.Background()), `query { adapterManagement { oauthState { applications { applicationId clientId projectLabel grantCount accountCount } } } }`, nil)
	if len(stateResponse.Errors) != 0 {
		t.Fatalf("OAuth state GraphQL errors = %#v", stateResponse.Errors)
	}
	state := stateResponse.Data["adapterManagement"].(map[string]any)["oauthState"].(map[string]any)
	applications := state["applications"].([]any)
	if len(applications) != 1 || applications[0].(map[string]any)["clientId"] != "ordinary-client" {
		t.Fatalf("OAuth state GraphQL projection = %#v", state)
	}
	paths, err := home.FromRoot(resolver.home.Name())
	if err != nil {
		t.Fatal(err)
	}
	applicationDir, err := paths.AdapterOAuthApplicationDir(application.ApplicationID)
	if err != nil {
		t.Fatal(err)
	}
	credentialPath := filepath.Join(applicationDir, "credentials", application.CredentialGeneration+".json")
	credential, err := os.ReadFile(credentialPath)
	if err != nil || !bytes.Contains(credential, []byte("secret-marker")) || bytes.Contains(credential, []byte("raw-upload-marker")) {
		t.Fatalf("persisted OAuth credential = %s, %v", credential, err)
	}
}

func rustAPIPortAdapterOAuthSetup(t *testing.T) {
	t.Helper()
	resolver, service := openAdapterParityService(t)
	if err := service.SetOAuthCallback("http://localhost:3737/adapter/oauth/callback"); err != nil {
		t.Fatal(err)
	}
	oauth, err := service.OAuthSnapshot()
	if err != nil || len(oauth.Profiles) == 0 {
		t.Fatalf("OAuth profiles = %#v, %v", oauth.Profiles, err)
	}
	profile := oauth.Profiles[0]
	first := installAdapterParityDefinitionNamed(t, service, "oauth-service-one", "v1", "", map[string]any{"kind": "oauth2_authorization_code_pkce", "profile_digest": profile.ProfileDigest}, "lookup")
	second := installAdapterParityDefinitionNamed(t, service, "oauth-service-two", "v1", "", map[string]any{"kind": "oauth2_authorization_code_pkce", "profile_digest": profile.ProfileDigest}, "lookup-second")
	firstReviewed, err := service.Approve(context.Background(), first.SemanticDigest)
	if err != nil {
		t.Fatal(err)
	}
	conversation, err := resolver.Store.EnsurePrimaryConversation(context.Background(), "openrouter", "", time.Now())
	if err != nil {
		t.Fatal(err)
	}
	interventions := rustAPIAdapterInterventions(t, resolver, conversation.ID)
	reviewPosition, setupPosition := -1, -1
	for index, value := range interventions {
		typeName, _ := value["__typename"].(string)
		if typeName == "AdapterDefinition" && reviewPosition == -1 {
			reviewPosition = index
		}
		if typeName == "AdapterOauthClientSetupIntervention" && setupPosition == -1 {
			setupPosition = index
		}
	}
	if reviewPosition == -1 || setupPosition == -1 || reviewPosition >= setupPosition {
		t.Fatalf("OAuth review/setup ordering = %#v", interventions)
	}
	secondReviewed, err := service.Approve(context.Background(), second.SemanticDigest)
	if err != nil {
		t.Fatal(err)
	}
	interventions = rustAPIAdapterInterventions(t, resolver, conversation.ID)
	var setups []map[string]any
	for _, value := range interventions {
		if value["__typename"] == "AdapterOauthClientSetupIntervention" {
			setups = append(setups, value)
		}
	}
	if len(setups) != 1 {
		t.Fatalf("shared OAuth setup interventions = %#v", interventions)
	}
	dependencies, ok := setups[0]["dependentDefinitions"].([]any)
	if !ok || len(dependencies) != 2 {
		t.Fatalf("shared OAuth setup dependencies = %#v", setups[0])
	}
	want := map[string]bool{firstReviewed.SemanticDigest: true, secondReviewed.SemanticDigest: true}
	for _, value := range dependencies {
		dependency, ok := value.(map[string]any)
		if !ok || !want[dependency["semanticDigest"].(string)] {
			t.Fatalf("shared OAuth setup dependency = %#v", value)
		}
	}
	document := []byte(`{"installed":{"client_id":"shared-client","client_secret":"shared-secret","discard":"raw-upload-marker"}}`)
	application, err := service.ImportOAuthApplication(profile.ProfileDigest, nil, document, nil)
	if err != nil {
		t.Fatal(err)
	}
	accountResponse := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(context.Background()), `query($conversation:String!) {
  pendingHumanInterventions(conversationId: $conversation, first: 50) {
    __typename
    ... on AdapterOauthAccountSetupIntervention {
      setupKey nextAction { kind applicationId } dependentDefinitions { semanticDigest }
    }
  }
}`, map[string]any{"conversation": conversation.ID})
	if len(accountResponse.Errors) != 0 {
		t.Fatalf("OAuth account setup errors = %#v", accountResponse.Errors)
	}
	accountValues := accountResponse.Data["pendingHumanInterventions"].([]any)
	if len(accountValues) != 1 {
		t.Fatalf("OAuth account setup interventions = %#v", accountValues)
	}
	accountSetup := accountValues[0].(map[string]any)
	if accountSetup["setupKey"] != "application:"+application.ApplicationID {
		t.Fatalf("OAuth account setup key = %#v", accountSetup)
	}
	dependencies, ok = accountSetup["dependentDefinitions"].([]any)
	if !ok || len(dependencies) != 2 {
		t.Fatalf("OAuth account setup dependencies = %#v", accountSetup)
	}
	unauthorizedResponse := rustAPIRawGraphQLContext(t, resolver, context.Background(), `mutation($input: StartAdapterOauthSetupInput!) {
	  startAdapterOauthSetup(input: $input) { attemptId authorizationUrl }
}`, map[string]any{"input": map[string]any{
		"applicationId": application.ApplicationID, "expectedApplicationRevision": application.Revision,
		"semanticDigest": firstReviewed.SemanticDigest, "operationIds": []string{"lookup"},
	}})
	if len(unauthorizedResponse.Errors) == 0 {
		t.Fatal("unauthorized OAuth setup was accepted")
	}
	startResponse := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(context.Background()), `mutation($input: StartAdapterOauthSetupInput!) {
  startAdapterOauthSetup(input: $input) { attemptId authorizationUrl }
}`, map[string]any{"input": map[string]any{
		"applicationId": application.ApplicationID, "expectedApplicationRevision": application.Revision,
		"semanticDigest": firstReviewed.SemanticDigest, "operationIds": []string{"lookup"},
	}})
	if len(startResponse.Errors) != 0 {
		t.Fatalf("OAuth setup errors = %#v", startResponse.Errors)
	}
	attempt, ok := startResponse.Data["startAdapterOauthSetup"].(map[string]any)
	if !ok || attempt["attemptId"] == "" || attempt["authorizationUrl"] == "" {
		t.Fatalf("OAuth setup attempt = %#v", startResponse.Data)
	}
	parsed, err := url.Parse(attempt["authorizationUrl"].(string))
	if err != nil || parsed.Query().Get("redirect_uri") != "http://localhost:3737/adapter/oauth/callback" {
		t.Fatalf("OAuth setup redirect URI = %#v, %v", attempt, err)
	}
}

func rustAPIAdapterInterventions(t *testing.T, resolver *Resolver, conversationID string) []map[string]any {
	t.Helper()
	response := rustAPIRawGraphQLContext(t, resolver, auth.WithDesktopAccess(context.Background()), `query {
  pendingHumanInterventions(conversationId: "`+conversationID+`", first: 50) {
    __typename
    ... on AdapterDefinition { semanticDigest }
    ... on AdapterOauthClientSetupIntervention {
      profileDigest
      dependentDefinitions { semanticDigest }
    }
  }
}`, nil)
	if len(response.Errors) != 0 {
		t.Fatalf("adapter interventions errors = %#v", response.Errors)
	}
	values, ok := response.Data["pendingHumanInterventions"].([]any)
	if !ok {
		t.Fatalf("adapter interventions data = %#v", response.Data)
	}
	result := make([]map[string]any, 0, len(values))
	for _, value := range values {
		entry, ok := value.(map[string]any)
		if !ok {
			t.Fatalf("adapter intervention = %#v", value)
		}
		result = append(result, entry)
	}
	return result
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
	integrations, err := resolver.adapterIntegrations(auth.WithDesktopAccess(context.Background()))
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
