package webtool

import (
	"bufio"
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

func init() {
	worker := false
	for _, argument := range os.Args {
		worker = worker || argument == "--noema-browser-worker-v1"
	}
	if !worker {
		return
	}
	lossMode := os.Args[len(os.Args)-1] == "2"
	fmt.Println(`{"version":1,"ready":true}`)
	scanner := bufio.NewScanner(os.Stdin)
	for scanner.Scan() {
		var request struct {
			Tool      string         `json:"tool"`
			Arguments map[string]any `json:"arguments"`
		}
		if json.Unmarshal(scanner.Bytes(), &request) != nil {
			os.Exit(2)
		}
		if request.Tool == BrowseCloseName {
			fmt.Println(`{"version":1,"response":{"provider":"obscura","state":"closed"}}`)
			continue
		}
		if request.Tool == BrowseInteractName && lossMode {
			os.Exit(0)
		}
		if request.Tool == BrowseInteractName && request.Arguments["snapshot_revision"] != float64(41) {
			fmt.Println(`{"version":1,"error":"stale_snapshot"}`)
			continue
		}
		fmt.Println(`{"version":1,"response":{"provider":"obscura","state":"open","snapshot":{"url":"https://1.1.1.1/page","title":"Page","text":"Visible text","snapshot_revision":41,"elements":[{"reference":"e1","role":"link","name":"Next","href":"https://8.8.8.8/next","disabled":false},{"reference":"e2","role":"button","name":"Submit","disabled":false,"submission":{"destination":"https://1.1.1.1/submit","method":"POST","fields":[{"name":"q","value":"safe"}],"omitted_control_count":1,"truncated":false}}],"truncated":false},"screenshot":{"media_type":"image/png","data":"aW1hZ2U=","width":640,"height":480}}}`)
	}
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
	service, err := New(database, accounts, nil, executable, 2, 1024)
	if err != nil {
		t.Fatal(err)
	}
	defer service.Close()
	if !service.BrowserAvailable(ctx) {
		t.Fatal("browser is unavailable")
	}

	opened := service.ExecuteBrowser(ctx, "conversation:one", BrowseOpenName,
		json.RawMessage(`{"url":"https://1.1.1.1/start"}`), "test:open")
	if !opened.Success || !strings.Contains(string(opened.Stored), `"screenshot"`) ||
		strings.Contains(string(opened.Model), `"screenshot"`) || !strings.Contains(string(opened.Model), `"snapshot_revision":1`) {
		t.Fatalf("open = stored %s, model %s, success %t", opened.Stored, opened.Model, opened.Success)
	}
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
	if contextValue["submission"] == nil {
		t.Fatalf("submission context = %#v", contextValue)
	}
	clicked := service.ExecuteBrowser(ctx, "conversation:one", BrowseInteractName,
		json.RawMessage(`{"snapshot_revision":1,"ref":"e2","action":"click"}`), "test:click")
	if !clicked.Success || !strings.Contains(string(clicked.Model), `"snapshot_revision":2`) {
		t.Fatalf("interact = %s, %t", clicked.Model, clicked.Success)
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

	opened = service.ExecuteBrowser(ctx, "task:one:1", BrowseOpenName,
		json.RawMessage(`{"url":"https://1.1.1.1/start"}`), "test:loss-open")
	if !opened.Success {
		t.Fatalf("loss open = %s", opened.Model)
	}
	lost := service.ExecuteBrowser(ctx, "task:one:1", BrowseInteractName,
		json.RawMessage(`{"snapshot_revision":1,"ref":"e2","action":"click"}`), "test:loss")
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
}
