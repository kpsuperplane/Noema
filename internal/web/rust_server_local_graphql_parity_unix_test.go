//go:build unix

package web_test

import (
	"context"
	"io"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	noemaauth "github.com/kpsuperplane/noema/internal/auth"
	noemagraphql "github.com/kpsuperplane/noema/internal/graphql"
	"github.com/kpsuperplane/noema/internal/home"
	web "github.com/kpsuperplane/noema/internal/web"
)

// Rust source: crates/noema-server/src/web/local_graphql/tests.rs::socket_serves_local_authority_with_private_permissions_and_cleanup.
func TestRustServer_socket_serves_local_authority_with_private_permissions_and_cleanup(t *testing.T) {
	root := t.TempDir()
	path := filepath.Join(root, "run", "graphql.sock")
	graphql := localParityGraphQL(t)
	server, err := web.NewLocalGraphQLServer(path, graphql, noemagraphql.Schema())
	if err != nil {
		t.Fatal(err)
	}
	parent, err := os.Stat(filepath.Dir(path))
	if err != nil {
		t.Fatal(err)
	}
	if got := parent.Mode().Perm(); got != 0o700 {
		t.Fatalf("socket parent mode = %o, want 700", got)
	}
	socket, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	if got := socket.Mode().Perm(); got != 0o600 {
		t.Fatalf("socket mode = %o, want 600", got)
	}
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	result := make(chan error, 1)
	go func() { result <- server.Serve(ctx) }()
	transport := &http.Transport{DialContext: func(context.Context, string, string) (net.Conn, error) {
		return net.Dial("unix", path)
	}}
	client := &http.Client{Transport: transport}
	defer transport.CloseIdleConnections()
	request, err := http.NewRequest(http.MethodPost, "http://local/graphql", strings.NewReader(`{"query":"{ task(taskId: \"task:transport\") { taskId } }"}`))
	if err != nil {
		t.Fatal(err)
	}
	request.Header.Set("Content-Type", "application/json")
	response, err := client.Do(request)
	if err != nil {
		t.Fatal(err)
	}
	body, err := io.ReadAll(response.Body)
	_ = response.Body.Close()
	if err != nil {
		t.Fatal(err)
	}
	if response.StatusCode != http.StatusOK || !strings.Contains(string(body), "Noema store is unavailable") {
		t.Fatalf("local GraphQL response = %d %q", response.StatusCode, body)
	}
	cancel()
	select {
	case err := <-result:
		if err != nil {
			t.Fatal(err)
		}
	case <-time.After(time.Second):
		t.Fatal("local GraphQL server did not stop")
	}
	if _, err := os.Stat(path); !os.IsNotExist(err) {
		t.Fatalf("socket remained after shutdown: %v", err)
	}
}

// Rust source: crates/noema-server/src/web/local_graphql/tests.rs::router_has_local_authority_and_exposes_only_bounded_graphql_post.
func TestRustServer_router_has_local_authority_and_exposes_only_bounded_graphql_post(t *testing.T) {
	path := filepath.Join(t.TempDir(), "run", "graphql.sock")
	server, err := web.NewLocalGraphQLServer(path, localParityGraphQL(t), noemagraphql.Schema())
	if err != nil {
		t.Fatal(err)
	}
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	result := make(chan error, 1)
	go func() { result <- server.Serve(ctx) }()
	transport := &http.Transport{DialContext: func(context.Context, string, string) (net.Conn, error) {
		return net.Dial("unix", path)
	}}
	client := &http.Client{Transport: transport}
	defer transport.CloseIdleConnections()
	for _, target := range []string{"/graphql", "/auth/recovery"} {
		response, err := client.Get("http://local" + target)
		if err != nil {
			t.Fatal(err)
		}
		_ = response.Body.Close()
		if response.StatusCode != http.StatusNotFound {
			t.Fatalf("GET %s = %d", target, response.StatusCode)
		}
	}
	request, err := http.NewRequest(http.MethodPost, "http://local/graphql", strings.NewReader(`{"query":"{ task(taskId: \"task:transport\") { taskId } }"}`))
	if err != nil {
		t.Fatal(err)
	}
	request.Header.Set("Content-Type", "application/json")
	response, err := client.Do(request)
	if err != nil {
		t.Fatal(err)
	}
	_ = response.Body.Close()
	if response.StatusCode != http.StatusOK || response.Header.Get("Cache-Control") != "no-store" {
		t.Fatalf("GraphQL POST = %d %#v", response.StatusCode, response.Header)
	}
	oversized, err := http.NewRequest(http.MethodPost, "http://local/graphql", strings.NewReader(strings.Repeat("x", 64*1024+1)))
	if err != nil {
		t.Fatal(err)
	}
	response, err = client.Do(oversized)
	if err != nil {
		t.Fatal(err)
	}
	_ = response.Body.Close()
	if response.StatusCode != http.StatusRequestEntityTooLarge {
		t.Fatalf("oversized GraphQL POST = %d", response.StatusCode)
	}
	cancel()
	select {
	case err := <-result:
		if err != nil {
			t.Fatal(err)
		}
	case <-time.After(time.Second):
		t.Fatal("local GraphQL server did not stop")
	}
}

func localParityGraphQL(t *testing.T) http.Handler {
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
	resolver := noemagraphql.NewResolver(nil, root, nil, nil, nil, nil, nil, nil, nil, nil)
	graphql := noemagraphql.NewHandler(resolver)
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		graphql.ServeHTTP(w, r.WithContext(noemaauth.WithDesktopAccess(r.Context())))
	})
}
