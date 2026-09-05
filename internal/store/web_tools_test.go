package store

import (
	"context"
	"database/sql"
	"fmt"
	"path/filepath"
	"testing"
	"time"
)

func TestWebToolSchemaBindingsAndObservedURLs(t *testing.T) {
	for _, version := range []int{0, 27} {
		t.Run(fmt.Sprintf("version_%d", version), func(t *testing.T) {
			path := filepath.Join(t.TempDir(), "noema.sqlite3")
			if version == 27 {
				database, err := sql.Open("sqlite3", "file:"+filepath.ToSlash(path))
				if err != nil {
					t.Fatal(err)
				}
				if _, err := database.Exec(schemaAtVersion(27) + "; PRAGMA user_version=27"); err != nil {
					t.Fatal(err)
				}
				database.Close()
			}
			database, err := Open(context.Background(), path)
			if err != nil {
				t.Fatal(err)
			}
			defer database.Close()
			if err := database.EnsureBuiltinProviderAccounts(context.Background(), time.Now()); err != nil {
				t.Fatal(err)
			}
			if err := database.SaveWebProviderBinding(context.Background(), "web.search", "provider_account:duckduckgo_public:system", time.Now()); err != nil {
				t.Fatal(err)
			}
			route, err := database.WebProviderRoute(context.Background(), "web.search")
			if err != nil || len(route) != 1 || route[0].ProviderAccountID != "provider_account:duckduckgo_public:system" {
				t.Fatalf("route = %#v, %v", route, err)
			}
			urls := []string{"https://1.1.1.1/a", "https://1.0.0.1/b"}
			if err := database.ObserveURLs(context.Background(), "search_result", "result:1", urls, time.Now()); err != nil {
				t.Fatal(err)
			}
			observed, err := database.URLWasObserved(context.Background(), urls[1])
			if err != nil || !observed {
				t.Fatalf("observed = %v, %v", observed, err)
			}
		})
	}
}
