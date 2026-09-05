package webtool

import (
	"context"
	"encoding/json"
	"fmt"
	"github.com/coder/websocket"
	"github.com/coder/websocket/wsjson"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

func init() {
	port := ""
	for i, argument := range os.Args {
		if argument == "--port" && i+1 < len(os.Args) {
			port = os.Args[i+1]
		}
	}
	if port == "" {
		return
	}
	listener, err := net.Listen("tcp4", "127.0.0.1:"+port)
	if err != nil {
		os.Exit(2)
	}
	fmt.Fprintln(os.Stderr, "Obscura CDP server listening on ws://127.0.0.1:"+port)
	_ = http.Serve(listener, http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		conn, err := websocket.Accept(w, r, nil)
		if err != nil {
			return
		}
		defer conn.CloseNow()
		invalid := false
		for {
			var request struct {
				ID        int            `json:"id"`
				Method    string         `json:"method"`
				Params    map[string]any `json:"params"`
				SessionID string         `json:"sessionId"`
			}
			if wsjson.Read(r.Context(), conn, &request) != nil {
				return
			}
			result := any(map[string]any{})
			failure := false
			switch request.Method {
			case "Target.createTarget":
				result = map[string]any{"targetId": "page"}
			case "Target.attachToTarget":
				result = map[string]any{"sessionId": "session"}
			case "Page.getFrameTree":
				result = map[string]any{"frameTree": map[string]any{"frame": map[string]any{"id": "frame"}}}
			case "Page.navigate":
				failure = request.Params["url"] == "https://1.1.1.1/fail"
			case "Page.captureScreenshot":
				result = map[string]any{"data": "aW1hZ2U="}
			case "Runtime.evaluate":
				expression, _ := request.Params["expression"].(string)
				if strings.Contains(expression, `const value = "loss"`) {
					os.Exit(0)
				}
				if strings.Contains(expression, `const value = "invalid"`) {
					invalid = true
				}
				for _, event := range []struct{ value, kind, frame string }{{"server-error", "Document", "frame"}, {"asset-error", "Script", "frame"}, {"child-error", "Document", "child"}} {
					if strings.Contains(expression, `const value = "`+event.value+`"`) {
						_ = wsjson.Write(r.Context(), conn, map[string]any{"method": "Network.responseReceived", "sessionId": request.SessionID, "params": map[string]any{"type": event.kind, "frameId": event.frame, "response": map[string]any{"status": 502}}})
					}
				}
				if strings.Contains(expression, `const value = "oversized"`) {
					_ = conn.Write(r.Context(), websocket.MessageText, []byte(strings.Repeat("x", browserFrameLimit+1)))
					return
				}
				value := any(true)
				if strings.Contains(expression, "const body=") {
					value = json.RawMessage(`{"url":"https://1.1.1.1/page","title":"Page","text":"Visible text","width":640,"height":480,"elements":[{"reference":"e1","role":"link","name":"Next","href":"https://8.8.8.8/next","disabled":false},{"reference":"e2","role":"button","name":"Submit","disabled":false,"submission":{"destination":"https://1.1.1.1/submit","method":"POST","fields":[{"name":"q","value":"safe"}],"omitted_control_count":1,"truncated":false}},{"reference":"e3","role":"link","name":"Private","href":"http://127.0.0.1/private","disabled":false}]}`)
					if invalid {
						value = "invalid"
					}
				}
				result = map[string]any{"result": map[string]any{"value": value}}
			}
			response := map[string]any{"id": request.ID, "sessionId": request.SessionID, "result": result}
			if failure {
				delete(response, "result")
				response["error"] = map[string]any{"code": -32000, "message": "navigation failed"}
			}
			if wsjson.Write(r.Context(), conn, response) != nil {
				return
			}
		}
	}))
	os.Exit(0)
}

func TestBrowserWorkerProtocolPolicyAndLifecycle(t *testing.T) {
	ctx := context.Background()
	database, err := store.Open(ctx, filepath.Join(t.TempDir(), "noema.sqlite3"))
	if err != nil {
		t.Fatal(err)
	}
	defer database.Close()
	accounts, err := provider.NewAccountService(t.TempDir(), database)
	if err != nil {
		t.Fatal(err)
	}
	if err := accounts.Initialize(ctx, time.Now()); err != nil {
		t.Fatal(err)
	}
	executable, err := os.Executable()
	if err != nil {
		t.Fatal(err)
	}
	service, err := New(database, accounts, nil, nil, t.TempDir(), executable, 2, 1024)
	if err != nil {
		t.Fatal(err)
	}
	defer service.Close()
	if !service.BrowserAvailable(ctx) {
		t.Fatal("browser is unavailable")
	}
	service.browserPath = ""
	if !service.BrowserAvailable(ctx) {
		t.Fatal("installable default browser is unavailable")
	}
	service.browserPath = executable

	opened := service.ExecuteBrowser(ctx, "conversation:one", BrowseOpenName,
		json.RawMessage(`{"url":"https://1.1.1.1/start"}`), "test:open")
	if !opened.Success || !strings.Contains(string(opened.Stored), `"screenshot"`) ||
		!strings.Contains(string(opened.Stored), `"Visible text"`) || strings.Contains(string(opened.Stored), `"submission"`) ||
		strings.Contains(string(opened.Stored), `127.0.0.1`) || strings.Contains(string(opened.Model), `"screenshot"`) ||
		!strings.Contains(string(opened.Model), `"snapshot_revision":1`) {
		t.Fatalf("open = stored %s, model %s, success %t", opened.Stored, opened.Model, opened.Success)
	}
	service.expireBrowser("conversation:one", currentBrowserSession(t, service, "conversation:one"))
	observed, err := database.URLWasObserved(ctx, "https://8.8.8.8/next")
	if err != nil || !observed {
		t.Fatalf("observed link = %t, %v", observed, err)
	}
	authority, err := service.BrowserAuthority(ctx, "conversation:one", BrowseInteractName,
		json.RawMessage(`{"snapshot_revision":1,"ref":"e2","action":"click"}`))
	if err != nil || authority.SnapshotRevision != 1 {
		t.Fatalf("authority = %#v, %v", authority, err)
	}
	contextValue := service.BrowserActionContext("conversation:one", BrowseInteractName,
		json.RawMessage(`{"snapshot_revision":1,"ref":"e2","action":"click"}`))
	target, _ := contextValue["target"].(map[string]any)
	page, _ := contextValue["page"].(map[string]any)
	if target["submission"] == nil || target["name"] != "Submit" || page["title"] != "Page" {
		t.Fatalf("submission context = %#v", contextValue)
	}
	clicked := service.ExecuteBrowser(ctx, "conversation:one", BrowseInteractName,
		json.RawMessage(`{"snapshot_revision":1,"ref":"e2","action":"click"}`), "test:click")
	if !clicked.Success || !strings.Contains(string(clicked.Model), `"snapshot_revision":2`) {
		t.Fatalf("interact = %s, %t", clicked.Model, clicked.Success)
	}
	historyContext := service.BrowserActionContext("conversation:one", BrowseHistoryName,
		json.RawMessage(`{"snapshot_revision":2,"action":"back"}`))
	if historyContext["kind"] != "browser_history" || historyContext["page"].(map[string]any)["title"] != "Page" {
		t.Fatalf("history context = %#v", historyContext)
	}
	stale := service.ExecuteBrowser(ctx, "conversation:one", BrowseHistoryName,
		json.RawMessage(`{"snapshot_revision":1,"action":"back"}`), "test:stale")
	if stale.Success || !strings.Contains(string(stale.Model), "stale_snapshot") {
		t.Fatalf("stale = %s", stale.Model)
	}
	closed := service.ExecuteBrowser(ctx, "conversation:one", BrowseCloseName, json.RawMessage(`{}`), "test:close")
	closedAgain := service.ExecuteBrowser(ctx, "conversation:one", BrowseCloseName, json.RawMessage(`{}`), "test:close-again")
	if !closed.Success || !closedAgain.Success {
		t.Fatal("close is not idempotent")
	}
	reopened := service.ExecuteBrowser(ctx, "conversation:one", BrowseOpenName,
		json.RawMessage(`{"url":"https://1.1.1.1/start"}`), "test:reopen")
	if !reopened.Success || service.CurrentBrowserAuthority(ctx, authority,
		json.RawMessage(`{"snapshot_revision":1,"ref":"e2","action":"click"}`)) {
		t.Fatal("replacement session accepted old browser authority")
	}
	service.CloseBrowser("conversation:one")

	opened = service.ExecuteBrowser(ctx, "task:one:1", BrowseOpenName,
		json.RawMessage(`{"url":"https://1.1.1.1/start"}`), "test:loss-open")
	if !opened.Success {
		t.Fatalf("loss open = %s", opened.Model)
	}
	revision := currentBrowserRevision(t, service, "task:one:1")
	upload := service.ExecuteBrowser(ctx, "task:one:1", BrowseInteractName,
		json.RawMessage(fmt.Sprintf(`{"snapshot_revision":%d,"ref":"e2","action":"upload_file","artifact_id":"artifact:one","artifact_version_id":"version:one"}`, revision)), "test:upload")
	if upload.Success || !strings.Contains(string(upload.Model), "retry_later") {
		t.Fatalf("upload = %#v", upload)
	}
	lost := service.ExecuteBrowser(ctx, "task:one:1", BrowseInteractName,
		json.RawMessage(fmt.Sprintf(`{"snapshot_revision":%d,"ref":"e2","action":"press_key","value":"loss"}`, revision)), "test:loss")
	if lost.Success || !lost.OutcomeUncertain || !strings.Contains(string(lost.Model), "outcome_uncertain") {
		t.Fatalf("lost interact = %#v", lost)
	}
	missing := service.ExecuteBrowser(ctx, "task:one:1", BrowseSnapshotName, json.RawMessage(`{}`), "test:missing")
	if missing.Success || !strings.Contains(string(missing.Model), "session_not_found") {
		t.Fatalf("missing = %s", missing.Model)
	}
	blocked := service.ExecuteBrowser(ctx, "conversation:blocked", BrowseOpenName,
		json.RawMessage(`{"url":"http://127.0.0.1/private"}`), "test:blocked")
	if blocked.Success || !strings.Contains(string(blocked.Model), "invalid_input") {
		t.Fatalf("blocked = %s", blocked.Model)
	}
	failed := service.ExecuteBrowser(ctx, "conversation:failed", BrowseOpenName,
		json.RawMessage(`{"url":"https://1.1.1.1/fail"}`), "test:failed")
	if failed.Success || currentBrowserSession(t, service, "conversation:failed").timer == nil || !strings.Contains(string(failed.Stored), "CDP Page.navigate failed (-32000)") {
		t.Fatalf("failed open = %#v", failed)
	}
	invalidOpen := service.ExecuteBrowser(ctx, "conversation:invalid", BrowseOpenName,
		json.RawMessage(`{"url":"https://1.1.1.1/start"}`), "test:invalid-open")
	invalidRevision := currentBrowserRevision(t, service, "conversation:invalid")
	invalid := service.ExecuteBrowser(ctx, "conversation:invalid", BrowseInteractName,
		json.RawMessage(fmt.Sprintf(`{"snapshot_revision":%d,"ref":"e2","action":"fill","value":"invalid"}`, invalidRevision)), "test:invalid")
	if !invalidOpen.Success || !invalid.OutcomeUncertain {
		t.Fatalf("invalid response = %#v", invalid)
	}
	filtered := BrowserModelPayload(json.RawMessage(`{"status":"succeeded","result":{"snapshot":{"text":"kept"},"screenshot":{"data":"private"}}}`))
	if strings.Contains(string(filtered), "private") || !strings.Contains(string(filtered), "kept") {
		t.Fatalf("filtered browser payload = %s", filtered)
	}

}

func currentBrowserRevision(t *testing.T, service *Service, owner string) uint64 {
	t.Helper()
	session := currentBrowserSession(t, service, owner)
	session.mu.Lock()
	defer session.mu.Unlock()
	return session.publicRevision
}

func currentBrowserSession(t *testing.T, service *Service, owner string) *browserSession {
	t.Helper()
	service.browserMu.Lock()
	session := service.browsers[owner]
	service.browserMu.Unlock()
	if session == nil {
		t.Fatal("browser session is unavailable")
	}
	return session
}

func TestObscuraTracksOnlyCurrentMainDocumentFailures(t *testing.T) {
	path, _ := os.Executable()
	process, err := startBrowserProcess(t.Context(), path, 1024)
	if err != nil {
		t.Fatal(err)
	}
	defer process.close()
	session := &browserSession{process: process, publicRevision: 1}
	for _, value := range []string{"server-error", "normal", "asset-error", "child-error"} {
		result, failure := executeObscuraBrowser(t.Context(), session, BrowseInteractName, map[string]any{"action": "click", "ref": "e2", "value": value})
		if failure != nil || (result.State == "outcome_uncertain") != (value == "server-error") {
			t.Fatalf("%s = %#v, %#v", value, result, failure)
		}
	}
}

func TestObscuraRejectsOversizedResponseAfterAction(t *testing.T) {
	path, _ := os.Executable()
	process, err := startBrowserProcess(t.Context(), path, 1024)
	if err != nil {
		t.Fatal(err)
	}
	defer process.close()
	_, failure := executeObscuraBrowser(t.Context(), &browserSession{process: process, publicRevision: 1}, BrowseInteractName, map[string]any{"action": "fill", "ref": "e2", "value": "oversized"})
	if failure == nil || !failure.uncertain || !failure.drop {
		t.Fatalf("oversized response = %#v", failure)
	}
}
