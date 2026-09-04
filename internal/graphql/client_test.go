package graphql

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/artifact"
	"github.com/kpsuperplane/noema/internal/auth"
	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestNativeClientGraphQLListsCurrentAndRevokesIt(t *testing.T) {
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
	taskStore, err := store.Open(ctx, paths.Database())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = taskStore.Close() })
	now := time.Now()
	oldSession, newSession := sha256.Sum256([]byte("old")), sha256.Sum256([]byte("new"))
	if err := taskStore.CreateAnonymousSession(ctx, oldSession, now); err != nil {
		t.Fatal(err)
	}
	if err := taskStore.RegisterPasskey(
		ctx, store.HumanPasskey{CredentialID: "AQ", CredentialJSON: `{}`},
		store.RegistrationInitial, oldSession, newSession, now,
	); err != nil {
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
	clientID := "noema-desktop:abcdefghijklmnop"
	redirect := "http://127.0.0.1:49152/oauth/callback"
	challenge := "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
	code, access, refresh := "graphql-code", "graphql-access", "graphql-refresh"
	if err := taskStore.InsertNativeOAuthCode(
		ctx, sha256.Sum256([]byte(code)), clientID, "Noema Desktop", redirect,
		challenge, now.Unix(), now.Add(10*time.Minute).Unix(),
	); err != nil {
		t.Fatal(err)
	}
	if err := taskStore.ExchangeNativeOAuthCode(
		ctx, sha256.Sum256([]byte(code)), clientID, redirect,
		"dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
		"10000000000000000000000000000000", sha256.Sum256([]byte(access)),
		sha256.Sum256([]byte(refresh)), now.Unix(),
	); err != nil {
		t.Fatal(err)
	}
	artifacts, err := artifact.New(root, taskStore)
	if err != nil {
		t.Fatal(err)
	}
	handler := authentication.Handler(NewHandler(NewResolver(
		taskStore, root, authentication, nil, nil, nil, nil, artifacts, nil,
	)))
	list := nativeGraphQLRequest(t, access, `{ clients { clientId displayName createdAt revokedAt isCurrent } }`)
	listed := httptest.NewRecorder()
	handler.ServeHTTP(listed, list)
	var payload struct {
		Data struct {
			Clients []modelClient `json:"clients"`
		} `json:"data"`
	}
	if err := json.Unmarshal(listed.Body.Bytes(), &payload); err != nil || listed.Code != http.StatusOK ||
		len(payload.Data.Clients) != 1 || !payload.Data.Clients[0].IsCurrent {
		t.Fatalf("client list = %d %s, %v", listed.Code, listed.Body.String(), err)
	}
	revoke := nativeGraphQLRequest(t, access, `mutation { revokeClient(clientId: "`+clientID+`") { clientId revokedAt isCurrent } }`)
	revoked := httptest.NewRecorder()
	handler.ServeHTTP(revoked, revoke)
	if revoked.Code != http.StatusOK || !bytes.Contains(revoked.Body.Bytes(), []byte(`"isCurrent":true`)) ||
		!bytes.Contains(revoked.Body.Bytes(), []byte(`"revokedAt":`)) {
		t.Fatalf("client revocation = %d %s", revoked.Code, revoked.Body.String())
	}
	rejected := httptest.NewRecorder()
	handler.ServeHTTP(rejected, nativeGraphQLRequest(t, access, `{ clients { clientId } }`))
	if rejected.Code != http.StatusUnauthorized {
		t.Fatalf("revoked bearer = %d", rejected.Code)
	}
}

type modelClient struct {
	ClientID  string `json:"clientId"`
	IsCurrent bool   `json:"isCurrent"`
}

func nativeGraphQLRequest(t *testing.T, access string, query string) *http.Request {
	t.Helper()
	body, err := json.Marshal(map[string]string{"query": query})
	if err != nil {
		t.Fatal(err)
	}
	request := httptest.NewRequest(http.MethodPost, "http://localhost:3737/graphql", bytes.NewReader(body))
	request.Host = "localhost:3737"
	request.Header.Set("Content-Type", "application/json")
	request.Header.Set("Authorization", "Bearer "+access)
	return request
}
