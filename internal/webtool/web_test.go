package webtool

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestWebToolContractsProvidersAndExtraction(t *testing.T) {
	ctx := context.Background()
	root := t.TempDir()
	database, err := store.Open(ctx, filepath.Join(root, "noema.sqlite3"))
	if err != nil {
		t.Fatal(err)
	}
	defer database.Close()
	accounts, err := provider.NewAccountService(root, database)
	if err != nil {
		t.Fatal(err)
	}
	if err := accounts.Initialize(ctx, time.Now()); err != nil {
		t.Fatal(err)
	}
	secret, _ := provider.NewSecret("exa-secret")
	account, err := accounts.CreateSecretAccount(ctx, "exa", "Exa test", secret, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	var calls int
	server := httptest.NewServer(http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		calls++
		if request.Header.Get("x-api-key") != "exa-secret" {
			t.Errorf("missing API key")
		}
		writer.Header().Set("Content-Type", "application/json")
		if calls == 3 {
			writer.WriteHeader(http.StatusUnauthorized)
			return
		}
		if strings.HasSuffix(request.URL.Path, "/search") {
			_, _ = writer.Write([]byte(`{"results":[{"title":" One ","url":"https://1.1.1.1/a","highlights":[" useful  result "]}]}`))
			return
		}
		_, _ = writer.Write([]byte(`{"results":[{"title":"Page","url":"https://1.1.1.1/final","text":"# Page\nUseful text"}]}`))
	}))
	defer server.Close()
	service, err := New(database, accounts, nil)
	if err != nil {
		t.Fatal(err)
	}
	service.endpoints["exa"] = server.URL
	if err := database.SaveWebProviderBinding(ctx, SearchName, account.ID, time.Now()); err != nil {
		t.Fatal(err)
	}
	search, ok := service.Execute(ctx, SearchName, json.RawMessage(`{"query":"  go  ","max_results":99}`), "test:search")
	if !ok || !strings.Contains(string(search), `"provider":"exa"`) || !strings.Contains(string(search), `"rank":1`) {
		t.Fatalf("search = %s, %v", search, ok)
	}
	if err := database.SaveWebProviderBinding(ctx, FetchName, account.ID, time.Now()); err != nil {
		t.Fatal(err)
	}
	fetch, ok := service.Execute(ctx, FetchName, json.RawMessage(`{"url":"https://1.1.1.1/a","max_chars":10}`), "test:fetch")
	if !ok || !strings.Contains(string(fetch), `"final_url":"https://1.1.1.1/final"`) || !strings.Contains(string(fetch), `"content_kind":"raw_markdown"`) {
		t.Fatalf("fetch = %s, %v", fetch, ok)
	}
	if _, ok := service.Execute(ctx, SearchName, json.RawMessage(`{"query":"auth check"}`), "test:auth"); ok {
		t.Fatal("authentication rejection succeeded")
	}
	rejected, err := accounts.LoadAccount(ctx, account.ID)
	if err != nil || rejected.Status != provider.StatusUnauthenticated {
		t.Fatalf("rejected account = %#v, %v", rejected, err)
	}
	if calls != 3 {
		t.Fatalf("calls = %d", calls)
	}
	if _, err := parseFetch(json.RawMessage(`{"url":"https://user:pass@1.1.1.1/"}`)); err == nil {
		t.Fatal("credential URL accepted")
	}
	if _, err := parseSearch(json.RawMessage(`{"query":"go","extra":"secret"}`)); err == nil || strings.Contains(err.Error(), "secret") {
		t.Fatalf("unsafe parser error: %v", err)
	}
	nested, err := parseSearch(json.RawMessage(`{"arguments":{"query":" nested ","max_results":0}}`))
	if err != nil || nested.Query != "nested" || nested.MaxResults != 1 {
		t.Fatalf("nested search = %#v, %v", nested, err)
	}
	if _, err := parseFetch(json.RawMessage(`{"max_chars":-1,"url":"https://1.1.1.1/"}`)); err == nil {
		t.Fatal("negative fetch bound accepted")
	}
	article := `<html><head><title>Example</title></head><body><article><h1>Example</h1><p>` + strings.Repeat("Readable article text. ", 40) + `</p><a href="/next">Next</a></article></body></html>`
	extracted, err := service.extractedHTML(ctx, fetchRequest{MaxChars: 20_000}, "https://1.1.1.1/", "https://1.1.1.1/", []byte(article))
	if err != nil || extracted.Extraction != "readability_markdown" || !strings.Contains(extracted.Content, "Readable article") || len(extracted.Links) != 1 {
		t.Fatalf("extracted = %#v, %v", extracted, err)
	}
	if _, err := normalizePublicURL(ctx, "http://127.0.0.1/"); err == nil {
		t.Fatal("private URL accepted")
	}
}
