package webtool

import (
	"encoding/base64"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/artifact"
	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

type kernelTestCall struct {
	method, path, authorization, code string
	body                              map[string]any
}

type kernelTestServer struct {
	mu          sync.Mutex
	calls       []kernelTestCall
	interaction func(string) (int, map[string]any)
}

func (server *kernelTestServer) handler(response http.ResponseWriter, request *http.Request) {
	call := kernelTestCall{method: request.Method, path: request.URL.Path, authorization: request.Header.Get("Authorization")}
	if request.Body != nil {
		_ = json.NewDecoder(request.Body).Decode(&call.body)
		call.code, _ = call.body["code"].(string)
	}
	server.mu.Lock()
	server.calls = append(server.calls, call)
	server.mu.Unlock()
	response.Header().Set("Content-Type", "application/json")
	switch {
	case request.Method == http.MethodPost && request.URL.Path == "/browsers":
		_, _ = response.Write([]byte(`{"session_id":"kernel-session_1"}`))
	case request.Method == http.MethodPost && request.URL.Path == "/browsers/kernel-session_1/playwright/execute":
		status, value := http.StatusOK, kernelTestExecution(200)
		if server.interaction != nil {
			status, value = server.interaction(call.code)
		}
		response.WriteHeader(status)
		_ = json.NewEncoder(response).Encode(value)
	case request.Method == http.MethodDelete && request.URL.Path == "/browsers/kernel-session_1":
		response.WriteHeader(http.StatusNoContent)
	default:
		http.Error(response, "unexpected", http.StatusNotFound)
	}
}

func kernelTestExecution(mainStatus int) map[string]any {
	return map[string]any{"success": true, "result": map[string]any{"ok": true, "main_document_status": mainStatus,
		"snapshot": map[string]any{"url": "https://8.8.8.8/kernel", "title": "Kernel", "text": "Remote page",
			"elements":   []any{map[string]any{"reference": "e1", "role": "button", "name": "Continue", "disabled": false}},
			"screenshot": base64.StdEncoding.EncodeToString([]byte("image")), "width": 800, "height": 600}}}
}

type kernelTestFixture struct {
	service   *Service
	database  *store.Store
	accounts  *provider.AccountService
	artifacts *artifact.Service
	kernelID  string
}

func newKernelTestFixture(t *testing.T, endpoint string, obscuraFirst bool) kernelTestFixture {
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
	database, err := store.Open(t.Context(), paths.Database())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = database.Close() })
	accounts, err := provider.NewAccountService(paths.Root(), database)
	if err != nil || accounts.Initialize(t.Context(), time.Now()) != nil {
		t.Fatalf("accounts = %v", err)
	}
	secret, _ := provider.NewSecret("kernel-key")
	kernel, err := accounts.CreateSecretAccount(t.Context(), kernelProvider, "Test", secret, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	route := []string{kernel.ID}
	if obscuraFirst {
		route = []string{obscuraAccount, kernel.ID}
	}
	if err := database.SaveBrowserProviderRoute(t.Context(), route, time.Now()); err != nil {
		t.Fatal(err)
	}
	artifacts, err := artifact.New(root, database, nil)
	if err != nil {
		t.Fatal(err)
	}
	executable, _ := os.Executable()
	service, err := New(database, accounts, nil, artifacts, paths.Root(), executable, 2, 1024)
	if err != nil {
		t.Fatal(err)
	}
	service.endpoints[kernelProvider] = endpoint
	t.Cleanup(service.Close)
	return kernelTestFixture{service: service, database: database, accounts: accounts, artifacts: artifacts, kernelID: kernel.ID}
}

func TestKernelScriptsKeepModelInputsAsDataAndBlockPrivateTargets(t *testing.T) {
	malicious := `";globalThis.pwned=true;//`
	script, err := kernelScript(BrowseInteractName, map[string]any{"ref": "e1", "action": "fill", "value": malicious}, nil)
	if err != nil || !strings.Contains(script, `const value="\";globalThis.pwned=true;//"`) || strings.Contains(script, `const value="";globalThis`) {
		t.Fatalf("encoded script = %q, %v", script, err)
	}
	for _, fragment := range []string{"privateIpv4", "privateIpv6", "blockedName", "route.abort()", "'hidden','password','file'"} {
		if !strings.Contains(script, fragment) {
			t.Fatalf("request or review guard %q is absent", fragment)
		}
	}
	response, err := kernelBrowserResponse(&kernelRawSnapshot{URL: "https://8.8.8.8/page", Screenshot: "invalid", Width: 800, Height: 600}, 12000)
	if err != nil || response.Snapshot == nil || response.Screenshot != nil {
		t.Fatalf("optional screenshot invalidated snapshot: %#v, %v", response, err)
	}
}

func TestKernelRouteSwitchReusesRemoteSessionAndPublicRevisions(t *testing.T) {
	stub := &kernelTestServer{interaction: func(code string) (int, map[string]any) {
		if strings.Contains(code, "waitForFunction") {
			return http.StatusGatewayTimeout, nil
		}
		return http.StatusOK, kernelTestExecution(200)
	}}
	remote := httptest.NewServer(http.HandlerFunc(stub.handler))
	defer remote.Close()
	fixture := newKernelTestFixture(t, remote.URL, true)
	opened := fixture.service.ExecuteBrowser(t.Context(), "conversation:one", BrowseOpenName,
		json.RawMessage(`{"url":"https://1.1.1.1/start"}`), "test:open")
	if !opened.Success {
		t.Fatalf("Obscura open = %s", opened.Model)
	}
	authority, err := fixture.service.BrowserAuthority(t.Context(), "conversation:one", BrowseSwitchName,
		json.RawMessage(`{"snapshot_revision":1,"url":"https://8.8.8.8/kernel"}`))
	if err != nil || authority.ProviderAccountID != fixture.kernelID || authority.RoutePosition != 1 {
		t.Fatalf("switch authority = %#v, %v", authority, err)
	}
	switched := fixture.service.ExecuteBrowser(t.Context(), "conversation:one", BrowseSwitchName,
		json.RawMessage(`{"snapshot_revision":1,"url":"https://8.8.8.8/kernel"}`), "test:switch")
	if !switched.Success || !strings.Contains(string(switched.Model), `"provider":"kernel"`) ||
		!strings.Contains(string(switched.Model), `"snapshot_revision":2`) || strings.Contains(string(switched.Model), "screenshot") {
		t.Fatalf("switch = stored %s, model %s", switched.Stored, switched.Model)
	}
	active, err := fixture.service.BrowserAuthority(t.Context(), "conversation:one", BrowseOpenName,
		json.RawMessage(`{"url":"https://8.8.8.8/next"}`))
	if err != nil || active.ProviderAccountID != fixture.kernelID || active.RoutePosition != 1 {
		t.Fatalf("active provider authority = %#v, %v", active, err)
	}
	snapshot := fixture.service.ExecuteBrowser(t.Context(), "conversation:one", BrowseSnapshotName, json.RawMessage(`{}`), "test:snapshot")
	if !snapshot.Success || !strings.Contains(string(snapshot.Model), `"snapshot_revision":3`) {
		t.Fatalf("snapshot = %s", snapshot.Model)
	}
	waited := fixture.service.ExecuteBrowser(t.Context(), "conversation:one", BrowseWaitName,
		json.RawMessage(`{"condition":{"text":"ready"},"timeout_ms":100}`), "test:wait")
	recovered := fixture.service.ExecuteBrowser(t.Context(), "conversation:one", BrowseSnapshotName, json.RawMessage(`{}`), "test:recover")
	if waited.Success || !strings.Contains(string(waited.Model), `"error":"timeout"`) || !recovered.Success {
		t.Fatalf("wait recovery = %s, %s", waited.Model, recovered.Model)
	}
	closed := fixture.service.ExecuteBrowser(t.Context(), "conversation:one", BrowseCloseName, json.RawMessage(`{}`), "test:close")
	if !closed.Success || !strings.Contains(string(closed.Model), `"provider":"kernel"`) {
		t.Fatalf("close = %s", closed.Model)
	}
	stub.mu.Lock()
	calls := append([]kernelTestCall(nil), stub.calls...)
	stub.mu.Unlock()
	if len(calls) != 6 || calls[0].body["headless"] != true || calls[0].body["timeout_seconds"] != float64(1800) ||
		calls[1].body["timeout_sec"] != float64(30) || calls[1].path != "/browsers/kernel-session_1/playwright/execute" ||
		calls[2].path != calls[1].path || calls[5].method != http.MethodDelete {
		t.Fatalf("Kernel calls = %#v", calls)
	}
	for _, call := range calls {
		if call.authorization != "Bearer kernel-key" {
			t.Fatalf("authorization = %q", call.authorization)
		}
	}
	failureStub := &kernelTestServer{interaction: func(string) (int, map[string]any) {
		return http.StatusOK, map[string]any{"success": false, "error": "navigation failed"}
	}}
	failureRemote := httptest.NewServer(http.HandlerFunc(failureStub.handler))
	defer failureRemote.Close()
	failureFixture := newKernelTestFixture(t, failureRemote.URL, false)
	if err := failureFixture.database.SaveBrowserProviderRoute(t.Context(), []string{failureFixture.kernelID, obscuraAccount}, time.Now()); err != nil {
		t.Fatal(err)
	}
	failed := failureFixture.service.ExecuteBrowser(t.Context(), "conversation:failed-open", BrowseOpenName,
		json.RawMessage(`{"url":"https://8.8.8.8/start"}`), "test:failed-open")
	authority, err = failureFixture.service.BrowserAuthority(t.Context(), "conversation:failed-open", BrowseSwitchName,
		json.RawMessage(`{"url":"https://1.1.1.1/recover"}`))
	switched = failureFixture.service.ExecuteBrowser(t.Context(), "conversation:failed-open", BrowseSwitchName,
		json.RawMessage(`{"url":"https://1.1.1.1/recover"}`), "test:recover")
	if failed.Success || strings.Contains(string(failed.Model), "snapshot_revision") || err != nil || authority.RoutePosition != 1 || !switched.Success {
		t.Fatalf("failed open switch = %s, %#v, %v", failed.Model, authority, err)
	}
}

func TestKernelInteractionMarksMainDocumentAndLostResponsesUncertain(t *testing.T) {
	for _, test := range []struct {
		name                       string
		interact                   func(string) (int, map[string]any)
		wantDetail                 string
		wantRevision               string
		wantUncertain, wantRecover bool
		wantDelete                 bool
	}{
		{name: "main document 5xx", wantUncertain: true, wantRecover: true, wantRevision: `"snapshot_revision":2`, interact: func(code string) (int, map[string]any) {
			if strings.Contains(code, "recordMainDocument") {
				return http.StatusOK, kernelTestExecution(503)
			}
			return http.StatusOK, kernelTestExecution(200)
		}},
		{name: "provider 5xx", wantDetail: "HTTP 500", wantUncertain: true, wantRecover: true, wantRevision: `"snapshot_revision":1`, interact: func(code string) (int, map[string]any) {
			if strings.Contains(code, "recordMainDocument") {
				return http.StatusInternalServerError, nil
			}
			return http.StatusOK, kernelTestExecution(200)
		}},
		{name: "invalid result", wantDetail: "invalid result", wantUncertain: true, wantRecover: true, wantRevision: `"snapshot_revision":1`, interact: func(code string) (int, map[string]any) {
			if strings.Contains(code, "recordMainDocument") {
				return http.StatusOK, map[string]any{"success": true, "result": "invalid"}
			}
			return http.StatusOK, kernelTestExecution(200)
		}},
		{name: "safe diagnostic category", wantDetail: "execution error with stderr", wantUncertain: true, wantRecover: true, wantRevision: `"snapshot_revision":1`, interact: func(code string) (int, map[string]any) {
			if strings.Contains(code, "recordMainDocument") {
				return http.StatusOK, map[string]any{"success": false, "error": map[string]any{"api_key": "remove-me"}, "stderr": "private page text"}
			}
			return http.StatusOK, kernelTestExecution(200)
		}},
		{name: "missing backend", wantDetail: "HTTP 404", wantRevision: `"snapshot_revision":1`, wantDelete: true, interact: func(code string) (int, map[string]any) {
			if strings.Contains(code, "recordMainDocument") {
				return http.StatusNotFound, nil
			}
			return http.StatusOK, kernelTestExecution(200)
		}},
	} {
		t.Run(test.name, func(t *testing.T) {
			stub := &kernelTestServer{interaction: test.interact}
			remote := httptest.NewServer(http.HandlerFunc(stub.handler))
			defer remote.Close()
			fixture := newKernelTestFixture(t, remote.URL, false)
			if err := fixture.database.SaveBrowserProviderRoute(t.Context(), []string{fixture.kernelID, obscuraAccount}, time.Now()); err != nil {
				t.Fatal(err)
			}
			if result := fixture.service.ExecuteBrowser(t.Context(), "task:task:0123456789abcdef0123456789abcdef:1", BrowseOpenName,
				json.RawMessage(`{"url":"https://8.8.8.8/start"}`), "test:open"); !result.Success {
				t.Fatalf("open = %s", result.Model)
			}
			result := fixture.service.ExecuteBrowser(t.Context(), "task:task:0123456789abcdef0123456789abcdef:1", BrowseInteractName,
				json.RawMessage(`{"snapshot_revision":1,"ref":"e1","action":"click"}`), "test:interact")
			if result.Success || result.OutcomeUncertain != test.wantUncertain || !strings.Contains(string(result.Model), test.wantRevision) ||
				test.wantDetail != "" && !strings.Contains(string(result.Model), test.wantDetail) || strings.Contains(string(result.Model), "remove-me") ||
				strings.Contains(string(result.Model), "private page text") {
				t.Fatalf("interaction = %#v", result)
			}
			if test.wantRecover {
				recovered := fixture.service.ExecuteBrowser(t.Context(), "task:task:0123456789abcdef0123456789abcdef:1", BrowseSnapshotName, json.RawMessage(`{}`), "test:recover")
				if !recovered.Success {
					t.Fatalf("session did not recover: %s", recovered.Model)
				}
			} else if _, err := fixture.service.BrowserAuthority(t.Context(), "task:task:0123456789abcdef0123456789abcdef:1", BrowseSwitchName,
				json.RawMessage(`{"snapshot_revision":1,"url":"https://1.1.1.1/recover"}`)); err != nil {
				t.Fatalf("lost switch route: %v", err)
			}
			if test.wantDelete {
				stub.mu.Lock()
				deleted := stub.calls[len(stub.calls)-1].method == http.MethodDelete
				stub.mu.Unlock()
				if !deleted {
					t.Fatal("unusable Kernel backend was not deleted")
				}
			}
		})
	}
}

func TestKernelSessionStopsAtCredentialRevisionChange(t *testing.T) {
	stub := &kernelTestServer{}
	remote := httptest.NewServer(http.HandlerFunc(stub.handler))
	defer remote.Close()
	fixture := newKernelTestFixture(t, remote.URL, false)
	if result := fixture.service.ExecuteBrowser(t.Context(), "conversation:revision", BrowseOpenName,
		json.RawMessage(`{"url":"https://8.8.8.8/start"}`), "test:open"); !result.Success {
		t.Fatalf("open = %s", result.Model)
	}
	replacement, _ := provider.NewSecret("replacement-key")
	if _, err := fixture.accounts.SaveSecret(t.Context(), fixture.kernelID, replacement, time.Now()); err != nil {
		t.Fatal(err)
	}
	result := fixture.service.ExecuteBrowser(t.Context(), "conversation:revision", BrowseSnapshotName, json.RawMessage(`{}`), "test:snapshot")
	if result.Success || !strings.Contains(string(result.Model), "session_not_found") {
		t.Fatalf("changed credential session = %s", result.Model)
	}
}

func TestKernelUploadBindsOneExactTaskArtifactBeforeTransmission(t *testing.T) {
	stub := &kernelTestServer{}
	remote := httptest.NewServer(http.HandlerFunc(stub.handler))
	defer remote.Close()
	fixture := newKernelTestFixture(t, remote.URL, false)
	task, err := fixture.database.CreateTask(t.Context(), "task:0123456789abcdef0123456789abcdef", "Upload", "correlation:test:upload", time.Now())
	if err != nil {
		t.Fatal(err)
	}
	mediaType := "text/plain"
	created, err := fixture.artifacts.CreateLocal(t.Context(), artifact.LocalInput{Owner: store.ArtifactOwner{ObjectType: "task", ObjectID: task.ID},
		Title: "Receipt", Kind: "source_file", Filename: "receipt.txt", Bytes: []byte("exact private bytes"), MediaType: &mediaType,
		CreatedByActorID: "human:local"})
	if err != nil {
		t.Fatal(err)
	}
	owner := "task:" + task.ID + ":1"
	if result := fixture.service.ExecuteBrowser(t.Context(), owner, BrowseOpenName, json.RawMessage(`{"url":"https://8.8.8.8/start"}`), "test:open"); !result.Success {
		t.Fatalf("open = %s", result.Model)
	}
	arguments, _ := json.Marshal(map[string]any{"snapshot_revision": 1, "ref": "e1", "action": "upload_file",
		"artifact_id": created.Artifact.ID, "artifact_version_id": created.CurrentVersion.ID})
	stub.mu.Lock()
	before := len(stub.calls)
	stub.mu.Unlock()
	authority, err := fixture.service.BrowserAuthority(t.Context(), owner, BrowseInteractName, arguments)
	stub.mu.Lock()
	afterReview := len(stub.calls)
	stub.mu.Unlock()
	if err != nil || authority.ArtifactID != created.Artifact.ID || authority.ArtifactVersionID != created.CurrentVersion.ID ||
		authority.ArtifactFilename != "receipt.txt" || authority.ArtifactByteSize != 19 || afterReview != before {
		t.Fatalf("upload authority = %#v, %v, calls %d -> %d", authority, err, before, afterReview)
	}
	result := fixture.service.ExecuteBrowser(t.Context(), owner, BrowseInteractName, arguments, "action:approved")
	if !result.Success || !strings.Contains(string(result.Model), `"upload":{"artifact_id":"`+created.Artifact.ID+`"`) ||
		!strings.Contains(string(result.Model), `"artifact_version_id":"`+created.CurrentVersion.ID+`"`) ||
		!strings.Contains(string(result.Model), `"filename":"receipt.txt"`) || !strings.Contains(string(result.Model), `"media_type":"text/plain"`) ||
		!strings.Contains(string(result.Model), `"byte_size":19`) {
		t.Fatalf("upload = %s", result.Model)
	}
	stub.mu.Lock()
	code := stub.calls[len(stub.calls)-1].code
	stub.mu.Unlock()
	if !strings.Contains(code, "locator.setInputFiles(upload)") || !strings.Contains(code, base64.StdEncoding.EncodeToString([]byte("exact private bytes"))) ||
		!strings.Contains(code, `name:"receipt.txt"`) {
		t.Fatalf("upload script did not bind exact artifact: %s", code)
	}
	for _, invalid := range []struct{ owner, artifactID string }{{"conversation:one", created.Artifact.ID}, {"task:task:fedcba9876543210fedcba9876543210:1", created.Artifact.ID}, {owner, "artifact:wrong"}} {
		var input map[string]any
		_ = json.Unmarshal(arguments, &input)
		input["artifact_id"] = invalid.artifactID
		raw, _ := json.Marshal(input)
		if _, err := fixture.service.BrowserAuthority(t.Context(), invalid.owner, BrowseInteractName, raw); err == nil {
			t.Fatalf("invalid upload was accepted for %q", invalid.owner)
		}
	}
}
